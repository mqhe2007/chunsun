//! 知识库文档批注路由（需求 u-WPvdvYh4Fw）。
//!
//! 批注是对**已有文档内容**的定点反馈，宿主用 `docRef` 表达：
//! `constitution` | `memory` | `{documentId}`。前两者是项目级系统单例（与知识域的
//! `system` 条目同名），其余一律按自定义文档 id 解析 → `doc_kind = 'document'`。
//!
//! 权限分三层，不要合并：
//! 1. **读 / 建**：沿用知识库既有项目可见性（成员 ∪ 创建者 ∪ 平台 ADMIN），
//!    不可见一律 404 `PROJECT_NOT_FOUND`（与知识域一致，不是 403）。
//! 2. **改 / 删**：**作者本人 ∪ 项目所有者**（所有者 = 项目创建者 `project.user_id`
//!    ∪ 平台 ADMIN，与 `core/permission_policy` 的 owner 档口径一致）。
//!    两者都不满足才 403 `ANNOTATION_FORBIDDEN`。所有者的存在意义是治理他人留下的
//!    批注（作者离职 / 批注本身要撤下），因此鉴权到项目创建者级而非作者级。
//! 3. **结案 / 重新打开**（PATCH status）：**所有成员可用**。选项 B 既定：
//!    AI 可自行把批注置 resolved（平台主打 AI 能力，低成本闭环），人可重新打开
//!    退回 open 兜底。故这里**不能**套用第 2 层的作者校验，否则 AI 无法结案。
//!
//! 锚点漂移（stale）在**列表读取时**判定：文档正文变了，批注本身没变，
//! 因此不在写入路径判；置 stale 只改状态、**绝不删除**（批注是文档变更因果史）。
//! 判定口径必须与前端高亮定位（`console/src/utils/annotationAnchor.ts`）保持一致，
//! 后端这一份供 Agent 侧读取时复用同一规则。

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::core::serde_ext::double_option;
use crate::repos::project as proj_repo;
use crate::repos::project_knowledge as ctx_repo;
use crate::repos::project_knowledge_annotation as ann_repo;
use crate::repos::project_memory as mem_repo;
use crate::routes::validate::{optional_enum, optional_string, required_string};
use crate::services::project_access::visible_project_id;
use crate::state::AppState;

/// 批注状态合法值。`stale` 由服务端漂移检测写入，也允许显式传（前端「重新定位失败」
/// 的即时反馈），但**不允许**用它隐藏批注——stale 只影响展示标记，不影响可见性。
const STATUSES: &[&str] = &["open", "resolved", "stale"];
/// 结案结论：已按批注处理 / 判断无需处理。二者必须能区分，否则「AI 结案」会变成
/// 无法追责的黑盒。
const OUTCOMES: &[&str] = &["addressed", "dismissed"];

const SYSTEM_CONSTITUTION: &str = "constitution";
const SYSTEM_MEMORY: &str = "memory";

/// `body` 是裸 `t.String()`：空串合法、无上限（与知识文档 `content` 同口径）。
const BODY_MIN: usize = 0;
const BODY_MAX: usize = usize::MAX;
/// 锚点文本与前后文片段同为裸 String，无上限。
const ANCHOR_MAX: usize = usize::MAX;

/// 结案依据（`resolvedNote`）：选项 B 的「可核查兜底」要求每条 resolved 能回溯到
/// 具体改动，这里给一个宽松上限防止把整篇文档塞进来。
const NOTE_MAX: usize = 2000;

const STALE_DETECTED_KEY: &str = "staleDetected";

// ------------------------------------------------------------------ docRef 解析

/// `docRef` → `(doc_kind, document_id)`。宪法 / 记忆映射为 `document_id = NULL`。
fn parse_doc_ref(doc_ref: &str) -> (&'static str, Option<String>) {
    match doc_ref {
        SYSTEM_CONSTITUTION => ("constitution", None),
        SYSTEM_MEMORY => ("memory", None),
        other => ("document", Some(other.to_string())),
    }
}

