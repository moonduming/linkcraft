use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::warn;

use crate::state::AppState;
use common::rate_limiter::rate_limit;

/// 获取真实 IP
pub async fn real_ip_layer(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let headers = req.headers();

    // 优先从 X-Forwarded-For 取最后一个 IP（nginx 追加 $remote_addr 在末尾）；
    // 若无则退回 X-Real-IP；最终再退回 "unknown"。
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.rsplit(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    // 将拥有所有权的 String 放入 extensions（不要插入 &str/Option<&str>）。
    req.extensions_mut().insert(ip);

    Ok(next.run(req).await)
}

/// ip 限流
pub async fn ip_rate_limiter(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // 优先取 real_ip_layer 注入的真实 IP；否则回退到 TCP 源地址
    let ip: String = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_else(|| addr.ip().to_string());

    let key = format!("rate_limit:ip:{}", ip);
    // 从配置中获取限流参数
    let (limit, window_secs) = {
        let cfg = state.config.read().await;
        (cfg.ip_rate_limit, cfg.ip_rate_limit_window)
    };

    // ip 限流校验
    {
        let mut conn = state.redis_pool.get().await.map_err(|e| {
            warn!("ip_rate_limiter: Redis 获取连接失败: err={}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Redis err: {}", e),
            )
        })?;

        if let Err(e) = rate_limit(&key, limit, window_secs, &mut conn).await {
            warn!("ip_rate_limiter ip限流校验失败: ip={}, err={}", ip, e);
            // todo 返回429是否合适？里面还有redis操作错误的返回
            return Err((StatusCode::TOO_MANY_REQUESTS, "Redis error".into()));
        }
    }

    Ok(next.run(req).await)
}
