use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::json;

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    command: MemoryCmd,
}

#[derive(Subcommand)]
enum MemoryCmd {
    /// 拉取项目级记忆（跨需求复用的填坑/经验/沉淀，属于项目知识库之一）
    Get {
        #[arg(long)]
        json: bool,
    },
    /// 全量覆盖写回项目级记忆（Markdown 文本，仅可编辑不可删除）
    Put {
        /// snapshot Markdown 字符串，例如 '## 填坑记录\n- ...'
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMemoryRow {
    id: String,
    project_id: String,
    snapshot: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct MemoryResponse {
    success: bool,
    data: Option<ProjectMemoryRow>,
    error: Option<String>,
}

fn memory_path(project_id: &str) -> String {
    format!("/projects/{project_id}/memory")
}

fn run_memory_get(json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = memory_path(&config.project_id);

    let handle_not_found = |json: bool| -> CmdResult {
        if json {
            return print_json(&json!({ "exists": false }));
        }
        println!("[chunsun] 暂无项目级记忆（Memory）。");
        println!("  写入：chunsun memory put --snapshot '## 填坑记录\\n- ...'");
        Ok(())
    };

    match api.get::<MemoryResponse>(&path) {
        Ok(result) => {
            if !result.success {
                let err = result.error.unwrap_or_else(|| "获取项目记忆失败".into());
                if err == "MEMORY_NOT_FOUND" {
                    return handle_not_found(json);
                }
                return Err(CmdError::new(err));
            }
            let data = result
                .data
                .ok_or_else(|| CmdError::new("获取项目记忆失败"))?;
            if json {
                return print_json(&data);
            }
            println!("项目记忆: {}", data.id);
            println!("更新: {}", data.updated_at);
            println!("snapshot:");
            match &data.snapshot {
                Some(text) if !text.is_empty() => println!("{text}"),
                _ => println!("（空）"),
            }
            Ok(())
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("MEMORY_NOT_FOUND") {
                handle_not_found(json)
            } else {
                Err(err.into())
            }
        }
    }
}

fn run_memory_put(snapshot_raw: String, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = memory_path(&config.project_id);

    let result: MemoryResponse = api.put(&path, json!({ "snapshot": snapshot_raw }))?;
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| "写入项目记忆失败".into()),
        ));
    }
    let data = result
        .data
        .ok_or_else(|| CmdError::new("写入项目记忆失败"))?;

    if json {
        return print_json(&data);
    }
    println!("[chunsun] 项目记忆已写入：{}", data.project_id);
    println!("  更新: {}", data.updated_at);
    let chars = data.snapshot.as_ref().map(|s| s.chars().count()).unwrap_or(0);
    println!("  字符数: {chars}");
    Ok(())
}

pub fn run(args: MemoryArgs) -> CmdResult {
    match args.command {
        MemoryCmd::Get { json } => run_memory_get(json),
        MemoryCmd::Put { snapshot, json } => run_memory_put(snapshot, json),
    }
}
