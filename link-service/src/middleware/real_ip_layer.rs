use axum::{
    body::Body, 
    http::Request, 
    middleware::Next, 
    response::Response, 
    http::StatusCode
};


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
