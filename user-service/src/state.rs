use crate::config::AppConfig;
use deadpool_redis::Pool;
use sqlx::MySqlPool;
use tokio::sync::RwLock;

pub struct AppState {
    pub mysql_pool: MySqlPool,
    pub redis_pool: Pool,
    pub config: RwLock<AppConfig>,
}