/// 宿主存在性校验。系统文档恒存在（缺行等价于空正文）；自定义文档须存在。
async fn ensure_host(
    state: &AppState,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
) -> Result<(), AppError> {
    if doc_kind == "document" {
        let id = document_id.unwrap_or_default();
        ctx_repo::find_knowledge_document(&state.pool(), project_id, id)
            .await?
            .ok_or_else(|| AppError::not_found("CONTEXT_DOC_NOT_FOUND"))?;
    }
    Ok(())
}

/// 批注所属项目的访问上下文：项目 id + 项目所有者（创建者）user_id。
///
/// 比 `visible()` 多带回 **所有者**，因为改 / 删批注要判到 owner 级：所有者的定义
/// 与 `core/permission_policy` 的 owner 档保持一致 —— 项目创建者 `project.user_id`，
/// 平台 ADMIN 由 `is_platform_admin` 另判（不落库，故这里不体现）。
struct AnnotationScope {
    project_id: String,
    owner_id: String,
    is_platform_admin: bool,
}

impl AnnotationScope {
    /// 该用户能否改 / 删**任意**批注（所有者通道）。
    ///
    /// 作者通道不在这里判 —— 那是逐条批注的 `created_by` 比对，见 [`can_moderate`]。
    fn owns_project(&self, user_id: &str) -> bool {
        self.is_platform_admin || self.owner_id == user_id
    }
}

/// 可见性校验并解析所有者。不可见一律 404 `PROJECT_NOT_FOUND`（与知识域一致）。
///
/// 比 `visible()` 多一次 `project` 行查询（`ProjectBrief` 投影很小）。之所以不把
/// `visible()` 改成返回所有者：知识域有大量调用方只关心 id，改签名会牵连一片。
async fn annotation_scope(
    state: &AppState,
    session: &crate::auth::AuthSession,
    project_id: &str,
) -> Result<AnnotationScope, AppError> {
    let is_platform_admin = session.user.role == "ADMIN";
    let pid = visible_project_id(
        &state.pool(),
        project_id,
        &session.user.user_id,
        is_platform_admin,
    )
    .await?;
    let owner_id = proj_repo::get_project_by_id_only(&state.pool(), &pid)
        .await?
        // 走到这里项目必然存在（visible_project_id 刚查过），缺失只能是并发删除
        .ok_or_else(|| AppError::not_found("PROJECT_NOT_FOUND"))?
        .user_id;
    Ok(AnnotationScope {
        project_id: pid,
        owner_id,
        is_platform_admin,
    })
}

/// 能否改 / 删**这一条**批注：作者本人，或项目所有者 / 平台 ADMIN。
///
/// 前端按钮的显示依据来自 DTO 的 `canModerate`，由本函数算出 —— 权限判定只有这
/// 一份，前后端不各写一套，避免漂移。
fn can_moderate(
    scope: &AnnotationScope,
    row: &ann_repo::KnowledgeAnnotationRow,
    user_id: &str,
) -> bool {
    row.created_by == user_id || scope.owns_project(user_id)
}

/// 取宿主正文（用于锚点漂移判定）。系统文档缺行时为 ""。
async fn host_content(
    state: &AppState,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
) -> Result<String, AppError> {
    match doc_kind {
        "constitution" => {
            let policy = ctx_repo::get_project_policy(&state.pool(), project_id).await?;
            Ok(policy.map_or(String::new(), |p| p.constitution_md))
        }
        "memory" => {
            let mem = mem_repo::get_project_memory(&state.pool(), project_id).await?;
            Ok(mem.and_then(|m| m.snapshot).unwrap_or_default())
        }
        _ => {
            let id = document_id.unwrap_or_default();
            let doc = ctx_repo::find_knowledge_document(&state.pool(), project_id, id).await?;
            Ok(doc.map_or(String::new(), |d| d.content))
        }
    }
}

// ------------------------------------------------------------------ 锚点漂移

