use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::ApiClient;
use crate::commands::{print_conflict, print_json, CmdError, CmdResult};
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
        /// 文档 ID、"constitution" 或 "memory"
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
    /// 待处理批注：全局清单、结案与重新打开
    Annotation {
        #[command(subcommand)]
        command: KnowledgeAnnotationCommand,
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
        /// 乐观锁版本号：**必填**。取自 `knowledge doc <id>` 输出的「版本」或 --json 的 revision。
        /// 首次创建传 0（表示"我认为这行还不存在"）；更新传你读到的那一版。
        ///
        /// 这里用 clap 的必填（非 Option）而不是自己判空：缺了就在**本地**失败，
        /// 不发出一次注定 400 的请求，错误信息也由 clap 统一给出用法提示。
        #[arg(long)]
        revision: i64,
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

#[derive(Subcommand)]
enum KnowledgeAnnotationCommand {
    /// 列出项目全部待处理批注（open + stale，覆盖 eager / lazy 文档）
    List {
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 结案批注；必须说明已处理或不采纳，并给出可核查依据
    Resolve {
        /// 批注 ID
        annotation_id: String,
        /// addressed（已处理）或 dismissed（不采纳）
        #[arg(long)]
        outcome: String,
        /// 改了哪里，或为什么不采纳
        #[arg(long)]
        note: String,
        /// 输出原始 JSON
        #[arg(long)]
        json: bool,
    },
    /// 重新打开已结案批注
    Reopen {
        /// 批注 ID
        annotation_id: String,
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
    /// 乐观锁版本号。写回时必须原样带上（`--revision`）。
    ///
    /// `#[serde(default)]` 是为了**向前兼容旧后端**：升级期里 CLI 可能先于实例更新，
    /// 那时响应里没有这个字段，硬解析会退化成「解析响应失败」这种什么都看不出的错。
    /// 缺省为 0 之后，写回会得到后端的 `MISSING_REVISION`/冲突，错误信息指得准。
    #[serde(default)]
    revision: i64,
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

    if let Some(KnowledgeCommand::Annotation { command }) = args.command {
        let base = format!(
            "/projects/{}/knowledge/annotations",
            config.project_id
        );
        return match command {
            KnowledgeAnnotationCommand::List { json } => {
                let raw: Value = api.get(&base)?;
                let data = raw.get("data").unwrap_or(&raw);
                if json {
                    print_json(data)
                } else {
                    print_pending_annotations(data)
                }
            }
            KnowledgeAnnotationCommand::Resolve {
                annotation_id,
                outcome,
                note,
                json,
            } => {
                if outcome != "addressed" && outcome != "dismissed" {
                    return Err(CmdError::new(
                        "--outcome 只能是 addressed 或 dismissed",
                    ));
                }
                if note.trim().is_empty() {
                    return Err(CmdError::new("--note 必须填写可核查的处理依据"));
                }
                let raw: Value = api.patch(
                    &format!("{base}/{annotation_id}"),
                    json!({
                        "status": "resolved",
                        "outcome": outcome,
                        "resolvedNote": note.trim(),
                    }),
                )?;
                let data = raw.get("data").unwrap_or(&raw);
                if json {
                    print_json(data)
                } else {
                    println!("[chunsun] 批注已结案：{annotation_id}");
                    println!("  结论：{outcome}");
                    println!("  依据：{}", note.trim());
                    Ok(())
                }
            }
            KnowledgeAnnotationCommand::Reopen {
                annotation_id,
                json,
            } => {
                let raw: Value = api.patch(
                    &format!("{base}/{annotation_id}"),
                    json!({ "status": "open" }),
                )?;
                let data = raw.get("data").unwrap_or(&raw);
                if json {
                    print_json(data)
                } else {
                    println!("[chunsun] 批注已重新打开：{annotation_id}");
                    Ok(())
                }
            }
        };
    }

    // 子命令：单条文档查询。
    //
    // 注意这条路径**不经过 `KnowledgeDocDto`**（它取的是 `Value`，既能拿知识文档
    // 也能拿宪法 / memory 这两种形状不同的系统文档），所以 revision 的展示是在
    // `print_doc_detail` 里按 `Value` 取的 —— 加字段时**不要只改 DTO**，
    // 否则 `--json` 有、人读输出没有，Agent 会以为自己读到了但拿不到版本号。
    if let Some(KnowledgeCommand::Doc { doc_id, json }) = args.command {
        let path = match doc_id.as_str() {
            "constitution" => format!(
                "/projects/{}/knowledge/constitution",
                config.project_id
            ),
            "memory" => format!(
                "/projects/{}/knowledge/documents/memory",
                config.project_id
            ),
            _ => format!(
                "/projects/{}/knowledge/documents/{}",
                config.project_id, doc_id
            ),
        };
        let raw: Value = api.get(&path)?;
        let data = raw.get("data").unwrap_or(&raw).clone();
        if json {
            return print_json(&data);
        }
        return print_doc_detail(&data);
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
        revision,
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
        if doc_id == "constitution" || doc_id == "memory" {
            if title.is_some() || strategy.is_some() || sort_order.is_some() || parent.is_some() {
                return Err(CmdError::new(
                    "系统文档只能通过 --content 更新正文",
                ));
            }
            let content = content.ok_or_else(|| CmdError::new("请提供 --content"))?;
            // 宪法走 /knowledge/constitution，memory 走 /memory —— 两者是不同的写入点
            // （`upsert_project_policy` 与 `upsert_project_memory`），但都从 `revision = 0`
            // 开始表示「尚不存在」。`--revision 0` 在这条路径上是新建语义。
            let (path, payload) = if doc_id == "constitution" {
                (
                    format!("/projects/{}/knowledge/constitution", config.project_id),
                    json!({ "content": content, "revision": revision }),
                )
            } else {
                (
                    format!("/projects/{}/memory", config.project_id),
                    json!({ "snapshot": content, "revision": revision }),
                )
            };
            // 不直接 `?`：409 要走 print_conflict 给出可操作提示，
            // 而 `From<ApiError> for CmdError` 只会保留一行 Display 文本
            // （那行文本里没有 currentRevision / currentContent —— 它们在 data 里）。
            let raw: Value = match api.put(&path, payload) {
                Ok(v) => v,
                Err(e) if e.code() == Some("CONSTITUTION_CONFLICT")
                    || e.code() == Some("MEMORY_CONFLICT") =>
                {
                    return Err(print_conflict(&e))
                }
                Err(e) => return Err(e.into()),
            };
            let data = raw.get("data").unwrap_or(&raw);
            if json {
                return print_json(data);
            }
            println!("[chunsun] 系统知识文档已更新：{doc_id}");
            // 与知识文档同理由：把新版本号交出去，省掉写后的那次读。
            // constitution 走 `constitution_dto`、memory 走 `project_memory_dto`，
            // 两者都带 revision，所以这里取同一把钥匙。
            if let Some(rev) = data.get("revision").and_then(|v| v.as_i64()) {
                println!("  新版本（下次 --revision）：{rev}");
            }
            return Ok(());
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
        // revision 由 clap 保证存在，这里是 `--revision` 与补丁字段的**本地**空补丁检查。
        // 后端也会判（400 EMPTY_PATCH），但本地先拦一次可以省掉一次往返，
        // 而且这条错误比后端那条更清楚：它直接列出可用的 flag。
        body.insert("revision".into(), json!(revision));
        let result: KnowledgeDocResponse = match api.put(
            &format!(
                "/projects/{}/knowledge/documents/{}",
                config.project_id, doc_id
            ),
            Value::Object(body),
        ) {
            Ok(r) => r,
            Err(e) if e.code() == Some("KNOWLEDGE_DOC_CONFLICT") => {
                return Err(print_conflict(&e))
            }
            Err(e) => return Err(e.into()),
        };
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
        // 写回后立刻把**新**版本号打出来：Agent 连续做几次编辑时不必再读一次。
        // 这是「读-改-写」循环里最容易漏的一次往返 —— 漏了就得重新 GET。
        println!("  新版本（下次 --revision）：{}", data.revision);
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
    println!("单条查询（面包屑/子文档 + 正文）：chunsun knowledge doc <docId|constitution|memory> [--json]");
    println!("待处理批注：chunsun knowledge annotation list [--json]");
    println!("创建知识文档：chunsun knowledge create --title <标题> [--content <正文>] [--strategy eager|lazy] [--parent <主文档ID>]");
    println!("更新知识文档：chunsun knowledge update <docId> [--title <标题>] [--content <正文>] [--strategy eager|lazy] [--sort-order <N>] [--parent <主文档ID|空串=解除>]");
    println!("（保持不支持删除知识文档）");
    println!("需求工作记忆：chunsun requirement memory get|put <需求ID>");
    println!("项目级记忆：chunsun memory get|put");
    Ok(())
}

fn print_pending_annotations(data: &Value) -> CmdResult {
    let annotations = data
        .get("annotations")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    println!("待处理知识批注（{}）：", annotations.len());
    if annotations.is_empty() {
        println!("  （无）");
        return Ok(());
    }
    for annotation in annotations {
        let id = annotation.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let status = annotation
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("open");
        let doc_ref = annotation
            .get("docRef")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let title = annotation
            .get("documentTitle")
            .and_then(|v| v.as_str())
            .unwrap_or("未知文档");
        let strategy = annotation
            .get("loadStrategy")
            .and_then(|v| v.as_str())
            .unwrap_or("eager");
        let body = annotation
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!("  - [{status}] {title} (doc={doc_ref}, strategy={strategy}, id={id})");
        println!("    {body}");
    }
    println!("深读目标文档：chunsun knowledge doc <docRef> --json");
    println!("结案：chunsun knowledge annotation resolve <批注ID> --outcome addressed|dismissed --note '<依据>'");
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

    // 版本号必须出现在**人读输出**里，不能只给 `--json`：Agent 读一次之后紧接着
    // 就要写回，而写回的 `--revision` 是必填的。藏在 JSON 里等于逼每一步都多一次
    // `--json` 解析。缺失时（旧后端）直接不打印，而不是显示一个会误导人的 0。
    if let Some(rev) = data.get("revision").and_then(|v| v.as_i64()) {
        println!("  版本（写回时传 --revision）：{rev}");
    }

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

    #[test]
    fn pending_annotation_output_shape_accepts_open_and_stale() {
        let data = json!({
            "annotations": [
                {
                    "id": "a-open",
                    "status": "open",
                    "docRef": "constitution",
                    "documentTitle": "项目宪法",
                    "loadStrategy": "eager",
                    "body": "补充示例"
                },
                {
                    "id": "a-stale",
                    "status": "stale",
                    "docRef": "lazy-doc",
                    "documentTitle": "长文档",
                    "loadStrategy": "lazy",
                    "body": "原锚点已变化"
                }
            ]
        });
        assert!(print_pending_annotations(&data).is_ok());
    }
}
