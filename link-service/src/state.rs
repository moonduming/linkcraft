use crate::config::AppConfig;
use crate::services::background_jobs::BackgroundJob;
use dashmap::DashSet;
use deadpool_redis::Pool;
use sqlx::MySqlPool;
use tokio::sync::{RwLock, mpsc::Sender};

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum ScheduledJobKind {
    SyncClick,
    SyncVisitLog,
    DeleteExpired,
}

pub struct AppState {
    pub mysql_pool: MySqlPool,
    pub redis_pool: Pool,
    pub config: RwLock<AppConfig>,
    pub bg_jobs_tx: Sender<BackgroundJob>,
    pub pending_set: DashSet<ScheduledJobKind>,
}
