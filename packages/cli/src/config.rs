use std::process;

use crate::default_api_url;

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub api_base_url: String,
    pub token: String,
    pub project_id: String,
}

pub fn load_config() -> CliConfig {
    let project_id = std::env::var("_CHUNSUN_PROJECT_ID")
        .or_else(|_| std::env::var("CHUNSUN_PROJECT_ID"))
        .unwrap_or_default();

    CliConfig {
        api_base_url: resolve_api_base_url(),
        token: resolve_secret_key(),
        project_id,
    }
}

fn resolve_secret_key() -> String {
    if let Ok(sk) = std::env::var("CHUNSUN_SECRET_KEY") {
        if !sk.is_empty() {
            return sk;
        }
    }
    if let Ok(legacy) = std::env::var("CHUNSUN_TOKEN") {
        if !legacy.is_empty() {
            eprintln!(
                "[chunsun] 警告：CHUNSUN_TOKEN 已废弃，请改用 CHUNSUN_SECRET_KEY（格式：sk_xxx）"
            );
            return legacy;
        }
    }
    eprintln!("[chunsun] 缺少环境变量 CHUNSUN_SECRET_KEY，请先设置后再运行。");
    eprintln!("  示例: export CHUNSUN_SECRET_KEY=sk_<your-key>");
    eprintln!("  或者：在项目根目录下的 .env 文件中写入 CHUNSUN_SECRET_KEY=sk_<your-key>");
    process::exit(1);
}

/// API 服务地址：
/// 1. 运行时 CHUNSUN_API_URL
/// 2. 编译默认 CHUNSUN_DEFAULT_API_URL
/// 3. 源码直跑：localhost + PORT + API_PREFIX
pub fn resolve_api_base_url() -> String {
    if let Ok(override_url) = std::env::var("CHUNSUN_API_URL") {
        let trimmed = override_url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let embedded = default_api_url().trim();
    if !embedded.is_empty() {
        return embedded.to_string();
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "11111".into());
    let raw_prefix = std::env::var("API_PREFIX").unwrap_or_else(|_| "/api/v1".into());
    let prefix = if raw_prefix.starts_with('/') {
        raw_prefix
    } else {
        format!("/{raw_prefix}")
    };
    let prefix = prefix.trim_end_matches('/');
    let prefix = if prefix.is_empty() { "/api/v1" } else { prefix };
    format!("http://127.0.0.1:{port}{prefix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_runtime_override() {
        std::env::set_var("CHUNSUN_API_URL", "https://example.com/api/v1");
        assert_eq!(resolve_api_base_url(), "https://example.com/api/v1");
        std::env::remove_var("CHUNSUN_API_URL");
    }
}
