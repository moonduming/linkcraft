use redis::{aio::ConnectionManager, AsyncCommands};
use tracing::warn;

pub async fn rate_limit(
    key: &str,
    limit: i64,
    window_secs: i64,
    redis_mgr: &mut ConnectionManager
) -> Result<(), String> {
    let count: i64 = redis_mgr.incr(key, 1).await.map_err(|e| {
        warn!("rate_limiter: Redis Incr 失败, key={}, err={}", key, e);
        format!("Redis Incr err: {}", e)
    })?;
    if count == 1 {
        // 第一次请求，设置过期时间
        let _: () = redis_mgr.expire(&key, window_secs)
            .await
            .map_err(|e| {
                warn!("rate_limiter: Redis Expire 失败, key={}, err={}", key, e);
                format!("Redis Expire err: {}", e)
            })?;
    }
    
    if count > limit {
        warn!("rate_limiter: 访问超限, key={}, limit={}, window={}", key, limit, window_secs);
        // 超出限制
        return Err("Too many requests".into())
    }
    Ok(())
}