/// 把 Markdown 源码规范化成「可见文本」形态，供锚点匹配。
///
/// 规范化规则（前后端必须一致）：
/// - 行内标记字符 `* _ ` ~ [ ]` 剥除（`**粗体**` → `粗体`）；
/// - 链接语法 `[文本](url)` → `文本`；图片整体剥除；
/// - 所有**空白字符折叠为单个空格**（Markdown 换行在渲染后是空格）。
///
/// 这一层存在的原因：批注锚点存的是**渲染后可见文本**，而正文存的是 Markdown 源码。
/// 直接拿源码 index 匹配会在行内标记处错位。
fn markdown_visible_text(md: &str) -> String {
    // 1. 图片整体剔除
    let mut out = String::with_capacity(md.len());
    let bytes: Vec<char> = md.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        // ![alt](url) / [文本](url)
        if c == '!' && i + 1 < bytes.len() && bytes[i + 1] == '[' {
            if let Some(end) = skip_bracket_link(&bytes, i + 1) {
                i = end;
                continue;
            }
        }
        if c == '[' {
            if let Some((label, end)) = read_bracket_link(&bytes, i) {
                out.push_str(&label);
                i = end;
                continue;
            }
        }
        // 行内标记字符与非可见语法字符：剥除
        if matches!(c, '*' | '_' | '`' | '~') {
            i += 1;
            continue;
        }
        // 行内代码 / 强调界的 HTML 转义在可见文本里不存在，源码里的反斜杠转义剥掉
        if c == '\\' && i + 1 < bytes.len() {
            out.push(bytes[i + 1]);
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    // 2. 空白折叠
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(c);
            prev_space = false;
        }
    }
    collapsed.trim().to_string()
}

/// `[label](url)`：返回 label 与结束下标（url 右括号之后）。
fn read_bracket_link(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start + 1;
    let mut label = String::new();
    let mut depth = 1usize;
    while i < chars.len() {
        match chars[i] {
            '[' => {
                depth += 1;
                label.push('[');
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                label.push(']');
            }
            c => label.push(c),
        }
        i += 1;
    }
    if i >= chars.len() || depth != 0 {
        return None;
    }
    // 必须是紧跟的 (url)
    if i + 1 >= chars.len() || chars[i + 1] != '(' {
        // 引用式链接 [label][ref] / 脚注：仅取 label，不动后缀
        return Some((label, i + 1));
    }
    let mut j = i + 2;
    while j < chars.len() && chars[j] != ')' {
        j += 1;
    }
    if j >= chars.len() {
        return None;
    }
    Some((label, j + 1))
}

/// `![alt](url)`：整体跳过，返回结束下标。
fn skip_bracket_link(chars: &[char], start: usize) -> Option<usize> {
    let (_, end) = read_bracket_link(chars, start)?;
    Some(end)
}

/// 批注锚点能否在当前正文里定位到。
///
/// 判定漂移：保守地做规范化子串查找，**只有两种形态都找不到才算漂移**。
/// 锚点为空（整篇批注）恒视为可定位——它本来就不依赖正文。
///
/// 关键：anchor_text 是浏览器 DOM selection.toString() 的已渲染可见文本，
/// 不是 Markdown 源码，所以只做空白折叠，不再剥除 markdown 标记字符。
/// 正文侧先尝试轻量“可见文本”，再用折叠后的源码兜底，以免把 `foo_bar`、
/// 字面量 `*` 等可见字符误当成 Markdown 语法而错误置 stale。
pub fn anchor_resolvable(content: &str, anchor_text: &str) -> bool {
    let anchor = fold_whitespace(anchor_text.trim());
    if anchor.is_empty() {
        return true;
    }
    if markdown_visible_text(content).contains(&anchor) {
        return true;
    }
    // The lightweight visible-text pass above intentionally removes Markdown
    // delimiters. Some of those characters are also ordinary visible text
    // (`foo_bar`, literal `*`, ...), so fall back to the folded source before
    // declaring an anchor stale. This is deliberately conservative: a false
    // stale is worse than keeping a quote that still exists in source form.
    fold_whitespace(content).contains(&anchor)
}

fn fold_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// 列表读取时判定漂移并落库：返回本次**新置为 stale** 的批注 id。
async fn detect_stale(
    state: &AppState,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
    rows: &[ann_repo::KnowledgeAnnotationRow],
) -> Result<Vec<String>, AppError> {
    let pending: Vec<&ann_repo::KnowledgeAnnotationRow> = rows
        .iter()
        .filter(|r| r.status == "open" && r.anchor_text.as_deref().is_some_and(|a| !a.trim().is_empty()))
        .collect();
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let content = host_content(state, project_id, doc_kind, document_id).await?;
    let stale: Vec<String> = pending
        .iter()
        .filter(|r| !anchor_resolvable(&content, r.anchor_text.as_deref().unwrap_or("")))
        .map(|r| r.id.clone())
        .collect();
    if !stale.is_empty() {
        ann_repo::mark_stale(&state.pool(), &stale).await?;
    }
    Ok(stale)
}

