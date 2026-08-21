use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    command: Option<ContextCmd>,

    /// 仅无子命令时：输出项目整体开发上下文 JSON
    #[arg(long, global = false)]
    json: bool,
}

#[derive(Subcommand)]
enum ContextCmd {
    /// 拉取需求工作记忆（RequirementContext）
    Get {
        /// 需求 ID
        req: String,
        #[arg(long)]
        json: bool,
    },
    /// 增量写回需求工作记忆（顶层 key 合并后 PUT）
    Put {
        /// 需求 ID
        req: String,
        /// snapshot JSON 字符串，例如 '{"lastRunSummary":{...},"codeLandmarks":[...]}'
        #[arg(long)]
        snapshot: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Deserialize)]
struct ContextItem {
    // 后端自定义 context 文档经 serializeContextDocument 仅含 id/title/content/sortOrder/updatedAt，
    // 无 key/system；constitution 文档才有 key+system。全部设为可缺省以兼容真实响应体。
    #[serde(default)]
    key: Option<String>,
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContextResponse {
    success: bool,
    data: Option<ContextData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContextData {
    project: ProjectInfo,
    #[serde(default)]
    contexts: Vec<ContextItem>,
    summary: Summary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInfo {
    name: String,
    description: Option<String>,
    env_var_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Summary {
    requirements: CountBy,
    // 后端 summary 当前仅返回 requirements 与 envVars，无 board；置可缺省避免解码失败
    #[serde(default)]
    board: Option<CountBy>,
    #[serde(default, rename = "envVars")]
    env_vars: Option<EnvCount>,
}

#[derive(Debug, Deserialize)]
struct CountBy {
    total: u64,
}

#[derive(Debug, Deserialize)]
struct EnvCount {
    total: u64,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequirementContextRow {
    id: String,
    requirement_id: String,
    #[allow(dead_code)]
    project_id: String,
    snapshot: Value,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct RequirementContextResponse {
    success: bool,
    data: Option<RequirementContextRow>,
    error: Option<String>,
}

fn req_context_path(project_id: &str, req: &str) -> String {
    format!("/projects/{project_id}/requirements/{req}/context")
}

/// 顶层 key 增量合并：patch 覆盖同名 key，其余保留。
fn merge_snapshot(existing: &Value, patch: &Value) -> Value {
    let mut base = match existing {
        Value::Object(map) => Value::Object(map.clone()),
        _ => json!({}),
    };
    if let Some(obj) = patch.as_object() {
        if let Some(base_obj) = base.as_object_mut() {
            for (k, v) in obj {
                base_obj.insert(k.clone(), v.clone());
            }
        }
    }
    base
}

fn fetch_existing_snapshot(api: &ApiClient, path: &str) -> Result<Value, CmdError> {
    match api.get::<Value>(path) {
        Ok(raw) => {
            if raw.get("success").and_then(|v| v.as_bool()) == Some(true) {
                Ok(raw
                    .get("data")
                    .and_then(|d| d.get("snapshot"))
                    .cloned()
                    .unwrap_or_else(|| json!({})))
            } else {
                Ok(json!({}))
            }
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("CONTEXT_NOT_FOUND") || msg.contains("(404)") {
                Ok(json!({}))
            } else {
                Err(err.into())
            }
        }
    }
}

fn run_project_summary(json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let result: ContextResponse = api.get(&format!("/projects/{}/context", config.project_id))?;
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| "获取项目上下文失败".into()),
        ));
    }
    let data = result
        .data
        .ok_or_else(|| CmdError::new("获取项目上下文失败"))?;

    if json {
        let raw: Value = api.get(&format!("/projects/{}/context", config.project_id))?;
        if let Some(d) = raw.get("data") {
            return print_json(d);
        }
        return print_json(&raw);
    }

    println!("项目: {}", data.project.name);
    if let Some(desc) = &data.project.description {
        if !desc.is_empty() {
            println!("描述: {desc}");
        }
    }
    println!("\n需求: {}", data.summary.requirements.total);
    println!("看板: {}", data.summary.board.map(|b| b.total).unwrap_or(0));
    let env_total = data
        .summary
        .env_vars
        .as_ref()
        .map(|e| e.total)
        .or(data.project.env_var_count)
        .unwrap_or(0);
    println!("环境变量: {env_total}（不含值；用 chunsun env list / get）");

    println!("\ncontexts ({})：", data.contexts.len());
    if data.contexts.is_empty() {
        println!("  （无）");
    } else {
        for c in &data.contexts {
            let tag = if c.system { "system" } else { "custom" };
            let key = c
                .key
                .clone()
                .or_else(|| c.id.clone())
                .unwrap_or_default();
            let trimmed = c.content.trim();
            let preview = if trimmed.is_empty() {
                "（空）".to_string()
            } else if trimmed.chars().count() > 60 {
                format!("{}…", trimmed.chars().take(60).collect::<String>())
            } else {
                trimmed.to_string()
            };
            println!("  - [{tag}] {} (key={})", c.title, key);
            println!("    {preview}");
        }
    }
    println!("\n完整 JSON 含正文：chunsun context --json");
    println!("需求工作记忆：chunsun context get|put <需求ID>");
    Ok(())
}

fn run_get(req: String, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = req_context_path(&config.project_id, &req);

    match api.get::<RequirementContextResponse>(&path) {
        Ok(result) => {
            if !result.success {
                let err = result.error.unwrap_or_else(|| "获取工作记忆失败".into());
                if err == "CONTEXT_NOT_FOUND" {
                    if json {
                        return print_json(&json!({ "exists": false, "requirementId": req }));
                    }
                    println!("[chunsun] 暂无工作记忆（Context）。");
                    println!(
                        "  写入：chunsun context put {req} --snapshot '{{\"lastRunSummary\":{{}}}}'"
                    );
                    return Ok(());
                }
                return Err(CmdError::new(err));
            }
            let data = result
                .data
                .ok_or_else(|| CmdError::new("获取工作记忆失败"))?;
            if json {
                return print_json(&data);
            }
            println!("需求: {}", data.requirement_id);
            println!("Context: {}", data.id);
            println!("更新: {}", data.updated_at);
            println!("snapshot:");
            println!("{}", serde_json::to_string_pretty(&data.snapshot)?);
            Ok(())
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("CONTEXT_NOT_FOUND") {
                if json {
                    return print_json(&json!({ "exists": false, "requirementId": req }));
                }
                println!("[chunsun] 暂无工作记忆（Context）。");
                println!(
                    "  写入：chunsun context put {req} --snapshot '{{\"lastRunSummary\":{{}}}}'"
                );
                Ok(())
            } else {
                Err(err.into())
            }
        }
    }
}

fn run_put(req: String, snapshot_raw: String, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = req_context_path(&config.project_id, &req);

    let patch: Value = serde_json::from_str(&snapshot_raw)
        .map_err(|e| CmdError::new(format!("--snapshot 不是合法 JSON：{e}")))?;
    if !patch.is_object() {
        return Err(CmdError::new("--snapshot 须为 JSON 对象"));
    }

    let existing = fetch_existing_snapshot(&api, &path)?;
    let merged = merge_snapshot(&existing, &patch);

    let result: RequirementContextResponse =
        api.put(&path, json!({ "snapshot": merged.clone() }))?;
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| "写入工作记忆失败".into()),
        ));
    }
    let data = result
        .data
        .ok_or_else(|| CmdError::new("写入工作记忆失败"))?;

    if json {
        return print_json(&data);
    }
    println!("[chunsun] 工作记忆已写入：{}", data.requirement_id);
    println!("  更新: {}", data.updated_at);
    let keys: Vec<&str> = merged
        .as_object()
        .map(|o| o.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    if !keys.is_empty() {
        println!("  snapshot keys: {}", keys.join(", "));
    }
    Ok(())
}

