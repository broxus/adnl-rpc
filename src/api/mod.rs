use std::convert::Infallible;
use std::sync::Arc;

use futures::future;
use http::Response;
use hyper::Body;
use serde::Serialize;
use ton_block::MsgAddress;
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};
use warp_json_rpc::filters as json_rpc;
use warp_json_rpc::Builder;

use crate::config::Config;
use crate::models::Message;

const RPC_API_PATH: &str = "rpc";

type State = Arc<()>;

// This is a workaround for not being able to create a `warp_json_rpc::Response` without a
// `warp_json_rpc::Builder`.
fn new_error_response(error: warp_json_rpc::Error) -> Response<Body> {
    #[derive(Serialize)]
    struct JsonRpcErrorResponse {
        jsonrpc: String,
        id: Option<()>,
        error: warp_json_rpc::Error,
    }

    let json_response = JsonRpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id: None,
        error,
    };

    let body = Body::from(serde_json::to_vec(&json_response).unwrap());
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(body)
        .unwrap()
}

pub async fn serve(config: Config) {
    let state = State::new(());
    let state = warp::any().map(move || Arc::clone(&state));

    let unknown_method = warp::path(RPC_API_PATH)
        .and(warp_json_rpc::filters::json_rpc())
        .and_then(move |response_builder: warp_json_rpc::Builder| async move {
            response_builder
                .error(warp_json_rpc::Error::METHOD_NOT_FOUND)
                .map_err(|_| warp::reject())
        });

    let parse_failure = warp::path(RPC_API_PATH).and_then(move || async move {
        let error_response = new_error_response(warp_json_rpc::Error::PARSE_ERROR);
        Ok::<_, Rejection>(error_response)
    });

    let send = warp::path(RPC_API_PATH)
        .and(state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("send"))
        .and(json_rpc::params::<(String, String)>())
        .and_then(send_message);

    let routes = send.or(unknown_method).or(parse_failure);

    let service = warp_json_rpc::service(routes);
    let make_svc =
        hyper::service::make_service_fn(move |_| future::ok::<_, Infallible>(service.clone()));
    hyper::Server::bind(&config.listen_address)
        .serve(make_svc)
        .await
        .unwrap();
}

async fn send_message(
    state: State,
    res: Builder,
    (msg, addr): (String, String),
) -> Result<impl Reply, Infallible> {
    log::info!("Got send_message request. Msg={},Addr={}", msg, addr);
    Ok(res.success("lol").unwrap())
}
