use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use bb8::Pool;
use futures::channel::mpsc;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use crate::config::Config;
use crate::models::Address;
use crate::ton::adnl_pool::AdnlManageConnection;

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
}

impl State {
    pub(crate) async fn add_connection(&self, websocket: WebSocket) {
        let (_tx, rx) = mpsc::unbounded::<WebsocketResponseMessage>();
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
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "messageType", content = "payload")]
pub enum WebsocketRequestMessage {
    #[serde(rename_all = "camelCase")]
    SubscribeForTransactions { address: String },
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
