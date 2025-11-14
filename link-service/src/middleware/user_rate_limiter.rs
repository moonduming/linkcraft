use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Extension, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::warn;

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
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("user_rate_limiter: 获取 Redis 连接失败: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Redis error".into())
        })?;

        if let Err(e) = rate_limit(&key, limit, window_secs, &mut conn).await {
            warn!(
                "user_rate_limiter: 限流失败, user_id={}, error={}",
                user_id, e
            );
            return Err((StatusCode::TOO_MANY_REQUESTS, "Too many requests".into()));
        }
    }

    Ok(next.run(req).await)
}
