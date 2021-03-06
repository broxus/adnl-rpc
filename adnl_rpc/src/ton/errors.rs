use ton_api::ton;

pub type QueryResult<T> = Result<T, QueryError>;

#[derive(thiserror::Error, Clone, Debug)]
pub enum QueryError {
    #[error("Connection error")]
    ConnectionError,
    #[error("Failed to serialize message")]
    FailedToSerialize,
    #[error("Lite server error. code: {}, reason: {}", .0.code(), .0.message())]
    LiteServer(ton::lite_server::Error),
    #[error("Invalid account state proof")]
    InvalidAccountStateProof,
    #[error("Invalid block")]
    InvalidBlock,
    #[error("Unknown")]
    Unknown,
    #[error("Not ready")]
    NotReady,
}

impl QueryError {
    pub fn code(&self) -> i64 {
        match self {
            QueryError::ConnectionError => -32001,
            QueryError::FailedToSerialize => -32002,
            QueryError::LiteServer(_) => -32003,
            QueryError::InvalidAccountStateProof => -32004,
            QueryError::InvalidBlock => -32006,
            QueryError::NotReady => -32007,
            QueryError::Unknown => -32603,
        }
    }
}
