use futures::channel::mpsc;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use warp::filters::ws;
use warp::filters::ws::WebSocket;

#[derive(Debug, Clone)]
pub struct State {}

impl State {
    pub(crate) async fn add_connection(&self, websocket: WebSocket) {
        let (_tx, rx) = mpsc::unbounded::<WebsocketResponseMessage>();
        let (ws_tx, mut ws_rx) = websocket.split();

        tokio::task::spawn(
            rx.map(|message| {
                Ok(ws::Message::text(
                    serde_json::to_string(&message).unwrap(),
                ))
            })
            .forward(ws_tx)
            // .map(|result| {
            //     if let Err(e) = result {
            //         log::debug!("websocket send error: {}", e);
            //     }
            // }),
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
