use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bb8::Pool;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use tokio::sync::RwLock;
use ton_block::MsgAddressInt;
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use adnl_rpc_models::{WsRequestMessage, WsResponseMessage};

use crate::config::Config;
use crate::ton::adnl_pool::AdnlManageConnection;

const TIMEOUT_SECS: u64 = 1;

static CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct State {
    pub pool: Pool<AdnlManageConnection>,
    pub address_subscriptions: Arc<RwLock<AddressSubscriptionsMap>>,
}

impl State {
    pub async fn new(config: Config) -> Result<Self> {
        let builder = Pool::builder();
        let pool = builder
            .max_size(100)
            .min_idle(Some(5))
            .max_lifetime(None)
            .build(AdnlManageConnection::connect(
                config.adnl_config.tonlib_config()?,
            ))
            .await?;

        Ok(Self {
            pool,
            address_subscriptions: Default::default(),
        })
    }

    pub async fn transaction_monitoring(self) {
        loop {
            tokio::time::sleep(Duration::from_secs(TIMEOUT_SECS)).await;
            let addresses_callbacks = self.address_subscriptions.read().await.clone();
            let mut connection = match self.pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("connection error: {:?}", e);
                    continue;
                }
            };
            for (address, callbacks) in addresses_callbacks.into_iter() {
                let _res = match connection.query(&Default::default()).await {
                    Ok(conn) => conn,
                    Err(e) => {
                        log::error!("query error: {:?}", e);
                        continue;
                    }
                };

                for (id, mut callback) in callbacks {
                    if callback
                        .send(WsResponseMessage::Transaction(serde_json::Value::Null))
                        .await
                        .is_err()
                    {
                        let mut addresses_callbacks = self.address_subscriptions.write().await;
                        let is_empty = if let Some(hash) = addresses_callbacks.get_mut(&address) {
                            hash.remove(&id);
                            hash.is_empty()
                        } else {
                            false
                        };
                        if is_empty {
                            addresses_callbacks.remove(&address);
                        }
                    }
                }
            }
        }
    }

    pub async fn add_connection(&self, websocket: WebSocket) {
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
}

type AddressSubscriptionsMap = HashMap<MsgAddressInt, HashMap<usize, WsTx>>;

type WsTx = mpsc::UnboundedSender<WsResponseMessage>;
