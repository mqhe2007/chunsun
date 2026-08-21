use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::ApiClient;
use crate::commands::init::{resolve_init_cwd, RepositorySummary};
use crate::commands::{CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    command: RepoCmd,
}

#[derive(Subcommand)]
enum RepoCmd {
    /// 列出当前项目下的 repositories
    List {
        /// 显式指定工作目录，默认优先使用 INIT_CWD
        #[arg(long)]
        cwd: Option<String>,
    },
    /// 向平台注册一个新的 repository
    Register {
        #[arg(long)]
        name: String,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long)]
        root_hint: Option<String>,
        #[arg(long)]
        cwd: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct RepositoryListResponse {
    success: bool,
    data: Option<Vec<RepositorySummary>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepositoryCreateResponse {
    success: bool,
    data: Option<RepositorySummary>,
    error: Option<String>,
}

fn fetch_repositories(
    api: &ApiClient,
    project_id: &str,
) -> Result<Vec<RepositorySummary>, CmdError> {
    let result: RepositoryListResponse =
        api.get(&format!("/projects/{project_id}/repositories"))?;
    if !result.success {
        return Err(CmdError::new(
            result
                .error
                .unwrap_or_else(|| "加载仓库列表失败".into()),
        ));
    }
    Ok(result.data.unwrap_or_default())
}

pub fn run(args: RepoArgs) -> CmdResult {
    match args.command {
        RepoCmd::List { cwd } => {
            let _ = resolve_init_cwd(cwd.as_deref());
            let config = load_config();
            let api = ApiClient::new(&config)?;
            let repositories = fetch_repositories(&api, &config.project_id)?;

            if repositories.is_empty() {
                println!("[chunsun] 当前项目暂无 repository。");
                return Ok(());
            }

            println!(
                "[chunsun] 当前项目共有 {} 个 repository：",
                repositories.len()
            );
            for repository in repositories {
                let mut tags = vec![repository.slug.clone()];
                if repository.is_default {
                    tags.push("default".into());
                }
                if let Some(hint) = &repository.root_hint {
                    tags.push(format!("root={hint}"));
                }
                println!(
                    "  {}  {}  ({})",
                    repository.id,
                    repository.name,
                    tags.join(", ")
                );
            }
            Ok(())
        }
        RepoCmd::Register {
            name,
            slug,
            root_hint,
            cwd,
        } => {
            let _ = resolve_init_cwd(cwd.as_deref());
            let config = load_config();
            let api = ApiClient::new(&config)?;

            let mut body = Map::new();
            body.insert("name".into(), json!(name));
            if let Some(s) = slug {
                body.insert("slug".into(), json!(s));
            }
            body.insert(
                "rootHint".into(),
                json!(root_hint.unwrap_or_else(|| ".".into())),
            );

            let result: RepositoryCreateResponse = api.post(
                &format!("/projects/{}/repositories", config.project_id),
                Value::Object(body),
            )?;

            if !result.success {
                return Err(CmdError::new(
                    result
                        .error
                        .unwrap_or_else(|| "创建 repository 失败".into()),
                ));
            }

            let data = result.data.unwrap();
            println!(
                "[chunsun] repository 已创建：{} ({}, {})",
                data.name, data.id, data.slug
            );
            Ok(())
        }
    }
}
