use std::convert::Infallible;

use warp::Reply;
use warp_json_rpc::{Builder, Error};

use adnl_rpc_models::{GetContractState, GetTransactions, SendMessage, TransactionId};

use crate::api::State;

pub async fn send_message(
    state: State,
    res: Builder,
    req: SendMessage,
) -> Result<impl Reply, Infallible> {
    log::info!("Got send_message request. Req={:?}", req);
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

pub async fn get_contract_state(
    _state: State,
    res: Builder,
    req: GetContractState,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_contract_state request. Req={:?}", req);
    Ok(res.success("lol").unwrap())
}

pub async fn get_transactions(
    _state: State,
    res: Builder,
    req: GetTransactions,
) -> Result<impl Reply, Infallible> {
    log::info!("Got get_transactions request. Req={:?}", req);
    Ok(res.success("lol").unwrap())
}

pub async fn get_latest_key_block(_state: State, res: Builder) -> Result<impl Reply, Infallible> {
    log::info!("Got get_latest_key_block request.");
    Ok(res.success("lol").unwrap())
}

pub async fn get_blockchain_config(_state: State, res: Builder) -> Result<impl Reply, Infallible> {
    log::info!("Got get_blockchain_config request.");
    Ok(res.success("lol").unwrap())
}
