use axum::{
    Router,
    routing::{get, post},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tower_http::trace::TraceLayer;
use tracing_subscriber::fmt::time::LocalTime;

use common::db;
use user_service::config::AppConfig;
use user_service::handlers;
use user_service::middleware::{ip_rate_limiter, real_ip_layer};
use user_service::state::AppState;

#[tokio::main]
async fn main() {
    // 初始化全局日志
    tracing_subscriber::fmt::fmt()
        .with_timer(LocalTime::rfc_3339())
        .init();

    let cfg = AppConfig::from_env().expect("load config");
    // 初始化数据
    let mysql_pool = db::new_mysql_pool(
        &cfg.database_url,
        cfg.mysql_max_connections,
        cfg.mysql_acquire_timeout_ms,
        cfg.mysql_query_timeout_ms,
        cfg.mysql_lock_wait_timeout_s,
    )
    .await
    .unwrap();

    let redis_pool = db::new_redis_pool(
        &cfg.redis_url,
        cfg.redis_pool_size,
        cfg.redis_timeout_wait_ms,
        cfg.redis_timeout_create_ms,
        cfg.redis_timeout_recycle_ms,
    )
    .unwrap();

    let addr = cfg.addr.clone();

    let state = Arc::new(AppState {
        mysql_pool,
        redis_pool,
        config: RwLock::new(cfg),
    });

    let public = Router::new()
        .route("/login", post(handlers::login))
        .route("/register", post(handlers::register))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ip_rate_limiter,
        ));

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .merge(public)
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(real_ip_layer))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.unwrap();
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    let make_svc = app.into_make_service_with_connect_info::<SocketAddr>();

    axum::serve(listener, make_svc)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();
}
