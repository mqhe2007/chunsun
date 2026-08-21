//! 认证业务服务（1:1 移植自 `packages/backend/src/services/authService.ts`）。
//!
//! 与旧实现的行为约定必须逐条保持：
//! - 邀请码：开启 invite-only 时缺码报 INVITE_CODE_REQUIRED；给了码但无效报 INVALID_INVITE_CODE。
//! - 注册顺序：邀请码校验 → 邮箱查重 → 密码强度 → Argon2id → 建号。
//! - 登录顺序：锁定检查 → 用户存在 → 状态 → 邮箱验证 → 密码比对。
//! - resend / forgot 对不存在的账号**静默成功**（防账号枚举）。

use chrono::{Duration, Utc};
use sqlx::PgPool;

use crate::api::AppError;
use crate::auth::{sign_jwt, AuthUser};
use crate::core::password::{hash_password, rehash_if_legacy, verify_password};
use crate::core::tokens::generate_secure_token;
use crate::repos::{email_token, invitation, user};
use crate::services::notification::{notify_user, NotificationData};
use crate::services::security;
use crate::services::settings;
use crate::services::email;

const VERIFICATION_TOKEN_TTL_HOURS: i64 = 24;
const RESET_TOKEN_TTL_HOURS: i64 = 1;
const SECURE_TOKEN_BYTES: usize = 32;

/// 认证域失败语义，映射表对齐 `routes/auth.ts` 的 `mapAuthError`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFailure {
    InviteCodeRequired,
    InvalidInviteCode,
    EmailExists,
    InvalidCredentials,
    EmailNotVerified,
    AccountInactive,
    AccountLocked,
    InvalidOrExpiredToken,
    /// 携带以 `;` 拼接的具体校验错误，作为响应 message。
    WeakPassword(String),
}

impl AuthFailure {
    pub fn status_and_code(&self) -> (axum::http::StatusCode, &'static str) {
        use axum::http::StatusCode as S;
        match self {
            Self::InviteCodeRequired => (S::BAD_REQUEST, "INVITE_CODE_REQUIRED"),
            Self::InvalidInviteCode => (S::BAD_REQUEST, "INVALID_INVITE_CODE"),
            Self::EmailExists => (S::CONFLICT, "EMAIL_EXISTS"),
            Self::InvalidCredentials => (S::UNAUTHORIZED, "INVALID_CREDENTIALS"),
            Self::EmailNotVerified => (S::FORBIDDEN, "EMAIL_NOT_VERIFIED"),
            Self::AccountInactive => (S::FORBIDDEN, "ACCOUNT_INACTIVE"),
            Self::AccountLocked => (S::LOCKED, "ACCOUNT_LOCKED"),
            Self::InvalidOrExpiredToken => (S::BAD_REQUEST, "INVALID_OR_EXPIRED_TOKEN"),
            Self::WeakPassword(_) => (S::BAD_REQUEST, "WEAK_PASSWORD"),
        }
    }
}

impl From<AuthFailure> for AppError {
    fn from(f: AuthFailure) -> Self {
        let (status, code) = f.status_and_code();
        let err = AppError::new(status, code);
        match f {
            AuthFailure::WeakPassword(detail) => err.with_message(detail),
            _ => err,
        }
    }
}

pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub qq: Option<String>,
    pub nickname: Option<String>,
}

pub struct RegisterResult {
    pub user_id: String,
    pub email: String,
}

pub struct AuthToken {
    pub token: String,
    pub expires_in: String,
}