// ------------------------------------------------------------------ DTO

/// `can_moderate` 由 [`can_moderate`] 算出并由调用方传入：编辑 / 删除按钮的显示依据。
///
/// 只给**人看的接口**带这个字段。Agent 侧注入块（`services/project_knowledge_annotation`
/// 的 `summarize` / `build_block`）不带 —— 那是机器读的清单，权限位对它无意义。
fn annotation_dto(row: &ann_repo::KnowledgeAnnotationRow, can_moderate: bool) -> Value {
    json!({
        "canModerate": can_moderate,
        "id": row.id,
        "docRef": row.document_id.clone().unwrap_or_else(|| row.doc_kind.clone()),
        "docKind": row.doc_kind,
        "documentId": row.document_id,
        "anchorText": row.anchor_text,
        "anchorPrefix": row.anchor_prefix,
        "anchorSuffix": row.anchor_suffix,
        "body": row.body,
        "status": row.status,
        "outcome": row.outcome,
        "resolvedNote": row.resolved_note,
        "resolvedBy": row.resolved_by,
        "resolvedAt": row.resolved_at.as_ref().map(dt_value),
        "createdBy": row.created_by,
        "createdAt": dt_value(&row.created_at),
        "updatedAt": dt_value(&row.updated_at),
    })
}

fn pending_annotation_dto(
    row: &ann_repo::KnowledgeAnnotationRow,
    documents: &HashMap<String, (String, String)>,
    can_moderate: bool,
) -> Value {
    let mut dto = annotation_dto(row, can_moderate);
    let (title, strategy) = match row.doc_kind.as_str() {
        SYSTEM_CONSTITUTION => ("项目宪法", "eager"),
        SYSTEM_MEMORY => ("项目记忆", "eager"),
        _ => row
            .document_id
            .as_ref()
            .and_then(|id| documents.get(id))
            .map(|(title, strategy)| (title.as_str(), strategy.as_str()))
            .unwrap_or(("未知文档", "lazy")),
    };
    dto["documentTitle"] = Value::String(title.to_string());
    dto["loadStrategy"] = Value::String(strategy.to_string());
    dto
}

fn resolution_details<'a>(
    status: &str,
    outcome: Option<&'a str>,
    note: Option<&'a str>,
) -> Result<(Option<&'a str>, Option<&'a str>), AppError> {
    if status != "resolved" {
        return Ok((None, None));
    }
    let outcome = outcome.ok_or_else(|| {
        AppError::bad_request("ANNOTATION_OUTCOME_REQUIRED")
            .with_message("结案必须明确 outcome=addressed|dismissed")
    })?;
    let note = note.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
        AppError::bad_request("ANNOTATION_RESOLUTION_NOTE_REQUIRED")
            .with_message("结案必须填写可核查的处理依据")
    })?;
    Ok((Some(outcome), Some(note.trim())))
}

