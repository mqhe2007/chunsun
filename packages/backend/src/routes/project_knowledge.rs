//! 项目知识路由（1:1 移植自 `routes/projectContexts.ts` + `routes/projectContext.ts`）。
//!
//! 两个旧文件合并成一个模块：它们共用 `listProjectKnowledge`，且 `/knowledge` 与
//! `/context`（兼容旧路径）共用同一组 handler。六条端点全部走 `auth_middleware`，
//! 权限档**只有项目可见性**——不可见一律 404 `PROJECT_NOT_FOUND`（不是 403）。
//!
//! 三个必须逐字节复刻的怪癖（全部实测自旧后端，不是推断）：
//!
//! 1. **`sortOrder` 落库前向零截断**：`3.7 → 3`、`-3.7 → -3`、`-0.5 → 0` 全部
//!    静默放行；只有超出 `int4` 范围（`±2147483648`）才被 PG 拒绝 → 500。
//!    见 [`crate::core::js_number::prisma_int`]。
//! 2. **空 `PUT {}` 不刷新 `updatedAt`**：Prisma 对空 `data` 退化成纯读，
//!    `@updatedAt` 不动。这与 defect 域「空补丁也刷新」的行为**相反**，
//!    因为那边显式写了字段。仓储层 [`crate::repos::project_knowledge::update_knowledge_document`]
//!    里的提前返回就是为这个。
//! 3. **`title` 的 trim 时机**：POST 在路由层 trim 后判空 → 纯空格是
//!    400 `TITLE_REQUIRED`；PUT 在仓储层 trim 且**不判空** → 纯空格存成空串、200。
//!    同一个字段两条路径两种结局，不要顺手统一。
//!
//! 另有一处死代码需要保留形状：`PUT /knowledge/constitution` 被静态路由抢先命中，
//! 所以 `docId == "constitution"` 分支里的 400 `USE_CONSTITUTION_ENDPOINT`
//! 永远不会触发（axum 的 matchit 与 Elysia 一样静态段优先）。而
//! `DELETE /knowledge/constitution` 没有静态路由，**会**命中并返回 400
//! `CONSTITUTION_NOT_DELETABLE`。

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::core::js_number::prisma_int;
use crate::core::serde_ext::double_option;
use crate::repos::project_knowledge as ctx_repo;
use crate::repos::project_env_var::count_env_vars_by_project;
use crate::repos::requirement::count_requirements_by_status;
use crate::routes::dto::{constitution_dto, knowledge_doc_dto};
use crate::routes::validate::{
    optional_number, optional_string, required_revision, required_string,
};
use crate::services::project_knowledge_annotation as ann_service;
use crate::services::project_access::visible_project_id;
use crate::services::project_knowledge::list_project_knowledge;
use crate::state::AppState;

/// `title` 是 `t.String({ minLength: 1, maxLength: 200 })`。
const TITLE_MIN: usize = 1;
const TITLE_MAX: usize = 200;
/// `content` 是裸 `t.String()`：空串合法、无上限。
const CONTENT_MIN: usize = 0;
const CONTENT_MAX: usize = usize::MAX;

