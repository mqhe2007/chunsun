//! 认证域路由（1:1 移植自 `packages/backend/src/routes/auth.ts`）。
//!
//! 路由分两组：
//! - 严格限流的公开组：register / verify-email / resend-verification / forgot-password /
//!   reset-password / login。
//! - 需认证组（不受严格限流）：secret-key-info。

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::repos::project;
use crate::services::auth as auth_service;
use crate::services::settings;
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

/// 轻量邮箱格式校验（对齐 format: "email" 的实际拦截效果）。
fn check_email(value: &str) -> Result<(), AppError> {
    let bytes_ok = value.len() <= 100 && !value.contains(char::is_whitespace);
    let shape_ok = match value.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        }
        None => false,
    };
    if bytes_ok && shape_ok {
        Ok(())
    } else {
        Err(AppError::unprocessable("VALIDATION_ERROR").with_message("email 格式不正确"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBody {
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub qq: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct EmailBody {
    pub email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordBody {
    pub token: String,
    pub new_password: String,
}

async fn registration_config(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let invite_only = settings::is_invite_only_registration(&state.pool()).await?;
    Ok(ok(json!({ "inviteOnly": invite_only })))
}

async fn register(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<RegisterBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    check_email(&body.email)?;
    check_len("password", &body.password, 6, 100)?;
    if let Some(v) = body.invite_code.as_deref() {
        check_len("inviteCode", v, 0, 64)?;
    }
    if let Some(v) = body.qq.as_deref() {
        check_len("qq", v, 0, 20)?;
    }
    if let Some(v) = body.nickname.as_deref() {
        check_len("nickname", v, 0, 50)?;
    }

    let result = auth_service::register_user(
        &state.pool(),
        &state.config(),
        auth_service::RegisterInput {
            email: body.email,
            password: body.password,
            invite_code: body.invite_code,
            qq: body.qq,
            nickname: body.nickname,
        },
    )
    .await?;

    Ok(ok(json!({ "userId": result.user_id, "email": result.email })))
}

async fn login(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<LoginBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    check_email(&body.email)?;
    check_len("password", &body.password, 6, 100)?;

    let token = auth_service::login_user(&state.pool(), &state.config(), &body.email, &body.password)
        .await?;

    Ok(ok(json!({ "token": token.token, "expiresIn": token.expires_in })))
}

async fn verify_email(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<TokenBody>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    if body.token.is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("token 不能为空"));
    }
    auth_service::verify_email(&state.pool(), &body.token).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

async fn resend_verification(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<EmailBody>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    check_email(&body.email)?;
    auth_service::resend_verification_email(&state.pool(), &state.config(), &body.email).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

async fn forgot_password(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<EmailBody>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    check_email(&body.email)?;
    auth_service::request_password_reset(&state.pool(), &state.config(), &body.email).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

async fn reset_password(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<ResetPasswordBody>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    if body.token.is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("token 不能为空"));
    }
    check_len("newPassword", &body.new_password, 6, 100)?;
    auth_service::reset_password(&state.pool(), &body.token, &body.new_password).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

/// GET /auth/secret-key-info —— 仅 Secret Key 通道可用。
async fn secret_key_info(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let Some(project_id) = session.project_id else {
        return Err(AppError::bad_request("NOT_SECRET_KEY_AUTH")
            .with_message("此接口仅支持 Secret Key 认证"));
    };

    let Some(p) = project::get_project_by_id_only(&state.pool(), &project_id).await? else {
        return Err(AppError::not_found("PROJECT_NOT_FOUND"));
    };

    Ok(ok(json!({
        "projectId": p.id,
        "projectName": p.name,
        "userId": session.user.user_id,
    })))
}

/// 组装认证域路由。
pub fn router(state: AppState) -> Router<AppState> {
    // 公开组：严格限流
    let public = Router::new()
        .route("/registration-config", get(registration_config))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/verify-email", post(verify_email))
        .route("/resend-verification", post(resend_verification))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::rate_limit::auth_rate_limit,
        ));

    // 需认证组：不受严格限流
    let protected = Router::new()
        .route("/secret-key-info", get(secret_key_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ));

    public.merge(protected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_validation_matches_expected_shapes() {
        assert!(check_email("a@b.com").is_ok());
        assert!(check_email("user.name+tag@sub.domain.cn").is_ok());
        assert!(check_email("no-at-sign").is_err());
        assert!(check_email("@nolocal.com").is_err());
        assert!(check_email("a@nodot").is_err());
        assert!(check_email("a b@c.com").is_err());
    }

    #[test]
    fn length_validation_uses_char_count() {
        assert!(check_len("nickname", "ab", 2, 50).is_ok());
        assert!(check_len("nickname", "a", 2, 50).is_err());
        // 中文按字符计数，不按字节
        assert!(check_len("nickname", "中文昵称", 2, 50).is_ok());
    }
}
