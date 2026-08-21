//! 统一认证守卫：JWT Bearer Token（Web 前端）+ Secret Key（CLI / Skill 工作流）。
//!
//! 语义对齐 `packages/backend/src/middleware/auth.ts`：
//! - 先试 JWT（HS256，payload 需含 userId + email）；成功注入 AuthUser。
//! - 再试 Secret Key（`sk_` 前缀）：按 project.secret_key 反查并 JOIN user，
//!   要求 user.status == ACTIVE；注入 AuthUser + authProjectId。

use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::state::AppState;

pub const ROLE_USER: &str = "USER";

/// 解析 `JWT_EXPIRES_IN`（如 `2h` / `7d` / `60m` / `30s` / 纯秒数）为秒。
///
/// 对齐 @elysiajs/jwt（底层 jose）接受的时长写法；无法解析时回落到默认 2h。
pub fn parse_expires_in(raw: &str) -> i64 {
    const DEFAULT: i64 = 2 * 60 * 60;
    let s = raw.trim().to_lowercase();
    if s.is_empty() {
        return DEFAULT;
    }
    if let Ok(secs) = s.parse::<i64>() {
        return if secs > 0 { secs } else { DEFAULT };
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: i64 = match num.trim().parse() {
        Ok(v) if v > 0 => v,
        _ => return DEFAULT,
    };
    match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        "w" => n * 604_800,
        _ => DEFAULT,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
    pub role: String,
}

/// 认证会话：SK 通道额外携带 project_id（区分 CLI 与 Web 调用）。
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub user: AuthUser,
    pub project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    #[serde(rename = "userId")]
    user_id: String,
    email: String,
    #[serde(rename = "role", default)]
    role: Option<String>,
}

/// 纯函数：JWT HS256 校验（与 @elysiajs/jwt / jose HS256 兼容）。
pub fn verify_jwt(token: &str, secret: &str) -> Result<AuthUser, AppError> {
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::unauthorized())?;
    Ok(AuthUser {
        user_id: data.claims.user_id,
        email: data.claims.email,
        role: data.claims.role.unwrap_or_else(|| ROLE_USER.to_string()),
    })
}

/// 签发 JWT（HS256），claims 形状对齐旧后端 `{ userId, email, role, iat, exp }`。
pub fn sign_jwt(user: &AuthUser, secret: &str, expires_in: &str) -> Result<String, AppError> {
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "userId": user.user_id,
        "email": user.email,
        "role": user.role,
        "iat": now,
        "exp": now + parse_expires_in(expires_in),
    });
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(AppError::from)
}

/// 提取认证会话：先 JWT 后 SK。返回 401 语义与旧后端一致。
pub async fn extract_session(
    token: Option<&str>,
    pool: &PgPool,
    secret: &str,
) -> Result<AuthSession, AppError> {
    let token = token.ok_or_else(AppError::unauthorized)?;

    // 1. JWT
    if let Ok(user) = verify_jwt(token, secret) {
        return Ok(AuthSession { user, project_id: None });
    }

    // 2. Secret Key
    if token.starts_with("sk_") {
        #[derive(Debug, FromRow)]
        struct SkProjectAuthRow {
            project_id: String,
            user_id: String,
            email: String,
            role: String,
            status: String,
        }

        let row: Option<SkProjectAuthRow> = sqlx::query_as(
            r#"SELECT p.id AS project_id, u.id AS user_id, u.email,
                      u.role::text AS role, u.status::text AS status
               FROM project p
               JOIN "user" u ON u.id = p.user_id
               WHERE p.secret_key = $1"#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            if row.status == "ACTIVE" {
                return Ok(AuthSession {
                    user: AuthUser {
                        user_id: row.user_id,
                        email: row.email,
                        role: row.role,
                    },
                    project_id: Some(row.project_id),
                });
            }
        }
        return Err(AppError::unauthorized());
    }

    Err(AppError::unauthorized())
}

/// 从请求扩展中取已认证会话（需先经 auth_middleware 填充）。
pub struct CurrentUser(pub AuthSession);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthSession>()
            .cloned()
            .map(CurrentUser)
            .ok_or_else(AppError::unauthorized)
    }
}

