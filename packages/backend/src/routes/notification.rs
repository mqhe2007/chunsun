//! 通知路由（1:1 移植自 `notification.ts`）。
//!
//! 全部 4 端点都在 `auth_middleware` 之下，**任意已登录用户**即可（无 ADMIN 要求）：
//! 未登录 → 401 UNAUTHORIZED。数据按 `userId` 隔离，`:id/read` 越权或不存 → 404 NOTIFICATION_NOT_FOUND。

use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ok_no_data, ok_with_meta, AppError};
use crate::auth::CurrentUser;
use crate::core::js_number::js_number;
use crate::repos::notification::{
    count_unread, list_notifications, mark_all_as_read, mark_as_read, notification_dto,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct NotifQuery {
    page: Option<String>,
    #[serde(rename = "pageSize")]
    page_size: Option<String>,
    #[serde(rename = "unreadOnly")]
    unread_only: Option<String>,
}

/// 复刻 `Number(query.page ? query.page : 1)` + `page && pageSize` 的 truthy 分页门控。
fn resolve_pagination(
    page_raw: Option<&str>,
    size_raw: Option<&str>,
) -> (Option<i64>, Option<i64>, i64, i64) {
    let page_f = page_raw.map(js_number).unwrap_or(1.0);
    let size_f = size_raw.map(js_number).unwrap_or(20.0);
    let use_paginate = page_f.is_finite() && page_f != 0.0 && size_f.is_finite() && size_f != 0.0;
    if use_paginate {
        let page = page_f.trunc() as i64;
        let size = size_f.trunc() as i64;
        (
            Some(page),
            Some(size),
            page,
            size,
        )
    } else {
        // 非 truthy：不加分页（返回全量），meta 仍用 1/20 作为展示值。
        (None, None, 1, 20)
    }
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Query(q): Query<NotifQuery>,
) -> Result<Json<crate::api::ApiResponse<Vec<Value>>>, AppError> {
    let unread_only = q.unread_only.as_deref() == Some("true");
    let (page, page_size, page_meta, size_meta) =
        resolve_pagination(q.page.as_deref(), q.page_size.as_deref());
    let (items, total) = list_notifications(
        &state.pool(),
        &session.user.user_id,
        unread_only,
        page,
        page_size,
    )
    .await?;
    let meta = json!({
        "total": total,
        "page": page_meta,
        "pageSize": size_meta,
        "totalPages": (total + size_meta - 1) / size_meta,
    });
    Ok(ok_with_meta(
        items.iter().map(notification_dto).collect::<Vec<_>>(),
        meta,
    ))
}

async fn unread_count(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<crate::api::ApiResponse<Value>>, AppError> {
    let n = count_unread(&state.pool(), &session.user.user_id).await?;
    Ok(ok(json!({ "count": n })))
}

async fn read(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<crate::api::ApiResponse<Value>>, AppError> {
    let notif = mark_as_read(&state.pool(), &id, &session.user.user_id).await?;
    match notif {
        Some(n) => Ok(ok(notification_dto(&n))),
        None => Err(AppError::not_found("NOTIFICATION_NOT_FOUND")),
    }
}

async fn read_all(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    mark_all_as_read(&state.pool(), &session.user.user_id).await?;
    Ok(ok_no_data())
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list))
        .route("/unread-count", axum::routing::get(unread_count))
        .route("/{id}/read", axum::routing::patch(read))
        .route("/read-all", axum::routing::post(read_all))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
