use axum::{
    Router,
    routing::{get, post},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpListener,
    sync::{RwLock, mpsc::channel},
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::fmt::time::LocalTime;

use common::db;
use dashmap::DashSet;
use link_service::handlers;
use link_service::middleware::{
    auth::jwt_auth, ip_rate_limiter::ip_rate_limiter, real_ip_layer::real_ip_layer,
    user_rate_limiter::user_rate_limiter,
};
use link_service::state::AppState;
use link_service::{
    config::AppConfig,
    services::background_jobs::{BackgroundJob, spawn_background_workers},
};

#[tokio::main]
async fn main() {
    // 初始化全局日志
    tracing_subscriber::fmt::fmt()
        .with_timer(LocalTime::rfc_3339())
        .init();

    let cfg = AppConfig::from_env().expect("load config");
    // 初始化数据库连接池
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

    // 构建管道
    let (tx, rx) = channel::<BackgroundJob>(cfg.bg_redis_queue_cap);
    let bg_redis_max_concurrency = cfg.bg_redis_max_concurrency;

    let state = Arc::new(AppState {
        mysql_pool,
        redis_pool,
        config: RwLock::new(cfg),
        bg_jobs_tx: tx,
        pending_set: DashSet::new(),
    });

    spawn_background_workers(state.clone(), rx, bg_redis_max_concurrency);

    let public = Router::new()
        .route("/s/{short_code}", get(handlers::redirect))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ip_rate_limiter,
        ));

    // 保护路由
    let protected = Router::new()
        .route("/shorten", post(handlers::create))
        .route("/links", get(handlers::list_links))
        .route("/delete", post(handlers::delete_links))
        .route("/stats", get(handlers::get_link_stats))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            user_rate_limiter,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            jwt_auth,
        ));

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .merge(protected)
        .merge(public)
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(real_ip_layer))
        .with_state(state);

    // 启动服务
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