// ------------------------------------------------------------------ handlers

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAnnotationBody {
    #[serde(default, deserialize_with = "double_option")]
    body: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    anchor_text: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    anchor_prefix: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    anchor_suffix: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchAnnotationBody {
    #[serde(default, deserialize_with = "double_option")]
    body: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    status: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    outcome: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    resolved_note: Option<Option<String>>,
}

/// `GET /projects/:id/knowledge/annotations`
///
/// Agent 启动时使用的项目级待处理队列。与 eager/lazy 无关，open 与 stale 都返回；
/// 同时先对仍为 open 的锚点做漂移检测，保证返回的是当前真实状态。
async fn list_pending_annotations(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let scope = annotation_scope(&state, &session, &project_id).await?;
    let pid = &scope.project_id;
    let rows = ann_repo::list_pending_annotations_for_project(&state.pool(), pid).await?;

    let mut grouped: HashMap<(String, Option<String>), Vec<ann_repo::KnowledgeAnnotationRow>> =
        HashMap::new();
    for row in &rows {
        grouped
            .entry((row.doc_kind.clone(), row.document_id.clone()))
            .or_default()
            .push(row.clone());
    }
    let mut stale_ids = Vec::new();
    for ((doc_kind, document_id), group) in grouped {
        stale_ids.extend(
            detect_stale(
                &state,
                pid,
                &doc_kind,
                document_id.as_deref(),
                &group,
            )
            .await?,
        );
    }

    let rows = if stale_ids.is_empty() {
        rows
    } else {
        ann_repo::list_pending_annotations_for_project(&state.pool(), pid).await?
    };
    let documents: HashMap<String, (String, String)> =
        ctx_repo::list_knowledge_document_nodes(&state.pool(), pid)
            .await?
            .into_iter()
            .map(|node| {
                (
                    node.doc.id,
                    (node.doc.title, node.doc.load_strategy),
                )
            })
            .collect();
    let user_id = &session.user.user_id;
    let items: Vec<Value> = rows
        .iter()
        .map(|row| pending_annotation_dto(row, &documents, can_moderate(&scope, row, user_id)))
        .collect();
    Ok(ok(json!({
        "total": items.len(),
        "annotations": items,
        STALE_DETECTED_KEY: stale_ids,
    })))
}

/// `GET /projects/:id/knowledge/documents/:docRef/annotations`
async fn list_annotations(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_ref)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let scope = annotation_scope(&state, &session, &project_id).await?;
    let pid = &scope.project_id;
    let (doc_kind, document_id) = parse_doc_ref(&doc_ref);
    ensure_host(&state, pid, doc_kind, document_id.as_deref()).await?;

    let rows = ann_repo::list_annotations(&state.pool(), pid, doc_kind, document_id.as_deref()).await?;
    let stale_ids = detect_stale(&state, pid, doc_kind, document_id.as_deref(), &rows).await?;

    // 置 stale 后重新取一次，保证返回的就是落库后的真实状态
    let rows = if stale_ids.is_empty() {
        rows
    } else {
        ann_repo::list_annotations(&state.pool(), pid, doc_kind, document_id.as_deref()).await?
    };

    let user_id = &session.user.user_id;
    let items: Vec<Value> = rows
        .iter()
        .map(|row| annotation_dto(row, can_moderate(&scope, row, user_id)))
        .collect();
    Ok(ok(json!({
        "annotations": items,
        STALE_DETECTED_KEY: stale_ids,
    })))
}

/// `POST /projects/:id/knowledge/documents/:docRef/annotations`
async fn create_annotation(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_ref)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<CreateAnnotationBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let scope = annotation_scope(&state, &session, &project_id).await?;
    let pid = &scope.project_id;
    let (doc_kind, document_id) = parse_doc_ref(&doc_ref);
    ensure_host(&state, pid, doc_kind, document_id.as_deref()).await?;

    let text = required_string("body", &body.body, BODY_MIN, BODY_MAX)?;
    if text.trim().is_empty() {
        return Err(AppError::bad_request("ANNOTATION_BODY_REQUIRED"));
    }
    let anchor_text = optional_string("anchorText", &body.anchor_text, 0, ANCHOR_MAX)?;
    let anchor_prefix = optional_string("anchorPrefix", &body.anchor_prefix, 0, ANCHOR_MAX)?;
    let anchor_suffix = optional_string("anchorSuffix", &body.anchor_suffix, 0, ANCHOR_MAX)?;

    // 锚点文本为空串等价于「整篇批注」，统一落 NULL，避免 "" 与 NULL 两种表示
    let anchor_text = anchor_text.filter(|a| !a.trim().is_empty());

    let row = ann_repo::create_annotation(
        &state.pool(),
        pid,
        doc_kind,
        document_id.as_deref(),
        anchor_text.as_deref(),
        anchor_prefix.as_deref(),
        anchor_suffix.as_deref(),
        text,
        &session.user.user_id,
    )
    .await?;
    // 刚建的批注作者恒为自己 → canModerate 必为 true，走同一判定保持口径一致
    Ok(ok(annotation_dto(
        &row,
        can_moderate(&scope, &row, &session.user.user_id),
    )))
}

