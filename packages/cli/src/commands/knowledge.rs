use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct KnowledgeArgs {
    /// 输出项目知识概览 JSON
    #[arg(long)]
    json: bool,
    /// 按加载策略过滤（eager / lazy）；不传返回全部
    #[arg(long)]
    strategy: Option<String>,
    #[command(subcommand)]
    command: Option<KnowledgeCommand>,
}

#[derive(Subcommand)]
enum KnowledgeCommand {
    /// 单条查询知识文档（含宪法）
    Doc {
        /// 文档 ID 或 "constitution"
        doc_id: String,
    },
    /// 知识目录（所有文档元信息，不含正文）
    Index,
    /// 创建知识文档（保持不支持删除）
    Create {
        /// 文档标题（必填）
        #[arg(long)]
        title: String,
        /// 文档正文（可选，默认空串）
        #[arg(long)]
        content: Option<String>,
        /// 加载策略 eager / lazy（默认 eager）
        #[arg(long)]
        strategy: Option<String>,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 更新知识文档（至少提供一个字段；保持不支持删除）
    Update {
        /// 文档 ID
        doc_id: String,
        /// 新标题
        #[arg(long)]
        title: Option<String>,
        /// 新正文
        #[arg(long)]
        content: Option<String>,
        /// 新加载策略 eager / lazy
        #[arg(long)]
        strategy: Option<String>,
        /// 新排序值（向零截断）
        #[arg(long)]
        sort_order: Option<i64>,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Deserialize)]
struct KnowledgeResponse {
    success: bool,
    data: Option<KnowledgeData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeData {
    project: ProjectInfo,
    #[serde(default)]
    contexts: Vec<KnowledgeItem>,
    summary: Summary,
}

/// 创建/更新知识文档的响应（`knowledge_doc_dto` 形状：id/title/content/sortOrder/loadStrategy/updatedAt）。
#[derive(Debug, Deserialize)]
struct KnowledgeDocResponse {
    success: bool,
    data: Option<KnowledgeDocDto>,
    error: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeDocDto {
    id: String,
    title: String,
    content: String,
    sort_order: i64,
    load_strategy: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInfo {
    name: String,
    description: Option<String>,
    env_var_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeItem {
    #[serde(default)]
    key: Option<String>,
    title: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "loadStrategy")]
    load_strategy: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Summary {
    requirements: CountBy,
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

pub fn run(args: KnowledgeArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;

    // 子命令：单条文档查询
    if let Some(KnowledgeCommand::Doc { doc_id }) = args.command {
        let path = if doc_id == "constitution" {
            format!("/projects/{}/knowledge/constitution", config.project_id)
        } else {
            format!("/projects/{}/knowledge/documents/{}", config.project_id, doc_id)
        };
        let raw: Value = api.get(&path)?;
        if let Some(d) = raw.get("data") {
            return print_json(d);
        }
        return print_json(&raw);
    }

    // 子命令：知识目录
    if let Some(KnowledgeCommand::Index) = args.command {
        let path = format!("/projects/{}/knowledge/index", config.project_id);
        let raw: Value = api.get(&path)?;
        if let Some(d) = raw.get("data") {
            if let Some(index) = d.get("index") {
                if let Some(arr) = index.as_array() {
                    println!("知识目录（共 {} 条，不含正文）：", arr.len());
                    for item in arr {
                        let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("");
                        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let system = item.get("system").and_then(|v| v.as_bool()).unwrap_or(false);
                        let ls = item.get("loadStrategy").and_then(|v| v.as_str()).unwrap_or("eager");
                        let tag = if system { "system" } else { "custom" };
                        println!("  - [{tag}] {title} (key={key}, strategy={ls})");
                    }
                    return Ok(());
                }
            }
            return print_json(d);
        }
        return print_json(&raw);
    }

    // 子命令：创建知识文档（POST /knowledge/documents）
    if let Some(KnowledgeCommand::Create {
        title,
        content,
        strategy,
        json,
    }) = args.command
    {
        if let Some(s) = &strategy {
            if s != "eager" && s != "lazy" {
                return Err(CmdError::new("--strategy 只能是 eager 或 lazy"));
            }
        }
        let mut body = Map::new();
        body.insert("title".into(), json!(title));
        body.insert("content".into(), json!(content.unwrap_or_default()));
        if let Some(s) = strategy {
            body.insert("loadStrategy".into(), json!(s));
        }
        let result: KnowledgeDocResponse = api.post(
            &format!("/projects/{}/knowledge/documents", config.project_id),
            Value::Object(body),
        )?;
        if !result.success {
            return Err(CmdError::new(
                result.error.unwrap_or_else(|| "创建知识文档失败".into()),
            ));
        }
        let data = result
            .data
            .ok_or_else(|| CmdError::new("创建知识文档失败"))?;
        if json {
            return print_json(&data);
        }
        println!("[chunsun] 知识文档已创建：{}", data.id);
        println!("  标题：{}", data.title);
        println!("  加载策略：{}", data.load_strategy);
        return Ok(());
    }

    // 子命令：更新知识文档（PUT /knowledge/documents/:docId，至少提供一个字段）
    if let Some(KnowledgeCommand::Update {
        doc_id,
        title,
        content,
        strategy,
        sort_order,
        json,
    }) = args.command
    {
        if let Some(s) = &strategy {
            if s != "eager" && s != "lazy" {
                return Err(CmdError::new("--strategy 只能是 eager 或 lazy"));
            }
        }
        let mut body = Map::new();
        if let Some(t) = title {
            body.insert("title".into(), json!(t));
        }
        if let Some(c) = content {
            body.insert("content".into(), json!(c));
        }
        if let Some(s) = strategy {
            body.insert("loadStrategy".into(), json!(s));
        }
        if let Some(n) = sort_order {
            body.insert("sortOrder".into(), json!(n));
        }
        if body.is_empty() {
            return Err(CmdError::new(
                "请提供至少一个要更新的字段（--title、--content、--strategy 或 --sort-order）",
            ));
        }
        let result: KnowledgeDocResponse = api.put(
            &format!(
                "/projects/{}/knowledge/documents/{}",
                config.project_id, doc_id
            ),
            Value::Object(body),
        )?;
        if !result.success {
            return Err(CmdError::new(
                result.error.unwrap_or_else(|| "更新知识文档失败".into()),
            ));
        }
        let data = result
            .data
            .ok_or_else(|| CmdError::new("更新知识文档失败"))?;
        if json {
            return print_json(&data);
        }
        println!("[chunsun] 知识文档已更新：{}", data.id);
        println!("  标题：{}", data.title);
        println!("  加载策略：{}", data.load_strategy);
        return Ok(());
    }

    // 概览：支持 strategy 过滤
    let mut path = format!("/projects/{}/knowledge", config.project_id);
    if let Some(s) = &args.strategy {
        if s != "eager" && s != "lazy" {
            return Err(CmdError::new("--strategy 只能是 eager 或 lazy"));
        }
        path = format!("/projects/{}/knowledge/documents?strategy={}", config.project_id, s);
    }

    if args.json {
        let raw: Value = api.get(&path)?;
        if let Some(d) = raw.get("data") {
            return print_json(d);
        }
        return print_json(&raw);
    }

    // strategy 过滤时返回的是文档列表形状（{contexts:[...]}），不是概览形状，
    // 必须在本函数用概览形状反序列化之前处理，否则 KnowledgeResponse 解析失败。
    if let Some(s) = &args.strategy {
        let raw: Value = api.get(&path)?;
        if let Some(d) = raw.get("data").and_then(|v| v.get("contexts")) {
            if let Some(arr) = d.as_array() {
                println!("知识文档（strategy={s}，共 {} 条）：", arr.len());
                for item in arr {
                    let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let system = item.get("system").and_then(|v| v.as_bool()).unwrap_or(false);
                    let ls = item.get("loadStrategy").and_then(|v| v.as_str()).unwrap_or("eager");
                    let tag = if system { "system" } else { "custom" };
                    println!("  - [{tag}] {title} (key={key}, strategy={ls})");
                }
                return Ok(());
            }
        }
        return print_json(&raw);
    }

    let result: KnowledgeResponse = api.get(&path)?;
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| "获取项目知识失败".into()),
        ));
    }

