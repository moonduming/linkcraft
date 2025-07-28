use tracing_subscriber::fmt::time::LocalTime;
use tokio::{sync::{Mutex, RwLock}, net::TcpListener};
use std::sync::Arc;
use axum::{routing::{get, post}, Router};
use tower_http::trace::TraceLayer;

use common::db;
use link_service::config::AppConfig;
use link_service::state::AppState;
use link_service::middleware::{
    user_rate_limiter::user_rate_limiter, 
    auth::jwt_auth,
    real_ip_layer::real_ip_layer,
    ip_rate_limiter::ip_rate_limiter,
};
use link_service::handlers;



#[tokio::main]
async fn main() {
    // 初始化全局日志
    tracing_subscriber::fmt::fmt()
        .with_timer(LocalTime::rfc_3339())
        .init();

    let cfg = AppConfig::from_env().expect("load config");
    // 初始化数据
    let mysql_pool = db::new_mysql_pool(&cfg.database_url, cfg.max_connections)
        .await
        .expect("create mysql pool");
    let redis = db::new_redis_client(&cfg.redis_url)
        .await
        .expect("create redis client");

    let addr = cfg.addr.clone();

    // 初始化 4 条 Redis 连接并包进 Arc<Mutex<_>>
    let mut managers = Vec::new();
    for _ in 0..4 {
        let mgr = redis.get_connection_manager().await.unwrap();
        managers.push(Mutex::new(mgr));
    }

    let state = Arc::new(AppState {
        mysql_pool,
        managers,
        config: RwLock::new(cfg),
    });

    let public = Router::new()
        .route("/s/{short_code}", get(handlers::redirect))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            ip_rate_limiter
        ));

    // 保护路由
    let protected = Router::new()
        .route("/shorten", post(handlers::create))
        .route("/links", get(handlers::list_links))
        .route("/delete", post(handlers::delete_links))
        .route("/stats", get(handlers::get_link_stats))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            user_rate_limiter
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(), 
            jwt_auth
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
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };
    let make_svc = app.into_make_service();

    axum::serve(listener, make_svc)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();
}
