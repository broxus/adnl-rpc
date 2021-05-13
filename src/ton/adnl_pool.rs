use adnl::client::{AdnlClient, AdnlClientConfig, AdnlConnection};
use anyhow::Error;
use async_trait::async_trait;
use bb8::PooledConnection;
use std::ops::DerefMut;

pub struct AdnlConnectionManager {
    inner: AdnlClient,
}

impl AdnlConnectionManager {
    pub fn connect(config: AdnlClientConfig) -> Self {
        Self {
            inner: AdnlClient::new(config),
        }
    }
}

#[async_trait]
impl bb8::ManageConnection for AdnlConnectionManager {
    type Connection = AdnlConnection;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.inner
            .get_connection()
            .await
            .map_err(|e| Error::msg(e.to_string()))
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        conn.deref_mut()
            .ping()
            .await
            .map(|_| ())
            .map_err(|e| Error::msg(e.to_string()))
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}
