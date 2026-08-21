//! 活动域路由（1:1 移植自 `packages/backend/src/routes/activity.ts`）。
//!
//! 两条只读端点，都在 `auth_middleware` 之下，权限只有一档——项目可见性，
//! 不可见 404 `PROJECT_NOT_FOUND`。没有写端点（活动由各业务域旁路写入）。
//!
//! | 端点 | query | 说明 |
//! | --- | --- | --- |
//! | `GET /projects/{id}/activities` | `limit` | 按 `createdAt DESC` 取 N 条，带 user 关联 |
//! | `GET /projects/{id}/activity-heatmap` | `days` / `year` | 按 UTC 日聚合的活跃图 |
//!
//! ## `limit` 的三段式怪癖
//!
//! 旧实现一行 `Math.min(Number(query.limit ?? 30) || 30, 100)` 藏了三层语义：
//!
//! 1. **缺省不过 `Number(string)`**：`?? 30` 给的是数字 30，不是字符串。
//! 2. **`|| 30` 吃掉 falsy**：`""`→0、`"abc"`→NaN、`"0"`→0、`"-0"`→-0 统统回落 30。
//! 3. **夹取只封顶不保底**：`Math.min(x, 100)` 不拦负数，`?limit=-5` 会原样进
//!    Prisma 的 `take: -5` —— 这是合法的**反向取**（取最早的 5 条，仍按 desc 返回），
//!    不是错误。Prisma 的 `take` **不校验** Int 类型/范围：小数被截断（`3.7 → 3`）、
//!    超 Int32 范围直接返回全量（`-99999999999 → 全部`），都不会报错；只有 `-Infinity`
//!    （`Number("-Infinity")` 非有限、且 `0/NaN` 兜底吃不到）会撞上 Prisma 的 Int 校验
//!    直接抛，旧后端没捕获 → 500，这里用 [`AppError::internal`] 复刻。
//!
//! ## `year` / `days` 的优先级
//!
//! `parseYear` 只认 [2000, 2100] 的整数（先 `Number` 再 `Math.trunc`），**任何不合法的
//! `year` 都静默回落到 days 模式**——包括 `?year=`（`Number("")` = 0，不在区间）。
//! 只有 year 解析成功时 days 才被跳过。

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::api::{ok, ApiResponse, AppError};
use crate::auth::CurrentUser;
use crate::core::activity_heatmap::{aggregate_heatmap, clamp_days, heatmap_date_range, HeatmapOpts};
use crate::core::js_number::js_number;
use crate::repos::project_activity;
use crate::routes::dto::{activity_dto, heatmap_dto};
use crate::services::project_access::visible_project_id;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct ListQuery {
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HeatmapQuery {
    days: Option<String>,
    year: Option<String>,
}

// ── helpers ─────────────────────────────────────────────────────────────

/// 复刻 `Math.min(Number(query.limit ?? 30) || 30, 100)`，并对齐 Prisma 的 `take` 真实行为。
///
/// Prisma 的 `take` **不校验** Int 类型/范围：小数被截断（`3.7 → 3`）、超 Int32 范围直接返回全量
/// （`-99999999999 → 全部`），都不会报错。唯一会撞 Prisma 抛错的是 `-Infinity`（非有限且
/// `0/NaN` 兜底吃不到），旧后端未捕获 → 500，这里用 [`AppError::internal`] 复刻。
fn resolve_limit(raw: Option<&str>) -> Result<i64, AppError> {
    // 缺省走 `?? 30`（数字），不经过 Number(string)
    let n = raw.map_or(30.0, js_number);
    // `|| 30`：Number 能产出的 falsy 只有 0 / -0 / NaN
    let n = if n == 0.0 || n.is_nan() { 30.0 } else { n };
    let limit = n.min(100.0);

    // 仅 -Infinity 非有限（Infinity 已被 Math.min 夹成 100）
    if !limit.is_finite() {
        return Err(AppError::internal(format!("INVALID_TAKE:{limit}")));
    }
    // 截断小数对齐 Prisma；i64 容纳超 Int32 反向 take（对齐全量返回），不报错
    Ok(limit as i64)
}

/// 复刻 `parseYear`：缺省 → None；非有限 → None；截断后必须落在 [2000, 2100]。
fn parse_year(input: Option<&str>) -> Option<i32> {
    let n = js_number(input?);
    if !n.is_finite() {
        return None;
    }
    let year = n.trunc();
    if (2000.0..=2100.0).contains(&year) {
        Some(year as i32)
    } else {
        None
    }
}

// ── handlers ────────────────────────────────────────────────────────────

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let project_id = visible_project_id(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;

    let take = resolve_limit(q.limit.as_deref())?;
    let rows = project_activity::list_project_activities(&state.pool(), &project_id, take).await?;

    Ok(ok(rows.iter().map(activity_dto).collect()))
}

async fn heatmap(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    Query(q): Query<HeatmapQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let project_id = visible_project_id(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;

    let year = parse_year(q.year.as_deref());
    // year 命中时 days 整段跳过（旧实现是三元里的 `undefined`）
    let days = if year.is_some() {
        None
    } else {
        Some(clamp_days(q.days.as_deref().map(js_number)))
    };
    let range = heatmap_date_range(HeatmapOpts { year, days }, chrono::Utc::now());

    let created_ats = project_activity::list_activity_created_ats(
        &state.pool(),
        &project_id,
        range.start,
        range.end_plus_one_day,
    )
    .await?;

    let (max, entries) = aggregate_heatmap(&range, &created_ats);
    Ok(ok(heatmap_dto(range.window_days, max, &entries)))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/projects/{projectId}/activities", get(list))
        .route("/projects/{projectId}/activity-heatmap", get(heatmap))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_defaults_to_30_for_absent_and_falsy() {
        assert_eq!(resolve_limit(None).unwrap(), 30);
        // Number("") = 0 → falsy → 30
        assert_eq!(resolve_limit(Some("")).unwrap(), 30);
        // Number("abc") = NaN → falsy → 30
        assert_eq!(resolve_limit(Some("abc")).unwrap(), 30);
        assert_eq!(resolve_limit(Some("0")).unwrap(), 30);
        assert_eq!(resolve_limit(Some("-0")).unwrap(), 30);
    }

    #[test]
    fn limit_is_capped_at_100_but_not_floored() {
        assert_eq!(resolve_limit(Some("5")).unwrap(), 5);
        assert_eq!(resolve_limit(Some("100")).unwrap(), 100);
        assert_eq!(resolve_limit(Some("101")).unwrap(), 100);
        assert_eq!(resolve_limit(Some("1e3")).unwrap(), 100);
        assert_eq!(resolve_limit(Some("Infinity")).unwrap(), 100);
        // JS Number 会 trim 两侧空白、认十六进制
        assert_eq!(resolve_limit(Some(" 7 ")).unwrap(), 7);
        assert_eq!(resolve_limit(Some("0x10")).unwrap(), 16);
        // 负数不被夹取，原样进 Prisma 的反向 take
        assert_eq!(resolve_limit(Some("-5")).unwrap(), -5);
    }

    #[test]
    fn limit_rejects_only_nonfinite() {
        // 仅 -Infinity 非有限 → Prisma 抛 → 500
        assert!(resolve_limit(Some("-Infinity")).is_err());
        // 小数截断（对齐 Prisma take:3.7 → 3）
        assert_eq!(resolve_limit(Some("3.7")).unwrap(), 3);
        assert_eq!(resolve_limit(Some("-3.7")).unwrap(), -3);
        // 超 Int32 范围：负数返回全量（对齐 Prisma take:-99999999999 → 全量）
        assert_eq!(resolve_limit(Some("-99999999999")).unwrap(), -99999999999);
        assert_eq!(resolve_limit(Some("99999999999")).unwrap(), 100);
    }

    #[test]
    fn year_only_accepts_2000_to_2100() {
        assert_eq!(parse_year(None), None);
        assert_eq!(parse_year(Some("2026")), Some(2026));
        assert_eq!(parse_year(Some("2000")), Some(2000));
        assert_eq!(parse_year(Some("2100")), Some(2100));
        assert_eq!(parse_year(Some("2026.9")), Some(2026)); // Math.trunc
        assert_eq!(parse_year(Some("1999")), None);
        assert_eq!(parse_year(Some("2101")), None);
        // Number("") = 0 → 不在区间 → 回落 days 模式（不是「今年」）
        assert_eq!(parse_year(Some("")), None);
        assert_eq!(parse_year(Some("abc")), None);
        assert_eq!(parse_year(Some("Infinity")), None);
    }
}
