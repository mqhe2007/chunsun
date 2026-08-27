use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct RequirementArgs {
    #[command(subcommand)]
    command: RequirementCmd,
}

#[derive(Subcommand)]
enum RequirementCmd {
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        application: Option<String>,
        #[arg(long)]
        module: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Create {
        #[arg(long)]
        description: String,
        #[arg(long)]
        application: Option<String>,
        #[arg(long)]
        module: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Update {
        id: String,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        application: Option<String>,
        #[arg(long)]
        module: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 需求工作记忆（Requirement Memory）
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// 拉取需求工作记忆
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

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureBrief {
    slug: String,
    chinese_name: String,
    current_stage: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequirementSummary {
    id: String,
    application: Option<String>,
    module: Option<String>,
    description: String,
    status: String,
    #[serde(default)]
    features: Option<Vec<FeatureBrief>>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    success: bool,
    data: Option<Vec<RequirementSummary>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemResponse {
    success: bool,
    data: Option<RequirementSummary>,
    error: Option<String>,
}

// ---------- 需求工作记忆（Requirement Memory） ----------

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequirementMemoryRow {
    id: String,
    requirement_id: String,
    #[allow(dead_code)]
    project_id: String,
    snapshot: Value,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct MemoryResponse {
    success: bool,
    data: Option<RequirementMemoryRow>,
    error: Option<String>,
}

fn memory_path(project_id: &str, req: &str) -> String {
    format!("/projects/{project_id}/requirements/{req}/memory")
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
            if msg.contains("MEMORY_NOT_FOUND") || msg.contains("(404)") {
                Ok(json!({}))
            } else {
                Err(err.into())
            }
        }
    }
}

fn run_memory_get(req: String, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = memory_path(&config.project_id, &req);

    match api.get::<MemoryResponse>(&path) {
        Ok(result) => {
            if !result.success {
                let err = result.error.unwrap_or_else(|| "获取工作记忆失败".into());
                if err == "MEMORY_NOT_FOUND" {
                    if json {
                        return print_json(&json!({ "exists": false, "requirementId": req }));
                    }
                    println!("[chunsun] 暂无工作记忆（Memory）。");
                    println!(
                        "  写入：chunsun requirement memory put {req} --snapshot '{{\"lastRunSummary\":{{}}}}'"
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
            println!("Memory: {}", data.id);
            println!("更新: {}", data.updated_at);
            println!("snapshot:");
            println!("{}", serde_json::to_string_pretty(&data.snapshot)?);
            Ok(())
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("MEMORY_NOT_FOUND") {
                if json {
                    return print_json(&json!({ "exists": false, "requirementId": req }));
                }
                println!("[chunsun] 暂无工作记忆（Memory）。");
                println!(
                    "  写入：chunsun requirement memory put {req} --snapshot '{{\"lastRunSummary\":{{}}}}'"
                );
                Ok(())
            } else {
                Err(err.into())
            }
        }
    }
}

fn run_memory_put(req: String, snapshot_raw: String, json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = memory_path(&config.project_id, &req);

    let patch: Value = serde_json::from_str(&snapshot_raw)
        .map_err(|e| CmdError::new(format!("--snapshot 不是合法 JSON：{e}")))?;
    if !patch.is_object() {
        return Err(CmdError::new("--snapshot 须为 JSON 对象"));
    }

    let existing = fetch_existing_snapshot(&api, &path)?;
    let merged = merge_snapshot(&existing, &patch);

    let result: MemoryResponse =
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

fn print_requirement(req: &RequirementSummary) {
    println!("{}  [{}]", req.id, req.status);
    if let Some(app) = &req.application {
        println!("应用：{app}");
    }
    if let Some(module) = &req.module {
        println!("模块：{module}");
    }
    println!("描述：{}", req.description);
    if let Some(features) = &req.features {
        if !features.is_empty() {
            println!("关联特性：");
            for f in features {
                println!(
                    "  {}  [{}]  {}",
                    f.slug, f.current_stage, f.chinese_name
                );
            }
        }
    }
    println!("创建时间：{}", req.created_at);
}

pub fn run(args: RequirementArgs) -> CmdResult {
    match args.command {
        RequirementCmd::Memory { action } => match action {
            MemoryAction::Get { req, json } => run_memory_get(req, json),
            MemoryAction::Put { req, snapshot, json } => run_memory_put(req, snapshot, json),
        },
        RequirementCmd::List {
            status,
            application,
            module,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut params = Vec::new();
            if let Some(s) = status {
                params.push(format!("status={}", urlencoding::encode(&s)));
            }
            if let Some(a) = application {
                params.push(format!("application={}", urlencoding::encode(&a)));
            }
            if let Some(m) = module {
                params.push(format!("module={}", urlencoding::encode(&m)));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let result: ListResponse = api.get(&format!(
                "/projects/{}/requirements{query}",
                config.project_id
            ))?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "获取需求列表失败".into()),
                ));
            }
            let data = result.data.unwrap_or_default();
            if json {
                return print_json(&data);
            }
            if data.is_empty() {
                println!("[chunsun] 暂无需求。");
                return Ok(());
            }
            for req in data {
                let mut tags = Vec::new();
                if let Some(a) = &req.application {
                    tags.push(a.as_str());
                }
                if let Some(m) = &req.module {
                    tags.push(m.as_str());
                }
                let tag_part = if tags.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", tags.join("/"))
                };
                println!("{}{tag_part}  {}  {}", req.id, req.status, req.description);
            }
            Ok(())
        }
        RequirementCmd::Create {
            description,
            application,
            module,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut body = Map::new();
            body.insert("description".into(), json!(description));
            if let Some(a) = application {
                body.insert("application".into(), json!(a));
            }
            if let Some(m) = module {
                body.insert("module".into(), json!(m));
            }
            let result: ItemResponse = api.post(
                &format!("/projects/{}/requirements", config.project_id),
                Value::Object(body),
            )?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "创建需求失败".into()),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            println!("[chunsun] 需求已创建：{}  [{}]", data.id, data.status);
            println!("  描述：{}", data.description);
            Ok(())
        }
        RequirementCmd::Show { id, json } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: ItemResponse = api.get(&format!(
                "/projects/{}/requirements/{}",
                config.project_id,
                urlencoding::encode(&id)
            ))?;
            if !result.success {
                return Err(CmdError::new(
                    result
                        .error
                        .unwrap_or_else(|| format!("未找到需求：{id}")),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            print_requirement(&data);
            Ok(())
        }
        RequirementCmd::Update {
            id,
            status,
            application,
            module,
            description,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut body = Map::new();
            if let Some(s) = status {
                body.insert("status".into(), json!(s));
            }
            if let Some(a) = application {
                body.insert("application".into(), json!(a));
            }
            if let Some(m) = module {
                body.insert("module".into(), json!(m));
            }
            if let Some(d) = description {
                body.insert("description".into(), json!(d));
            }
            if body.is_empty() {
                return Err(CmdError::new(
                    "请提供至少一个要更新的字段（--status、--application、--module 或 --description）",
                ));
            }
            let result: ItemResponse = api.patch(
                &format!("/projects/{}/requirements/{id}", config.project_id),
                Value::Object(body),
            )?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "更新需求失败".into()),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            println!("[chunsun] 需求已更新：{}  [{}]", data.id, data.status);
            Ok(())
        }
    }
}
