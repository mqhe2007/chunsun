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
