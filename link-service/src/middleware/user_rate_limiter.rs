use axum::{
    body::Body,
    extract::{State, Extension},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response
};
use tracing::warn;
use std::sync::Arc;
use crate::state::AppState;
use rand::{rng, seq::IndexedRandom};

use common::rate_limiter::rate_limit;


pub async fn user_rate_limiter(
    State(state): State<Arc<AppState>>,
    Extension(user_id): Extension<u64>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let key = format!("rate_limit:user:{}", user_id);
    // 从配置中读取限流参数
    let (limit, window_secs) = {
        let config = state.config.read().await;
        (config.user_rate_limit, config.user_rate_limit_window)
    };
    
    // 用户访问限流
    {
        // 获取redis连接
        let manager = state.managers
            .choose(&mut rng())
            .ok_or_else(|| {
                warn!("user_rate_limiter: 没有可用 Redis 连接池, user_id={}", user_id);
                (StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into())
            })?;

        let mut conn = manager.lock().await;
        
        if let Err(e) = rate_limit(&key, window_secs, limit, &mut conn).await {
            warn!("user_rate_limiter: 限流失败, user_id={}, error={}", user_id, e);
            return Err((StatusCode::TOO_MANY_REQUESTS, "Too many requests".into()));
        }
    }
    
    Ok(next.run(req).await)
}