#[derive(Debug, Deserialize)]
pub struct ConstitutionBody {
    #[serde(default, deserialize_with = "double_option")]
    pub content: Option<Option<String>>,
    /// 乐观锁版本号。**必填**——`0` 表示「我认为宪法还不存在」。
    ///
    /// 刻意不用 `double_option`：版本号的「缺省」与「显式 null」是同一种意思
    /// （没说清基于哪一版），没有第三态，见 [`required_revision`]。
    #[serde(default)]
    pub revision: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateKnowledgeBody {
    #[serde(default, deserialize_with = "double_option")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub content: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub load_strategy: Option<Option<String>>,
    /// 所属主文档 id；不传 / null = 建为根文档
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKnowledgeBody {
    #[serde(default, deserialize_with = "double_option")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub content: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub sort_order: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub load_strategy: Option<Option<String>>,
    /// 双层语义：不传 = 不改；null / 空串 = 解除关联（回到根）
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<String>>,
    /// 乐观锁版本号。**必填**，见 [`required_revision`]。
    #[serde(default)]
    pub revision: Option<i32>,
}

/// `GET /knowledge/documents?strategy=eager|lazy` 的 query 参数
#[derive(Debug, Deserialize)]
pub struct ListKnowledgeQuery {
    pub strategy: Option<String>,
}

/// `DELETE /knowledge/documents/:docId?withChildren=true` 的 query 参数。
/// 默认拒绝删有子文档的文档，显式 withChildren 才递归级联。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteKnowledgeQuery {
    #[serde(default)]
    pub with_children: bool,
}

fn is_admin(session: &crate::auth::AuthSession) -> bool {
    session.user.role == "ADMIN"
}

/// 项目可见性校验：成员或管理员可见（知识库 / 项目记忆等共享）。
pub async fn visible(
    state: &AppState,
    session: &crate::auth::AuthSession,
    project_id: &str,
) -> Result<String, AppError> {
    visible_project_id(
        &state.pool(),
        project_id,
        &session.user.user_id,
        is_admin(session),
    )
    .await
}

/// 校验父文档（主文档/分册关联）：必须同项目存在，且绝不能是自身或自身后代（防环）。
///
/// `self_id` 为 None（create，还没有自身）时只查存在性；update 时额外做环校验。
async fn validate_parent_doc(
    state: &AppState,
    project_id: &str,
    self_id: Option<&str>,
    parent_id: &str,
) -> Result<(), AppError> {
    if self_id == Some(parent_id) {
        return Err(AppError::bad_request("PARENT_CYCLE"));
    }
    let parent =
        ctx_repo::find_knowledge_document(&state.pool(), project_id, parent_id).await?;
    if parent.is_none() {
        // 跨项目父、不存在的 id、宪法/记忆等系统 key 都归这一类
        return Err(AppError::bad_request("INVALID_PARENT_DOC"));
    }
    if let Some(id) = self_id {
        if ctx_repo::is_in_subtree(&state.pool(), project_id, id, parent_id).await? {
            return Err(AppError::bad_request("PARENT_CYCLE"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 第 11 域

async fn list_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    Query(query): Query<ListKnowledgeQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let strategy = query.strategy.as_deref();
    // 校验 strategy 值
    if let Some(s) = strategy {
        if s != "eager" && s != "lazy" {
            return Err(AppError::bad_request("INVALID_LOAD_STRATEGY"));
        }
    }
    let contexts = list_project_knowledge(&state.pool(), &pid, strategy).await?;
    Ok(ok(json!({ "contexts": contexts })))
}

/// `GET /knowledge/documents/:docId`：单条文档查询（含宪法）。
///
/// 宪法走静态路由 `/knowledge/constitution` 的 GET，这里只处理自定义文档。
async fn get_knowledge_doc(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    if doc_id == "constitution" {
        // 宪法走单独的静态路由，这里不应命中（axum 静态段优先）
        return Err(AppError::bad_request("USE_CONSTITUTION_ENDPOINT"));
    }
    if doc_id == "memory" {
        let memory = crate::repos::project_memory::get_project_memory(&state.pool(), &pid).await?;
        let mut dto = json!({
            "key": "memory",
            "title": "项目记忆",
            "content": memory
                .as_ref()
                .and_then(|row| row.snapshot.as_deref())
                .unwrap_or(""),
            "system": true,
            "loadStrategy": "eager",
            // 缺行时给 0：与「首次写入传 revision = 0」的语义对齐
            // （`upsert_project_memory` 的 `expected_revision = 0` 表示调用方
            // 认为这行还不存在）。若这里给 null，Console 就得自己判空再补 0，
            // 那个分支迟早会被漏掉，而漏掉的表现是写不进去。
            "revision": memory.as_ref().map_or_else(absent_row_revision, |row| row.revision),
            "updatedAt": memory.as_ref().map(|row| dt_value(&row.updated_at)),
            "breadcrumb": [],
            "children": [],
        });
        if let Some(block) = ann_service::load_block_for_doc(
            &state.pool(),
            &pid,
            "memory",
            None,
        )
        .await?
        {
            dto["annotations"] = block;
        }
        return Ok(ok(dto));
    }
    let doc = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id).await?;
    let Some(doc) = doc else {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    };

    // 关联关系（主文档 ↔ 分册）：面包屑（根→父）与直接子文档清单。
    // 复用全量前序树一次查询，避免逐级 find 的 N+1。
    use std::collections::HashMap;
    let nodes = ctx_repo::list_knowledge_document_nodes(&state.pool(), &pid).await?;
    let by_id: HashMap<&str, &ctx_repo::KnowledgeDocNode> =
        nodes.iter().map(|n| (n.doc.id.as_str(), n)).collect();
    let mut breadcrumb: Vec<Value> = Vec::new();
    let mut cursor = doc.parent_id.clone();
    // guard 防手改出的环把面包屑撑爆（正常树深度远小于 64）
    for _ in 0..64 {
        let Some(parent) = cursor.as_deref().and_then(|pid| by_id.get(pid)) else {
            break;
        };
        breadcrumb.insert(0, json!({ "id": parent.doc.id, "title": parent.doc.title }));
        cursor = parent.doc.parent_id.clone();
    }
    let children: Vec<Value> = nodes
        .iter()
        .filter(|n| n.doc.parent_id.as_deref() == Some(doc.id.as_str()))
        .map(|n| {
            json!({
                "id": n.doc.id,
                "title": n.doc.title,
                "loadStrategy": n.doc.load_strategy,
            })
        })
        .collect();

    // 单篇深读 → 批注给**全量形态**（含锚点上下文与作者）。Agent 读到这里通常正是
    // 要按批注改这篇文档，摘要不够用。见 services::project_knowledge_annotation。
    let mut dto = knowledge_doc_dto(&doc);
    dto["breadcrumb"] = Value::Array(breadcrumb);
    dto["children"] = Value::Array(children);
    if let Some(block) = ann_service::load_block_for_doc(
        &state.pool(),
        &pid,
        "document",
        Some(&doc.id),
    )
    .await?
    {
        dto["annotations"] = block;
    }
    Ok(ok(dto))
}

async fn put_constitution(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<ConstitutionBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    // 可见性检查在 body 校验之后：ValidatedJson 是 extractor，先于 handler 跑。
    // 旧后端顺序相反（先查项目再校验 body），但两者只在「项目不可见 + body 非法」
    // 同时成立时才有差别，此时旧后端 404 / 新后端 422。对拍脚本避开该组合。
    let content = required_string("content", &body.content, CONTENT_MIN, CONTENT_MAX)?;
    // 版本校验在 body 校验之后、写库之前：先确认「补丁本身合法」，
    // 再确认「你知道自己基于哪一版」。顺序反过来的话，缺版本的请求会先拿到 400，
    // 而 body 里可能还藏着个 422 —— 一次性把所有 body 问题暴露出来更有用。
    let expected = required_revision("revision", body.revision)?;
    let policy = ctx_repo::upsert_project_policy(&state.pool(), &pid, content, expected).await?;

    match policy {
        ctx_repo::WriteOutcome::Ok(row) => Ok(ok(constitution_dto(&row))),
        // 行不存在且调用方声称的版本不是 0：调用方基于一个不存在的版本在写。
        // 归到 409 而不是 404 —— 宪法是项目的一部分，项目本身可见，
        // 「宪法行还没建」不是「资源不存在」，而是「你拿的版本号是错的」。
        ctx_repo::WriteOutcome::NotFound => Err(constitution_conflict(&pid, expected, None)),
        ctx_repo::WriteOutcome::Conflict(current) => {
            Err(constitution_conflict(&pid, expected, Some(&current)))
        }
    }
}

/// 409 `CONSTITUTION_CONFLICT`：附上当前版本号与当前正文，Agent 据此合并重试。
///
/// `current` 为 `None` 时（`NotFound` 分支）只给版本号：那种情况下没有「当前正文」
/// 可言，硬塞一个空串会让 Agent 以为宪法被清空了。
fn constitution_conflict(
    project_id: &str,
    your_revision: i32,
    current: Option<&ctx_repo::ProjectPolicyRow>,
) -> AppError {
    let mut data = json!({
        "projectId": project_id,
        "yourRevision": your_revision,
        "currentRevision": current.map(|c| c.revision).unwrap_or(0),
    });
    if let Some(row) = current {
        data["currentContent"] = Value::String(row.constitution_md.clone());
        data["updatedAt"] = dt_value(&row.updated_at);
    }
    AppError::conflict("CONSTITUTION_CONFLICT")
        .with_message("项目宪法已被他人修改")
        .with_hint("用 currentContent 合并你的改动，revision 取 currentRevision 后重试")
        .with_data(data)
}

async fn create_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateKnowledgeBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let title = required_string("title", &body.title, TITLE_MIN, TITLE_MAX)?;
    let content = optional_string("content", &body.content, CONTENT_MIN, CONTENT_MAX)?;
    let load_strategy = optional_string("loadStrategy", &body.load_strategy, 0, 16)?;

    // 校验 loadStrategy 值
    if let Some(ls) = load_strategy {
        if ls != "eager" && ls != "lazy" {
            return Err(AppError::bad_request("INVALID_LOAD_STRATEGY"));
        }
    }

    // minLength=1 只拦空串，纯空格要靠这里的 trim 判空 → 400（不是 422）
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::bad_request("TITLE_REQUIRED"));
    }

    // 空串等价于不挂父（CLI 友好）；显式挂父时先校验
    let parent_id = body.parent_id.flatten().filter(|s| !s.is_empty());
    if let Some(p) = parent_id.as_deref() {
        validate_parent_doc(&state, &pid, None, p).await?;
    }

    let doc = ctx_repo::create_knowledge_document(
        &state.pool(),
        &pid,
        title,
        content.unwrap_or(""),
        load_strategy,
        parent_id.as_deref(),
    )
    .await?;
    Ok(ok(knowledge_doc_dto(&doc)))
}

async fn update_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<UpdateKnowledgeBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    if doc_id == "constitution" {
        return Err(AppError::bad_request("USE_CONSTITUTION_ENDPOINT"));
    }