/// 注册。
pub async fn register_user(
    pool: &PgPool,
    config: &crate::config::AppConfig,
    input: RegisterInput,
) -> Result<RegisterResult, AppError> {
    let invite_only = settings::is_invite_only_registration(pool).await?;
    if invite_only && input.invite_code.as_deref().unwrap_or("").is_empty() {
        return Err(AuthFailure::InviteCodeRequired.into());
    }

    let mut used_invitation = None;
    if let Some(code) = input.invite_code.as_deref().filter(|c| !c.is_empty()) {
        match invitation::get_valid_invitation_code_by_code(pool, code).await? {
            Some(inv) => used_invitation = Some(inv),
            None => return Err(AuthFailure::InvalidInviteCode.into()),
        }
    }

    if user::get_user_by_email(pool, &input.email).await?.is_some() {
        return Err(AuthFailure::EmailExists.into());
    }

    let check = security::validate_password_with_policy(pool, &input.password).await?;
    if !check.valid {
        return Err(AuthFailure::WeakPassword(check.errors.join(";")).into());
    }

    let password_hash = hash_password(&input.password)?;

    let created = user::create_user(
        pool,
        user::CreateUserInput {
            email: input.email,
            password: password_hash,
            qq: input.qq,
            nickname: input.nickname,
            role: used_invitation.as_ref().map(|i| i.role.clone()),
            status: Some("ACTIVE".to_string()),
        },
    )
    .await?;

    let token = generate_secure_token(SECURE_TOKEN_BYTES);
    email_token::create_email_verification_token(
        pool,
        &created.id,
        &token,
        Utc::now() + Duration::hours(VERIFICATION_TOKEN_TTL_HOURS),
    )
    .await?;

    email::send_verification_email(pool, &created.email, &token, &config.public_origin).await;

    if let Some(inv) = used_invitation {
        invitation::increment_invitation_used_count(pool, &inv.id).await?;
    }

    Ok(RegisterResult { user_id: created.id, email: created.email })
}

/// 登录。
pub async fn login_user(
    pool: &PgPool,
    config: &crate::config::AppConfig,
    email_input: &str,
    password: &str,
) -> Result<AuthToken, AppError> {
    let identifier = email_input.to_lowercase();

    let lockout = security::check_login_lockout(pool, &identifier).await?;
    if lockout.locked {
        return Err(AuthFailure::AccountLocked.into());
    }

    let Some(found) = user::get_user_by_email(pool, email_input).await? else {
        security::record_failed_login(pool, &identifier, None).await?;
        return Err(AuthFailure::InvalidCredentials.into());
    };

    match found.status.as_str() {
        "INACTIVE" => return Err(AuthFailure::AccountInactive.into()),
        "LOCKED" => return Err(AuthFailure::AccountLocked.into()),
        _ => {}
    }
    if !found.email_verified {
        return Err(AuthFailure::EmailNotVerified.into());
    }

    if !verify_password(password, &found.password)? {
        let result = security::record_failed_login(pool, &identifier, Some(&found.id)).await?;
        if result.locked {
            return Err(AuthFailure::AccountLocked.into());
        }
        return Err(AuthFailure::InvalidCredentials.into());
    }

    if let Some(new_hash) = rehash_if_legacy(password, &found.password)? {
        user::update_user_password(pool, &found.id, &new_hash).await?;
    }

    security::record_successful_login(pool, &identifier).await?;

    let token = sign_jwt(
        &AuthUser {
            user_id: found.id,
            email: found.email,
            role: found.role,
        },
        &config.jwt_secret,
        &config.jwt_expires_in,
    )?;

    Ok(AuthToken { token, expires_in: config.jwt_expires_in.clone() })
}

/// 邮箱验证。
pub async fn verify_email(pool: &PgPool, token: &str) -> Result<(), AppError> {
    let record = email_token::get_email_verification_token_by_token(pool, token).await?;
    let Some(record) = record else {
        return Err(AuthFailure::InvalidOrExpiredToken.into());
    };
    if record.used_at.is_some() || record.expires_at < Utc::now() {
        return Err(AuthFailure::InvalidOrExpiredToken.into());
    }

    user::update_user_email_verified(pool, &record.user_id, true).await?;
    email_token::mark_email_verification_token_used(pool, &record.id).await?;

    notify_user(
        pool,
        NotificationData {
            user_id: record.user_id.clone(),
            ty: "email_verified".into(),
            title: "邮箱验证成功".into(),
            body: Some("你的邮箱已通过验证，现在可以正常使用春笋。".into()),
            link: None,
        },
    )
    .await?;

    Ok(())
}

/// 重发验证邮件（账号不存在 / 已验证均静默成功）。
pub async fn resend_verification_email(
    pool: &PgPool,
    config: &crate::config::AppConfig,
    email_input: &str,
) -> Result<(), AppError> {
    let Some(found) = user::get_user_by_email(pool, email_input).await? else {
        return Ok(());
    };
    if found.email_verified {
        return Ok(());
    }

    let token = generate_secure_token(SECURE_TOKEN_BYTES);
    email_token::create_email_verification_token(
        pool,
        &found.id,
        &token,
        Utc::now() + Duration::hours(VERIFICATION_TOKEN_TTL_HOURS),
    )
    .await?;

    email::send_verification_email(pool, &found.email, &token, &config.public_origin).await;

    Ok(())
}

