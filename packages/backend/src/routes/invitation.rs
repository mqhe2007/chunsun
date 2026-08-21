//! 邀请码路由（1:1 移植自 `invitation.ts`）。
//!
//! 全部 3 端点都在 `auth_middleware` 之下，且**仅 ADMIN**：非 ADMIN → 403 FORBIDDEN。
//!
//! | 端点 | 说明 |
//! | --- | --- |
//! | `GET /admin/invitations` | 列出全部邀请码（createdAt DESC） |
//! | `POST /admin/invitations` | 生成 code（16 随机字节 hex）、可选 role/maxUses/expiresAt/sendTo |
//! | `DELETE /admin/invitations/{id}` | 删除；不存在 → 500（对齐旧端 P2025 裸抛收敛） |
//!
//! `sendTo` 分支旧端调用邮件服务（SMTP 未配置时静默失败，**不影响响应**），这里跳过实际发送。

use axum::extract::{Path, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::api::{ok, ok_no_data, AppError};
use crate::auth::{AuthSession, CurrentUser};
use crate::repos::invitation::{
    create_invitation_code, delete_invitation_code, generate_invitation_code, invitation_dto,
    list_invitation_codes,
};
use crate::state::AppState;

fn require_admin(session: &AuthSession) -> Result<(), AppError> {
    if session.user.role != "ADMIN" {
        return Err(AppError::forbidden("FORBIDDEN"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct InvitationBody {
    role: Option<String>,
    #[serde(rename = "maxUses")]
    max_uses: Option<i32>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<String>,
    #[serde(rename = "sendTo")]
    send_to: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<crate::api::ApiResponse<Vec<serde_json::Value>>>, AppError> {
    require_admin(&session)?;
    let codes = list_invitation_codes(&state.pool()).await?;
    Ok(ok(codes.iter().map(invitation_dto).collect()))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<InvitationBody>,
) -> Result<Json<crate::api::ApiResponse<serde_json::Value>>, AppError> {
    require_admin(&session)?;
    let code = generate_invitation_code();
    let role = body.role.unwrap_or_else(|| "USER".to_string());
    let max_uses = body.max_uses.unwrap_or(1);
    let expires_at: Option<DateTime<Utc>> = match &body.expires_at {
        Some(s) if !s.is_empty() => Some(
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| AppError::internal("invalid expiresAt"))?,
        ),
        _ => None,
    };
    // 旧端 sendTo 分支调用邮件服务（SMTP 未配置静默失败，不影响响应），这里跳过实际发送。
    let _ = &body.send_to;
    let inv = create_invitation_code(
        &state.pool(),
        &code,
        &session.user.user_id,
        &role,
        max_uses,
        expires_at,
    )
    .await?;
    Ok(ok(invitation_dto(&inv)))
}

async fn delete(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    require_admin(&session)?;
    let n = delete_invitation_code(&state.pool(), &id).await?;
    if n == 0 {
        // 旧端对不存在 id 抛 P2025（未捕获）→ 500；这里复刻为 internal。
        return Err(AppError::internal("invitation code not found"));
    }
    Ok(ok_no_data())
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/admin/invitations",
            axum::routing::get(list).post(create),
        )
        .route("/admin/invitations/{id}", axum::routing::delete(delete))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