/// `PATCH /projects/:id/knowledge/annotations/:annotationId`
///
/// 两条路径的权限不同（见模块头注释）：改 `body` 要作者或项目所有者，
/// 迁 `status`（结案 / 重开）所有成员可用。
async fn patch_annotation(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, annotation_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<PatchAnnotationBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let scope = annotation_scope(&state, &session, &project_id).await?;
    let existing = ann_repo::find_annotation(&state.pool(), &scope.project_id, &annotation_id)
        .await?
        .ok_or_else(|| AppError::not_found("ANNOTATION_NOT_FOUND"))?;

    let new_body = optional_string("body", &body.body, BODY_MIN, BODY_MAX)?;
    let new_status = optional_enum("status", &body.status, STATUSES)?;
    let new_outcome = optional_enum("outcome", &body.outcome, OUTCOMES)?;
    let new_note = optional_string("resolvedNote", &body.resolved_note, 0, NOTE_MAX)?;

    // 改正文 = 作者本人，或项目所有者 / 平台 ADMIN 代改（治理他人批注）
    if new_body.is_some() && !can_moderate(&scope, &existing, &session.user.user_id) {
        return Err(AppError::forbidden("ANNOTATION_FORBIDDEN")
            .with_message("只有作者或项目所有者可以编辑批注"));
    }

    let status_change = match new_status.as_deref() {
        None => {
            // 只传 outcome / resolvedNote 而不迁移状态：不接受半截结案
            if new_outcome.is_some() || new_note.is_some() {
                return Err(AppError::bad_request("ANNOTATION_STATUS_REQUIRED")
                    .with_message("outcome / resolvedNote 仅在 status=resolved 时可写"));
            }
            None
        }
        Some(s) => {
            // 只对 resolved 记结论；open / stale 一律清空结案痕迹（重新打开必须
            // 把 outcome / note / resolvedBy 一起抹掉，否则回退后仍带着上一轮结论）。
            let (outcome, note) =
                resolution_details(s, new_outcome.as_deref(), new_note.as_deref())?;
            Some(ann_repo::StatusChange {
                status: s,
                outcome,
                note,
                by: &session.user.user_id,
            })
        }
    };

    let row = ann_repo::update_annotation(
        &state.pool(),
        &existing,
        new_body.as_deref(),
        status_change,
    )
    .await?;
    // 权限不变：授权到这条批注，改完仍是同一批注、同一批人可动
    Ok(ok(annotation_dto(
        &row,
        can_moderate(&scope, &existing, &session.user.user_id),
    )))
}

