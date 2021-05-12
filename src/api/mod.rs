use std::convert::Infallible;
use std::sync::Arc;

use crate::config::Config;
use crate::models::{Address, Message};
use futures::future;
use ton_block::MsgAddress;
use warp::Filter as _;
use warp::{Filter, Reply};
use warp_json_rpc::filters as json_rpc;
use warp_json_rpc::Builder;

type State = Arc<()>;

pub async fn serve(config: Config) {
    let state = State::new(());
    let state = warp::any().map(|| state.clone());

    let send = state
        .clone()
        .and(json_rpc::json_rpc())
        .and(json_rpc::method("send"))
        .and(json_rpc::params::<(Message, Address)>())
        .and_then(send_message);

    let routes = warp::filters::path::path("rpc").or(send);

    let service = warp_json_rpc::service(routes);
    let make_svc = hyper::service::make_service_fn(move |_| future::ok::<_, Infallible>(service));
    hyper::Server::bind(&config.listen_address)
        .serve(make_svc)
        .await
        .unwrap();
}

async fn send_message(
    state: State,
    res: Builder,
    (msg, addr): (Message, Address),
) -> Result<impl Reply, Infallible> {
    Ok(res.success("lol").unwrap())
}
