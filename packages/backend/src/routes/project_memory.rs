//! 项目级记忆路由：`GET/PUT /projects/{projectId}/memory`。
//!
//! - 「仅可编辑不可删除」：本模块只挂 GET/PUT，**不提供 DELETE**——与宪法一致，
//!   删除天然不可达（静态路由无 DELETE 方法时 axum 回 405）。
//! - 容量上限与需求工作记忆统一（`core::memory::MEMORY_SNAPSHOT_MAX_CHARS`，10_000）。
//! - 权限对齐知识库路由：项目成员可见即可读写（`visible` 校验）。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::memory::validate_memory_snapshot;
use crate::core::serde_ext::double_option;
use crate::repos::project_knowledge::WriteOutcome;
use crate::repos::project_memory::{
    get_project_memory, project_memory_dto, upsert_project_memory, ProjectMemoryRow,
};
use crate::routes::project_knowledge::visible;
use crate::routes::validate::required_revision;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct MemoryBody {
    #[serde(default, deserialize_with = "double_option")]
    snapshot: Option<Option<String>>,
    /// 乐观锁版本号。**必填**——`0` 表示「我认为项目记忆还不存在」。
    #[serde(default)]
    revision: Option<i32>,
}

async fn get_memory_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let row = get_project_memory(&state.pool(), &pid).await?;
    let Some(row) = row else {
        return Err(AppError::not_found("MEMORY_NOT_FOUND"));
    };
    // 批注注入（方案 A）：项目记忆是知识库成员（key=memory），Agent 读它时同样要
    // 看到 open 批注。缺行不注入，保持与旧响应一致。
    let mut dto = project_memory_dto(&row);
    if let Some(block) = crate::services::project_knowledge_annotation::load_block_for_doc(
        &state.pool(),
        &pid,
        "memory",
        None,
    )
    .await?
    {
        dto["annotations"] = block;
    }
    Ok(ok(dto))
}

async fn put_memory_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<MemoryBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let snapshot = validate_memory_snapshot(&body.snapshot)?;
    let expected = required_revision("revision", body.revision)?;
    // `validate_memory_snapshot` 返回的 `None` = 显式 null = 清空记忆，
    // 而不再是「字段缺失」（缺失已在它内部 400）。仓储层收 `Option<&str>`
    // 正是为了区分「置 NULL」与「不发 UPDATE」——后者现在到不了这里。
    let outcome = upsert_project_memory(&state.pool(), &pid, snapshot, expected).await?;

    match outcome {
        WriteOutcome::Ok(row) => Ok(ok(project_memory_dto(&row))),
        // 记忆行不存在且调用方声称的版本不是 0：项目记忆是「每项目一份」的固定成员，
        // 「行还没建」不是资源不存在，而是调用方拿的版本号不自洽 → 409。
        WriteOutcome::NotFound => Err(memory_conflict(&pid, expected, None)),
        WriteOutcome::Conflict(current) => Err(memory_conflict(&pid, expected, Some(&current))),
    }
}

/// 409 `MEMORY_CONFLICT`：附当前版本号 + 当前快照全文。
///
/// 快照有硬上限 10000 字符（`core::memory::MEMORY_SNAPSHOT_MAX_CHARS`），
/// 全文回传没有体积顾虑——这点与知识文档不同（那边 `CONTENT_MAX` 无上限，
/// 见 `routes::project_knowledge::knowledge_doc_conflict` 的讨论）。
fn memory_conflict(
    project_id: &str,
    your_revision: i32,
    current: Option<&ProjectMemoryRow>,
) -> AppError {
    let mut data = serde_json::json!({
        "projectId": project_id,
        "yourRevision": your_revision,
        "currentRevision": current.map(|c| c.revision).unwrap_or(0),
    });
    if let Some(row) = current {
        data["currentSnapshot"] = serde_json::json!(row.snapshot);
        data["updatedAt"] = serde_json::json!(row.updated_at);
    }
    AppError::conflict("MEMORY_CONFLICT")
        .with_message("项目记忆已被他人修改")
        .with_hint("用 currentSnapshot 合并你的改动，revision 取 currentRevision 后重试")
        .with_data(data)
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/memory",
            get(get_memory_handler).put(put_memory_handler),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