    let title = optional_string("title", &body.title, TITLE_MIN, TITLE_MAX)?;
    let content = optional_string("content", &body.content, CONTENT_MIN, CONTENT_MAX)?;
    let sort_order = optional_number("sortOrder", &body.sort_order)?;
    let load_strategy = optional_string("loadStrategy", &body.load_strategy, 0, 16)?;
    let expected = required_revision("revision", body.revision)?;

    // 空补丁一律 400。**这条路径原先是仓储层的提前返回**（复刻 Prisma 空 `data`
    // 退化成纯读、不刷 `updated_at` 的行为），现在拦在 handler：
    // 带版本号的空 PUT 是「我读过这一版但什么也不改」，没有意义；
    // 不带版本号的空 PUT 更是没意义。统一 400，不再有例外。
    //
    // 判空要连 `parent_id` 一起看：`Some(None)` 是「解除父关联」，是一次真实写入，
    // 而 `None` 才是「不改」。只看前四个字段会把解绑请求误判成空补丁。
    let is_empty_patch = title.is_none()
        && content.is_none()
        && sort_order.is_none()
        && load_strategy.is_none()
        && body.parent_id.is_none();
    if is_empty_patch {
        return Err(AppError::bad_request("EMPTY_PATCH")
            .with_message("请求体里没有任何要修改的字段")
            .with_hint("至少提供 title / content / sortOrder / loadStrategy / parentId 之一"));
    }

