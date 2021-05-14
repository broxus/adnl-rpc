pub mod service;
pub mod state;

use std::convert::Infallible;

use anyhow::Result;
use futures::future;
use http::Response;
use hyper::Body;
use serde::Serialize;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::{Filter, Rejection};
use warp_json_rpc::filters as json_rpc;

use adnl_rpc_models::{GetContractState, GetTransactions, SendMessage, TransactionId};

pub use self::state::*;
use crate::config::Config;

const RPC_API_PATH: &str = "rpc";

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

pub async fn serve(config: Config) -> Result<()> {
    let address = config.listen_address;
    let state = State::new(config).await?;

    tokio::spawn(state.clone().transaction_monitoring());

    let routes = rpc(state.clone()).or(ws_stream(state));

    let service = warp_json_rpc::service(routes);

    hyper::Server::bind(&address)
        .serve(hyper::service::make_service_fn(move |_| {
            future::ok::<_, Infallible>(service.clone())
        }))
        .await?;
    Ok(())
}

pub fn rpc(state: State) -> BoxedFilter<(impl warp::Reply,)> {
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

    send_message(state.clone())
        .or(get_contract_state(state.clone()))
        .or(get_transactions(state.clone()))
        .or(get_latest_key_block(state.clone()))
        .or(get_blockchain_config(state))
        .or(unknown_method)
        .or(parse_failure)
        .boxed()
}

pub fn send_message(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("sendMessage"))
        .and(json_rpc::params::<SendMessage>())
        .and_then(service::send_message)
        .boxed()
}

pub fn get_contract_state(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getContractState"))
        .and(json_rpc::params::<GetContractState>())
        .and_then(service::get_contract_state)
        .boxed()
}

pub fn get_transactions(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getTransactions"))
        .and(json_rpc::params::<GetTransactions>())
        .and_then(service::get_transactions)
        .boxed()
}

pub fn get_latest_key_block(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getLatestKeyBlock"))
        .and_then(service::get_latest_key_block)
        .boxed()
}

pub fn get_blockchain_config(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getBlockchainConfig"))
        .and_then(service::get_blockchain_config)
        .boxed()
}

pub fn ws_stream(state: State) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path::path("stream")
        .and(warp::path::end())
        .map(move || state.clone())
        .and(warp::ws())
        .map(|state: State, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| async move { state.add_connection(websocket).await })
        })
        .boxed()
}