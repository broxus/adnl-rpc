mod adnl_pool;
mod connection;
mod errors;
mod last_block;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use bb8::{Pool, PooledConnection};
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use tokio::sync::RwLock;
use ton_api::ton;
use ton_block::{BinTreeType, Deserializable, MsgAddressInt, Serializable};
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use adnl_rpc_models::{
    ExistingContract, GenTimings, RawBlock, RawContractState, RawTransactionsList, TransactionId,
    WsRequestMessage, WsResponseMessage,
};

use self::adnl_pool::AdnlManageConnection;
use self::connection::*;
pub use self::errors::*;
use self::last_block::LastBlock;
use crate::config::Config;

static CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);

pub struct State {
    pool: Pool<AdnlManageConnection>,
    last_block: LastBlock,
    address_subscriptions: RwLock<AddressSubscriptionsMap>,
    config: Config,
}

impl State {
    pub async fn new(config: Config) -> Result<Self> {
        let builder = Pool::builder();
        let pool = builder
            .max_size(config.max_connection_count)
            .min_idle(config.min_idle_connection_count)
            .max_lifetime(None)
            .build(AdnlManageConnection::new(
                config.adnl_config.tonlib_config()?,
            ))
            .await?;

        Ok(Self {
            pool,
            last_block: LastBlock::new(&config.last_block_cache_duration),
            address_subscriptions: Default::default(),
            config,
        })
    }