    // 校验 loadStrategy 值
    if let Some(ls) = load_strategy {
        if ls != "eager" && ls != "lazy" {
            return Err(AppError::bad_request("INVALID_LOAD_STRATEGY"));
        }
    }

    // 父关联：不传 = 不改；null / 空串 = 解除关联（回到根）
    let parent_update: Option<Option<String>> = match &body.parent_id {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => Some(if s.is_empty() { None } else { Some(s.clone()) }),
    };
    if let Some(Some(p)) = &parent_update {
        validate_parent_doc(&state, &pid, Some(&doc_id), p).await?;
    }

    let sort_order = match sort_order {
        // 越界的 sortOrder 在旧后端是 Prisma 未捕获异常 → 500
        Some(n) => Some(prisma_int(n).map_err(|_| {
            AppError::internal(format!(
                "Value out of range for the type: value \"{n}\" is out of range for type integer"
            ))
        })?),
        None => None,
    };

    // 存在性检查用 (id, projectId) 双条件，跨项目取不到别人的文档。
    // **保留这次预读**：它能区分「这个 docId 根本不属于你的项目」与「版本号不对」，
    // 前者是 404 后者是 409。仓储层的条件 UPDATE 只看 `id`，落空后重查也只按 id，
    // 分不出跨项目这一层——所以这个 404 必须在这里判。
    //
    // 只判存在性、不要行本身：版本号以**客户端传来的**为准，拿库里的行去覆盖它
    // 等于把乐观锁退化成「永远匹配」。
    if ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    }

    let doc = ctx_repo::update_knowledge_document(
        &state.pool(),
        &doc_id,
        expected,
        title,
        content,
        sort_order,
        load_strategy,
        parent_update.as_ref().map(|o| o.as_deref()),
    )
    .await?;

    match doc {
        ctx_repo::WriteOutcome::Ok(row) => Ok(ok(knowledge_doc_dto(&row))),
        // 走到这里说明上面的预读查到了、条件 UPDATE 却落空且重查也查不到 ——
        // 只可能是两次查询之间有人把文档删了。报 404 而不是 409：
        // 版本冲突的前提是「行还在、只是变了」。
        ctx_repo::WriteOutcome::NotFound => Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND")),
        ctx_repo::WriteOutcome::Conflict(current) => {
            Err(knowledge_doc_conflict(&doc_id, expected, &current))
        }
    }
}

