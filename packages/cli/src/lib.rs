pub mod api;
pub mod bootstrap;
pub mod commands;
pub mod config;
pub mod harness;
pub mod ide;
pub mod loading;
pub mod runtime_env;

pub use api::ApiClient;
pub use config::{load_config, resolve_api_base_url, CliConfig};
pub use runtime_env::load_runtime_env;

/// CLI 版本（来自根 package.json，由 build.rs 注入）
pub fn version() -> &'static str {
    env!("CHUNSUN_VERSION")
}

pub fn default_api_url() -> &'static str {
    option_env!("CHUNSUN_DEFAULT_API_URL").unwrap_or("")
}

pub fn default_cli_download_url() -> &'static str {
    option_env!("CHUNSUN_DEFAULT_CLI_DOWNLOAD_URL").unwrap_or("")
}
