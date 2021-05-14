use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bb8::Pool;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use crate::config::Config;
use crate::models::Address;
use crate::ton::adnl_pool::AdnlManageConnection;

const TIMEOUT_SECS: u64 = 1;

#[derive(Clone)]
pub struct State {
    pub pool: Pool<AdnlManageConnection>,
    pub addresses_callbacks: Arc<
        RwLock<
            HashMap<Address, HashMap<uuid::Uuid, mpsc::UnboundedSender<WebsocketResponseMessage>>>,
        >,
    >,
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
            addresses_callbacks: Default::default(),
        })
    }
    pub async fn transaction_monitoring(self) {
        loop {
            tokio::time::sleep(Duration::from_secs(TIMEOUT_SECS)).await;
            let addresses_callbacks = self.addresses_callbacks.read().await.clone();
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
                        .send(WebsocketResponseMessage::Transaction(
                            serde_json::Value::Null,
                        ))
                        .await
                        .is_err()
                    {
                        if let Some(hash) = self.addresses_callbacks.write().await.get_mut(&address)
                        {
                            hash.remove(&id);
                        }
                    }
                }
            }
        }
    }
}

impl State {
    pub(crate) async fn add_connection(&self, websocket: WebSocket) {
        let (tx, rx) = mpsc::unbounded::<WebsocketResponseMessage>();
        let (ws_tx, mut ws_rx) = websocket.split();

        tokio::task::spawn(
            rx.map(|message| Ok(ws::Message::text(serde_json::to_string(&message).unwrap())))
                .forward(ws_tx),
        );

        while let Some(Ok(message)) = ws_rx.next().await {
            let message: WebsocketRequestMessage = match message
                .to_str()
                .and_then(|s| serde_json::from_str::<WebsocketRequestMessage>(s).map_err(|_| ()))
            {
                Ok(x) => x,
                Err(e) => {
                    log::error!("error from websocket - {:?}", e);
                    continue;
                }
            };

            log::debug!("Received {:?}", message);

            match message {
                WebsocketRequestMessage::SubscribeForTransactions { address } => {
                    let mut addresses_callbacks = self.addresses_callbacks.write().await;
                    let id = uuid::Uuid::new_v4();
                    addresses_callbacks
                        .entry(address.clone())
                        .or_insert_with(HashMap::new)
                        .insert(id, tx.clone());
                }
                WebsocketRequestMessage::SubscribeForNewBlock => {}
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "messageType", content = "payload")]
pub enum WebsocketRequestMessage {
    #[serde(rename_all = "camelCase")]
    SubscribeForTransactions { address: Address },
    #[serde(rename_all = "camelCase")]
    SubscribeForNewBlock,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "messageType", content = "payload")]
pub enum WebsocketResponseMessage {
    Transaction(serde_json::Value),
    Block(serde_json::Value),
}
