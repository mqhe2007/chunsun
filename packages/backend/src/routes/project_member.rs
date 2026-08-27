//! 项目成员域路由（1:1 移植自 `packages/backend/src/routes/projectMember.ts`）。
//!
//! 全部端点落在 `auth_middleware` 之下。权限判定统一走 `services::project_access`
//! （对齐 `lib/projectAccess.ts`）。路径参数 `:memberId` 在旧后端实际即 `userId`。

use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::repos::project_member::MemberWithUser;
use crate::services::project_member as member_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteBody {
    pub identifier: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleBody {
    pub role: Option<String>,
}

fn member_dto(m: &MemberWithUser) -> serde_json::Value {
    json!({
        "id": m.id,
        "userId": m.user_id,
        "role": m.role,
        "createdAt": dt_value(&m.created_at),
        "user": {
            "id": m.u_id,
            "email": m.u_email,
            "nickname": m.u_nickname,
            "qq": m.u_qq,
        }
    })
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let members = member_service::list(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;
    let data: Vec<serde_json::Value> = members.iter().map(member_dto).collect();
    Ok(ok(json!(data)))
}

async fn invite(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<InviteBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    // 对齐旧后端 TypeBox：identifier 仅 minLength(1)，不做 trim（空白串是合法输入）。
    if body.identifier.is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("identifier 不能为空"));
    }
    if let Some(r) = &body.role {
        if r != "ADMIN" && r != "MEMBER" {
            return Err(AppError::unprocessable("VALIDATION_ERROR")
                .with_message("role 取值非法，仅允许 ADMIN/MEMBER"));
        }
    }
    let res = member_service::invite(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        &body.identifier,
        body.role,
        &state.config().public_origin,
    )
    .await?;
    Ok(ok(member_dto(&res.member)))
}

async fn update_role(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, member_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<RoleBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let role = body
        .role
        .ok_or_else(|| AppError::bad_request("EMPTY_PATCH"))?;
    if role != "ADMIN" && role != "MEMBER" {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("role 取值非法，仅允许 ADMIN/MEMBER"));
    }
    let m = member_service::update_role(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        &member_id,
        role,
        &state.config().public_origin,
    )
    .await?;
    Ok(ok(member_dto(&m)))
}

async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, member_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    member_service::remove(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        &member_id,
        &state.config().public_origin,
    )
    .await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

/// 组装项目成员域路由（全路径，由 main.rs `.merge` 挂载）。
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/members",
            get(list).post(invite),
        )
        .route(
            "/projects/{projectId}/members/{memberId}",
            patch(update_role).delete(remove),
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
    fn member_dto_shape_matches_legacy_serialize_member() {
        let m = MemberWithUser {
            id: "m1".into(),
            user_id: "u1".into(),
            role: "MEMBER".into(),
            created_at: chrono::Utc::now(),
            u_id: "u1".into(),
            u_email: "b@c.com".into(),
            u_nickname: Some("Bob".into()),
            u_qq: None,
        };
        let v = member_dto(&m);
        assert_eq!(v["id"], "m1");
        assert_eq!(v["userId"], "u1");
        assert_eq!(v["user"]["id"], "u1");
    }
}
