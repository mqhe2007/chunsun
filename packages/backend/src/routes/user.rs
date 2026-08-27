//! 用户域路由（1:1 移植自 `packages/backend/src/routes/user.ts`）。
//!
//! 全部端点落在 `auth_middleware` 之下（旧后端 `.use(authGuard)`）。
//! `/users/admin/*` 额外用 `AdminUser` 守卫（role != ADMIN → 403）。
//! 与旧后端的两处有意差异见 `services/user.rs` 文件头注释。

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::api::{ok, ok_with_meta, ApiResponse, AppError, ValidatedJson};
use crate::auth::{AdminUser, CurrentUser};
use crate::core::datetime::to_value as dt_value;
use crate::repos::user::User;
use crate::services::notification as notification_service;
use crate::services::user as user_service;
use crate::state::AppState;

/// 长度/格式校验，对齐 Elysia 的 `t.String({ minLength, maxLength, format: "email" })`。
fn check_len(field: &str, value: &str, min: usize, max: usize) -> Result<(), AppError> {
    let len = value.chars().count();
    if len < min || len > max {
        return Err(AppError::unprocessable("VALIDATION_ERROR")
            .with_message(format!("{field} 长度需在 {min}~{max} 之间")));
    }
    Ok(())
}

fn check_email(value: &str) -> Result<(), AppError> {
    let bytes_ok = value.len() <= 100 && !value.contains(char::is_whitespace);
    let shape_ok = match value.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        }
        None => false,
    };
    if bytes_ok && shape_ok {
        Ok(())
    } else {
        Err(AppError::unprocessable("VALIDATION_ERROR").with_message("email 格式不正确"))
    }
}

