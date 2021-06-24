use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Error;
use async_trait::async_trait;
use bb8::PooledConnection;
use std::ops::DerefMut;
use tiny_adnl::{AdnlTcpClient, AdnlTcpClientConfig};

pub struct AdnlManageConnection {
    config: AdnlTcpClientConfig,
    unreliability: Arc<AtomicUsize>,
}

impl AdnlManageConnection {
    pub fn new(config: AdnlTcpClientConfig, unreliability: Arc<AtomicUsize>) -> Self {
        Self {
            config,
            unreliability,
        }
    }

    fn bump_unreliability(&self) {
        self.unreliability.fetch_add(1, Ordering::Release);
    }

    fn reset_unreliability(&self) {
        self.unreliability.store(0, Ordering::Release);
    }
}

#[async_trait]
impl bb8::ManageConnection for AdnlManageConnection {
    type Connection = Arc<AdnlTcpClient>;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        log::debug!("Establishing adnl connection...");
        match AdnlTcpClient::connect(self.config.clone()).await {
            Ok(connection) => {
                // Note: don't reset unreliability here, make sure that `ping` will be successful
                log::debug!("Established adnl connection");
                Ok(connection)
            }
            Err(e) => {
                self.bump_unreliability();
                log::debug!("Failed to establish adnl connection");
                Err(e)
            }
        }
    }

    async fn is_valid(&self, conn: &mut PooledConnection<'_, Self>) -> Result<(), Self::Error> {
        log::trace!("Check if connection is valid...");
        match conn.deref_mut().ping(10).await {
            Ok(_) => {
                self.reset_unreliability();
                log::trace!("Connection is valid");
                Ok(())
            }
            Err(e) => {
                self.bump_unreliability();
                log::trace!("Connection is invalid");
                Err(e)
            }
        }
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}