    pub async fn spawn_indexer(self: &Arc<Self>) -> QueryResult<()> {
        let indexer = Arc::downgrade(self);
        let mut connection = self.acquire_connection().await?;
        let mut curr_mc_block_id = self.last_block.get_last_block(&mut connection).await?;

        tokio::spawn(async move {
            loop {
                let indexer = match indexer.upgrade() {
                    Some(indexer) => indexer,
                    None => return,
                };

                tokio::time::sleep(indexer.config.indexer_interval).await;
                log::debug!("Indexer step");

                let mut connection = match indexer.acquire_connection().await {
                    Ok(connection) => connection,
                    Err(_) => continue,
                };

                match indexer
                    .indexer_step(&mut connection, &curr_mc_block_id)
                    .await
                {
                    Ok(next_block_id) => curr_mc_block_id = next_block_id,
                    Err(e) => {
                        log::error!("Indexer step error: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn indexer_step(
        &self,
        connection: &mut PooledConnection<'_, AdnlManageConnection>,
        prev_mc_block_id: &ton::ton_node::blockidext::BlockIdExt,
    ) -> Result<ton::ton_node::blockidext::BlockIdExt> {
        let curr_mc_block_id = self.last_block.get_last_block(connection).await?;
        if prev_mc_block_id == &curr_mc_block_id {
            return Ok(curr_mc_block_id);
        }

        let curr_mc_block = query_block(connection, curr_mc_block_id.clone()).await?;
        let extra = curr_mc_block
            .extra
            .read_struct()
            .and_then(|extra| extra.read_custom())
            .map_err(|e| anyhow::anyhow!("Failed to parse block info: {:?}", e))?;

        let extra = match extra {
            Some(extra) => extra,
            None => return Ok(curr_mc_block_id),
        };

        let mut workchain = -1;
        extra
            .shards()
            .iterate_shards(|shard_id, shard| {
                log::debug!("Shard id: {:?}, shard block: {}", shard_id, shard.seq_no);

                Ok(true)
            })
            .map_err(|e| anyhow::anyhow!("Failed to iterate shards: {:?}", e))?;

        log::debug!("Next masterchain block id: {:?}", curr_mc_block_id);
        Ok(curr_mc_block_id)
    }

    pub async fn send_message(&self, message: ton_block::Message) -> QueryResult<()> {
        let mut connection = self.acquire_connection().await?;

        let data = message
            .serialize()
            .and_then(|cell| cell.write_to_bytes())
            .map_err(|_| QueryError::FailedToSerialize)?;

        query(
            &mut connection,
            ton::rpc::lite_server::SendMessage {
                body: ton::bytes(data),
            },
        )
        .await?;

        Ok(())
    }

    pub async fn get_contract_state(
        &self,
        address: MsgAddressInt,
    ) -> QueryResult<RawContractState> {
        use ton_block::HashmapAugType;

        log::debug!("Getting contract state: {}", address);

        let mut connection = self.acquire_connection().await?;

        log::debug!("Acquired connection");

        let last_block_id = self.last_block.get_last_block(&mut connection).await?;

        log::debug!("Got last block id");

        let response = query(
            &mut connection,
            ton::rpc::lite_server::GetAccountState {
                id: last_block_id,
                account: ton::lite_server::accountid::AccountId {
                    workchain: address.workchain_id(),
                    id: ton::int256(
                        ton_types::UInt256::from(address.address().get_bytestring(0)).into(),
                    ),
                },
            },
        )
        .await?
        .only();

        match ton_block::Account::construct_from_bytes(&response.state.0) {
            Ok(ton_block::Account::Account(account)) => {
                let q_roots =
                    ton_types::deserialize_cells_tree(&mut std::io::Cursor::new(&response.proof.0))
                        .map_err(|_| QueryError::InvalidAccountStateProof)?;
                if q_roots.len() != 2 {
                    return Err(QueryError::InvalidAccountStateProof);
                }

                let merkle_proof = ton_block::MerkleProof::construct_from_cell(q_roots[0].clone())
                    .map_err(|_| QueryError::InvalidAccountStateProof)?;
                let proof_root = merkle_proof.proof.virtualize(1);

                let ss = ton_block::ShardStateUnsplit::construct_from(&mut proof_root.into())
                    .map_err(|_| QueryError::InvalidAccountStateProof)?;

                let shard_info = ss
                    .read_accounts()
                    .and_then(|accounts| {
                        accounts.get(&ton_types::UInt256::from(
                            address.get_address().get_bytestring(0),
                        ))
                    })
                    .map_err(|_| QueryError::InvalidAccountStateProof)?;

                Ok(if let Some(shard_info) = shard_info {
                    RawContractState::Exists(ExistingContract {
                        account,
                        timings: GenTimings {
                            gen_lt: ss.gen_lt(),
                            gen_utime: ss.gen_time(),
                        },
                        last_transaction_id: TransactionId {
                            lt: shard_info.last_trans_lt(),
                            hash: *shard_info.last_trans_hash(),
                        },
                    })
                } else {
                    RawContractState::NotExists
                })
            }
            Ok(_) => Ok(RawContractState::NotExists),
            Err(_) => Err(QueryError::InvalidAccountState),
        }
    }

    pub async fn get_transactions(
        &self,
        address: MsgAddressInt,
        from: TransactionId,
        count: u8,
    ) -> QueryResult<RawTransactionsList> {
        let mut connection = self.acquire_connection().await?;

        let response = query(
            &mut connection,
            ton::rpc::lite_server::GetTransactions {
                count: count as i32,
                account: ton::lite_server::accountid::AccountId {
                    workchain: address.workchain_id() as i32,
                    id: ton::int256(
                        ton_types::UInt256::from(address.address().get_bytestring(0)).into(),
                    ),
                },
                lt: from.lt as i64,
                hash: from.hash.into(),
            },
        )
        .await?;

        Ok(RawTransactionsList {
            transactions: response.only().transactions.0,
        })
    }

    pub async fn get_latest_key_block(&self) -> QueryResult<RawBlock> {
        const MASTERCHAIN_SHARD: u64 = 0x8000000000000000;

        let mut connection = self.acquire_connection().await?;

        let last_block_id = self.last_block.get_last_block(&mut connection).await?;

        let block = query_block(&mut connection, last_block_id).await?;

        let info = block
            .info
            .read_struct()
            .map_err(|_| QueryError::InvalidBlock)?;

        if info.key_block() {
            Ok(RawBlock { block })
        } else {
            let block = query_block_by_seqno(
                &mut connection,
                ton::ton_node::blockid::BlockId {
                    workchain: -1,
                    shard: MASTERCHAIN_SHARD as i64,
                    seqno: info.prev_key_block_seqno() as i32,
                },
            )
            .await?;
            Ok(RawBlock { block })
        }
    }

    pub async fn handle_websocket(&self, websocket: WebSocket) {
        let (tx, rx) = mpsc::unbounded::<WsResponseMessage>();
        let (ws_tx, mut ws_rx) = websocket.split();

        let connection_id = CONNECTION_ID.fetch_add(1, Ordering::Relaxed);

        tokio::task::spawn(
            rx.map(|message| Ok(ws::Message::text(serde_json::to_string(&message).unwrap())))
                .forward(ws_tx),
        );

        while let Some(Ok(message)) = ws_rx.next().await {
            let message: WsRequestMessage = match message
                .to_str()
                .and_then(|s| serde_json::from_str::<WsRequestMessage>(s).map_err(|_| ()))
            {
                Ok(x) => x,
                Err(e) => {
                    log::error!("error from websocket - {:?}", e);
                    continue;
                }
            };

            log::debug!("Received {:?}", message);

            match message {
                WsRequestMessage::SubscribeAccount { address } => {
                    let mut addresses_callbacks = self.address_subscriptions.write().await;
                    addresses_callbacks
                        .entry(address.clone())
                        .or_insert_with(HashMap::new)
                        .insert(connection_id, tx.clone());
                }
                WsRequestMessage::SubscribeForNewBlock => {}
            }
        }
    }

    async fn acquire_connection(
        &self,
    ) -> Result<PooledConnection<'_, AdnlManageConnection>, QueryError> {
        acquire_connection(&self.pool).await
    }
}

type AddressSubscriptionsMap = HashMap<MsgAddressInt, HashMap<usize, WsTx>>;

type WsTx = mpsc::UnboundedSender<WsResponseMessage>;
