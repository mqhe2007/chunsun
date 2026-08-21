use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::json;

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;
use crate::runtime_env::{has_process_env_key, list_local_dotenv_keys};

#[derive(Args)]
pub struct EnvArgs {
    #[command(subcommand)]
    command: EnvCmd,
}

#[derive(Subcommand)]
enum EnvCmd {
    /// 列出环境变量：平台登记 ∪ 本地 .env* 键名（不含明文；可含仅本地项）
    List {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        prefix: Option<String>,
    },
    /// 获取单个环境变量生效值（本地优先，否则实时拉平台）
    Get {
        key: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvVarListItem {
    id: String,
    key: String,
    description: Option<String>,
    is_secret: bool,
    #[allow(dead_code)]
    value: Option<String>,
    has_value: bool,
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct EnvVarValue {
    id: String,
    key: String,
    value: String,
    description: Option<String>,
    is_secret: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    success: bool,
    data: Option<Vec<EnvVarListItem>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetResponse {
    success: bool,
    data: Option<EnvVarValue>,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MergedEnvListItem {
    key: String,
    /// platform = 仅平台；local = 仅本地 dotenv；both = 两边都有
    source: &'static str,
    on_platform: bool,
    has_local: bool,
    is_secret: Option<bool>,
    description: Option<String>,
    has_value: Option<bool>,
    id: Option<String>,
}

fn merge_env_list(
    platform_items: &[EnvVarListItem],
    local_keys: &[String],
    prefix: Option<&str>,
) -> Vec<MergedEnvListItem> {
    let platform_by_key: std::collections::HashMap<&str, &EnvVarListItem> = platform_items
        .iter()
        .map(|item| (item.key.as_str(), item))
        .collect();
    let local_set: std::collections::HashSet<&str> =
        local_keys.iter().map(String::as_str).collect();

    let mut all_keys: Vec<String> = platform_by_key
        .keys()
        .map(|k| (*k).to_string())
        .chain(local_set.iter().map(|k| (*k).to_string()))
        .collect();
    all_keys.sort();
    all_keys.dedup();

    let mut merged = Vec::new();
    for key in all_keys {
        if let Some(p) = prefix {
            if !key.starts_with(p) {
                continue;
            }
        }
        let platform = platform_by_key.get(key.as_str()).copied();
        let has_local_file = local_set.contains(key.as_str());
        let has_local = has_local_file || has_process_env_key(&key);

        if let Some(platform) = platform {
            if has_local_file {
                merged.push(MergedEnvListItem {
                    key,
                    source: "both",
                    on_platform: true,
                    has_local,
                    is_secret: Some(platform.is_secret),
                    description: platform.description.clone(),
                    has_value: Some(platform.has_value),
                    id: Some(platform.id.clone()),
                });
            } else {
                merged.push(MergedEnvListItem {
                    key,
                    source: "platform",
                    on_platform: true,
                    has_local,
                    is_secret: Some(platform.is_secret),
                    description: platform.description.clone(),
                    has_value: Some(platform.has_value),
                    id: Some(platform.id.clone()),
                });
            }
        } else {
            merged.push(MergedEnvListItem {
                key,
                source: "local",
                on_platform: false,
                has_local: true,
                is_secret: None,
                description: None,
                has_value: None,
                id: None,
            });
        }
    }
    merged
}

pub fn run(args: EnvArgs) -> CmdResult {
    match args.command {
        EnvCmd::List { json, prefix } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: ListResponse =
                api.get(&format!("/projects/{}/env-vars", config.project_id))?;
            if !result.success {
                return Err(CmdError::new(
                    result
                        .error
                        .unwrap_or_else(|| "获取环境变量清单失败".into()),
                ));
            }
            let platform_data = result.data.unwrap_or_default();
            let prefix = prefix
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let local_keys = list_local_dotenv_keys(None, None);
            let items = merge_env_list(&platform_data, &local_keys, prefix);

            if json {
                return print_json(&items);
            }

            if items.is_empty() {
                if let Some(p) = prefix {
                    println!(
                        "[chunsun] 无匹配前缀 {p} 的环境变量（平台 + 本地 .env*）。"
                    );
                } else {
                    println!(
                        "[chunsun] 平台与本地 .env* 皆无环境变量键。可在控制台添加共享项，或在本地 .env 写私有项。"
                    );
                }
                return Ok(());
            }

            let both: Vec<_> = items.iter().filter(|i| i.source == "both").collect();
            let platform_only: Vec<_> =
                items.iter().filter(|i| i.source == "platform").collect();
            let local_only: Vec<_> = items.iter().filter(|i| i.source == "local").collect();

            println!(
                "[chunsun] 环境变量清单（平台 {} + 本地 dotenv 键 {}；合并显示 {}{}）：",
                platform_data.len(),
                local_keys.len(),
                items.len(),
                prefix
                    .map(|p| format!("；前缀 {p}"))
                    .unwrap_or_default()
            );

            let print_group = |title: &str, group: &[&MergedEnvListItem]| {
                if group.is_empty() {
                    return;
                }
                println!("\n[{title}]");
                for item in group {
                    let mut flags = vec![item.source];
                    match item.is_secret {
                        Some(true) => flags.push("secret"),
                        Some(false) => flags.push("plain"),
                        None => {}
                    }
                    if item.on_platform && item.has_local {
                        flags.push("local-loaded");
                    }
                    let desc = item
                        .description
                        .as_ref()
                        .map(|d| format!(" — {d}"))
                        .unwrap_or_default();
                    println!("  {}  ({}){desc}", item.key, flags.join(", "));
                }
            };

            print_group("两边都有（本地优先）", &both);
            print_group("仅平台（执行前需 get 或写入本地）", &platform_only);
            print_group("仅本地 .env*（未登记平台）", &local_only);

            println!(
                "\n[chunsun] 提示：按即将执行的用例/脚本引用的 key 补齐；平台缺本地时用 `chunsun env get <KEY>`。列表不含明文。"
            );
            Ok(())
        }
        EnvCmd::Get { key, json } => {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                return Err(CmdError::new("KEY 不能为空"));
            }

            if has_process_env_key(trimmed) {
                let value = std::env::var(trimmed).unwrap_or_default();
                if json {
                    let payload = json!({
                        "key": trimmed,
                        "value": value,
                        "source": "local",
                        "description": null,
                    });
                    return print_json(&payload);
                }
                println!("[chunsun] {trimmed}={value}");
                println!("[chunsun] source=local（shell 或 .env，优先于平台）");
                return Ok(());
            }

            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: GetResponse = api.get(&format!(
                "/projects/{}/env-vars/by-key/{}",
                config.project_id,
                urlencoding::encode(trimmed)
            ))?;
            if !result.success {
                return Err(CmdError::new(
                    result
                        .error
                        .unwrap_or_else(|| format!("环境变量 {trimmed} 不存在")),
                ));
            }
            let data = result.data.unwrap();
            let payload = json!({
                "key": data.key,
                "value": data.value,
                "source": "platform",
                "isSecret": data.is_secret,
                "description": data.description,
            });

            if json {
                return print_json(&payload);
            }

            println!("[chunsun] {}={}", data.key, data.value);
            println!(
                "[chunsun] source=platform{}",
                if data.is_secret { " (secret)" } else { "" }
            );
            if let Some(desc) = &data.description {
                println!("[chunsun] {desc}");
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform_item(
        id: &str,
        key: &str,
        description: Option<&str>,
        is_secret: bool,
    ) -> EnvVarListItem {
        EnvVarListItem {
            id: id.into(),
            key: key.into(),
            description: description.map(str::to_string),
            is_secret,
            value: None,
            has_value: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn merges_platform_and_local_only_keys() {
        let platform = vec![
            platform_item("1", "SHARED_TOKEN", Some("shared"), true),
            platform_item("2", "PLATFORM_ONLY", None, false),
        ];
        let local = vec!["SHARED_TOKEN".into(), "YOYO_LOCAL_ONLY".into()];
        let merged = merge_env_list(&platform, &local, None);
        let keys: Vec<&str> = merged.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["PLATFORM_ONLY", "SHARED_TOKEN", "YOYO_LOCAL_ONLY"]
        );
        assert_eq!(
            merged.iter().find(|i| i.key == "SHARED_TOKEN").unwrap().source,
            "both"
        );
        assert_eq!(
            merged
                .iter()
                .find(|i| i.key == "PLATFORM_ONLY")
                .unwrap()
                .source,
            "platform"
        );
        assert_eq!(
            merged
                .iter()
                .find(|i| i.key == "YOYO_LOCAL_ONLY")
                .unwrap()
                .source,
            "local"
        );
    }

    #[test]
    fn filters_by_prefix() {
        let platform = vec![
            platform_item("1", "SHARED_TOKEN", Some("shared"), true),
            platform_item("2", "PLATFORM_ONLY", None, false),
        ];
        let local = vec!["YOYO_A".into(), "OTHER".into()];
        let merged = merge_env_list(&platform, &local, Some("YOYO_"));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].key, "YOYO_A");
        assert_eq!(merged[0].source, "local");
    }
}
