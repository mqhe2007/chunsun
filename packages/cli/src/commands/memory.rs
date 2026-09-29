use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::json;

use crate::api::ApiClient;
use crate::commands::{print_conflict, print_json, CmdError, CmdResult};
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
        /// 乐观锁版本号：**必填**。取自 `memory get` 输出的「版本」或 --json 的 revision。
        /// 尚无项目记忆时传 0。
        #[arg(long)]
        revision: i64,
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
    /// 乐观锁版本号。`#[serde(default)]` 的理由与知识文档那边相同（兼容旧后端：
    /// 缺字段时退化成 0，写回会拿到一个指得准的错误，而不是「解析响应失败」）。
    #[serde(default)]
    revision: i64,
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
            println!("版本（写回时传 --revision）: {}", data.revision);
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

fn run_memory_put(snapshot_raw: String, revision: i64, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = memory_path(&config.project_id);

    let result: MemoryResponse = match api.put(
        &path,
        json!({ "snapshot": snapshot_raw, "revision": revision }),
    ) {
        Ok(r) => r,
        // 409 不走 `?`：`From<ApiError> for CmdError` 只留一行 Display 文本，
        // 而冲突的关键信息（currentRevision / currentSnapshot）全在 data 里。
        Err(e) if e.code() == Some("MEMORY_CONFLICT") => return Err(print_conflict(&e)),
        Err(e) => return Err(e.into()),
    };
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
    // 写后即给新版本号：`memory put` 之后紧接着再 put 一次是常见动作
    // （分两段补充记录），少了这一行就要重新 get 一次。
    println!("  新版本（下次 --revision）: {}", data.revision);
    let chars = data.snapshot.as_ref().map(|s| s.chars().count()).unwrap_or(0);
    println!("  字符数: {chars}");
    Ok(())
}

pub fn run(args: MemoryArgs) -> CmdResult {
    match args.command {
        MemoryCmd::Get { json } => run_memory_get(json),
        MemoryCmd::Put {
            snapshot,
            revision,
            json,
        } => run_memory_put(snapshot, revision, json),
    }
}
