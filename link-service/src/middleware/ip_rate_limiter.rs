use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::warn;

use crate::state::AppState;
use axum::extract::ConnectInfo;
use common::rate_limiter::rate_limit;
use std::net::SocketAddr;

pub async fn ip_rate_limiter(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let ip = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or(addr.ip().to_string());

    let key = format!("rate_limit:ip:{}", ip);
    // 从配置中获取限流参数
    let (limit, window_secs) = {
        let cfg = state.config.read().await;
        (cfg.ip_rate_limit, cfg.ip_rate_limit_window)
    };

    // ip 限流校验
    {
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("ip_rate_limiter: 获取 Redis 连接失败: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Redis error".into())
        })?;

        if let Err(e) = rate_limit(&key, limit, window_secs, &mut conn).await {
            warn!("ip_rate_limiter ip限流校验失败: ip={}, err={}", ip, e);
            // todo 返回429是否合适？里面还有redis操作错误的返回
            return Err((StatusCode::TOO_MANY_REQUESTS, "Redis error".into()));
        }
    }

    Ok(next.run(req).await)
}