/// 平台 ADMIN 守卫：会话必须由 `auth_middleware` 填充（无会话 → 401），
/// 且 `role == "ADMIN"`，否则 403。语义对齐旧后端 `getAuthUser().role !== "ADMIN"` 的顺序。
pub struct AdminUser(pub AuthSession);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<AuthSession>()
            .cloned()
            .ok_or_else(AppError::unauthorized)?;
        if session.user.role != "ADMIN" {
            return Err(AppError::forbidden("FORBIDDEN"));
        }
        Ok(AdminUser(session))
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool()
    }
}

/// 401 响应（包络对齐旧后端）。
pub fn unauthorized_response() -> (StatusCode, axum::Json<crate::api::ApiResponse<()>>) {
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(crate::api::ApiResponse {
            success: false,
            data: None,
            error: Some("UNAUTHORIZED".to_string()),
            message: None,
            hint: None,
            meta: None,
        }),
    )
}

/// axum 中间件：解析 Authorization: Bearer，注入 AuthSession 到请求扩展。
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string);

    match extract_session(token.as_deref(), &state.pool(), &state.config().jwt_secret).await {
        Ok(session) => {
            req.extensions_mut().insert(session);
            next.run(req).await
        }
        Err(_) => {
            use axum::response::IntoResponse;
            unauthorized_response().into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    const TEST_SECRET: &str = "unit-test-jwt-secret";

    fn sign(claims: serde_json::Value) -> String {
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn jwt_roundtrip_with_user_id_email_role() {
        let token = sign(serde_json::json!({
            "userId": "u_123",
            "email": "a@b.com",
            "role": "ADMIN",
            "iat": 1700000000,
            "exp": 2000000000
        }));
        let user = verify_jwt(&token, TEST_SECRET).unwrap();
        assert_eq!(user.user_id, "u_123");
        assert_eq!(user.email, "a@b.com");
        assert_eq!(user.role, "ADMIN");
    }

    #[test]
    fn role_defaults_to_user_when_missing() {
        let token = sign(serde_json::json!({
            "userId": "u_1",
            "email": "x@y.z",
            "iat": 1700000000,
            "exp": 2000000000
        }));
        let user = verify_jwt(&token, TEST_SECRET).unwrap();
        assert_eq!(user.role, ROLE_USER);
    }

    #[test]
    fn rejects_wrong_secret_and_garbage() {
        let token = sign(serde_json::json!({
            "userId": "u_1", "email": "x@y.z", "iat": 1700000000, "exp": 2000000000
        }));
        assert!(verify_jwt(&token, "other-secret").is_err());
        assert!(verify_jwt("not-a-jwt", TEST_SECRET).is_err());
    }

    #[test]
    fn rejects_expired_token() {
        let token = sign(serde_json::json!({
            "userId": "u_1", "email": "x@y.z", "iat": 1700000000, "exp": 1700000100
        }));
        assert!(verify_jwt(&token, TEST_SECRET).is_err());
    }

    #[test]
    fn parses_duration_strings() {
        assert_eq!(parse_expires_in("30s"), 30);
        assert_eq!(parse_expires_in("60m"), 3600);
        assert_eq!(parse_expires_in("2h"), 7200);
        assert_eq!(parse_expires_in("7d"), 604_800);
        assert_eq!(parse_expires_in("1w"), 604_800);
        assert_eq!(parse_expires_in("3600"), 3600);
        // 非法输入回落默认 2h
        assert_eq!(parse_expires_in(""), 7200);
        assert_eq!(parse_expires_in("abc"), 7200);
        assert_eq!(parse_expires_in("0h"), 7200);
        assert_eq!(parse_expires_in("-5m"), 7200);
    }

    #[test]
    fn signed_token_roundtrips_through_verify() {
        let user = AuthUser {
            user_id: "u_9".into(),
            email: "sign@chunsun.dev".into(),
            role: "ADMIN".into(),
        };
        let token = sign_jwt(&user, TEST_SECRET, "2h").unwrap();
        let parsed = verify_jwt(&token, TEST_SECRET).unwrap();
        assert_eq!(parsed, user);
    }
}
