use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct DefectArgs {
    #[command(subcommand)]
    command: DefectCmd,
}

#[derive(Subcommand)]
enum DefectCmd {
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long)]
        req: Option<String>,
        #[arg(long)]
        q: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Create {
        /// 缺陷描述（--title 为兼容旧用法的别名，后端已无独立 title 字段）
        #[arg(long, alias = "title")]
        description: String,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long)]
        req: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Update {
        id: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        severity: Option<String>,
        #[arg(long)]
        req: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Delete {
        id: String,
        #[arg(long)]
        json: bool,
    },
    #[command(name = "convert-to-requirement", visible_alias = "convert")]
    ConvertToRequirement {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

/// 缺陷响应结构（与后端 `defect_dto` 对齐）。
///
/// 后端在 2026-08-14 迁移中删除了 defect.title / application / module 列，
/// 旧版 CLI 仍要求 `title: String`（非空），导致反序列化报
/// "missing field `title`"，统一表现为"解析响应失败：error decoding response body"。
#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DefectSummary {
    id: String,
    description: Option<String>,
    status: String,
    severity: String,
    requirement_id: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    success: bool,
    data: Option<Vec<DefectSummary>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemResponse {
    success: bool,
    data: Option<DefectSummary>,
    error: Option<String>,
}

fn status_label(status: &str) -> &str {
    match status {
        "open" => "待处理",
        "processing" => "处理中",
        "resolved" => "已解决",
        "closed" => "已关闭",
        _ => status,
    }
}

fn severity_label(severity: &str) -> &str {
    match severity {
        "critical" => "致命",
        "major" => "严重",
        "minor" => "一般",
        "trivial" => "轻微",
        _ => severity,
    }
}

/// 按字符数截断，超出部分用省略号表示。
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", chars[..max_chars].iter().collect::<String>())
    }
}

fn print_defect(d: &DefectSummary) {
    println!(
        "{}  [{}/{}]",
        d.id,
        severity_label(&d.severity),
        status_label(&d.status)
    );
    match &d.description {
        Some(desc) if !desc.is_empty() => println!("描述：{desc}"),
        _ => println!("描述：(无)"),
    }
    if let Some(req) = &d.requirement_id {
        println!("关联需求：{req}");
    }
    println!("创建时间：{}", d.created_at);
}

pub fn run(args: DefectArgs) -> CmdResult {
    match args.command {
        DefectCmd::List {
            status,
            severity,
            req,
            q,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut params = Vec::new();
            if let Some(s) = status {
                params.push(format!("status={}", urlencoding::encode(&s)));
            }
            if let Some(s) = severity {
                params.push(format!("severity={}", urlencoding::encode(&s)));
            }
            if let Some(r) = req {
                params.push(format!("requirementId={}", urlencoding::encode(&r)));
            }
            if let Some(q) = q {
                params.push(format!("q={}", urlencoding::encode(&q)));
            }
            let query = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let result: ListResponse =
                api.get(&format!("/projects/{}/defects{query}", config.project_id))?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "获取缺陷列表失败".into()),
                ));
            }
            let data = result.data.unwrap_or_default();
            if json {
                return print_json(&data);
            }
            if data.is_empty() {
                println!("[chunsun] 暂无缺陷。");
                return Ok(());
            }
            for d in data {
                let desc = d.description.as_deref().unwrap_or("(无描述)");
                println!(
                    "{}  {}/{}  {}",
                    d.id,
                    severity_label(&d.severity),
                    status_label(&d.status),
                    truncate(desc, 60)
                );
            }
            Ok(())
        }
        DefectCmd::Show { id, json } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: ItemResponse = api.get(&format!(
                "/projects/{}/defects/{}",
                config.project_id,
                urlencoding::encode(&id)
            ))?;
            if !result.success {
                return Err(CmdError::new(
                    result
                        .error
                        .unwrap_or_else(|| format!("未找到缺陷：{id}")),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            print_defect(&data);
            Ok(())
        }
        DefectCmd::Create {
            description,
            status,
            severity,
            req,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut body = Map::new();
            body.insert("description".into(), json!(description));
            if let Some(s) = status {
                body.insert("status".into(), json!(s));
            }
            if let Some(s) = severity {
                body.insert("severity".into(), json!(s));
            }
            if let Some(r) = req {
                body.insert("requirementId".into(), json!(r));
            }
            let result: ItemResponse = api.post(
                &format!("/projects/{}/defects", config.project_id),
                Value::Object(body),
            )?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "登记缺陷失败".into()),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 缺陷已登记：{}  [{}/{}]",
                data.id,
                severity_label(&data.severity),
                status_label(&data.status)
            );
            if let Some(desc) = &data.description {
                println!("  描述：{}", truncate(desc, 80));
            }
            Ok(())
        }
        DefectCmd::Update {
            id,
            description,
            status,
            severity,
            req,
            json,
        } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let mut body = Map::new();
            if let Some(d) = description {
                body.insert("description".into(), json!(d));
            }
            if let Some(s) = status {
                body.insert("status".into(), json!(s));
            }
            if let Some(s) = severity {
                body.insert("severity".into(), json!(s));
            }
            if let Some(r) = req {
                body.insert("requirementId".into(), json!(r));
            }
            if body.is_empty() {
                return Err(CmdError::new(
                    "请提供至少一个要更新的字段（--description、--status、--severity 或 --req）",
                ));
            }
            let result: ItemResponse = api.patch(
                &format!(
                    "/projects/{}/defects/{}",
                    config.project_id,
                    urlencoding::encode(&id)
                ),
                Value::Object(body),
            )?;
            if !result.success {
                return Err(CmdError::new(
                    result.error.unwrap_or_else(|| "更新缺陷失败".into()),
                ));
            }
            let data = result.data.unwrap();
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 缺陷已更新：{}  [{}/{}]",
                data.id,
                severity_label(&data.severity),
                status_label(&data.status)
            );
            Ok(())
        }
        DefectCmd::Delete { id, json } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: Value = api.delete(&format!(
                "/projects/{}/defects/{}",
                config.project_id,
                urlencoding::encode(&id)
            ))?;
            if result.get("success").and_then(|v| v.as_bool()) != Some(true) {
                return Err(CmdError::new(
                    result
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("删除缺陷失败")
                        .to_string(),
                ));
            }
            if json {
                if let Some(data) = result.get("data") {
                    return print_json(data);
                }
                return print_json(&result);
            }
            println!("[chunsun] 缺陷已删除：{id}");
            Ok(())
        }
        DefectCmd::ConvertToRequirement { id, json } => {
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let result: Value = api.post(
                &format!(
                    "/projects/{}/defects/{}/convert-to-requirement",
                    config.project_id,
                    urlencoding::encode(&id)
                ),
                json!({}),
            )?;
            if result.get("success").and_then(|v| v.as_bool()) != Some(true) {
                return Err(CmdError::new(
                    result
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("缺陷转需求失败")
                        .to_string(),
                ));
            }
            let data = result.get("data").cloned().unwrap_or(Value::Null);
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 缺陷已转为需求：{}  [{}]",
                data.get("id").and_then(|v| v.as_str()).unwrap_or("?"),
                data.get("status").and_then(|v| v.as_str()).unwrap_or("?")
            );
            println!("  下一步可 /探索 <需求ID> 拆解为特性进入主线。");
            Ok(())
        }
    }
}
