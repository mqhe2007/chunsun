//! 安装向导公开接口（仅在实例未就绪时接受写入）。

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::services::setup::{self, AdminParams, CompleteSetupInput, DatabaseParams};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBody {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
    #[serde(default)]
    pub ssl: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminBody {
    pub email: String,
    pub password: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteBody {
    pub database: DatabaseBody,
    pub public_origin: String,
    pub admin: AdminBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDbBody {
    pub database: DatabaseBody,
}

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
        Err(AppError::unprocessable("VALIDATION_ERROR").with_message("管理员邮箱格式不正确"))
    }
}

fn db_params(body: DatabaseBody) -> Result<DatabaseParams, AppError> {
    if body.host.trim().is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("数据库主机不能为空"));
    }
    if body.user.trim().is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("数据库用户不能为空"));
    }
    if body.name.trim().is_empty() {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("数据库名不能为空"));
    }
    if body.port == 0 {
        return Err(AppError::unprocessable("VALIDATION_ERROR").with_message("数据库端口无效"));
    }
    Ok(DatabaseParams {
        host: body.host.trim().to_string(),
        port: body.port,
        user: body.user.trim().to_string(),
        password: body.password,
        name: body.name.trim().to_string(),
        ssl: body.ssl,
    })
}

async fn status(State(state): State<AppState>) -> Json<ApiResponse<Value>> {
    ok(json!({
        "needed": !state.is_ready(),
        "listenPort": state.listen_port(),
    }))
}

async fn test_database(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<TestDbBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    if state.is_ready() {
        return Err(AppError::conflict("SETUP_ALREADY_COMPLETE").with_message("实例已完成安装"));
    }
    let db = db_params(body.database)?;
    setup::test_database(&setup::database_url(&db)).await?;
    Ok(ok(json!({ "ok": true })))
}

async fn complete(
    State(state): State<AppState>,
    ValidatedJson(body): ValidatedJson<CompleteBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    check_email(&body.admin.email)?;
    let input = CompleteSetupInput {
        database: db_params(body.database)?,
        public_origin: body.public_origin,
        admin: AdminParams {
            email: body.admin.email.trim().to_string(),
            password: body.admin.password,
            nickname: body.admin.nickname.filter(|s| !s.trim().is_empty()),
        },
    };
    setup::complete(&state, input).await?;
    Ok(ok(json!({ "needed": false })))
}

pub fn router(state: AppState) -> Router<AppState> {
    let writes = Router::new()
        .route("/test-database", post(test_database))
        .route("/complete", post(complete))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::rate_limit::auth_rate_limit,
        ));
    Router::new()
        .route("/status", get(status))
        .merge(writes)
}
