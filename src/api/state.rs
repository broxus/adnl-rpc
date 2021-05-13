use adnl::client::AdnlClientConfig;
use bb8::Pool;
use futures::channel::mpsc;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use warp::filters::ws;
use warp::filters::ws::WebSocket;

use crate::config::Config;
use crate::ton::adnl_pool::AdnlManageConnection;

pub struct State {
    pub pool: Pool<AdnlManageConnection>,
}

impl State {
    pub async fn new(config: Config) -> Self {
        let builder = Pool::builder();
        let pool = builder
            .build(AdnlManageConnection::connect(
                AdnlClientConfig::from_json_config(config.adnl_config).expect("wrong config"),
            ))
            .await
            .unwrap();
        Self { pool }
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
