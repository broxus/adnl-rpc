use std::convert::Infallible;
use std::sync::Arc;

use warp::Reply;
use warp_json_rpc::{Builder, Error};

use crate::api::State;

pub(crate) async fn send_message(
    state: Arc<State>,
    res: Builder,
    (msg, addr): (String, String),
) -> Result<impl Reply, Infallible> {
    log::info!("Got send_message request. Msg={},Addr={}", msg, addr);
    let mut connection = match state.pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Ok(res.error(Error::custom(1, "connection error")).unwrap()),
    };
    connection.query(&Default::default()).await.unwrap();
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_contract_state(
    _state: Arc<State>,
    res: Builder,
    addr: String,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_contract_state request. Addr={}", addr);
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_transactions(
    _state: Arc<State>,
    res: Builder,
    (addr, transaction_id, count): (String, String, u8),
) -> Result<impl Reply, Infallible> {
    log::info!(
        "Got get_transactions request. Addr={}, transaction_id={}, count={}",
        addr,
        transaction_id,
        count
    );
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_latest_key_block(
    _state: Arc<State>,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_latest_key_block request.");
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_blockchain_config(
    _state: Arc<State>,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_blockchain_config request.");
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn max_transactions_per_fetch(
    _state: Arc<State>,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got max_transactions_per_fetch request.");
    Ok(res.success("lol").unwrap())
}
