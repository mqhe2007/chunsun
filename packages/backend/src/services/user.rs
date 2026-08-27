//! 用户域业务服务（1:1 移植自 `packages/backend/src/routes/user.ts` 的 handler 逻辑）。
//!
//! 与旧实现的行为约定逐条保持：错误码映射、部分更新语义。
//! 与旧后端的一处有意差异（不影响对拍 DIFF，已锁定）：
//! admin 创建时**仅**唯一约束冲突（email）映射为 409，其余 DB 错误如实上抛
//!    （旧后端用 catch-all 把任意异常塌成 409）。

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::password::{hash_password, verify_password};
use crate::repos::user;
use crate::services::notification::{notify, NotifyRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserFailure {
    UserNotFound,
    InvalidCurrentPassword,
    UserAlreadyExists,
    CannotDeleteSelf,
}

impl UserFailure {
    pub fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::UserNotFound => (StatusCode::NOT_FOUND, "USER_NOT_FOUND"),
            Self::InvalidCurrentPassword => (StatusCode::BAD_REQUEST, "INVALID_CURRENT_PASSWORD"),
            Self::UserAlreadyExists => (StatusCode::CONFLICT, "USER_ALREADY_EXISTS"),
            Self::CannotDeleteSelf => (StatusCode::BAD_REQUEST, "CANNOT_DELETE_SELF"),
        }
    }
}

impl From<UserFailure> for AppError {
    fn from(f: UserFailure) -> Self {
        let (status, code) = f.status_and_code();
        AppError::new(status, code)
    }
}

pub struct UserProfileInput {
    pub nickname: Option<String>,
    pub qq: Option<String>,
}

pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
}

pub struct AdminCreateInput {
    pub email: String,
    pub password: String,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

pub async fn get_me(pool: &PgPool, user_id: &str) -> Result<user::User, AppError> {
    user::get_user_by_id(pool, user_id)
        .await?
        .ok_or_else(|| UserFailure::UserNotFound.into())
}

pub async fn update_profile(
    pool: &PgPool,
    user_id: &str,
    input: UserProfileInput,
) -> Result<user::User, AppError> {
    user::update_user_profile(pool, user_id, input.nickname.as_deref(), input.qq.as_deref()).await
}

pub async fn change_password(
    pool: &PgPool,
    user_id: &str,
    input: ChangePasswordInput,
    public_origin: &str,
) -> Result<(), AppError> {
    let found = user::get_user_by_id(pool, user_id)
        .await?
        .ok_or_else(|| AppError::from(UserFailure::UserNotFound))?;
    if !verify_password(&input.current_password, &found.password)? {
        return Err(UserFailure::InvalidCurrentPassword.into());
    }
    let password_hash = hash_password(&input.new_password)?;
    user::update_user_password(pool, user_id, &password_hash).await?;

    notify(
        pool,
        public_origin,
        NotifyRequest {
            event: "password_changed".into(),
            recipient_user_ids: vec![user_id.to_string()],
            actor_user_id: None,
            title: "密码已修改".into(),
            body: Some("你的账户密码刚刚被修改。如非本人操作，请立即联系管理员。".into()),
            link: Some("/settings/profile".into()),
            email_link: None,
        },
    )
    .await?;

    Ok(())
}

pub async fn search(
    pool: &PgPool,
    q: &str,
    exclude_id: &str,
    limit: i64,
) -> Result<Vec<user::User>, AppError> {
    user::search_users(pool, q, exclude_id, limit).await
}

pub async fn admin_list(
    pool: &PgPool,
    page: i64,
    page_size: i64,
) -> Result<user::UserListResult, AppError> {
    user::list_all_users(pool, page, page_size).await
}

pub async fn admin_create(
    pool: &PgPool,
    input: AdminCreateInput,
) -> Result<user::User, AppError> {
    let password_hash = hash_password(&input.password)?;
    match user::create_user_raw(
        pool,
        user::CreateUserInput {
            email: input.email,
            password: password_hash,
            // 旧后端 admin 创建不接收 qq（createUser 调用未传该字段），保持一致。
            qq: None,
            nickname: input.nickname,
            role: input.role,
            status: input.status,
        },
    )
    .await
    {
        Ok(created) => {
            // 管理员手工建号视为已验证，否则无法登录（EMAIL_NOT_VERIFIED）。
            user::update_user_email_verified(pool, &created.id, true).await?;
            user::get_user_by_id(pool, &created.id)
                .await?
                .ok_or_else(|| AppError::from(UserFailure::UserNotFound))
        }
        Err(e) => {
            let is_dup = matches!(e, sqlx::Error::Database(ref d) if d.is_unique_violation());
            if is_dup {
                Err(UserFailure::UserAlreadyExists.into())
            } else {
                Err(AppError::from(e))
            }
        }
    }
}

pub async fn admin_update(
    pool: &PgPool,
    id: &str,
    role: Option<String>,
    status: Option<String>,
) -> Result<user::User, AppError> {
    let mut u = user::get_user_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::from(UserFailure::UserNotFound))?;
    if let Some(r) = role {
        u = user::update_user_role(pool, id, &r).await?;
    }
    if let Some(s) = status {
        u = user::update_user_status(pool, id, &s).await?;
    }
    Ok(u)
}

pub async fn admin_delete(
    pool: &PgPool,
    current_user_id: &str,
    target_id: &str,
) -> Result<(), AppError> {
    if current_user_id == target_id {
        return Err(UserFailure::CannotDeleteSelf.into());
    }
    let _u = user::get_user_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::from(UserFailure::UserNotFound))?;
    user::delete_user_by_id(pool, target_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 错误映射表必须与旧后端 user.ts 的 `set.status` 完全一致。
    #[test]
    fn user_failure_maps_to_same_status_and_code_as_legacy() {
        let cases: Vec<(UserFailure, StatusCode, &str)> = vec![
            (UserFailure::UserNotFound, StatusCode::NOT_FOUND, "USER_NOT_FOUND"),
            (
                UserFailure::InvalidCurrentPassword,
                StatusCode::BAD_REQUEST,
                "INVALID_CURRENT_PASSWORD",
            ),
            (
                UserFailure::UserAlreadyExists,
                StatusCode::CONFLICT,
                "USER_ALREADY_EXISTS",
            ),
            (
                UserFailure::CannotDeleteSelf,
                StatusCode::BAD_REQUEST,
                "CANNOT_DELETE_SELF",
            ),
        ];
        for (failure, status, code) in cases {
            let err: AppError = failure.clone().into();
            assert_eq!(err.status, status, "status mismatch for {failure:?}");
            assert_eq!(err.code, code, "code mismatch for {failure:?}");
        }
    }
}
