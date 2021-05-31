use bb8::{Pool, PooledConnection};
use ton_api::ton;
use ton_block::Deserializable;

use super::errors::*;
use crate::ton::adnl_pool::AdnlManageConnection;

pub async fn query_block_by_seqno(
    connection: &mut PooledConnection<'_, AdnlManageConnection>,
    id: ton::ton_node::blockid::BlockId,
) -> QueryResult<ton_block::Block> {
    let block_id = query(
        connection,
        ton::rpc::lite_server::LookupBlock {
            mode: 0x1,
            id,
            lt: None,
            utime: None,
        },
    )
    .await?;

    query_block(connection, block_id.only().id).await
}

pub async fn query_block(
    connection: &mut PooledConnection<'_, AdnlManageConnection>,
    id: ton::ton_node::blockidext::BlockIdExt,
) -> QueryResult<ton_block::Block> {
    let block = query(connection, ton::rpc::lite_server::GetBlock { id }).await?;

    let block = ton_block::Block::construct_from_bytes(&block.only().data.0)
        .map_err(|_| QueryError::InvalidBlock)?;

    Ok(block)
}

pub async fn query<T>(
    connection: &mut PooledConnection<'_, AdnlManageConnection>,
    query: T,
) -> QueryResult<T::Reply>
where
    T: ton_api::Function,
{
    const MAX_RETIRES: usize = 3;
    const RETRY_INTERVAL: u64 = 100; // Milliseconds

    const ERR_NOT_READY: i32 = 651;

    let query_bytes = query
        .boxed_serialized_bytes()
        .map_err(|_| QueryError::FailedToSerialize)?;

    let query = ton::TLObject::new(ton::rpc::lite_server::Query {
        data: query_bytes.into(),
    });

    let mut retries = 0;
    loop {
        let response = connection
            .query(&query)
            .await
            .map_err(|_| QueryError::ConnectionError)?;

        match response.downcast::<T::Reply>() {
            Ok(reply) => return Ok(reply),
            Err(error) => match error.downcast::<ton::lite_server::Error>() {
                Ok(error) if retries < MAX_RETIRES && error.code() == &ERR_NOT_READY => {
                    tokio::time::sleep(std::time::Duration::from_millis(RETRY_INTERVAL)).await;
                    retries += 1;
                    continue;
                }
                Ok(error) => return Err(QueryError::LiteServer(error)),
                Err(_) => return Err(QueryError::Unknown),
            },
        }
    }
}

pub async fn acquire_connection(
    pool: &Pool<AdnlManageConnection>,
) -> QueryResult<PooledConnection<'_, AdnlManageConnection>> {
    pool.get().await.map_err(|e| {
        log::error!("connection error: {:#?}", e);
        QueryError::ConnectionError
    })
}
