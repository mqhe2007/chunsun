//! 依赖关系路由（需求/缺陷 Blocking / Blocked By）。
//!
//! 端点设计（挂在项目下，统一处理需求与缺陷两类节点）：
//! - `GET  /projects/{projectId}/dependencies`                                  — 项目内全部依赖边
//! - `GET  /projects/{projectId}/dependencies/{nodeType}/{nodeId}`              — 单节点直接 + 传递依赖
//! - `POST /projects/{projectId}/dependencies`                                  — 添加边 source→target
//! - `DELETE /projects/{projectId}/dependencies/{nodeType}/{nodeId}/{targetType}/{targetId}` — 移除边
//!
//! `nodeType` / `targetType` ∈ {requirement, defect}，非法值 422。
//! 权限档与 requirement/defect 一致：只判项目可见性。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::serde_ext::double_option;
use crate::routes::validate::required_string;
use crate::services::dependency::{
    self as dep_service, AddDependencyArgs, RemoveDependencyArgs,
};
use crate::state::AppState;

const NODE_TYPES: &[&str] = &["requirement", "defect"];
const NO_MAX: usize = usize::MAX;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDependencyBody {
    #[serde(default, deserialize_with = "double_option")]
    pub source_type: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub source_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub target_type: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub target_id: Option<Option<String>>,
}

// ── handlers ────────────────────────────────────────────────────────────

/// GET 项目内全部依赖边（供前端算「被阻塞」标识与依赖图可视化）。
async fn list_all(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    use crate::repos::dependency as dep_repo;
    let project_id =
        crate::services::project_access::visible_project_id(
            &state.pool(),
            &project_id,
            &session.user.user_id,
            session.user.role == "ADMIN",
        )
        .await?;
    let rows = dep_repo::list_all_in_project(&state.pool(), &project_id).await?;
    Ok(ok(
        rows.iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "projectId": r.project_id,
                    "sourceType": r.source_type,
                    "sourceId": r.source_id,
                    "targetType": r.target_type,
                    "targetId": r.target_id,
                    "createdAt": crate::core::datetime::to_value(&r.created_at),
                })
            })
            .collect(),
    ))
}

/// GET 单节点依赖（直接 + 传递）。
async fn get_node(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, node_type, node_id)): Path<(String, String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    if !NODE_TYPES.contains(&node_type.as_str()) {
        return Err(AppError::unprocessable("VALIDATION_ERROR")
            .with_message("nodeType 只能是 requirement / defect 之一"));
    }

    let summary = dep_service::get_dependencies(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        &node_type,
        &node_id,
    )
    .await?;

    let node_dto = |n: &crate::services::dependency::DependencyNode| {
        json!({
            "id": n.id,
            "kind": n.kind,
            "description": n.description,
            "status": n.status,
        })
    };

    Ok(ok(json!({
        "blocking": summary.blocking.iter().map(node_dto).collect::<Vec<_>>(),
        "blockedBy": summary.blocked_by.iter().map(node_dto).collect::<Vec<_>>(),
        "transitiveBlocking": summary.transitive_blocking.iter().map(node_dto).collect::<Vec<_>>(),
        "transitiveBlockedBy": summary.transitive_blocked_by.iter().map(node_dto).collect::<Vec<_>>(),
    })))
}

/// POST 添加依赖边。
async fn add(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<AddDependencyBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    let source_type = required_string("sourceType", &body.source_type, 1, NO_MAX)?;
    let source_id = required_string("sourceId", &body.source_id, 1, NO_MAX)?;
    let target_type = required_string("targetType", &body.target_type, 1, NO_MAX)?;
    let target_id = required_string("targetId", &body.target_id, 1, NO_MAX)?;

    for (field, val) in [("sourceType", source_type), ("targetType", target_type)] {
        if !NODE_TYPES.contains(&val) {
            return Err(AppError::unprocessable("VALIDATION_ERROR")
                .with_message(format!("{field} 只能是 requirement / defect 之一")));
        }
    }

    let row = dep_service::add_dependency(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        AddDependencyArgs {
            source_type,
            source_id,
            target_type,
            target_id,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(json!({
            "id": row.id,
            "projectId": row.project_id,
            "sourceType": row.source_type,
            "sourceId": row.source_id,
            "targetType": row.target_type,
            "targetId": row.target_id,
            "createdAt": crate::core::datetime::to_value(&row.created_at),
        }))),
    ))
}

/// DELETE 移除依赖边（source=node 指向 target 的边）。
async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, node_type, node_id, target_type, target_id)): Path<(String, String, String, String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    for val in [node_type.as_str(), target_type.as_str()] {
        if !NODE_TYPES.contains(&val) {
            return Err(AppError::unprocessable("VALIDATION_ERROR")
                .with_message("节点类型只能是 requirement / defect 之一"));
        }
    }

    let row = dep_service::remove_dependency(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        RemoveDependencyArgs {
            source_type: &node_type,
            source_id: &node_id,
            target_type: &target_type,
            target_id: &target_id,
        },
    )
    .await?;

    Ok(ok(json!({
        "id": row.id,
        "sourceType": row.source_type,
        "sourceId": row.source_id,
        "targetType": row.target_type,
        "targetId": row.target_id,
    })))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/dependencies",
            get(list_all).post(add),
        )
        .route(
            "/projects/{projectId}/dependencies/{nodeType}/{nodeId}",
            get(get_node),
        )
        .route(
            "/projects/{projectId}/dependencies/{nodeType}/{nodeId}/{targetType}/{targetId}",
            delete(remove),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dependency_graph::NodeKind;

    #[test]
    fn node_types_whitelist_matches_node_kind() {
        assert_eq!(NODE_TYPES, &["requirement", "defect"]);
        assert!(NODE_TYPES.contains(&NodeKind::Requirement.as_str()));
        assert!(NODE_TYPES.contains(&NodeKind::Defect.as_str()));
    }
}
