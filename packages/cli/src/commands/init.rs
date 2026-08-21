use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};
use serde::Deserialize;
use serde_json::json;

use crate::api::ApiClient;
use crate::commands::{CmdError, CmdResult};
use crate::config::load_config;
use crate::harness::install_skill_workspace_from_api;
use crate::ide::{
    default_ide_target, get_ide_target, IdeTarget, DEFAULT_IDE_ID, IDE_TARGETS,
};

#[derive(Args)]
pub struct InitArgs {
    /// 强制覆盖已有技能文件（模板版本变更时即使不加 -f 也会自动刷新）
    #[arg(short = 'f', long)]
    force: bool,
    /// 目标 IDE；省略时在终端交互式选择，非交互环境默认 cursor
    #[arg(long)]
    ide: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub slug: String,
    pub root_hint: Option<String>,
    pub is_default: bool,
    #[allow(dead_code)]
    pub created_at: String,
    #[allow(dead_code)]
    pub updated_at: String,
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

#[derive(Debug, Clone)]
pub enum RepositoryInitSelection {
    SingleDefault {
        repository: RepositorySummary,
    },
    DirMatch {
        repository: RepositorySummary,
    },
    Create {
        name: String,
        slug: String,
        root_hint: String,
    },
    Conflict {
        candidates: Vec<RepositorySummary>,
        slug: String,
    },
}

/// 供 repo 等命令复用：解析工作目录
pub fn resolve_init_cwd(explicit_cwd: Option<&str>) -> PathBuf {
    if let Some(cwd) = explicit_cwd {
        let p = PathBuf::from(cwd);
        if p.is_absolute() {
            return p;
        }
        return std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p);
    }
    if let Ok(init_cwd) = std::env::var("INIT_CWD") {
        if !init_cwd.is_empty() {
            return PathBuf::from(init_cwd);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn normalize_repository_slug(value: &str) -> String {
    let slug: String = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    // collapse consecutive dashes (JS replace /[^a-z0-9]+/g with "-")
    let mut out = String::new();
    let mut prev_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "repo".into()
    } else {
        out
    }
}

fn repository_matches_slug(repository: &RepositorySummary, slug: &str) -> bool {
    if repository.slug == slug {
        return true;
    }
    if normalize_repository_slug(&repository.name) == slug {
        return true;
    }
    match &repository.root_hint {
        None => false,
        Some(hint) if hint == "." => false,
        Some(hint) => {
            let base = Path::new(hint)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(hint);
            normalize_repository_slug(base) == slug
        }
    }
}

pub fn select_repository_for_init(
    cwd: &Path,
    _project_id: &str,
    repositories: &[RepositorySummary],
) -> RepositoryInitSelection {
    let directory_name = cwd
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let inferred_slug = normalize_repository_slug(&directory_name);

    let matched: Vec<RepositorySummary> = repositories
        .iter()
        .filter(|r| repository_matches_slug(r, &inferred_slug))
        .cloned()
        .collect();

    if matched.len() == 1 {
        return RepositoryInitSelection::DirMatch {
            repository: matched.into_iter().next().unwrap(),
        };
    }
    if matched.len() > 1 {
        return RepositoryInitSelection::Conflict {
            candidates: matched,
            slug: inferred_slug,
        };
    }

    let default_repository = if repositories.len() == 1 && repositories[0].is_default {
        Some(repositories[0].clone())
    } else {
        None
    };

    if let Some(repository) = default_repository {
        return RepositoryInitSelection::SingleDefault { repository };
    }

    RepositoryInitSelection::Create {
        name: directory_name,
        slug: inferred_slug,
        root_hint: ".".into(),
    }
}

/// 解析本次 init 使用的 IDE。
///
/// 优先级：显式 `--ide` > 交互式选择（TTY）> 默认 Cursor（非交互 / CI）。
pub fn resolve_ide_for_init(
    explicit_ide: Option<&str>,
    interactive: bool,
) -> Result<&'static IdeTarget, CmdError> {
    if let Some(id) = explicit_ide {
        return get_ide_target(id).ok_or_else(|| {
            let options: Vec<&str> = IDE_TARGETS.iter().map(|t| t.id.as_str()).collect();
            CmdError::new(format!(
                "未知的 IDE：{id}。可选值：{}",
                options.join(" / ")
            ))
        });
    }

    if !interactive {
        return Ok(default_ide_target());
    }

    let labels: Vec<&str> = IDE_TARGETS.iter().map(|t| t.label).collect();
    let default_idx = IDE_TARGETS
        .iter()
        .position(|t| t.id == DEFAULT_IDE_ID)
        .unwrap_or(0);
    let selected = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("选择目标 IDE（决定技能 / 斜线命令 / 门禁规则的安装目录）：")
        .default(default_idx)
        .items(&labels)
        .interact()
        .map_err(|e| CmdError::new(e.to_string()))?;

    Ok(IDE_TARGETS
        .get(selected)
        .unwrap_or_else(|| default_ide_target()))
}

pub fn run(args: InitArgs) -> CmdResult {
    let config = load_config();
    let cwd = std::env::current_dir()?;
    let project_id = config.project_id.clone();
    let project_name = std::env::var("_CHUNSUN_PROJECT_NAME")
        .or_else(|_| std::env::var("CHUNSUN_PROJECT_NAME"))
        .unwrap_or_else(|_| project_id.clone());

    if project_id.is_empty() {
        return Err(CmdError::new(
            "缺少项目 ID。请先配置有效的 SK（CHUNSUN_SECRET_KEY），CLI 会自动从服务端解析绑定的项目。",
        ));
    }

    let api = ApiClient::new(&config)?;
    let list_result: RepositoryListResponse =
        api.get(&format!("/projects/{project_id}/repositories"))?;
    if !list_result.success {
        return Err(CmdError::new(
            list_result
                .error
                .unwrap_or_else(|| "加载 repository 列表失败".into()),
        ));
    }
    let repositories = list_result.data.unwrap_or_default();

    let selection = select_repository_for_init(&cwd, &project_id, &repositories);

    let (repository, repository_action) = match selection {
        RepositoryInitSelection::Conflict { candidates, .. } => {
            let names: Vec<String> = candidates
                .iter()
                .map(|r| format!("{}/{}", r.name, r.slug))
                .collect();
            return Err(CmdError::new(format!(
                "当前目录匹配到多个 repository（{}），请先运行 `chunsun repo list` 查看，再用 `chunsun repo register` 管理。",
                names.join(", ")
            )));
        }
        RepositoryInitSelection::Create {
            name,
            slug,
            root_hint,
        } => {
            let create_result: RepositoryCreateResponse = api.post(
                &format!("/projects/{project_id}/repositories"),
                json!({ "name": name, "slug": slug, "rootHint": root_hint }),
            )?;
            if !create_result.success {
                return Err(CmdError::new(
                    create_result
                        .error
                        .unwrap_or_else(|| "自动创建 repository 失败".into()),
                ));
            }
            let repository = create_result.data.unwrap();
            let action = format!(
                "自动创建仓库：{} ({})",
                repository.name, repository.slug
            );
            (repository, action)
        }
        RepositoryInitSelection::SingleDefault { repository } => {
            let action = format!(
                "自动绑定唯一默认仓库：{} ({})",
                repository.name, repository.slug
            );
            (repository, action)
        }
        RepositoryInitSelection::DirMatch { repository } => {
            let action = format!(
                "按当前目录名匹配仓库：{} ({})",
                repository.name, repository.slug
            );
            (repository, action)
        }
    };

    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let ide = resolve_ide_for_init(args.ide.as_deref(), interactive)?;

    let result = install_skill_workspace_from_api(&api, &cwd, args.force, Some(ide))?;

    println!("[chunsun] 项目：{project_id} ({project_name})");
    println!(
        "[chunsun] 仓库已在平台绑定：{} ({})",
        repository.name, repository.slug
    );
    println!("[chunsun] {repository_action}");
    println!("[chunsun] 目标 IDE：{}", ide.label);
    println!(
        "[chunsun] Agent 技能已就绪：{}/SKILL.md",
        result.skill_root.display()
    );
    if result.refreshed {
        let from = match &result.previous_version {
            Some(prev) => format!("{prev} → {}", result.template_version),
            None => result.template_version.clone(),
        };
        println!(
            "[chunsun] 已从实例拉取模板并刷新技能/斜线命令/门禁（{from}），写入 {} 个文件。",
            result.written.len()
        );
    } else if !result.reused.is_empty() {
        println!(
            "[chunsun] 模板已是最新（{}），复用 {} 个文件（--force 可强制覆盖）。",
            result.template_version,
            result.reused.len()
        );
    }
    println!("[chunsun] 下一步：在 chat 中执行 `/探索 <需求ID>`，或说「开始探索需求」。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(
        id: &str,
        name: &str,
        slug: &str,
        root_hint: Option<&str>,
        is_default: bool,
    ) -> RepositorySummary {
        RepositorySummary {
            id: id.into(),
            project_id: "proj_123".into(),
            name: name.into(),
            slug: slug.into(),
            root_hint: root_hint.map(str::to_string),
            is_default,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn normalizes_directory_names_into_repository_slugs() {
        assert_eq!(normalize_repository_slug("Frontend App"), "frontend-app");
        assert_eq!(normalize_repository_slug(" server_api "), "server-api");
    }

    #[test]
    fn auto_binds_the_only_default_repository() {
        let selection = select_repository_for_init(
            Path::new("/workspace/project"),
            "proj_123",
            &[repo("repo_default", "default", "default", Some("."), true)],
        );
        assert!(matches!(
            selection,
            RepositoryInitSelection::SingleDefault { .. }
        ));
    }

    #[test]
    fn matches_existing_repository_by_directory_name() {
        let selection = select_repository_for_init(
            Path::new("/workspace/frontend"),
            "proj_123",
            &[
                repo("repo_frontend", "frontend", "frontend", Some("."), false),
                repo("repo_backend", "backend", "backend", Some("."), false),
            ],
        );
        match selection {
            RepositoryInitSelection::DirMatch { repository } => {
                assert_eq!(repository.slug, "frontend");
            }
            other => panic!("expected dir-match, got {other:?}"),
        }
    }

    #[test]
    fn auto_creates_when_no_candidate_matches() {
        let selection = select_repository_for_init(
            Path::new("/workspace/frontend"),
            "proj_123",
            &[repo("repo_backend", "backend", "backend", Some("."), false)],
        );
        match selection {
            RepositoryInitSelection::Create { slug, .. } => {
                assert_eq!(slug, "frontend");
            }
            other => panic!("expected create, got {other:?}"),
        }
    }

    #[test]
    fn conflict_when_multiple_repositories_match() {
        let selection = select_repository_for_init(
            Path::new("/workspace/frontend"),
            "proj_123",
            &[
                repo("repo_frontend", "frontend", "frontend", Some("."), false),
                repo(
                    "repo_frontend_alt",
                    "Frontend",
                    "frontend-alt",
                    Some("frontend"),
                    false,
                ),
            ],
        );
        match selection {
            RepositoryInitSelection::Conflict { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn resolve_ide_honors_explicit_over_interactive() {
        let target = resolve_ide_for_init(Some("trae"), true).unwrap();
        assert_eq!(target.id.as_str(), "trae");
    }

    #[test]
    fn resolve_ide_rejects_unknown() {
        let err = resolve_ide_for_init(Some("vscode"), false).unwrap_err();
        assert!(err.to_string().contains("未知的 IDE"));
    }

    #[test]
    fn resolve_ide_defaults_in_non_interactive() {
        let target = resolve_ide_for_init(None, false).unwrap();
        assert_eq!(target.id.as_str(), "cursor");
    }

    #[test]
    fn resolve_init_cwd_prefers_explicit() {
        let path = resolve_init_cwd(Some("/tmp/explicit"));
        assert!(path.to_string_lossy().contains("explicit"));
    }
}
