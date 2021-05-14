use std::convert::Infallible;

use warp::Reply;
use warp_json_rpc::{Builder, Error};

use crate::api::State;
use crate::models::{Address, Message, TransactionId};

pub(crate) async fn send_message(
    state: State,
    res: Builder,
    msg: Message,
) -> Result<impl Reply, Infallible> {
    log::info!("Got send_message request. Msg={:?}", msg);
    let mut connection = match state.pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            log::error!("connection error: {:#?}", e);
            return Ok(res.error(Error::custom(1, "connection error")).unwrap());
        }
    };
    connection.query(&Default::default()).await.unwrap();
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_contract_state(
    _state: State,
    res: Builder,
    addr: Address,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_contract_state request. Addr={}", addr);
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_transactions(
    _state: State,
    res: Builder,
    (addr, transaction_id, count): (Address, TransactionId, u8),
) -> Result<impl Reply, Infallible> {
    log::info!(
        "Got get_transactions request. Addr={}, transaction_id={:?}, count={}",
        addr,
        transaction_id,
        count
    );
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_latest_key_block(
    _state: State,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_latest_key_block request.");
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn get_blockchain_config(
    _state: State,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_blockchain_config request.");
    Ok(res.success("lol").unwrap())
}

pub(crate) async fn max_transactions_per_fetch(
    _state: State,
    res: Builder,
) -> Result<impl Reply, Infallible> {
    log::info!("Got max_transactions_per_fetch request.");
    Ok(res.success("lol").unwrap())
}
