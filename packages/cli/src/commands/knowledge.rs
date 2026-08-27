use clap::Args;
use serde::Deserialize;
use serde_json::Value;

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct KnowledgeArgs {
    /// 输出项目知识概览 JSON
    #[arg(long)]
    json: bool,
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
    let result: KnowledgeResponse = api.get(&format!("/projects/{}/knowledge", config.project_id))?;
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| "获取项目知识失败".into()),
        ));
    }
    let data = result
        .data
        .ok_or_else(|| CmdError::new("获取项目知识失败"))?;

    if args.json {
        let raw: Value = api.get(&format!("/projects/{}/knowledge", config.project_id))?;
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
    println!("\n完整 JSON 含正文：chunsun knowledge --json");
    println!("需求工作记忆：chunsun requirement memory get|put <需求ID>");
    Ok(())
}
