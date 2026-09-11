//! 知识文档只读分享：成员管理端点 + 匿名公开读取。
//!
//! 仅自定义知识文档可分享；`constitution` / `memory` → 400 `SHARE_SYSTEM_DOC_FORBIDDEN`。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::core::serde_ext::double_option;
use crate::core::tokens::{generate_secure_token, hash_token};
use crate::repos::project_knowledge as ctx_repo;
use crate::repos::project_knowledge_share as share_repo;
use crate::routes::project_knowledge::visible;
use crate::state::AppState;

const SYSTEM_KEYS: &[&str] = &["constitution", "memory"];

fn reject_system_doc(doc_id: &str) -> Result<(), AppError> {
    if SYSTEM_KEYS.contains(&doc_id) {
        return Err(AppError::bad_request("SHARE_SYSTEM_DOC_FORBIDDEN")
            .with_message("系统固定文档不支持分享"));
    }
    Ok(())
}

fn parse_expires_at(raw: Option<Option<String>>) -> Result<Option<Option<DateTime<Utc>>>, AppError> {
    match raw {
        None => Ok(None),
        Some(None) => Ok(Some(None)),
        Some(Some(s)) if s.trim().is_empty() => Ok(Some(None)),
        Some(Some(s)) => {
            let dt = DateTime::parse_from_rfc3339(s.trim())
                .map(|d| d.with_timezone(&Utc))
                .map_err(|_| {
                    AppError::unprocessable("INVALID_EXPIRES_AT")
                        .with_message("expiresAt 须为 RFC3339")
                })?;
            Ok(Some(Some(dt)))
        }
    }
}

fn share_url(public_origin: &str, token: &str) -> String {
    let base = public_origin.trim_end_matches('/');
    format!("{base}/share/k/{token}")
}

fn is_share_active(enabled: bool, expires_at: Option<DateTime<Utc>>) -> bool {
    if !enabled {
        return false;
    }
    match expires_at {
        None => true,
        Some(at) => at > Utc::now(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateShareBody {
    #[serde(default, deserialize_with = "double_option")]
    expires_at: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchShareBody {
    #[serde(default, deserialize_with = "double_option")]
    enabled: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option")]
    expires_at: Option<Option<String>>,
}

async fn get_share(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    reject_system_doc(&doc_id)?;
    let pid = visible(&state, &session, &project_id).await?;
    let doc = ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id)
        .await?
        .ok_or_else(|| AppError::not_found("CONTEXT_DOC_NOT_FOUND"))?;
    let _ = doc;
    let row = share_repo::get_by_project_document(&state.pool(), &pid, &doc_id).await?;
    match row {
        None => Ok(ok(json!({
            "hasToken": false,
            "enabled": false,
            "expiresAt": Value::Null,
            "urlPath": "/share/k/{token}",
        }))),
        Some(s) => Ok(ok(json!({
            "hasToken": true,
            "enabled": s.enabled,
            "expiresAt": s.expires_at.as_ref().map(dt_value),
            "urlPath": "/share/k/{token}",
            "active": is_share_active(s.enabled, s.expires_at),
        }))),
    }
}

async fn create_or_rotate_share(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<CreateShareBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    reject_system_doc(&doc_id)?;
    let pid = visible(&state, &session, &project_id).await?;
    ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id)
        .await?
        .ok_or_else(|| AppError::not_found("CONTEXT_DOC_NOT_FOUND"))?;

    let expires_at = parse_expires_at(body.expires_at)?.unwrap_or(None);
    let token = generate_secure_token(32);
    let token_hash = hash_token(&token);
    let row = share_repo::upsert_share(
        &state.pool(),
        &pid,
        &doc_id,
        &token_hash,
        true,
        expires_at,
        &session.user.user_id,
    )
    .await?;

    let url = share_url(&state.config().public_origin, &token);
    Ok(ok(json!({
        "token": token,
        "url": url,
        "enabled": row.enabled,
        "expiresAt": row.expires_at.as_ref().map(dt_value),
        "hasToken": true,
        "active": is_share_active(row.enabled, row.expires_at),
    })))
}

async fn patch_share(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<PatchShareBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    reject_system_doc(&doc_id)?;
    let pid = visible(&state, &session, &project_id).await?;
    ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id)
        .await?
        .ok_or_else(|| AppError::not_found("CONTEXT_DOC_NOT_FOUND"))?;

    let enabled = match body.enabled {
        None => None,
        Some(None) => None,
        Some(Some(v)) => Some(v),
    };
    let expires_at = parse_expires_at(body.expires_at)?;

    let updated = share_repo::update_share_meta(
        &state.pool(),
        &pid,
        &doc_id,
        enabled,
        expires_at,
    )
    .await?
    .ok_or_else(|| AppError::not_found("SHARE_NOT_FOUND"))?;

    Ok(ok(json!({
        "hasToken": true,
        "enabled": updated.enabled,
        "expiresAt": updated.expires_at.as_ref().map(dt_value),
        "urlPath": "/share/k/{token}",
        "active": is_share_active(updated.enabled, updated.expires_at),
    })))
}

async fn delete_share(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, doc_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    reject_system_doc(&doc_id)?;
    let pid = visible(&state, &session, &project_id).await?;
    ctx_repo::find_knowledge_document(&state.pool(), &pid, &doc_id)
        .await?
        .ok_or_else(|| AppError::not_found("CONTEXT_DOC_NOT_FOUND"))?;

    let updated = share_repo::disable_share(&state.pool(), &pid, &doc_id)
        .await?
        .ok_or_else(|| AppError::not_found("SHARE_NOT_FOUND"))?;

    Ok(ok(json!({
        "hasToken": true,
        "enabled": updated.enabled,
        "expiresAt": updated.expires_at.as_ref().map(dt_value),
        "urlPath": "/share/k/{token}",
        "active": false,
    })))
}

async fn get_public_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let invalid = || AppError::not_found("SHARE_INVALID");
    if token.len() < 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let token_hash = hash_token(&token);
    let row = share_repo::get_public_by_token_hash(&state.pool(), &token_hash)
        .await?
        .ok_or_else(invalid)?;
    if !is_share_active(row.enabled, row.expires_at) {
        return Err(invalid());
    }
    Ok(ok(json!({
        "title": row.title,
        "content": row.content,
        "updatedAt": dt_value(&row.updated_at),
        "projectName": row.project_name,
    })))
}

/// 需登录的分享管理路由。
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/knowledge/documents/{docId}/share",
            get(get_share)
                .post(create_or_rotate_share)
                .patch(patch_share)
                .delete(delete_share),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

/// 匿名公开读取（无 JWT）。
pub fn public_router(_state: AppState) -> Router<AppState> {
    Router::new().route(
        "/public/knowledge/shares/{token}",
        get(get_public_share),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_docs_are_rejected() {
        assert!(reject_system_doc("constitution").is_err());
        assert!(reject_system_doc("memory").is_err());
        assert!(reject_system_doc("abc123XYZ_01").is_ok());
    }

    #[test]
    fn expired_share_is_inactive() {
        let past = Utc::now() - chrono::Duration::hours(1);
        assert!(!is_share_active(true, Some(past)));
        assert!(is_share_active(true, None));
        assert!(!is_share_active(false, None));
    }
}