pub fn run(args: ContextArgs) -> CmdResult {
    match args.command {
        None => run_project_summary(args.json),
        Some(ContextCmd::Get { req, json }) => run_get(req, json),
        Some(ContextCmd::Put {
            req,
            snapshot,
            json,
        }) => run_put(req, snapshot, json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_snapshot_overlays_top_level_keys() {
        let existing = json!({
            "requirementSnapshot": { "boundary": "login" },
            "openDecisions": [{ "question": "old" }],
            "envRefs": ["A"]
        });
        let patch = json!({
            "openDecisions": [{ "question": "new" }],
            "codeLandmarks": [{ "path": "a.ts", "symbol": "foo" }]
        });
        let merged = merge_snapshot(&existing, &patch);
        assert_eq!(
            merged.get("requirementSnapshot").unwrap()["boundary"],
            "login"
        );
        assert_eq!(merged["openDecisions"][0]["question"], "new");
        assert_eq!(merged["codeLandmarks"][0]["path"], "a.ts");
        assert_eq!(merged["envRefs"][0], "A");
    }

    #[test]
    fn merge_snapshot_starts_empty_when_existing_invalid() {
        let merged = merge_snapshot(&Value::Null, &json!({ "lastRunSummary": { "ok": true } }));
        assert_eq!(merged["lastRunSummary"]["ok"], true);
    }
}
