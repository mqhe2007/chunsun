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
    /// 单条查询知识文档（含宪法；默认输出面包屑/子文档 + 正文，--json 输出原始 JSON）
    Doc {
        /// 文档 ID 或 "constitution"
        doc_id: String,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 知识目录（所有文档元信息，不含正文；树形展示主文档/分册）
    Index {
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
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
        /// 所属主文档 ID（可选；不传建为根文档）
        #[arg(long)]
        parent: Option<String>,
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
        /// 所属主文档 ID（传空串解除关联，回到根文档）
        #[arg(long)]
        parent: Option<String>,
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
    #[serde(default)]
    parent_id: Option<String>,
    updated_at: String,
}

/// 知识目录条目（`GET /knowledge/index`）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexItem {
    key: String,
    title: String,
    #[serde(default)]
    system: bool,
    #[serde(default)]
    load_strategy: Option<String>,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    depth: usize,
}

/// 把知识目录渲染成缩进树行（纯函数，单测覆盖）：
/// - depth 0 用 `- `，子级用 `└ ` 前缀（每层多缩进 2 格）；
/// - 有直接子文档的条目追加「主文档（N 分册）」标注。
fn render_index_lines(items: &[IndexItem]) -> Vec<String> {
    use std::collections::HashMap;
    let mut child_counts: HashMap<&str, usize> = HashMap::new();
    for item in items {
        if let Some(p) = item.parent_id.as_deref() {
            *child_counts.entry(p).or_insert(0) += 1;
        }
    }
    items
        .iter()
        .map(|item| {
            let tag = if item.system { "system" } else { "custom" };
            let ls = item.load_strategy.as_deref().unwrap_or("eager");
            let kids = child_counts.get(item.key.as_str()).copied().unwrap_or(0);
            let badge = if kids > 0 {
                format!(" · 主文档（{kids} 分册）")
            } else {
                String::new()
            };
            if item.depth == 0 {
                format!(
                    "  - [{tag}] {} (key={}, strategy={}){badge}",
                    item.title, item.key, ls
                )
            } else {
                format!(
                    "  {}└ {} (key={}, strategy={}){badge}",
                    "  ".repeat(item.depth),
                    item.title,
                    item.key,
                    ls
                )
            }
        })
        .collect()
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
    #[serde(default, rename = "parentId")]
    parent_id: Option<String>,
    #[serde(default)]
    depth: usize,
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
    if let Some(KnowledgeCommand::Doc { doc_id, json }) = args.command {
        let path = if doc_id == "constitution" {
            format!("/projects/{}/knowledge/constitution", config.project_id)
        } else {
            format!("/projects/{}/knowledge/documents/{}", config.project_id, doc_id)
        };
        let raw: Value = api.get(&path)?;
        let data = raw.get("data").unwrap_or(&raw);
        if json {
            return print_json(data);
        }
        return print_doc_detail(data);
    }

    // 子命令：知识目录（树形展示主文档/分册）
    if let Some(KnowledgeCommand::Index { json }) = args.command {
        let path = format!("/projects/{}/knowledge/index", config.project_id);
        let raw: Value = api.get(&path)?;
        if json {
            let data = raw.get("data").unwrap_or(&raw);
            return print_json(data);
        }
        if let Some(d) = raw.get("data") {
            if let Some(arr) = d.get("index").and_then(|v| v.as_array()) {
                let items: Vec<IndexItem> = arr
                    .iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect();
                println!("知识目录（共 {} 条，不含正文；└ 表示分册归属）：", items.len());
                for line in render_index_lines(&items) {
                    println!("{line}");
                }
                return Ok(());
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
        parent,
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
        if let Some(p) = parent.filter(|p| !p.is_empty()) {
            body.insert("parentId".into(), json!(p));
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
        println!(
            "  所属主文档：{}",
            data.parent_id.as_deref().unwrap_or("无（根文档）")
        );
        return Ok(());
    }

    // 子命令：更新知识文档（PUT /knowledge/documents/:docId，至少提供一个字段）
    if let Some(KnowledgeCommand::Update {
        doc_id,
        title,
        content,
        strategy,
        sort_order,
        parent,
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
        // --parent 传空串 = 解除关联（显式写入 null，而不是省略）
        if let Some(p) = parent {
            body.insert(
                "parentId".into(),
                if p.is_empty() { Value::Null } else { json!(p) },
            );
        }
        if body.is_empty() {
            return Err(CmdError::new(
                "请提供至少一个要更新的字段（--title、--content、--strategy、--sort-order 或 --parent）",
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
        println!(
            "  所属主文档：{}",
            data.parent_id.as_deref().unwrap_or("无（根文档）")
        );
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
    // 输出按 depth 缩进：过滤不会打乱层级（后端保持绝对 depth）。
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
                    let depth = item.get("depth").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let tag = if system { "system" } else { "custom" };
                    let prefix = if depth == 0 {
                        "  - ".to_string()
                    } else {
                        format!("  {}└ ", "  ".repeat(depth))
                    };
                    println!("{prefix}[{tag}] {title} (key={key}, strategy={ls})");
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
            let prefix = if c.depth == 0 {
                "  - ".to_string()
            } else {
                format!("  {}└ ", "  ".repeat(c.depth))
            };
            println!("{prefix}[{tag}] {} (key={}, strategy={})", c.title, key, ls);
            println!("    {preview}");
        }
    }
    println!("\n完整 JSON 含正文：chunsun knowledge --json");
    println!("按策略过滤：chunsun knowledge --strategy eager|lazy");
    println!("知识目录（元信息，不含正文；树形展示主文档/分册）：chunsun knowledge index");
    println!("单条查询（面包屑/子文档 + 正文）：chunsun knowledge doc <docId|constitution> [--json]");
    println!("创建知识文档：chunsun knowledge create --title <标题> [--content <正文>] [--strategy eager|lazy] [--parent <主文档ID>]");
    println!("更新知识文档：chunsun knowledge update <docId> [--title <标题>] [--content <正文>] [--strategy eager|lazy] [--sort-order <N>] [--parent <主文档ID|空串=解除>]");
    println!("（保持不支持删除知识文档）");
    println!("需求工作记忆：chunsun requirement memory get|put <需求ID>");
    println!("项目级记忆：chunsun memory get|put");
    Ok(())
}

/// 单条知识文档的人类可读输出：面包屑（所属主文档链）+ 子文档清单 + 正文。
///
/// `--json` 时走原始 JSON（字段含 `parentId` / `breadcrumb` / `children`）。
fn print_doc_detail(data: &Value) -> CmdResult {
    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let key = data
        .get("key")
        .or_else(|| data.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let ls = data
        .get("loadStrategy")
        .and_then(|v| v.as_str())
        .unwrap_or("eager");
    let system = data.get("system").and_then(|v| v.as_bool()).unwrap_or(false);
    println!("知识文档：{title} (id={key}, strategy={ls})");

    let breadcrumb: Vec<&Value> = data
        .get("breadcrumb")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    let children: Vec<&Value> = data
        .get("children")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    if !system {
        if breadcrumb.is_empty() {
            println!("所属主文档：无（根文档）");
        } else {
            let path = breadcrumb
                .iter()
                .map(|b| b.get("title").and_then(|v| v.as_str()).unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" / ");
            println!("所属主文档：{path}");
        }
        if children.is_empty() {
            println!("子文档：无");
        } else {
            println!("子文档（{}）：", children.len());
            for child in &children {
                let t = child.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let id = child.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let cls = child
                    .get("loadStrategy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("eager");
                println!("  └ {t} (id={id}, strategy={cls})");
            }
        }
    }

    println!("──────── 正文 ────────");
    let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");
    println!("{content}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, parent: Option<&str>, depth: usize, system: bool) -> IndexItem {
        IndexItem {
            key: key.to_string(),
            title: key.to_string(),
            system,
            load_strategy: Some("eager".to_string()),
            parent_id: parent.map(str::to_string),
            depth,
        }
    }

    #[test]
    fn render_index_lines_indents_children_under_parent() {
        let lines = render_index_lines(&[
            item("constitution", None, 0, true),
            item("master", None, 0, false),
            item("vol1", Some("master"), 1, false),
            item("sub", Some("vol1"), 2, false),
        ]);
        assert!(lines[0].starts_with("  - [system] constitution"));
        assert_eq!(lines[1], "  - [custom] master (key=master, strategy=eager) · 主文档（1 分册）");
        assert_eq!(lines[2], "    └ vol1 (key=vol1, strategy=eager) · 主文档（1 分册）");
        assert_eq!(lines[3], "      └ sub (key=sub, strategy=eager)");
    }

    #[test]
    fn render_index_lines_has_no_badge_without_children() {
        let lines = render_index_lines(&[item("solo", None, 0, false)]);
        assert!(!lines[0].contains("主文档"));
    }

    #[test]
    fn render_index_lines_counts_only_direct_children() {
        let lines = render_index_lines(&[
            item("a", None, 0, false),
            item("b", Some("a"), 1, false),
            item("c", Some("b"), 2, false),
        ]);
        assert!(lines[0].contains("主文档（1 分册）"));
        assert!(lines[1].contains("主文档（1 分册）"));
    }
}