    let data = result
        .data
        .ok_or_else(|| CmdError::new("获取项目知识失败"))?;

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

    println!("\n知识文档 ({})：", data.contexts.len());
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
            let ls = c.load_strategy.as_deref().unwrap_or("eager");
            let trimmed = c.content.trim();
            let preview = if trimmed.is_empty() {
                "（空）".to_string()
            } else if trimmed.chars().count() > 60 {
                format!("{}…", trimmed.chars().take(60).collect::<String>())
            } else {
                trimmed.to_string()
            };
            println!("  - [{tag}] {} (key={}, strategy={})", c.title, key, ls);
            println!("    {preview}");
        }
    }
    println!("\n完整 JSON 含正文：chunsun knowledge --json");
    println!("按策略过滤：chunsun knowledge --strategy eager|lazy");
    println!("知识目录（元信息，不含正文）：chunsun knowledge index");
    println!("单条查询：chunsun knowledge doc <docId|constitution>");
    println!("创建知识文档：chunsun knowledge create --title <标题> [--content <正文>] [--strategy eager|lazy]");
    println!("更新知识文档：chunsun knowledge update <docId> [--title <标题>] [--content <正文>] [--strategy eager|lazy] [--sort-order <N>]");
    println!("（保持不支持删除知识文档）");
    println!("需求工作记忆：chunsun requirement memory get|put <需求ID>");
    println!("项目级记忆：chunsun memory get|put");
    Ok(())
}
