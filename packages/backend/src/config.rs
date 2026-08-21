//! 应用配置：从环境变量（根目录 .env）加载。

use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    /// JWT 有效期字符串，如 "2h" / "30m"，兼容 jose 风格 timespan。
    pub jwt_expires_in: String,
    pub port: u16,
    pub api_prefix: String,
    pub node_env: Option<String>,
    /// 可选：环境变量 at-rest 加密密钥（原始/base64/hex）；缺省从 JWT_SECRET 派生。
    pub env_var_encryption_key: Option<String>,
    pub public_origin: String,
}

/// 默认监听端口。
pub const DEFAULT_LISTEN_PORT: u16 = 11111;

impl AppConfig {
    /// 监听端口：环境变量 `PORT` 或默认 `DEFAULT_LISTEN_PORT`（安装完成前即可绑定）。
    pub fn listen_port_from_env() -> u16 {
        env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_LISTEN_PORT)
    }

    /// 从进程环境加载。JWT_SECRET 缺失直接报错（与旧后端语义一致）。
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL is not set".to_string())?;
        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET is not set".to_string())?;
        let jwt_expires_in = env::var("JWT_EXPIRES_IN").unwrap_or_else(|_| "2h".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_LISTEN_PORT);
        let api_prefix = env::var("API_PREFIX").unwrap_or_else(|_| "/api/v1".to_string());
        let node_env = env::var("NODE_ENV").ok();
        let env_var_encryption_key = env::var("ENV_VAR_ENCRYPTION_KEY").ok();
        let public_origin = env::var("PUBLIC_ORIGIN").ok().unwrap_or_else(|| {
            let host = env::var("HOST").unwrap_or_else(|_| "localhost".to_string());
            format!("http://{host}:{port}")
        });

        Ok(Self {
            database_url,
            jwt_secret,
            jwt_expires_in,
            port,
            api_prefix,
            node_env,
            env_var_encryption_key,
            public_origin,
        })
    }

    /// 生产模式（NODE_ENV=production）时启用 HSTS。
    pub fn is_production(&self) -> bool {
        self.node_env.as_deref() == Some("production")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// env 变更是全局状态，测试并行会互相踩；串行化。
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn parses_defaults_with_required_keys() {
        let _guard = env_lock().lock().unwrap();
        unsafe {
            env::set_var("DATABASE_URL", "postgresql://u:p@localhost:5432/db");
            env::set_var("JWT_SECRET", "s3cret");
            env::remove_var("JWT_EXPIRES_IN");
            env::remove_var("PORT");
            env::remove_var("API_PREFIX");
            env::remove_var("NODE_ENV");
            env::remove_var("ENV_VAR_ENCRYPTION_KEY");
            env::remove_var("PUBLIC_ORIGIN");
            env::remove_var("HOST");
        }
        let cfg = AppConfig::from_env().unwrap();
        assert_eq!(cfg.jwt_expires_in, "2h");
        assert_eq!(cfg.port, DEFAULT_LISTEN_PORT);
        assert_eq!(cfg.api_prefix, "/api/v1");
        assert!(!cfg.is_production());
        assert_eq!(
            cfg.public_origin,
            format!("http://localhost:{DEFAULT_LISTEN_PORT}")
        );
    }

    #[test]
    fn missing_jwt_secret_errors() {
        let _guard = env_lock().lock().unwrap();
        unsafe {
            env::set_var("DATABASE_URL", "postgresql://u:p@localhost:5432/db");
            env::remove_var("JWT_SECRET");
        }
        assert!(AppConfig::from_env().is_err());
    }
}
