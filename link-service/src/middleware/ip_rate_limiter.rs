use std::sync::Arc;

use axum::{
    body::Body, 
    extract::State, 
    middleware::Next,
    response::Response, 
    http::{StatusCode, Request},
};
use rand::{rng, seq::IndexedRandom};
use tracing::warn;

use crate::state::AppState;
use common::rate_limiter::rate_limit;

pub async fn ip_rate_limiter(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let ip = req.extensions().get::<String>().ok_or_else(|| {
        warn!("ip_rate_limiter: 没有可用 IP");
        (StatusCode::INTERNAL_SERVER_ERROR, "No IP".into())
    })?;
    let key = format!("rate_limit:ip:{}", ip);
    // 从配置中获取限流参数
    let (limit, window_secs) = {
        let cfg = state.config.read().await;
        (cfg.ip_rate_limit, cfg.ip_rate_limit_window)
    };

    // ip 限流校验
    {
        let manager = state.managers
            .choose(&mut rng())
            .ok_or_else(|| {
                warn!("ip_rate_limiter: 没有可用 Redis 连接池, ip={}", ip);
                (StatusCode::INTERNAL_SERVER_ERROR, "No Redis manager".into())
            })?;
        let mut conn = manager.lock().await;

        if let Err(e) = rate_limit(&key, limit, window_secs, &mut conn).await {
            warn!("ip_rate_limiter ip限流校验失败: ip={}, err={}", ip, e);
            // todo 返回429是否合适？里面还有redis操作错误的返回
            return Err((StatusCode::TOO_MANY_REQUESTS, "Redis error".into()));
        }
    }

    Ok(next.run(req).await)

}