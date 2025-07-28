//! 数据库连接
use redis::{Client as RedisClient, RedisError};
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};


/// 创建 redis 连接
pub async fn new_redis_client(redis_url: &str) -> Result<RedisClient, RedisError> {
    RedisClient::open(redis_url)
}


/// 创建 mysql 连接池
pub async fn new_mysql_pool(mysql_url: &str, max_connections: u32) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(max_connections)
        .connect(mysql_url)
        .await
}