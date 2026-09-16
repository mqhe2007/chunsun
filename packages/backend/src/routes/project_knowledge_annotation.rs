//! 知识库文档批注路由（需求 u-WPvdvYh4Fw）。
//!
//! 批注是对**已有文档内容**的定点反馈，宿主用 `docRef` 表达：
//! `constitution` | `memory` | `{documentId}`。前两者是项目级系统单例（与知识域的
//! `system` 条目同名），其余一律按自定义文档 id 解析 → `doc_kind = 'document'`。
//!
//! 权限分三层，不要合并：
//! 1. **读 / 建**：沿用知识库既有项目可见性（成员 ∪ 创建者 ∪ 平台 ADMIN），
//!    不可见一律 404 `PROJECT_NOT_FOUND`（与知识域一致，不是 403）。
//! 2. **改 / 删**：必须是**作者本人** —— 项目级权限不够，须到作者级，
//!    非作者 403 `ANNOTATION_FORBIDDEN`。
//! 3. **结案 / 重新打开**（PATCH status）：**所有成员可用**。选项 B 既定：
//!    AI 可自行把批注置 resolved（平台主打 AI 能力，低成本闭环），人可重新打开
//!    退回 open 兜底。故这里**不能**套用第 2 层的作者校验，否则 AI 无法结案。
//!
//! 锚点漂移（stale）在**列表读取时**判定：文档正文变了，批注本身没变，
//! 因此不在写入路径判；置 stale 只改状态、**绝不删除**（批注是文档变更因果史）。
//! 判定口径必须与前端高亮定位（`console/src/utils/annotationAnchor.ts`）保持一致，
//! 后端这一份供 Agent 侧读取时复用同一规则。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::core::serde_ext::double_option;
use crate::repos::project_knowledge as ctx_repo;
use crate::repos::project_knowledge_annotation as ann_repo;
use crate::repos::project_memory as mem_repo;
use crate::routes::project_knowledge::visible;
use crate::routes::validate::{optional_enum, optional_string, required_string};
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

fn annotation_dto(row: &ann_repo::KnowledgeAnnotationRow) -> Value {
    json!({
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

/// `GET /projects/:id/knowledge/documents/:docRef/annotations`
async fn list_annotations(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_ref)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let (doc_kind, document_id) = parse_doc_ref(&doc_ref);
    ensure_host(&state, &pid, doc_kind, document_id.as_deref()).await?;

    let rows = ann_repo::list_annotations(&state.pool(), &pid, doc_kind, document_id.as_deref()).await?;
    let stale_ids = detect_stale(&state, &pid, doc_kind, document_id.as_deref(), &rows).await?;

    // 置 stale 后重新取一次，保证返回的就是落库后的真实状态
    let rows = if stale_ids.is_empty() {
        rows
    } else {
        ann_repo::list_annotations(&state.pool(), &pid, doc_kind, document_id.as_deref()).await?
    };

    let items: Vec<Value> = rows.iter().map(annotation_dto).collect();
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
    let pid = visible(&state, &session, &project_id).await?;
    let (doc_kind, document_id) = parse_doc_ref(&doc_ref);
    ensure_host(&state, &pid, doc_kind, document_id.as_deref()).await?;

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
        &pid,
        doc_kind,
        document_id.as_deref(),
        anchor_text.as_deref(),
        anchor_prefix.as_deref(),
        anchor_suffix.as_deref(),
        text,
        &session.user.user_id,
    )
    .await?;
    Ok(ok(annotation_dto(&row)))
}

/// `PATCH /projects/:id/knowledge/annotations/:annotationId`
///
/// 两条路径的权限不同（见模块头注释）：改 `body` 要作者，迁 `status` 所有成员可用。
async fn patch_annotation(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, annotation_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<PatchAnnotationBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let existing = ann_repo::find_annotation(&state.pool(), &pid, &annotation_id)
        .await?
        .ok_or_else(|| AppError::not_found("ANNOTATION_NOT_FOUND"))?;

    let new_body = optional_string("body", &body.body, BODY_MIN, BODY_MAX)?;
    let new_status = optional_enum("status", &body.status, STATUSES)?;
    let new_outcome = optional_enum("outcome", &body.outcome, OUTCOMES)?;
    let new_note = optional_string("resolvedNote", &body.resolved_note, 0, NOTE_MAX)?;

    // 改正文 = 编辑自己的批注 → 作者限定
    if new_body.is_some() && existing.created_by != session.user.user_id {
        return Err(AppError::forbidden("ANNOTATION_FORBIDDEN")
            .with_message("只有作者可以编辑批注"));
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
            let outcome = match s {
                "resolved" => Some(new_outcome.unwrap_or("addressed")),
                _ => None,
            };
            Some(ann_repo::StatusChange {
                status: s,
                outcome,
                note: new_note.as_deref(),
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
    Ok(ok(annotation_dto(&row)))
}

/// `DELETE /projects/:id/knowledge/annotations/:annotationId`（仅作者）
async fn delete_annotation(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, annotation_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let existing = ann_repo::find_annotation(&state.pool(), &pid, &annotation_id)
        .await?
        .ok_or_else(|| AppError::not_found("ANNOTATION_NOT_FOUND"))?;
    if existing.created_by != session.user.user_id {
        return Err(AppError::forbidden("ANNOTATION_FORBIDDEN")
            .with_message("只有作者可以删除批注"));
    }
    ann_repo::delete_annotation(&state.pool(), &annotation_id).await?;
    Ok(ok(json!({ "id": annotation_id })))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
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
}