fn check_enum(field: &str, value: &str, allowed: &[&str]) -> Result<(), AppError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::unprocessable("VALIDATION_ERROR")
            .with_message(format!("{field} 取值非法，仅允许 {allowed:?}")))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBody {
    pub nickname: Option<String>,
    pub qq: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCreateBody {
    pub email: String,
    pub password: String,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdateBody {
    pub role: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminListQuery {
    page: Option<i64>,
    #[serde(rename = "pageSize")]
    page_size: Option<i64>,
}

// ---- DTO 构建（字段裁剪严格对齐旧后端各端点） ----

fn user_me(u: &User) -> serde_json::Value {
    json!({
        "id": u.id,
        "email": u.email,
        "qq": u.qq,
        "nickname": u.nickname,
        "role": u.role,
        "status": u.status,
        "emailVerified": u.email_verified,
        "createdAt": dt_value(&u.created_at),
        "updatedAt": dt_value(&u.updated_at),
    })
}

fn user_me_profile(u: &User) -> serde_json::Value {
    let mut v = user_me(u);
    v.as_object_mut().unwrap().remove("emailVerified");
    v
}

fn user_admin(u: &User) -> serde_json::Value {
    json!({
        "id": u.id,
        "email": u.email,
        "qq": u.qq,
        "nickname": u.nickname,
        "role": u.role,
        "status": u.status,
        "emailVerified": u.email_verified,
        "createdAt": dt_value(&u.created_at),
        "updatedAt": dt_value(&u.updated_at),
    })
}

fn user_admin_created(u: &User) -> serde_json::Value {
    json!({
        "id": u.id,
        "email": u.email,
        "nickname": u.nickname,
        "role": u.role,
        "status": u.status,
        "createdAt": dt_value(&u.created_at),
    })
}

fn user_admin_updated(u: &User) -> serde_json::Value {
    json!({
        "id": u.id,
        "email": u.email,
        "nickname": u.nickname,
        "role": u.role,
        "status": u.status,
        "updatedAt": dt_value(&u.updated_at),
    })
}

fn user_search(u: &User) -> serde_json::Value {
    json!({
        "id": u.id,
        "email": u.email,
        "nickname": u.nickname,
        "qq": u.qq,
    })
}

// ---- 处理函数 ----

async fn me(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let u = user_service::get_me(&state.pool(), &session.user.user_id).await?;
    Ok(ok(user_me(&u)))
}

async fn update_profile(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    ValidatedJson(body): ValidatedJson<ProfileBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    if let Some(n) = &body.nickname {
        check_len("nickname", n, 0, 50)?;
    }
    if let Some(q) = &body.qq {
        check_len("qq", q, 0, 20)?;
    }
    let u = user_service::update_profile(
        &state.pool(),
        &session.user.user_id,
        user_service::UserProfileInput {
            nickname: body.nickname,
            qq: body.qq,
        },
    )
    .await?;
    Ok(ok(user_me_profile(&u)))
}

async fn change_password(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    ValidatedJson(body): ValidatedJson<ChangePasswordBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    check_len("currentPassword", &body.current_password, 6, 100)?;
    check_len("newPassword", &body.new_password, 6, 100)?;
    user_service::change_password(
        &state.pool(),
        &session.user.user_id,
        user_service::ChangePasswordInput {
            current_password: body.current_password,
            new_password: body.new_password,
        },
        &state.config().public_origin,
    )
    .await?;
    Ok(ok(json!({ "message": "Password changed successfully" })))
}

async fn get_notification_preferences(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let data =
        notification_service::preferences_dto(&state.pool(), &session.user.user_id).await?;
    Ok(ok(data))
}

async fn patch_notification_preferences(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    ValidatedJson(body): ValidatedJson<notification_service::PreferencesPatch>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let data = notification_service::patch_preferences(
        &state.pool(),
        &session.user.user_id,
        body,
    )
    .await?;
    Ok(ok(data))
}

async fn reset_notification_preferences(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let data =
        notification_service::reset_preferences(&state.pool(), &session.user.user_id).await?;
    Ok(ok(data))
}

async fn search(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Query(q): Query<SearchQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let q = q.q.unwrap_or_default().trim().to_string();
    if q.is_empty() {
        return Ok(ok(serde_json::Value::Array(vec![])));
    }
    let users = user_service::search(&state.pool(), &q, &session.user.user_id, 10).await?;
    let data: Vec<serde_json::Value> = users.iter().map(user_search).collect();
    Ok(ok(serde_json::Value::Array(data)))
}

async fn admin_list(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    Query(p): Query<AdminListQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let page = p.page.unwrap_or(1).max(1);
    let page_size = p.page_size.unwrap_or(20).max(1);
    let res = user_service::admin_list(&state.pool(), page, page_size).await?;
    let items: Vec<serde_json::Value> = res.items.iter().map(user_admin).collect();
    let total_pages = ((res.total as f64) / (page_size as f64)).ceil() as i64;
    let meta = json!({
        "total": res.total,
        "page": page,
        "pageSize": page_size,
        "totalPages": total_pages,
    });
    Ok(ok_with_meta(serde_json::Value::Array(items), meta))
}

async fn admin_create(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    ValidatedJson(body): ValidatedJson<AdminCreateBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    check_email(&body.email)?;
    check_len("password", &body.password, 6, 100)?;
    if let Some(n) = &body.nickname {
        check_len("nickname", n, 0, 50)?;
    }
    if let Some(r) = &body.role {
        check_enum("role", r, &["ADMIN", "USER"])?;
    }
    if let Some(s) = &body.status {
        check_enum("status", s, &["ACTIVE", "INACTIVE", "LOCKED"])?;
    }
    let u = user_service::admin_create(
        &state.pool(),
        user_service::AdminCreateInput {
            email: body.email,
            password: body.password,
            nickname: body.nickname,
            role: body.role,
            status: body.status,
        },
    )
    .await?;
    Ok(ok(user_admin_created(&u)))
}

async fn admin_update(
    State(state): State<AppState>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<String>,
    ValidatedJson(body): ValidatedJson<AdminUpdateBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    if let Some(r) = &body.role {
        check_enum("role", r, &["ADMIN", "USER"])?;
    }
    if let Some(s) = &body.status {
        check_enum("status", s, &["ACTIVE", "INACTIVE", "LOCKED"])?;
    }
    let u = user_service::admin_update(
        &state.pool(),
        &id,
        body.role,
        body.status,
    )
    .await?;
    Ok(ok(user_admin_updated(&u)))
}

async fn admin_delete(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    user_service::admin_delete(&state.pool(), &admin.user.user_id, &id).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

/// 组装用户域路由。
pub fn router(state: AppState) -> Router<AppState> {
    let protected = Router::new()
        .route("/me", get(me))
        .route("/me/profile", patch(update_profile))
        .route("/me/change-password", post(change_password))
        .route(
            "/me/notification-preferences",
            get(get_notification_preferences).patch(patch_notification_preferences),
        )
        .route(
            "/me/notification-preferences/reset",
            post(reset_notification_preferences),
        )
        .route("/search", get(search))
        .route("/admin/list", get(admin_list))
        .route("/admin/create", post(admin_create))
        .route("/admin/{id}", patch(admin_update))
        .route("/admin/{id}", delete(admin_delete))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ));
    protected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_and_length_validation_match_legacy() {
        assert!(check_email("a@b.com").is_ok());
        assert!(check_email("bad").is_err());
        assert!(check_len("nickname", "ab", 2, 50).is_ok());
        assert!(check_len("nickname", "a", 2, 50).is_err());
    }

    #[test]
    fn profile_dto_drops_email_verified() {
        let u = User {
            id: "u1".into(),
            email: "b@c.com".into(),
            password: "x".into(),
            qq: None,
            nickname: None,
            role: "USER".into(),
            status: "ACTIVE".into(),
            email_verified: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let full = user_me(&u);
        assert!(full.get("emailVerified").is_some());
        let profile = user_me_profile(&u);
        assert!(profile.get("emailVerified").is_none());
    }
}