/// 申请重置密码（账号不存在时静默成功）。
pub async fn request_password_reset(
    pool: &PgPool,
    config: &crate::config::AppConfig,
    email_input: &str,
) -> Result<(), AppError> {
    let Some(found) = user::get_user_by_email(pool, email_input).await? else {
        return Ok(());
    };

    let token = generate_secure_token(SECURE_TOKEN_BYTES);
    email_token::create_password_reset_token(
        pool,
        &found.id,
        &token,
        Utc::now() + Duration::hours(RESET_TOKEN_TTL_HOURS),
    )
    .await?;

    email::send_password_reset_email(pool, &found.email, &token, &config.public_origin).await;

    Ok(())
}

/// 执行重置密码。
pub async fn reset_password(
    pool: &PgPool,
    token: &str,
    new_password: &str,
) -> Result<(), AppError> {
    let record = email_token::get_password_reset_token_by_token(pool, token).await?;
    let Some(record) = record else {
        return Err(AuthFailure::InvalidOrExpiredToken.into());
    };
    if record.used_at.is_some() || record.expires_at < Utc::now() {
        return Err(AuthFailure::InvalidOrExpiredToken.into());
    }

    let check = security::validate_password_with_policy(pool, new_password).await?;
    if !check.valid {
        return Err(AuthFailure::WeakPassword(check.errors.join(";")).into());
    }

    let password_hash = hash_password(new_password)?;
    user::update_user_password(pool, &record.user_id, &password_hash).await?;
    email_token::mark_password_reset_token_used(pool, &record.id).await?;

    notify_user(
        pool,
        NotificationData {
            user_id: record.user_id.clone(),
            ty: "password_changed".into(),
            title: "密码已重置".into(),
            body: Some("你的账户密码刚刚被重置。如非本人操作，请立即联系管理员。".into()),
            link: None,
        },
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    /// 错误映射表必须与旧后端 mapAuthError 完全一致。
    #[test]
    fn auth_failure_maps_to_same_status_and_code_as_legacy() {
        let cases: Vec<(AuthFailure, StatusCode, &str)> = vec![
            (AuthFailure::InviteCodeRequired, StatusCode::BAD_REQUEST, "INVITE_CODE_REQUIRED"),
            (AuthFailure::InvalidInviteCode, StatusCode::BAD_REQUEST, "INVALID_INVITE_CODE"),
            (AuthFailure::EmailExists, StatusCode::CONFLICT, "EMAIL_EXISTS"),
            (AuthFailure::InvalidCredentials, StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS"),
            (AuthFailure::EmailNotVerified, StatusCode::FORBIDDEN, "EMAIL_NOT_VERIFIED"),
            (AuthFailure::AccountInactive, StatusCode::FORBIDDEN, "ACCOUNT_INACTIVE"),
            (AuthFailure::AccountLocked, StatusCode::LOCKED, "ACCOUNT_LOCKED"),
            (
                AuthFailure::InvalidOrExpiredToken,
                StatusCode::BAD_REQUEST,
                "INVALID_OR_EXPIRED_TOKEN",
            ),
        ];
        for (failure, status, code) in cases {
            let err: AppError = failure.clone().into();
            assert_eq!(err.status, status, "status mismatch for {failure:?}");
            assert_eq!(err.code, code, "code mismatch for {failure:?}");
            assert!(err.message.is_none(), "no message expected for {failure:?}");
        }
    }

    /// WEAK_PASSWORD 需要 400 + 把校验详情放进 message（旧后端 `message.replace("WEAK_PASSWORD:", "")`）。
    #[test]
    fn weak_password_carries_detail_message() {
        let err: AppError = AuthFailure::WeakPassword("密码长度至少 8 位;密码需包含数字".into()).into();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.code, "WEAK_PASSWORD");
        assert_eq!(err.message.as_deref(), Some("密码长度至少 8 位;密码需包含数字"));
    }
}