/// `DELETE /projects/:id/knowledge/annotations/:annotationId`（作者或项目所有者）
async fn delete_annotation(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, annotation_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let scope = annotation_scope(&state, &session, &project_id).await?;
    let existing = ann_repo::find_annotation(&state.pool(), &scope.project_id, &annotation_id)
        .await?
        .ok_or_else(|| AppError::not_found("ANNOTATION_NOT_FOUND"))?;
    if !can_moderate(&scope, &existing, &session.user.user_id) {
        return Err(AppError::forbidden("ANNOTATION_FORBIDDEN")
            .with_message("只有作者或项目所有者可以删除批注"));
    }
    ann_repo::delete_annotation(&state.pool(), &annotation_id).await?;
    Ok(ok(json!({ "id": annotation_id })))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/knowledge/annotations",
            get(list_pending_annotations),
        )
        .route(
            "/projects/{projectId}/knowledge/documents/{docRef}/annotations",
            get(list_annotations).post(create_annotation),
        )
        .route(
            "/projects/{projectId}/knowledge/annotations/{annotationId}",
            axum::routing::patch(patch_annotation).delete(delete_annotation),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_ref_mapping_matches_spec() {
        assert_eq!(parse_doc_ref("constitution"), ("constitution", None));
        assert_eq!(parse_doc_ref("memory"), ("memory", None));
        assert_eq!(
            parse_doc_ref("abc123XYZ_01"),
            ("document", Some("abc123XYZ_01".to_string()))
        );
    }

    #[test]
    fn anchor_match_ignores_inline_markup() {
        // 正文带 ** 粗体、` 行内代码、[链接](url)，锚点存的是渲染后可见文本
        let content = "我们约定 **必须** 使用 `nanoid` 生成 [主键](https://x.y/z)。";
        assert!(anchor_resolvable(content, "必须"));
        assert!(anchor_resolvable(content, "使用 nanoid 生成 主键"));
        assert!(anchor_resolvable(content, "我们约定 必须 使用 nanoid 生成 主键"));
    }

    #[test]
    fn anchor_match_folds_whitespace() {
        let content = "第一行\n第二行\n\n第三行";
        assert!(anchor_resolvable(content, "第一行 第二行"));
        assert!(anchor_resolvable(content, "第二行 第三行"));
    }

    #[test]
    fn anchor_match_keeps_visible_markdown_punctuation() {
        // Intraword underscores and escaped punctuation are visible text even
        // though the lightweight Markdown pass treats them as syntax.
        assert!(anchor_resolvable("文件名 foo_bar.md 说明", "foo_bar.md"));
        assert!(anchor_resolvable("星号 \\* 字面量", "星号 * 字面量"));
        assert!(anchor_resolvable("反引号 \\` 字面量", "反引号 ` 字面量"));
    }

    #[test]
    fn drifted_anchor_is_unresolvable() {
        let content = "文档已经被整段重写了。";
        assert!(!anchor_resolvable(content, "我们约定要用 nanoid"));
    }

    #[test]
    fn empty_anchor_means_document_level() {
        assert!(anchor_resolvable("随便什么正文", ""));
        assert!(anchor_resolvable("随便什么正文", "   "));
    }

    fn row_created_by(created_by: &str) -> ann_repo::KnowledgeAnnotationRow {
        ann_repo::KnowledgeAnnotationRow {
            id: "a1".into(),
            project_id: "p1".into(),
            doc_kind: "document".into(),
            document_id: Some("d1".into()),
            anchor_text: None,
            anchor_prefix: None,
            anchor_suffix: None,
            body: "改一下".into(),
            status: "open".into(),
            outcome: None,
            resolved_note: None,
            resolved_by: None,
            resolved_at: None,
            created_by: created_by.into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn scope(owner_id: &str, is_platform_admin: bool) -> AnnotationScope {
        AnnotationScope {
            project_id: "p1".into(),
            owner_id: owner_id.into(),
            is_platform_admin,
        }
    }

    #[test]
    fn author_can_moderate_own_annotation() {
        let ann = row_created_by("u_author");
        // 普通成员不是所有者，但作者身份足以改删
        assert!(can_moderate(&scope("u_owner", false), &ann, "u_author"));
    }

    #[test]
    fn project_owner_can_moderate_others_annotation() {
        let ann = row_created_by("u_author");
        assert!(can_moderate(&scope("u_owner", false), &ann, "u_owner"));
    }

    #[test]
    fn platform_admin_can_moderate_any_annotation() {
        let ann = row_created_by("u_author");
        // owner_id 与 admin 不同：平台 ADMIN 不落库，靠 is_platform_admin 通道放行
        assert!(can_moderate(&scope("u_owner", true), &ann, "u_admin"));
    }

    #[test]
    fn plain_member_cannot_moderate_others_annotation() {
        let ann = row_created_by("u_author");
        assert!(!can_moderate(&scope("u_owner", false), &ann, "u_other"));
    }

    #[test]
    fn dto_exposes_moderation_flag() {
        let ann = row_created_by("u_author");
        assert_eq!(annotation_dto(&ann, true)["canModerate"], json!(true));
        assert_eq!(annotation_dto(&ann, false)["canModerate"], json!(false));
        // 原有字段不受影响
        assert_eq!(annotation_dto(&ann, true)["createdBy"], json!("u_author"));
    }

    #[test]
    fn resolving_requires_explicit_outcome_and_note() {
        assert!(resolution_details("resolved", None, Some("改了第一节")).is_err());
        assert!(resolution_details("resolved", Some("addressed"), None).is_err());
        assert!(resolution_details("resolved", Some("dismissed"), Some("  ")).is_err());
        assert_eq!(
            resolution_details("resolved", Some("addressed"), Some("  已补充示例  "))
                .expect("valid resolution"),
            (Some("addressed"), Some("已补充示例"))
        );
    }
}
