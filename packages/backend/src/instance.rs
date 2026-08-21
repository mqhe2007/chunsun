//! 实例配置：安装向导写入可执行文件同级的 `chunsun.json`。
//!
//! 这不是给用户编辑的 dotenv。生产路径以向导为准；debug 构建仍可从环境变量启动以便本地开发。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{AppConfig, DEFAULT_LISTEN_PORT};

pub const INSTANCE_FILE_NAME: &str = "chunsun.json";
pub const INSTANCE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceFile {
    pub version: u32,
    pub database_url: String,
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expires")]
    pub jwt_expires_in: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String,
    pub public_origin: String,
    #[serde(default)]
    pub env_var_encryption_key: Option<String>,
}

fn default_jwt_expires() -> String {
    "2h".into()
}
fn default_port() -> u16 {
    DEFAULT_LISTEN_PORT
}
fn default_api_prefix() -> String {
    "/api/v1".into()
}

impl InstanceFile {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            version: INSTANCE_VERSION,
            database_url: config.database_url.clone(),
            jwt_secret: config.jwt_secret.clone(),
            jwt_expires_in: config.jwt_expires_in.clone(),
            port: config.port,
            api_prefix: config.api_prefix.clone(),
            public_origin: config.public_origin.clone(),
            env_var_encryption_key: config.env_var_encryption_key.clone(),
        }
    }

    pub fn into_config(self) -> AppConfig {
        AppConfig {
            database_url: self.database_url,
            jwt_secret: self.jwt_secret,
            jwt_expires_in: self.jwt_expires_in,
            port: self.port,
            api_prefix: self.api_prefix,
            node_env: if self.public_origin.starts_with("https://") {
                Some("production".into())
            } else {
                None
            },
            env_var_encryption_key: self.env_var_encryption_key,
            public_origin: self.public_origin,
        }
    }
}

/// 解析实例配置路径：`CHUNSUN_CONFIG` 优先，否则可执行文件同级 `chunsun.json`。
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("CHUNSUN_CONFIG") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join(INSTANCE_FILE_NAME)
}

pub fn load_file(path: &Path) -> Result<InstanceFile, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取实例配置失败: {e}"))?;
    let file: InstanceFile =
        serde_json::from_str(&raw).map_err(|e| format!("解析实例配置失败: {e}"))?;
    if file.database_url.trim().is_empty() || file.jwt_secret.trim().is_empty() {
        return Err("实例配置缺少 databaseUrl 或 jwtSecret".into());
    }
    if file.public_origin.trim().is_empty() {
        return Err("实例配置缺少 publicOrigin".into());
    }
    Ok(file)
}

pub fn save_file(path: &Path, file: &InstanceFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(file).map_err(|e| format!("序列化配置失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes()).map_err(|e| format!("写入配置失败: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("提交配置失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// 由向导字段拼 Postgres URL（密码/用户名做百分号编码）。
pub fn postgres_url(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    database: &str,
    ssl: bool,
) -> String {
    let user = urlencoding::encode(user);
    let password = urlencoding::encode(password);
    let sslmode = if ssl { "require" } else { "disable" };
    format!("postgresql://{user}:{password}@{host}:{port}/{database}?sslmode={sslmode}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn postgres_url_encodes_special_chars() {
        let url = postgres_url("db.local", 5432, "u@me", "p@ss:w/d", "app", false);
        assert!(url.starts_with("postgresql://u%40me:p%40ss%3Aw%2Fd@db.local:5432/app"));
        assert!(url.contains("sslmode=disable"));
    }

    #[test]
    fn roundtrip_instance_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(INSTANCE_FILE_NAME);
        let file = InstanceFile {
            version: 1,
            database_url: "postgresql://u:p@localhost:5432/db".into(),
            jwt_secret: "secret-secret-secret-secret-32ch".into(),
            jwt_expires_in: "2h".into(),
            port: DEFAULT_LISTEN_PORT,
            api_prefix: "/api/v1".into(),
            public_origin: "https://example.com".into(),
            env_var_encryption_key: None,
        };
        save_file(&path, &file).unwrap();
        let loaded = load_file(&path).unwrap();
        assert_eq!(loaded, file);
        let cfg = loaded.into_config();
        assert_eq!(cfg.public_origin, "https://example.com");
        assert!(cfg.is_production());
    }

    #[test]
    fn config_path_env_override() {
        let _guard = env_lock();
        unsafe {
            std::env::set_var("CHUNSUN_CONFIG", "/tmp/custom-chunsun.json");
        }
        assert_eq!(config_path(), PathBuf::from("/tmp/custom-chunsun.json"));
        unsafe {
            std::env::remove_var("CHUNSUN_CONFIG");
        }
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap()
    }
}