/// 409 `KNOWLEDGE_DOC_CONFLICT`：把当前正文整篇带回去，Agent 就地合并、改完重试。
///
/// **为什么整篇返回**：`CONTENT_MAX` 是 `usize::MAX`（旧后端是裸 `t.String()`），
/// 大文档确实可能几万字号。权衡后仍然全给——409 是罕见事件，一次大响应换掉
/// Agent「收到 409 → 再发一次 GET → 再合并」的整轮往返，对自动化更划算；
/// 且少一次往返就少一个「合并时又被改了」的窗口。
///
/// 客户端要拿全文必须走 `--json` 或 `ApiError::body()`：CLI 的人读输出只印摘要，
/// 见 `parse_error_detail` 的既有契约。
fn knowledge_doc_conflict(
    doc_id: &str,
    your_revision: i32,
    current: &ctx_repo::KnowledgeDocRow,
) -> AppError {
    AppError::conflict("KNOWLEDGE_DOC_CONFLICT")
        .with_message("文档已被他人修改，你的写入未生效")
        .with_hint("用 currentContent 合并你的改动，revision 取 currentRevision 后重试")
        .with_data(json!({
            "docId": doc_id,
            "yourRevision": your_revision,
            "currentRevision": current.revision,
            "currentTitle": current.title,
            "currentContent": current.content,
            "updatedAt": dt_value(&current.updated_at),
        }))
}

async fn delete_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
    Query(query): Query<DeleteKnowledgeQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    if doc_id == "constitution" {
        return Err(AppError::bad_request("CONSTITUTION_NOT_DELETABLE"));
    }

    let existing = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id).await?;
    if existing.is_none() {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    }

    // 默认拒绝删有子文档的文档（避免静默把分册提升为根）；显式 withChildren=true 才级联。
    let child_count = ctx_repo::count_children(&state.pool(), &pid, &doc_id).await?;
    if child_count > 0 && !query.with_children {
        return Err(AppError::bad_request("DOC_HAS_CHILDREN"));
    }

    let mut doomed = vec![doc_id.clone()];
    if child_count > 0 {
        doomed.extend(ctx_repo::list_descendant_ids(&state.pool(), &pid, &doc_id).await?);
    }

    // 文档删除是**宿主级联**：批注脱离宿主就没有锚定对象，留下只会变成孤儿数据。
    // 注意这不同于批注的 resolved/stale 保留策略——那边是「文档还在，批注作为变更
    // 因果史留存」；这边是文档本身没了，因果史的宿主已不存在。
    // 级联删除时子树每一篇的批注都按同一规则清理。
    for id in &doomed {
        crate::repos::project_knowledge_annotation::delete_annotations_for_document(
            &state.pool(),
            &pid,
            id,
        )
        .await?;
    }
    // 先删子树、后删根（FK 是 SET NULL，顺序不敏感，显式从深到浅更直观）
    for id in doomed.iter().rev() {
        ctx_repo::delete_knowledge_document(&state.pool(), id).await?;
    }
    // 回的是入参 docId，不是删掉那行的 id（两者相同，但形状要照抄）
    Ok(ok(json!({ "id": doc_id })))
}

