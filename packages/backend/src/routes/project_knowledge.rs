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
use crate::routes::validate::{optional_number, optional_string, required_string};
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
}

#[derive(Debug, Deserialize)]
pub struct CreateKnowledgeBody {
    #[serde(default, deserialize_with = "double_option")]
    pub title: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub content: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub load_strategy: Option<Option<String>>,
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
}

/// `GET /knowledge/documents?strategy=eager|lazy` 的 query 参数
#[derive(Debug, Deserialize)]
pub struct ListKnowledgeQuery {
    pub strategy: Option<String>,
}

fn is_admin(session: &crate::auth::AuthSession) -> bool {
    session.user.role == "ADMIN"
}

async fn visible(
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
    let doc = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id).await?;
    let Some(doc) = doc else {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    };
    Ok(ok(knowledge_doc_dto(&doc)))
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
    let policy = ctx_repo::upsert_project_policy(&state.pool(), &pid, content).await?;
    Ok(ok(constitution_dto(
        &policy.constitution_md,
        &policy.updated_at,
    )))
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

    let doc =
        ctx_repo::create_knowledge_document(&state.pool(), &pid, title, content.unwrap_or(""), load_strategy).await?;
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

    // 校验 loadStrategy 值
    if let Some(ls) = load_strategy {
        if ls != "eager" && ls != "lazy" {
            return Err(AppError::bad_request("INVALID_LOAD_STRATEGY"));
        }
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

    // 存在性检查用 (id, projectId) 双条件，跨项目取不到别人的文档
    let existing = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id).await?;
    let Some(existing) = existing else {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    };

    let doc =
        ctx_repo::update_knowledge_document(&state.pool(), &existing, title, content, sort_order, load_strategy)
            .await?;
    Ok(ok(knowledge_doc_dto(&doc)))
}

async fn delete_knowledge(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    if doc_id == "constitution" {
        return Err(AppError::bad_request("CONSTITUTION_NOT_DELETABLE"));
    }

    let existing = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id).await?;
    if existing.is_none() {
        return Err(AppError::not_found("CONTEXT_DOC_NOT_FOUND"));
    }
    ctx_repo::delete_knowledge_document(&state.pool(), &doc_id).await?;
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
/// 返回字段：key / title / system / loadStrategy，**不含 content**。
async fn get_knowledge_index(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let docs = ctx_repo::list_knowledge_documents(&state.pool(), &pid, None).await?;

    let mut items = Vec::with_capacity(docs.len() + 1);
    // 宪法恒为 eager，固定包含
    items.push(json!({
        "key": "constitution",
        "title": "项目宪法",
        "system": true,
        "loadStrategy": "eager",
    }));
    for doc in &docs {
        items.push(json!({
            "key": doc.id,
            "title": doc.title,
            "system": false,
            "loadStrategy": doc.load_strategy,
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
    Ok(ok(json!({
        "key": "constitution",
        "title": "项目宪法",
        "content": constitution,
        "system": true,
        "loadStrategy": "eager",
        "updatedAt": updated_at.map(|t| dt_value(&t)),
    })))
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

#[cfg(test)]
mod tests {
    use super::*;

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
