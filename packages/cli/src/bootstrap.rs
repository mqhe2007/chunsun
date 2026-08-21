use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::ApiClient;
use crate::config::load_config;

pub fn should_skip_bootstrap(argv: &[String]) -> bool {
    let first = argv.first().map(String::as_str);
    let has_help = argv.iter().any(|a| a == "--help" || a == "-h" || a == "help");
    let has_version = argv.iter().any(|a| a == "--version" || a == "-v");
    let is_no_auth = matches!(first, Some("update") | Some("_refresh-templates"));
    first.is_none() || is_no_auth || has_help || has_version
}

#[derive(Debug, Deserialize)]
struct SecretKeyInfoResponse {
    success: bool,
    data: Option<SecretKeyInfo>,
    error: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SecretKeyInfo {
    project_id: String,
    #[allow(dead_code)]
    project_name: String,
}

#[derive(Debug, serde::Serialize, Deserialize)]
struct SkCacheFile {
    token_hash: u64,
    project_id: String,
    project_name: String,
    cached_at: u64,
}

const SK_CACHE_TTL_SECS: u64 = 6 * 60 * 60;

fn token_hash(token: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sk_cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".chunsun").join("cache").join("sk-info.json")
}

fn read_sk_cache(token: &str) -> Option<SecretKeyInfo> {
    let path = sk_cache_path();
    let raw = fs::read_to_string(path).ok()?;
    let cached: SkCacheFile = serde_json::from_str(&raw).ok()?;
    if cached.token_hash != token_hash(token) {
        return None;
    }
    if now_secs().saturating_sub(cached.cached_at) > SK_CACHE_TTL_SECS {
        return None;
    }
    Some(SecretKeyInfo {
        project_id: cached.project_id,
        project_name: cached.project_name,
    })
}

fn write_sk_cache(token: &str, info: &SecretKeyInfo) {
    let path = sk_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let payload = SkCacheFile {
        token_hash: token_hash(token),
        project_id: info.project_id.clone(),
        project_name: info.project_name.clone(),
        cached_at: now_secs(),
    };
    if let Ok(raw) = serde_json::to_string(&payload) {
        let _ = fs::write(path, raw);
    }
}

fn apply_project_env(info: &SecretKeyInfo) {
    std::env::set_var("_CHUNSUN_PROJECT_ID", &info.project_id);
    std::env::set_var("_CHUNSUN_PROJECT_NAME", &info.project_name);
}

/// 从 Secret Key 解析绑定项目 ID，写入环境变量。
pub fn bootstrap_secret_key() -> Result<(), String> {
    let config = load_config();
    if !config.token.starts_with("sk_") {
        return Ok(());
    }

    // 已有项目 ID（环境变量或上次写入）则跳过网络
    if !config.project_id.is_empty() {
        std::env::set_var("_CHUNSUN_PROJECT_ID", &config.project_id);
        return Ok(());
    }

    if let Some(cached) = read_sk_cache(&config.token) {
        apply_project_env(&cached);
        return Ok(());
    }

    let client = ApiClient::new(&config).map_err(|e| e.to_string())?;
    let res: SecretKeyInfoResponse = client
        .get("/auth/secret-key-info")
        .map_err(|_e| {
            format!(
                "无法连接到服务器（{}），请检查网络或服务是否可用。",
                config.api_base_url
            )
        })?;

    if res.success {
        if let Some(data) = res.data {
            write_sk_cache(&config.token, &data);
            apply_project_env(&data);
            return Ok(());
        }
    }

    Err(res
        .error
        .unwrap_or_else(|| "Secret Key 无效或已撤销，请在项目详情页重新生成。".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_bootstrap_for_help_version_update() {
        assert!(should_skip_bootstrap(&[]));
        assert!(should_skip_bootstrap(&["update".into()]));
        assert!(should_skip_bootstrap(&["_refresh-templates".into()]));
        assert!(should_skip_bootstrap(&["--help".into()]));
        assert!(should_skip_bootstrap(&["-v".into()]));
        assert!(!should_skip_bootstrap(&["init".into()]));
    }
}
