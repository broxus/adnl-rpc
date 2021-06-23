use std::sync::Arc;

use anyhow::Error;
use async_trait::async_trait;
use bb8::PooledConnection;
use std::ops::DerefMut;
use tiny_adnl::{AdnlTcpClient, AdnlTcpClientConfig};

pub struct AdnlManageConnection {
    config: AdnlTcpClientConfig,
}

impl AdnlManageConnection {
    pub fn new(config: AdnlTcpClientConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl bb8::ManageConnection for AdnlManageConnection {
    type Connection = Arc<AdnlTcpClient>;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        log::debug!("Establishing adnl connection...");
        let connection = AdnlTcpClient::connect(self.config.clone()).await?;
        log::debug!("Established adnl connection");

        Ok(connection)
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        log::trace!("Check if connection is valid...");
        conn.deref_mut().ping(10).await?;
        log::trace!("Connection is valid");

        Ok(())
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}