/// `DELETE /knowledge/constitution`：宪法不可删除。
///
/// 静态路由 `/knowledge/constitution` 只挂了 PUT，若不单独挂 DELETE，
/// axum 对该路径的 DELETE 会命中静态路由（无 DELETE 方法）直接回 405，
/// 落不到 `:docId` 通配路由的 CONSTITUTION_NOT_DELETABLE 分支。
/// 故显式挂 DELETE，先校验项目可见性（不可见 → 404），再回 400 对齐旧后端。
async fn delete_constitution(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let _pid = visible(&state, &session, &project_id).await?;
    Err(AppError::bad_request("CONSTITUTION_NOT_DELETABLE"))
}

// ---------------------------------------------------------------- 第 12 域

/// `Object.fromEntries(reqCounts.map(r => [r.status, r._count]))`。
///
/// 空项目下是 `{}` 而不是四个状态补零。
fn by_status_map(groups: &[(String, i64)]) -> Value {
    let mut map = Map::new();
    for (status, count) in groups {
        map.insert(status.clone(), Value::from(*count));
    }
    Value::Object(map)
}

/// `GET /knowledge/index`：知识目录（所有文档元信息，不含正文）。
///
/// 固定 eager 加载，Agent 启动时拉取，用于感知有哪些 lazy 文档可按需拉取。
/// 返回字段：key / title / system / loadStrategy / parentId / depth，**不含 content**。
/// 顺序为**前序**（父后紧跟其子树）——Agent / CLI 按序渲染即可看到主文档与分册的从属关系。
async fn get_knowledge_index(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let nodes = ctx_repo::list_knowledge_document_nodes(&state.pool(), &pid).await?;

    let mut items = Vec::with_capacity(nodes.len() + 2);
    // 宪法恒为 eager，固定包含（系统项恒为根）
    items.push(json!({
        "key": "constitution",
        "title": "项目宪法",
        "system": true,
        "loadStrategy": "eager",
        "parentId": Value::Null,
        "depth": 0,
    }));
    // 项目记忆恒为 eager，固定包含（属于项目知识库之一，仅可编辑不可删除）
    items.push(json!({
        "key": "memory",
        "title": "项目记忆",
        "system": true,
        "loadStrategy": "eager",
        "parentId": Value::Null,
        "depth": 0,
    }));
    for node in &nodes {
        items.push(json!({
            "key": node.doc.id,
            "title": node.doc.title,
            "system": false,
            "loadStrategy": node.doc.load_strategy,
            "parentId": node.doc.parent_id,
            "depth": node.depth,
        }));
    }
    Ok(ok(json!({ "index": items })))
}

async fn get_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    // 这里需要项目的 name/description，不能只拿 visible_project_id 的 id
    let project = crate::repos::project::get_project_by_id(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        is_admin(&session),
    )
    .await?
    .ok_or_else(|| AppError::not_found("PROJECT_NOT_FOUND"))?;

    let req_counts = count_requirements_by_status(&state.pool(), &project.id).await?;
    let env_var_count = count_env_vars_by_project(&state.pool(), &project.id).await?;
    let contexts = list_project_knowledge(&state.pool(), &project.id, None).await?;

    let total: i64 = req_counts.iter().map(|(_, c)| c).sum();

    Ok(ok(json!({
        "project": {
            "id": project.id,
            "name": project.name,
            "description": project.description,
            "envVarCount": env_var_count,
        },
        "contexts": contexts,
        "summary": {
            "requirements": {
                "total": total,
                // 蛇形命名，与同层的 camelCase `envVarCount` 不一致，但旧后端就是这样
                "by_status": by_status_map(&req_counts),
            },
            "envVars": { "total": env_var_count },
        },
    })))
}

