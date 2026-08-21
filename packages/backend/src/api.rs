//! 统一响应包络与错误类型。
//!
//! 对齐旧后端契约：`{ success: boolean, data?: T, error?: string, message?: string, hint?: string }`，
//! 该包络被 `packages/cli/src/api.rs` 的 `parse_error_detail` 反向消费。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// 分页等元信息（仅 `GET /users/admin/list` 等端点使用）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: None,
            hint: None,
            meta: None,
        }
    }

    pub fn ok_no_data() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
            message: None,
            hint: None,
            meta: None,
        }
    }

    /// 带分页元信息的成功响应（对齐旧后端 `GET /users/admin/list` 的顶层 `meta`）。
    pub fn ok_with_meta(data: T, meta: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            message: None,
            hint: None,
            meta: Some(meta),
        }
    }
}

/// 业务/系统错误 → HTTP 响应。`code` 即旧后端的 `error` 字段（如 UNAUTHORIZED）。
#[derive(Debug, Clone, thiserror::Error)]
#[error("{code}")]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: Option<String>,
    pub hint: Option<String>,
    /// 业务错误的附带数据（如 409 的 runId/blockers），对齐旧端 error 包络的 `data` 字段。
    pub data: Option<Value>,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            message: None,
            hint: None,
            data: None,
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED")
    }

    pub fn not_found(code: &'static str) -> Self {
        Self::new(StatusCode::NOT_FOUND, code)
    }

    pub fn conflict(code: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, code)
    }

    pub fn bad_request(code: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code)
    }

    /// 字段级校验失败。旧后端由 Elysia/TypeBox 统一返回 422，
    /// 这里保持**状态码一致**，仅把报文换成标准包络（旧的裸 TypeBox 报文无消费方）。
    pub fn unprocessable(code: &'static str) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code)
    }

    pub fn forbidden(code: &'static str) -> Self {
        Self::new(StatusCode::FORBIDDEN, code)
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            message: Some(message.into()),
            hint: None,
            data: None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiResponse::<Value> {
            success: false,
            data: self.data,
            error: Some(self.code.to_string()),
            message: self.message,
            hint: self.hint,
            meta: None,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        AppError::internal("数据库错误")
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        tracing::error!(error = %e, "jwt error");
        AppError::internal("令牌处理错误")
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "internal error");
        AppError::internal("内部错误")
    }
}

/// 便捷函数：构造成功响应。
pub fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse::ok(data))
}

/// 便捷函数：构造无 data 的成功响应（如 204 语义的写入端点）。
pub fn ok_no_data<T: Serialize>() -> Json<ApiResponse<T>> {
    Json(ApiResponse::ok_no_data())
}

/// 便捷函数：构造带顶层 `meta` 的成功响应（分页端点用）。
pub fn ok_with_meta<T: Serialize>(data: T, meta: serde_json::Value) -> Json<ApiResponse<T>> {
    Json(ApiResponse::ok_with_meta(data, meta))
}

/// JSON 请求体提取器，逐条对齐旧后端（Elysia + TypeBox）的失败状态码。
///
/// 不能直接用 axum 的 `Json<T>`：它的拒绝分类与 Elysia 差三处，实测口径如下
/// （旧后端 `POST /projects/:id/repositories`）：
///
/// | 请求体 | 旧后端 | axum `Json<T>` | 本提取器 |
/// | --- | --- | --- | --- |
/// | `not-json`（语法错） | **400** 纯文本 `Bad Request` | 400 | 400 + 包络 |
/// | 空 body | **422** `Expected object` | 400（EOF 也算语法错） | 422 |
/// | 缺 `Content-Type` | **422** `Expected object`（body 当 undefined） | **415** | 422 |
/// | `[1,2]` / 字段类型错 | 422 | 422 | 422 |
///
/// 「缺 Content-Type → 422」这条尤其不能省：axum 默认 415 直接拒收，而 Elysia 是把
/// body 视作 `undefined` 后交给 schema 判空。若改成放行解析，`{"name":"x"}` 会被当成
/// 合法请求**真的建库**，比状态码不一致严重得多。
///
/// 报文一律换成 `{success:false,error:...}` 包络（旧后端 400 分支吐的是纯文本），
/// 保证任何失败路径都能被 CLI 的 `parse_error_detail` 正常解析。
pub struct ValidatedJson<T>(pub T);

impl<T, S> axum::extract::FromRequest<S> for ValidatedJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(
        req: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let has_json_content_type = req
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(is_json_mime)
            .unwrap_or(false);

        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| AppError::bad_request("BAD_REQUEST").with_message(e.body_text()))?;

        // 缺 Content-Type 或空 body：旧后端都走「body 是 undefined」→ 422
        if !has_json_content_type || bytes.is_empty() {
            return Err(
                AppError::unprocessable("VALIDATION_ERROR").with_message("Expected object")
            );
        }

        match serde_json::from_slice::<T>(&bytes) {
            Ok(value) => Ok(ValidatedJson(value)),
            // 语法层面就不是 JSON → 400（对齐 Elysia 的裸 `Bad Request`）
            Err(e) if e.is_syntax() || e.is_eof() => {
                Err(AppError::bad_request("BAD_REQUEST").with_message(e.to_string()))
            }
            // 是合法 JSON 但结构 / 类型不符 → 422
            Err(e) => Err(AppError::unprocessable("VALIDATION_ERROR").with_message(e.to_string())),
        }
    }
}

/// `application/json` 及其 `+json` 后缀族（忽略 charset 等参数与大小写）。
fn is_json_mime(raw: &str) -> bool {
    let mime = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    mime == "application/json" || mime.ends_with("+json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_mime_matching_tolerates_params_and_case() {
        assert!(is_json_mime("application/json"));
        assert!(is_json_mime("Application/JSON; charset=utf-8"));
        assert!(is_json_mime("application/merge-patch+json"));
        assert!(!is_json_mime("text/plain"));
        assert!(!is_json_mime(""));
    }

    #[test]
    fn envelope_omits_empty_fields() {
        let body = ApiResponse::ok(42);
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"success":true,"data":42}"#);
    }

    /// 字段级校验必须落在 422：旧后端由 Elysia/TypeBox 统一返回 422，
    /// 状态码是唯一被前端/CLI 依赖的部分，报文形态可以不同。
    #[test]
    fn validation_errors_use_422_like_legacy() {
        let e = AppError::unprocessable("VALIDATION_ERROR").with_message("email 格式不正确");
        assert_eq!(e.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(e.code, "VALIDATION_ERROR");
        // 业务类错误仍走各自状态码
        assert_eq!(AppError::bad_request("INVALID_INVITE_CODE").status, StatusCode::BAD_REQUEST);
        assert_eq!(AppError::conflict("EMAIL_EXISTS").status, StatusCode::CONFLICT);
    }

    #[test]
    fn error_envelope_has_code() {
        let body = ApiResponse::<()> {
            success: false,
            data: None,
            error: Some("UNAUTHORIZED".to_string()),
            message: None,
            hint: None,
            meta: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(
            json,
            r#"{"success":false,"error":"UNAUTHORIZED"}"#
        );
    }
}
