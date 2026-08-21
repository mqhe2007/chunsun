//! 安装向导：试连数据库、迁移、创建首个管理员、写入实例配置。

use crate::api::AppError;
use crate::config::AppConfig;
use crate::core::password::hash_password;
use crate::core::security_policy::{validate_password, PasswordPolicy};
use crate::core::tokens::generate_secure_token;
use crate::db;
use crate::instance::{self, InstanceFile};
use crate::repos::user::{self, CreateUserInput};
use crate::state::AppState;

pub struct DatabaseParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
    pub ssl: bool,
}

pub struct AdminParams {
    pub email: String,
    pub password: String,
    pub nickname: Option<String>,
}

pub struct CompleteSetupInput {
    pub database: DatabaseParams,
    pub public_origin: String,
    pub admin: AdminParams,
}

pub fn database_url(db: &DatabaseParams) -> String {
    instance::postgres_url(
        &db.host,
        db.port,
        &db.user,
        &db.password,
        &db.name,
        db.ssl,
    )
}

pub async fn test_database(url: &str) -> Result<(), AppError> {
    let pool = db::create_pool(url).await.map_err(db_connect_error)?;
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(db_connect_error)?;
    pool.close().await;
    Ok(())
}

pub async fn complete(state: &AppState, input: CompleteSetupInput) -> Result<(), AppError> {
    if state.is_ready() {
        return Err(AppError::conflict("SETUP_ALREADY_COMPLETE").with_message("实例已完成安装"));
    }
    let _guard = state.setup_lock().await;
    if state.is_ready() {
        return Err(AppError::conflict("SETUP_ALREADY_COMPLETE").with_message("实例已完成安装"));
    }

    validate_admin(&input.admin)?;
    let public_origin = normalize_origin(&input.public_origin)?;
    let database_url = database_url(&input.database);

    let pool = db::create_pool(&database_url)
        .await
        .map_err(db_connect_error)?;

    if std::env::var("CHUNSUN_SKIP_MIGRATE").as_deref() != Ok("1") {
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "setup migrate failed");
                AppError::bad_request("SETUP_MIGRATE_FAILED")
                    .with_message(format!("数据库迁移失败: {e}"))
            })?;
    }

    create_admin(&pool, &input.admin).await?;

    let jwt_secret = generate_secure_token(32);
    let config = AppConfig {
        database_url,
        jwt_secret,
        jwt_expires_in: "2h".into(),
        port: state.listen_port(),
        api_prefix: "/api/v1".into(),
        node_env: if public_origin.starts_with("https://") {
            Some("production".into())
        } else {
            None
        },
        env_var_encryption_key: None,
        public_origin,
    };
    instance::save_file(state.config_path(), &InstanceFile::from_config(&config)).map_err(
        |e| AppError::internal(format!("写入实例配置失败: {e}")),
    )?;

    state.mark_ready(pool, config);
    tracing::info!(path = %state.config_path().display(), "安装完成，实例配置已写入");
    Ok(())
}

fn validate_admin(admin: &AdminParams) -> Result<(), AppError> {
    let check = validate_password(&admin.password, &PasswordPolicy::default());
    if !check.valid {
        return Err(AppError::bad_request("WEAK_PASSWORD").with_message(check.errors.join(";")));
    }
    Ok(())
}

fn normalize_origin(raw: &str) -> Result<String, AppError> {
    let origin = raw.trim().trim_end_matches('/').to_string();
    if origin.is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("站点地址不能为空"));
    }
    if !(origin.starts_with("http://") || origin.starts_with("https://")) {
        return Err(AppError::unprocessable("VALIDATION_ERROR")
            .with_message("站点地址需以 http:// 或 https:// 开头"));
    }
    Ok(origin)
}

async fn create_admin(pool: &sqlx::PgPool, admin: &AdminParams) -> Result<(), AppError> {
    if user::get_user_by_email(pool, &admin.email).await?.is_some() {
        return Err(AppError::conflict("EMAIL_EXISTS").with_message(
            "该邮箱已存在。若这是已有数据库，请换一个管理员邮箱，或清空库后重试。",
        ));
    }
    let password_hash = hash_password(&admin.password)?;
    let created = user::create_user(
        pool,
        CreateUserInput {
            email: admin.email.clone(),
            password: password_hash,
            qq: None,
            nickname: admin.nickname.clone(),
            role: Some("ADMIN".into()),
            status: Some("ACTIVE".into()),
        },
    )
    .await?;
    user::update_user_email_verified(pool, &created.id, true).await?;
    Ok(())
}

fn db_connect_error(e: sqlx::Error) -> AppError {
    tracing::warn!(error = %e, "database connect failed");
    AppError::bad_request("DATABASE_UNREACHABLE").with_message(format!("无法连接数据库: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_origin_without_scheme() {
        assert!(normalize_origin("example.com").is_err());
        assert_eq!(
            normalize_origin("https://a.example/").unwrap(),
            "https://a.example"
        );
    }

    #[test]
    fn admin_password_requires_number_and_length() {
        let weak = AdminParams {
            email: "a@b.com".into(),
            password: "short".into(),
            nickname: None,
        };
        assert!(validate_admin(&weak).is_err());
        let ok = AdminParams {
            email: "a@b.com".into(),
            password: "password1".into(),
            nickname: None,
        };
        assert!(validate_admin(&ok).is_ok());
    }
}
