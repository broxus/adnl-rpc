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
    let response = connection
        .query(&ton::TLObject::new(query))
        .await
        .map_err(|_| QueryError::ConnectionError)?;

    match response.downcast::<T::Reply>() {
        Ok(reply) => Ok(reply),
        Err(error) => match error.downcast::<ton::lite_server::Error>() {
            Ok(error) => Err(QueryError::LiteServer(error)),
            Err(_) => Err(QueryError::Unknown),
        },
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
