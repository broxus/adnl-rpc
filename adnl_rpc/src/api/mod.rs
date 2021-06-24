use std::convert::Infallible;
use std::sync::Arc;

use anyhow::Result;
use futures::future;
use http::Response;
use hyper::Body;
use serde::Serialize;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::{Filter, Rejection};
use warp_json_rpc::filters as json_rpc;

use adnl_rpc_models::{GetContractState, GetTransactions, SendMessage};

use crate::config::Config;
use crate::ton::*;

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
    let state = Arc::new(State::new(config).await?);

    state.start_masterchain_cache_updater();

    let routes = rpc(state.clone()).or(ws_stream(state));

    let service = warp_json_rpc::service(routes);
    log::info!("Started server");
    hyper::Server::bind(&address)
        .serve(hyper::service::make_service_fn(move |_| {
            future::ok::<_, Infallible>(service.clone())
        }))
        .await?;
    Ok(())
}

pub fn rpc(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
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

    healthcheck(state.clone())
        .or(send_message(state.clone()))
        .or(send_message(state.clone()))
        .or(get_contract_state(state.clone()))
        .or(get_transactions(state.clone()))
        .or(get_latest_key_block(state))
        .or(unknown_method)
        .or(parse_failure)
        .with(warp::compression::gzip())
        .boxed()
}

fn wrap(
    res: warp_json_rpc::Builder,
    result: QueryResult<impl serde::Serialize + 'static>,
) -> Result<impl warp::Reply, Infallible> {
    Ok(match result {
        Ok(result) => res.success(result),
        Err(error) => res.error(warp_json_rpc::Error::custom(
            error.code(),
            error.to_string(),
        )),
    }
    .unwrap())
}

pub fn healthcheck(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path::end()
        .map(move || state.clone())
        .and(warp::get())
        .map(|state: Arc<State>| {
            warp::reply::with_status(
                "",
                if state.is_ok() {
                    http::StatusCode::OK
                } else {
                    http::StatusCode::SERVICE_UNAVAILABLE
                },
            )
        })
        .boxed()
}

pub fn send_message(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    log::debug!("sendMessage");
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("sendMessage"))
        .and(json_rpc::params())
        .and_then(|state: Arc<State>, res, req: SendMessage| async move {
            wrap(res, state.send_message(req.message).await)
        })
        .boxed()
}

pub fn get_transactions(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    log::debug!("getTransactions");
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getTransactions"))
        .and(json_rpc::params())
        .and_then(|state: Arc<State>, res, req: GetTransactions| async move {
            wrap(
                res,
                state
                    .get_transactions(req.address, req.transaction_id, req.count)
                    .await,
            )
        })
        .boxed()
}

pub fn get_contract_state(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    log::debug!("getContractState");
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getContractState"))
        .and(json_rpc::params::<GetContractState>())
        .and_then(|state: Arc<State>, res, req: GetContractState| async move {
            wrap(res, state.get_contract_state(req.address).await)
        })
        .boxed()
}

pub fn get_latest_key_block(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    log::debug!("getLatestKeyBlock");
    warp::path(RPC_API_PATH)
        .map(move || state.clone())
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("getLatestKeyBlock"))
        .and_then(
            |state: Arc<State>, res| async move { wrap(res, state.get_latest_key_block().await) },
        )
        .boxed()
}

pub fn ws_stream(state: Arc<State>) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path::path("stream")
        .and(warp::path::end())
        .map(move || state.clone())
        .and(warp::ws())
        .map(|state: Arc<State>, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| async move { state.handle_websocket(websocket).await })
        })
        .boxed()
}