/// `GET /knowledge/constitution`：获取项目宪法。
async fn get_constitution(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let policy = ctx_repo::get_project_policy(&state.pool(), &pid).await?;
    let constitution = policy.as_ref().map_or("", |p| p.constitution_md.as_str());
    let updated_at = policy.as_ref().map(|p| p.updated_at);
    let mut dto = json!({
        "key": "constitution",
        "title": "项目宪法",
        "content": constitution,
        "system": true,
        "loadStrategy": "eager",
        // 宪法行尚未建立时给 0（= 调用方可以创建），建立了才给真实版本。
        // 为什么不复用 `constitution_dto`：那条形状少一个 `loadStrategy`，
        // 而 GET 与 PUT 的响应在 Console 里走同一个 `applyDoc`。
        "revision": policy.as_ref().map_or_else(absent_row_revision, |p| p.revision),
        "updatedAt": updated_at.map(|t| dt_value(&t)),
    });
    // 单篇形态 → 批注全量（宪法是 Agent 每次启动必读的，批注直接影响执行策略）
    if let Some(block) =
        ann_service::load_block_for_doc(&state.pool(), &pid, "constitution", None).await?
    {
        dto["annotations"] = block;
    }
    Ok(ok(dto))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // 知识目录（所有文档元信息，不含正文，固定 eager 加载）
        .route("/projects/{projectId}/knowledge/index", get(get_knowledge_index))
        // 项目知识概览（含项目信息、需求/环境变量统计、知识文档列表）
        .route("/projects/{projectId}/knowledge", get(get_knowledge))
        // 知识文档 CRUD
        .route("/projects/{projectId}/knowledge/documents", get(list_knowledge))
        .route("/projects/{projectId}/knowledge/documents", post(create_knowledge))
        .route(
            "/projects/{projectId}/knowledge/constitution",
            get(get_constitution).put(put_constitution).delete(delete_constitution),
        )
        .route("/projects/{projectId}/knowledge/documents/{docId}", get(get_knowledge_doc))
        .route("/projects/{projectId}/knowledge/documents/{docId}", put(update_knowledge))
        .route(
            "/projects/{projectId}/knowledge/documents/{docId}",
            delete(delete_knowledge),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

/// 系统行（宪法 / 项目记忆）不存在时下发的版本号。
///
/// 必须是 **0**，不能是 `null` 或 `1`：
/// - 0 是「我认为这行还不存在」的写入意图，`upsert_project_policy` /
///   `upsert_project_memory` 的 `expected_revision = 0` 分支据此走 INSERT；
/// - 若给 `null`，客户端带版本写回时要自己判空补 0，漏掉就写不进去；
/// - 若给 1，客户端会带着「库里有第一版」的错觉去写，落空成 409 ——
///   一个凭空造出来的冲突，用户无从理解（他明明是从这个接口读的版本）。
fn absent_row_revision() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 空行的版本号必须能被写路径接受：这是「首次写宪法 / 首次写项目记忆」
    /// 这条链路的接缝——读侧给 0、写侧认 0，两边一旦不一致，
    /// 表现是新建项目后第一次保存永远失败。
    #[test]
    fn absent_system_row_reports_revision_zero() {
        assert_eq!(absent_row_revision(), 0);
        assert!(
            crate::repos::project_knowledge::revision_matches(absent_row_revision(), 0),
            "读侧下发的缺行版本号必须能通过「行不存在」的写侧判定"
        );
    }

    #[test]
    fn by_status_is_empty_object_when_no_requirements() {
        assert_eq!(by_status_map(&[]), json!({}));
    }

    #[test]
    fn by_status_does_not_pad_missing_statuses() {
        let groups = vec![("pending".to_string(), 2), ("completed".to_string(), 1)];
        assert_eq!(by_status_map(&groups), json!({"pending": 2, "completed": 1}));
    }

    #[test]
    fn sort_order_truncates_toward_zero_before_range_check() {
        assert_eq!(prisma_int(3.7), Ok(3));
        assert_eq!(prisma_int(-3.7), Ok(-3));
        assert_eq!(prisma_int(-0.5), Ok(0));
        // 越界才是 500
        assert!(prisma_int(2_147_483_648.0).is_err());
    }
}
