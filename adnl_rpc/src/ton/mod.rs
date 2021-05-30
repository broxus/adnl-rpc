use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use bb8::{Pool, PooledConnection};
use futures::channel::mpsc;
use futures::StreamExt;
use tokio::sync::RwLock;
use ton_api::ton;
use ton_block::{Deserializable, MsgAddressInt, Serializable};
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use adnl_rpc_models::{
    ExistingContract, GenTimings, RawBlock, RawContractState, RawTransactionsList, TransactionId,
    WsRequestMessage, WsResponseMessage,
};

use crate::config::Config;

use self::adnl_pool::AdnlManageConnection;
use self::connection::*;
pub use self::errors::*;
use self::last_block::LastBlock;

mod adnl_pool;
mod connection;
mod errors;
mod last_block;

static CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);

pub struct State {
    pool: Pool<AdnlManageConnection>,
    last_block: LastBlock,
    address_subscriptions: RwLock<AddressSubscriptionsMap>,
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
        })
    }

    pub async fn send_message(&self, message: ton_block::Message) -> QueryResult<()> {
        let mut connection = self.acquire_connection().await?;

        let cells = message
            .write_to_new_cell()
            .map_err(|_| QueryError::FailedToSerialize)?
            .into();

        let serialized =
            ton_types::serialize_toc(&cells).map_err(|_| QueryError::FailedToSerialize)?;

        query(
            &mut connection,
            ton::rpc::lite_server::SendMessage {
                body: ton::bytes(serialized),
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
            _ => Ok(RawContractState::NotExists),
        }
    }

    pub async fn get_transactions(
        &self,
        address: MsgAddressInt,
        from: Option<TransactionId>,
        count: u8,
    ) -> QueryResult<RawTransactionsList> {
        let mut connection = self.acquire_connection().await?;

        let from = match from {
            Some(id) => id,
            None => match self.get_contract_state(address.clone()).await? {
                RawContractState::Exists(contract) => contract.last_transaction_id,
                RawContractState::NotExists => {
                    let transactions =
                        ton_types::serialize_toc(&ton_types::Cell::default()).unwrap();

                    return Ok(RawTransactionsList { transactions });
                }
            },
        };

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
