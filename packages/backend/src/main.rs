//! 春笋平台入口：安装向导未完成时可先监听；完成后同一进程提供 API、控制台与 CLI 下载。

mod api;
mod auth;
mod config;
mod core;
mod db;
mod harness_template;
mod instance;
mod middleware;
mod repos;
mod routes;
mod services;
mod state;
mod static_files;

use std::net::SocketAddr;

use axum::http::{header, HeaderValue, Method};
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};

use config::AppConfig;
use state::AppState;

/// 启动时执行数据库迁移。
///
/// `migrations/` 下的 SQL 由 `sqlx::migrate!` 在编译期嵌入二进制，因此部署时
/// 只需分发可执行文件，不必附带迁移目录。baseline 是幂等的，对已有库为 no-op。
///
/// 设 `CHUNSUN_SKIP_MIGRATE=1` 可跳过（例如库由 DBA 单独管控的场景）。
async fn run_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    if std::env::var("CHUNSUN_SKIP_MIGRATE").as_deref() == Ok("1") {
        tracing::warn!("CHUNSUN_SKIP_MIGRATE=1，跳过数据库迁移");
        return Ok(());
    }
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("数据库迁移已就绪");
    Ok(())
}

enum Boot {
    Ready(AppConfig),
    Setup { port: u16 },
}

fn resolve_boot() -> anyhow::Result<Boot> {
    // 仅 debug 读取仓库 .env，方便本地开发；发布二进制不以 dotenv 为产品面。
    #[cfg(debug_assertions)]
    let _ = dotenvy::dotenv();

    if std::env::var("CHUNSUN_FORCE_SETUP").as_deref() == Ok("1") {
        return Ok(Boot::Setup {
            port: AppConfig::listen_port_from_env(),
        });
    }

    let path = instance::config_path();
    if path.is_file() {
        let file = instance::load_file(&path).map_err(|e| anyhow::anyhow!(e))?;
        tracing::info!(path = %path.display(), "已加载实例配置");
        return Ok(Boot::Ready(file.into_config()));
    }

    if std::env::var("DATABASE_URL").is_ok() && std::env::var("JWT_SECRET").is_ok() {
        let config = AppConfig::from_env().map_err(|e| anyhow::anyhow!(e))?;
        tracing::info!("使用环境变量启动（开发/运维逃生舱）");
        return Ok(Boot::Ready(config));
    }

    Ok(Boot::Setup {
        port: AppConfig::listen_port_from_env(),
    })
}

fn cors_layer(state: AppState) -> CorsLayer {
    let methods = [
        Method::GET,
        Method::POST,
        Method::DELETE,
        Method::PUT,
        Method::PATCH,
        Method::HEAD,
        Method::OPTIONS,
    ];
    let headers = [header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT];

    CorsLayer::new()
        .allow_methods(methods)
        .allow_headers(headers)
        .allow_origin(AllowOrigin::predicate(
            move |origin: &HeaderValue, _parts| origin_allowed(origin, &state),
        ))
}

fn origin_allowed(origin: &HeaderValue, state: &AppState) -> bool {
    if !state.is_ready() {
        return true;
    }
    let Ok(origin_str) = origin.to_str() else {
        return false;
    };
    let Some(cfg) = state.try_config() else {
        return true;
    };
    is_allowed_origin(origin_str, &cfg.public_origin)
}

fn is_allowed_origin(origin: &str, public_origin: &str) -> bool {
    let origin = origin.trim_end_matches('/');
    if origin == public_origin.trim_end_matches('/') {
        return true;
    }
    const DEV_ORIGINS: &[&str] = &[
        "http://127.0.0.1:11111",
        "http://localhost:11111",
    ];
    DEV_ORIGINS.contains(&origin)
}

fn build_api(state: AppState) -> Router<AppState> {
    let setup = routes::setup::router(state.clone());
    let ready = Router::new()
        .route("/health/db", axum::routing::get(routes::health::health_db))
        .nest("/auth", routes::auth::router(state.clone()))
        .nest("/users", routes::user::router(state.clone()))
        .merge(routes::project::router(state.clone()))
        .merge(routes::project_env_var::router(state.clone()))
        .merge(routes::project_member::router(state.clone()))
        .merge(routes::project_secret_key::router(state.clone()))
        .merge(routes::repository::router(state.clone()))
        .merge(routes::requirement::router(state.clone()))
        .merge(routes::defect::router(state.clone()))
        .merge(routes::activity::router(state.clone()))
        .merge(routes::project_knowledge::router(state.clone()))
        .merge(routes::system_setting::router(state.clone()))
        .merge(routes::instance_config::router(state.clone()))
        .merge(routes::invitation::router(state.clone()))
        .merge(routes::harness::router(state.clone()))
        .merge(routes::harness_template::router(state.clone()))
        .nest("/notifications", routes::notification::router(state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_ready::require_ready,
        ));

    Router::new()
        .route("/health", axum::routing::get(routes::health::health))
        .nest("/setup", setup)
        .merge(ready)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::security_headers::security_headers,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit::general_rate_limit,
        ))
        .layer(cors_layer(state.clone()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let boot = resolve_boot()?;
    let (state, port, prefix, setup_pending) = match boot {
        Boot::Ready(config) => {
            tracing::info!(port = config.port, prefix = %config.api_prefix, "启动春笋平台");
            let pool = db::create_pool(&config.database_url).await?;
            run_migrations(&pool).await?;
            let port = config.port;
            let prefix = config.api_prefix.clone();
            (AppState::new(pool, config), port, prefix, false)
        }
        Boot::Setup { port } => {
            let path = instance::config_path();
            tracing::info!(
                port,
                config = %path.display(),
                "未检测到实例配置，进入安装向导"
            );
            (
                AppState::setup(port, path),
                port,
                "/api/v1".to_string(),
                true,
            )
        }
    };

    let api = build_api(state.clone()).with_state(state);
    let app = Router::new()
        .nest(&prefix, api)
        .fallback(static_files::fallback);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    if setup_pending {
        tracing::info!(%addr, "listening — 打开 http://127.0.0.1:{port}/console/setup 完成安装");
    } else {
        tracing::info!(%addr, "listening");
    }
    axum::serve(listener, app).await?;
    Ok(())
}
