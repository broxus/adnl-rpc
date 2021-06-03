use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bb8::PooledConnection;
use ton_api::ton;
use ton_api::ton::ton_node::blockidext::BlockIdExt;

use super::adnl_pool::AdnlManageConnection;
use super::connection::*;
use super::errors::*;

pub struct LastBlock {
    state: parking_lot::RwLock<LastBlockState>,
    threshold: Duration,
    in_process: AtomicBool,
}

impl LastBlock {
    pub fn new(threshold: &Duration) -> Self {
        Self {
            state: parking_lot::RwLock::new(LastBlockState::new()),
            threshold: *threshold,
            in_process: AtomicBool::new(false),
        }
    }

    pub async fn last_cached_blocks(&self) -> impl Iterator<Item = BlockIdExt> {
        self.state.read().blocks.clone().into_iter()
    }

    pub async fn get_last_block(
        &self,
        connection: &mut PooledConnection<'_, AdnlManageConnection>,
    ) -> QueryResult<ton::ton_node::blockidext::BlockIdExt> {
        let now = {
            let state = self.state.read();

            let now = Instant::now();

            match &state.id {
                Some((result, last)) => {
                    if now.duration_since(*last) < self.threshold
                        || self
                            .in_process
                            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                            .is_err()
                    {
                        return result.clone();
                    }
                    now
                }
                None => now,
            }
        };

        log::debug!("Getting mc block");

        let id = query(connection, &ton::rpc::lite_server::GetMasterchainInfo)
            .await
            .and_then(QueryReply::try_into_data)
            .map(|result| result.only().last);

        log::debug!("Got mc block");

        let mut state = self.state.write();

        state.id = Some((id.clone(), now));

        if let Ok(new_id) = &id {
            match state.blocks.front() {
                Some(latest_id) if new_id.seqno > latest_id.seqno => {
                    if state.blocks.len() >= MAX_ENQUEUED_BLOCKS {
                        state.blocks.pop_back();
                    }
                    state.blocks.push_front(new_id.clone());
                }
                None => state.blocks.push_front(new_id.clone()),
                _ => {}
            }
        }

        self.in_process.store(false, Ordering::Release);

        id
    }
}

struct LastBlockState {
    id: Option<(QueryResult<BlockIdExt>, Instant)>,
    blocks: VecDeque<BlockIdExt>,
}

impl LastBlockState {
    fn new() -> Self {
        Self {
            id: None,
            blocks: VecDeque::with_capacity(MAX_ENQUEUED_BLOCKS),
        }
    }
}

const MAX_ENQUEUED_BLOCKS: usize = 5;
