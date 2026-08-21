//! 全局应用状态。安装未完成时没有连接池；完成后热加载。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use sqlx::PgPool;

use crate::config::AppConfig;
use crate::middleware::rate_limit::{RateLimitConfig, RateLimiter};

#[derive(Clone)]
struct Ready {
    pool: PgPool,
    config: AppConfig,
}

struct Inner {
    ready: RwLock<Option<Ready>>,
    config_path: PathBuf,
    listen_port: u16,
    rate_limiter: RateLimiter,
    auth_rate_limiter: RateLimiter,
    setup_lock: tokio::sync::Mutex<()>,
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

impl AppState {
    pub fn new(pool: PgPool, config: AppConfig) -> Self {
        let listen_port = config.port;
        Self {
            inner: Arc::new(Inner {
                ready: RwLock::new(Some(Ready { pool, config })),
                config_path: crate::instance::config_path(),
                listen_port,
                rate_limiter: RateLimiter::new(RateLimitConfig::GENERAL),
                auth_rate_limiter: RateLimiter::new(RateLimitConfig::AUTH),
                setup_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    pub fn setup(listen_port: u16, config_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                ready: RwLock::new(None),
                config_path,
                listen_port,
                rate_limiter: RateLimiter::new(RateLimitConfig::GENERAL),
                auth_rate_limiter: RateLimiter::new(RateLimitConfig::AUTH),
                setup_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.inner.ready.read().expect("ready lock").is_some()
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.inner.config_path
    }

    pub fn listen_port(&self) -> u16 {
        self.inner.listen_port
    }

    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.inner.rate_limiter
    }

    pub fn auth_rate_limiter(&self) -> &RateLimiter {
        &self.inner.auth_rate_limiter
    }

    pub async fn setup_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.inner.setup_lock.lock().await
    }

    pub fn mark_ready(&self, pool: PgPool, config: AppConfig) {
        *self.inner.ready.write().expect("ready lock") = Some(Ready { pool, config });
    }

    /// 就绪后修改内存配置并执行回调（由调用方负责落盘）。
    pub fn with_ready_config<F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut AppConfig) -> Result<(), String>,
    {
        let mut guard = self.inner.ready.write().expect("ready lock");
        let ready = guard
            .as_mut()
            .ok_or_else(|| "实例尚未就绪".to_string())?;
        f(&mut ready.config)
    }

    pub fn try_config(&self) -> Option<AppConfig> {
        self.inner
            .ready
            .read()
            .expect("ready lock")
            .as_ref()
            .map(|r| r.config.clone())
    }

    pub fn pool(&self) -> PgPool {
        self.inner
            .ready
            .read()
            .expect("ready lock")
            .as_ref()
            .expect("pool used before setup")
            .pool
            .clone()
    }

    pub fn config(&self) -> AppConfig {
        self.try_config().expect("config used before setup")
    }
}
