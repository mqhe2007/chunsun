//! 项目环境变量路由（1:1 移植自 `packages/backend/src/routes/projectEnvVar.ts`）。
//!
//! 六条端点挂在 `/projects/{projectId}/env-vars` 下，全部走 `auth_middleware`。
//!
//! 两处容易踩的地方：
//! 1. **`/count` 与 `/{varId}` 同层**——静态段必须优先匹配，否则 `GET /count` 会被当成
//!    读某个 id。这里 `/count` 只注册 GET、`/{varId}` 只注册 PATCH/DELETE，天然无歧义。
//! 2. **create 返回 201**（本仓第一个非 200 的成功码），delete 返回 `{success:true}`
//!    不带 `data`——两者都是旧后端的既有形状，不要顺手统一成 200 + data。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::serde_ext::double_option;
use crate::routes::dto::{env_var_list_item_dto, env_var_value_dto};
use crate::routes::validate::{
    nullable_optional_string, optional_bool, optional_string, required_string,
};
use crate::services::project_env_var::{
    self as env_var_service, Caller, CreateEnvVarRequest, EnvCrypto, UpdateEnvVarRequest,
};
use crate::config::AppConfig;
use crate::state::AppState;

/// `value` 是 `t.String()`：必填、无 minLength/maxLength，所以空串合法、长度不设上限。
const VALUE_MIN: usize = 0;
const VALUE_MAX: usize = usize::MAX;
const KEY_MIN: usize = 1;
const KEY_MAX: usize = 128;
/// `description` 是 `t.Union([t.String({maxLength:500}), t.Null()])`——没有 minLength，空串合法。
const DESC_MIN: usize = 0;
const DESC_MAX: usize = 500;

fn caller<'a>(session: &'a crate::auth::AuthSession) -> Caller<'a> {
    Caller {
        user_id: &session.user.user_id,
        is_admin: session.user.role == "ADMIN",
        sk_project_id: session.project_id.as_deref(),
    }
}

fn crypto(config: &AppConfig) -> EnvCrypto<'_> {
    EnvCrypto {
        encryption_key: config.env_var_encryption_key.as_deref(),
        jwt_secret: &config.jwt_secret,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvVarBody {
    #[serde(default, deserialize_with = "double_option")]
    pub key: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub value: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub is_secret: Option<Option<bool>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvVarBody {
    #[serde(default, deserialize_with = "double_option")]
    pub key: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub value: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub is_secret: Option<Option<bool>>,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let rows = env_var_service::list_env_vars(&state.pool(), caller(&session), &project_id).await?;
    Ok(ok(Value::Array(
        rows.iter().map(env_var_list_item_dto).collect(),
    )))
}

async fn count(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let count = env_var_service::count_env_vars(&state.pool(), caller(&session), &project_id).await?;
    Ok(ok(serde_json::json!({ "count": count })))
}

async fn by_key(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, key)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let config = state.config();
    let (row, plain) = env_var_service::get_env_var_value(
        &state.pool(),
        caller(&session),
        crypto(&config),
        &project_id,
        &key,
    )
    .await?;
    Ok(ok(env_var_value_dto(&row, &plain)))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateEnvVarBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    let key = required_string("key", &body.key, KEY_MIN, KEY_MAX)?;
    let value = required_string("value", &body.value, VALUE_MIN, VALUE_MAX)?;
    // POST 语义下「不传」与「传 null」同义（旧实现 `body.description ?? null`），摊平即可
    let description =
        nullable_optional_string("description", &body.description, DESC_MIN, DESC_MAX)?.flatten();
    let is_secret = optional_bool("isSecret", &body.is_secret)?;

    let config = state.config();
    let row = env_var_service::create_env_var(
        &state.pool(),
        caller(&session),
        crypto(&config),
        &project_id,
        CreateEnvVarRequest {
            key,
            value,
            description,
            is_secret,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(env_var_list_item_dto(&row))),
    ))
}

async fn update(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, var_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<UpdateEnvVarBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let key = optional_string("key", &body.key, KEY_MIN, KEY_MAX)?;
    let value = optional_string("value", &body.value, VALUE_MIN, VALUE_MAX)?;
    // PATCH 下 `description: null` 是「清空」，与「不传」语义不同，必须保留三态
    let description = nullable_optional_string("description", &body.description, DESC_MIN, DESC_MAX)?;
    let is_secret = optional_bool("isSecret", &body.is_secret)?;

    let config = state.config();
    let row = env_var_service::update_env_var(
        &state.pool(),
        caller(&session),
        crypto(&config),
        &project_id,
        &var_id,
        UpdateEnvVarRequest {
            key,
            value,
            description,
            is_secret,
        },
    )
    .await?;

    Ok(ok(env_var_list_item_dto(&row)))
}

async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, var_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    env_var_service::delete_env_var(&state.pool(), caller(&session), &project_id, &var_id).await?;
    Ok(Json(ApiResponse::ok_no_data()))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/projects/{projectId}/env-vars", get(list).post(create))
        .route("/projects/{projectId}/env-vars/count", get(count))
        .route("/projects/{projectId}/env-vars/by-key/{key}", get(by_key))
        .route(
            "/projects/{projectId}/env-vars/{varId}",
            delete(remove).patch(update),
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
    fn create_body_parses_three_states() {
        let b: CreateEnvVarBody = serde_json::from_str(r#"{"key":"A","value":"v"}"#).unwrap();
        assert_eq!(b.description, None);
        assert_eq!(b.is_secret, None);

        let b: CreateEnvVarBody =
            serde_json::from_str(r#"{"key":"A","value":"v","description":null,"isSecret":null}"#)
                .unwrap();
        assert_eq!(b.description, Some(None));
        assert_eq!(b.is_secret, Some(None), "显式 null 必须与缺省区分，否则 422 会漏判");

        let b: CreateEnvVarBody =
            serde_json::from_str(r#"{"key":"A","value":"","isSecret":false}"#).unwrap();
        assert_eq!(b.value, Some(Some(String::new())));
        assert_eq!(b.is_secret, Some(Some(false)));
    }

    #[test]
    fn empty_value_is_accepted_but_missing_is_not() {
        let empty = Some(Some(String::new()));
        assert_eq!(required_string("value", &empty, VALUE_MIN, VALUE_MAX).unwrap(), "");
        assert!(required_string("value", &None, VALUE_MIN, VALUE_MAX).is_err());
    }

    /// POST 摊平后缺省与 null 同为 None；PATCH 不摊平，两者语义不同。
    #[test]
    fn description_flattening_differs_between_post_and_patch() {
        let null = Some(None);
        let patch = nullable_optional_string("description", &null, DESC_MIN, DESC_MAX).unwrap();
        assert_eq!(patch, Some(None), "PATCH 下显式 null = 清空");
        assert_eq!(patch.flatten(), None, "POST 摊平后与缺省同义");

        // description 只有 maxLength，没有 minLength —— 空串合法
        let empty = Some(Some(String::new()));
        assert_eq!(
            nullable_optional_string("description", &empty, DESC_MIN, DESC_MAX).unwrap(),
            Some(Some(""))
        );
        let too_long = Some(Some("x".repeat(501)));
        assert!(nullable_optional_string("description", &too_long, DESC_MIN, DESC_MAX).is_err());
    }
}
