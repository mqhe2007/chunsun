//! 春笋 harness 域路由（1:1 移植自 `harness.ts`）。
//!
//! 全部端点都在 `auth_middleware` 之下。鉴权档与旧端一致：
//! - 未登录（无 user）→ 401 UNAUTHORIZED
//! - 项目不可见（非创建者/成员且非 ADMIN）→ 404 PROJECT_NOT_FOUND（与旧端一致，非 403）
//! - 需求不存在（带 requirementId 的端点）→ 404 REQUIREMENT_NOT_FOUND
//!
//! 状态投影：Run/Scenario/Case 的写操作会同步回写 Requirement.status（running/completed/abandoned；finished 投影为 running）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{AppError, ApiResponse};
use crate::auth::{AuthSession, CurrentUser};
use crate::core::js_number::prisma_int;
use crate::core::serde_ext::double_option;
use crate::routes::validate::validation_error;
use crate::repos::harness::{
    case_dto, memory_dto, scenario_dto, step_dto, run_dto, CaseRow, UpsertCaseInput,
    UpsertScenarioInput,
};
use crate::repos::harness::{
    check_completion_gate, create_run as repo_create_run, create_step as repo_create_step,
    delete_case_by_id, delete_scenario_by_id, get_case_by_id, get_memory, get_run_by_id,
    list_cases_by_requirement, list_runs_by_requirement, list_scenarios as repo_list_scenarios,
    list_steps_by_run, reset_requirement, set_case_status, set_run_status, set_scenario_status,
    takeover_running_run, upsert_case, upsert_memory, upsert_scenario,
};
use crate::repos::project::get_project_by_id;
use crate::repos::requirement::get_requirement_by_id;
use crate::state::AppState;

// ---------- 路径 / 查询 / 请求体 ----------

#[derive(Debug, Deserialize)]
struct ReqParams {
    #[serde(rename = "projectId")]
    project_id: String,
    #[serde(rename = "requirementId")]
    requirement_id: String,
}

#[derive(Debug, Deserialize)]
struct RunParams {
    #[serde(rename = "projectId")]
    project_id: String,
    #[serde(rename = "requirementId")]
    requirement_id: String,
    #[serde(rename = "runId")]
    run_id: String,
}

#[derive(Debug, Deserialize)]
struct ScenarioParams {
    #[serde(rename = "projectId")]
    project_id: String,
    #[serde(rename = "requirementId")]
    requirement_id: String,
    #[serde(rename = "scenarioId")]
    scenario_id: String,
}

#[derive(Debug, Deserialize)]
struct CaseParams {
    #[serde(rename = "projectId")]
    project_id: String,
    #[serde(rename = "requirementId")]
    requirement_id: String,
    #[serde(rename = "caseId")]
    case_id: String,
}

#[derive(Debug, Deserialize)]
struct ListScenariosQuery {
    #[serde(rename = "includeCases")]
    include_cases: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunStatusBody {
    status: String,
    #[serde(rename = "endReason", default)]
    end_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StepBody {
    kind: String,
    summary: String,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    artifacts: Option<Value>,
}

// 旧端 body schema 是 `t.Object({ snapshot: t.Any() })`；TypeBox 的 t.Any() **允许缺字段**，
// 缺失时把 undefined 透给 Prisma —— 分两条路：
//   update 路径（context 已存在）：`data: { snapshot: undefined }` 退化为空 data，跳过该列；
//   create 路径（context 不存在）：必填缺参 → 500（Argument `snapshot` is missing）。
// 显式 null 则真落库：列是 `Json` 非空，存 jsonb 'null'。故必须三态，扁平 Option 会把 null 吞成缺失。
#[derive(Debug, Deserialize)]
struct MemoryBody {
    #[serde(default, deserialize_with = "double_option")]
    snapshot: Option<Option<Value>>,
}

#[derive(Debug, Deserialize)]
struct ScenarioBody {
    key: String,
    title: String,
    // 三态：旧端 `description: input.description`，undefined → Prisma 跳过该列（保留原值），
    // 显式 null → 置 NULL。扁平 Option 会把「未传」误当「清空」。
    #[serde(default, deserialize_with = "double_option")]
    description: Option<Option<String>>,
    // 旧端是 `t.Number()`（JS number），写库时 Prisma 向零截断到 Int。
    // 用 i32 接会把 `3.7` 挡在 422，与旧端 200+截断不符。
    #[serde(rename = "sortOrder", default)]
    sort_order: Option<f64>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CaseBody {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "scenarioId", default)]
    scenario_id: Option<String>,
    #[serde(rename = "scenarioKey", default)]
    scenario_key: Option<String>,
    title: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    steps: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    expected: Option<Option<String>>,
    #[serde(rename = "localPath", default, deserialize_with = "double_option")]
    local_path: Option<Option<String>>,
    #[serde(rename = "executionPlan", default)]
    execution_plan: Option<String>,
    #[serde(rename = "sortOrder", default)]
    sort_order: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CaseStatusBody {
    status: String,
    #[serde(rename = "actualResult", default, deserialize_with = "double_option")]
    actual_result: Option<Option<String>>,
    #[serde(rename = "executedBy", default)]
    executed_by: Option<String>,
    #[serde(rename = "localPath", default, deserialize_with = "double_option")]
    local_path: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct SyncResultItem {
    id: String,
    status: String,
    #[serde(rename = "actualResult", default, deserialize_with = "double_option")]
    actual_result: Option<Option<String>>,
    #[serde(rename = "executedBy", default)]
    executed_by: Option<String>,
    #[serde(rename = "localPath", default, deserialize_with = "double_option")]
    local_path: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct SyncResultsBody {
    results: Vec<SyncResultItem>,
}

#[derive(Debug, Deserialize)]
struct ScenarioStatusBody {
    status: String,
}

// ---------- 校验助手 ----------
//
// harness 的 status / kind / executionPlan / executedBy 在库里是 **PostgreSQL 原生 enum**，
// 非法值直接写库会撞 `invalid input value for enum` → 500。旧端由 TypeBox t.Union 在
// 校验层拦成 422，所以新端必须在 handler 里先做白名单，别把脏值扔给 PG。

const RUN_STATUSES: &[&str] = &["running", "completed", "finished", "abandoned"];
const STEP_KINDS: &[&str] = &[
    "think", "code", "test", "verify", "ask_user", "info", "reflect",
];
const SCENARIO_STATUSES: &[&str] = &["pending", "passing", "failing", "blocked", "waived"];
const CASE_STATUSES: &[&str] = &["pending", "passed", "failed", "blocked", "skipped"];
const CASE_KINDS: &[&str] = &["unit", "integration", "e2e"];
const EXECUTION_PLANS: &[&str] = &["auto", "manual"];
const EXECUTED_BYS: &[&str] = &["agent", "manual"];

fn ensure_enum(field: &str, value: &str, allowed: &[&str]) -> Result<(), AppError> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(validation_error(format!(
        "{field} 只能是 {} 之一",
        allowed.join(" / ")
    )))
}

fn ensure_enum_opt(
    field: &str,
    value: &Option<String>,
    allowed: &[&str],
) -> Result<(), AppError> {
    match value {
        None => Ok(()),
        Some(v) => ensure_enum(field, v, allowed),
    }
}

/// `t.String({ minLength, maxLength })`：长度按 JS 的 UTF-16 code unit 计。
fn ensure_len(field: &str, value: &str, min: usize, max: usize) -> Result<(), AppError> {
    let len = value.encode_utf16().count();
    if len < min || len > max {
        return Err(validation_error(format!(
            "{field} 长度需在 {min}~{max} 之间"
        )));
    }
    Ok(())
}

/// `sortOrder` 落库是 Prisma Int：向零截断，int4 越界则 Prisma 抛未捕获异常 → 500。
fn sort_order_int(value: Option<f64>) -> Result<Option<i32>, AppError> {
    match value {
        None => Ok(None),
        Some(n) => Ok(Some(prisma_int(n).map_err(|_| {
            AppError::internal(format!(
                "Value out of range for the type: value \"{n}\" is out of range for type integer"
            ))
        })?)),
    }
}

/// JS 的 `if (!x)`：`undefined` / `null` / `""` 一律视作缺失。
/// 旧端 case PUT 用 `!scenarioId && body.scenarioKey` 这类真值判断做字段回退，
/// 空串必须跟缺省走同一分支，否则会把 `key=""` 拿去查场景而误报 404。
fn truthy(value: &Option<String>) -> Option<&str> {
    match value {
        Some(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    }
}

// ---------- 响应辅助 ----------

fn ok_val(v: Value) -> (StatusCode, Json<ApiResponse<Value>>) {
    (StatusCode::OK, Json(ApiResponse::ok(v)))
}
fn created_val(v: Value) -> (StatusCode, Json<ApiResponse<Value>>) {
    (StatusCode::CREATED, Json(ApiResponse::ok(v)))
}

async fn check_project(
    state: &AppState,
    project_id: &str,
    session: &AuthSession,
) -> Result<(), AppError> {
    let is_admin = session.user.role == "ADMIN";
    let p = get_project_by_id(&state.pool(), project_id, &session.user.user_id, is_admin).await?;
    if p.is_none() {
        return Err(AppError::not_found("PROJECT_NOT_FOUND"));
    }
    Ok(())
}

async fn check_requirement(
    state: &AppState,
    requirement_id: &str,
    project_id: &str,
) -> Result<(), AppError> {
    let r = get_requirement_by_id(&state.pool(), requirement_id, project_id).await?;
    if r.is_none() {
        return Err(AppError::not_found("REQUIREMENT_NOT_FOUND"));
    }
    Ok(())
}

// ---------- Run ----------

async fn list_runs(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let runs = list_runs_by_requirement(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let data: Vec<Value> = runs.iter().map(run_dto).collect();
    Ok(ok_val(json!(data)))
}

async fn create_run_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;
    // 撞锁：已有 running 的 Run → 409
    let existing = list_runs_by_requirement(&state.pool(), &p.requirement_id, &p.project_id).await?;
    if existing.iter().any(|r| r.status == "running") {
        let running = existing.iter().find(|r| r.status == "running").unwrap();
        return Err(AppError::conflict("RUN_ALREADY_RUNNING")
            .with_hint("该需求已有 Run 在跑；如需接管（僵尸 Run 人工接管），请调用 takeover 端点")
            .with_data(json!({
            "runId": running.id,
            "lastActiveAt": running.updated_at,
        })));
    }
    let run = repo_create_run(&state.pool(), &p.requirement_id, &p.project_id).await?;
    Ok(created_val(run_dto(&run)))
}

async fn takeover_run(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;
    let taken = takeover_running_run(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let run = repo_create_run(&state.pool(), &p.requirement_id, &p.project_id).await?;
    Ok(created_val(json!({
        "run": run_dto(&run),
        "takenOver": taken.as_ref().map(run_dto),
    })))
}

async fn patch_run(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<RunParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<RunStatusBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_enum("status", &body.status, RUN_STATUSES)?;
    check_project(&state, &p.project_id, &session).await?;
    let run = get_run_by_id(&state.pool(), &p.run_id, &p.requirement_id).await?;
    let Some(run) = run else {
        return Err(AppError::not_found("RUN_NOT_FOUND"));
    };
    if body.status == "completed" {
        let (ok, blockers) =
            check_completion_gate(&state.pool(), &p.requirement_id, &p.project_id).await?;
        if !ok {
            return Err(AppError::conflict("COMPLETION_GATE_NOT_MET").with_hint(
                "场景须全部 passing/waived 且无 open decisions 才能 completed",
            ).with_data(json!({ "blockers": blockers })));
        }
    }
    let updated = set_run_status(
        &state.pool(),
        &run.id,
        &p.requirement_id,
        &body.status,
        body.end_reason.as_deref(),
    )
    .await?;
    let Some(updated) = updated else {
        return Err(AppError::not_found("RUN_NOT_FOUND"));
    };
    Ok(ok_val(run_dto(&updated)))
}

// ---------- Step ----------

async fn create_step_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<RunParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<StepBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_enum("kind", &body.kind, STEP_KINDS)?;
    ensure_len("summary", &body.summary, 1, 500)?;
    check_project(&state, &p.project_id, &session).await?;
    let run = get_run_by_id(&state.pool(), &p.run_id, &p.requirement_id).await?;
    let Some(run) = run else {
        return Err(AppError::not_found("RUN_NOT_FOUND"));
    };
    if run.status != "running" {
        return Err(AppError::conflict("RUN_NOT_RUNNING"));
    }
    let step = repo_create_step(
        &state.pool(),
        &run.id,
        &p.requirement_id,
        &body.kind,
        &body.summary,
        body.detail.as_deref(),
        &body.artifacts,
    )
    .await?;
    let Some(step) = step else {
        return Err(AppError::not_found("RUN_NOT_FOUND"));
    };
    Ok(created_val(step_dto(&step)))
}

async fn list_steps(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<RunParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let run = get_run_by_id(&state.pool(), &p.run_id, &p.requirement_id).await?;
    let Some(_) = run else {
        return Err(AppError::not_found("RUN_NOT_FOUND"));
    };
    let steps = list_steps_by_run(&state.pool(), &p.run_id).await?;
    let data: Vec<Value> = steps.iter().map(step_dto).collect();
    Ok(ok_val(json!(data)))
}

// ---------- Context ----------

async fn get_memory_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let ctx = get_memory(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let Some(ctx) = ctx else {
        return Err(AppError::not_found("MEMORY_NOT_FOUND"));
    };
    Ok(ok_val(memory_dto(&ctx)))
}

async fn put_memory(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<MemoryBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;
    // 三态原样下传，缺失/显式 null 的分叉由仓储层按 update / create 路径各自复刻。
    let snapshot = body.snapshot.as_ref().map(|v| v.as_ref());
    let ctx = upsert_memory(&state.pool(), &p.requirement_id, &p.project_id, snapshot).await?;
    Ok(ok_val(memory_dto(&ctx)))
}

// ---------- reset ----------

async fn reset_requirement_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;
    let run = reset_requirement(&state.pool(), &p.requirement_id, &p.project_id).await?;
    Ok(created_val(json!({ "run": run_dto(&run) })))
}

// ---------- Scenario ----------

async fn list_scenarios_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
    Query(q): Query<ListScenariosQuery>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let scenarios = repo_list_scenarios(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let include = q.include_cases.as_deref() == Some("true");
    if !include {
        let data: Vec<Value> = scenarios.iter().map(scenario_dto).collect();
        return Ok(ok_val(json!(data)));
    }
    let cases = list_cases_by_requirement(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let mut by_scenario: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    for c in &cases {
        by_scenario
            .entry(c.scenario_id.clone())
            .or_default()
            .push(case_dto(c));
    }
    let data: Vec<Value> = scenarios
        .iter()
        .map(|s| {
            let mut obj = scenario_dto(s);
            obj["cases"] = json!(by_scenario.get(&s.id).cloned().unwrap_or_default());
            obj
        })
        .collect();
    Ok(ok_val(json!(data)))
}

async fn put_scenario(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<ScenarioBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_len("key", &body.key, 1, 120)?;
    ensure_len("title", &body.title, 1, 300)?;
    ensure_enum_opt("status", &body.status, SCENARIO_STATUSES)?;
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;
    let sort_order = sort_order_int(body.sort_order)?;
    let input = UpsertScenarioInput {
        key: &body.key,
        title: &body.title,
        // 三态：未传 → Prisma 跳过该列（保留原值）；显式 null → 置 NULL。
        description: match &body.description {
            None => None,
            Some(v) => Some(v.as_deref()),
        },
        sort_order,
        status: body.status.as_deref(),
    };
    let row = upsert_scenario(&state.pool(), &p.requirement_id, &p.project_id, &input).await?;
    Ok(ok_val(scenario_dto(&row)))
}

async fn patch_scenario_status(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ScenarioParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<ScenarioStatusBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_enum("status", &body.status, SCENARIO_STATUSES)?;
    check_project(&state, &p.project_id, &session).await?;
    let row =
        set_scenario_status(&state.pool(), &p.scenario_id, &p.requirement_id, &body.status).await?;
    let Some(row) = row else {
        return Err(AppError::not_found("SCENARIO_NOT_FOUND"));
    };
    Ok(ok_val(scenario_dto(&row)))
}

async fn delete_scenario(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ScenarioParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let deleted = delete_scenario_by_id(&state.pool(), &p.scenario_id, &p.requirement_id).await?;
    let Some(deleted) = deleted else {
        return Err(AppError::not_found("SCENARIO_NOT_FOUND"));
    };
    Ok(ok_val(json!({ "id": deleted.id })))
}

// ---------- Case ----------

async fn list_cases(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let cases = list_cases_by_requirement(&state.pool(), &p.requirement_id, &p.project_id).await?;
    let data: Vec<Value> = cases.iter().map(case_dto).collect();
    Ok(ok_val(json!(data)))
}

async fn put_case(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<CaseBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_len("title", &body.title, 1, 300)?;
    ensure_enum_opt("kind", &body.kind, CASE_KINDS)?;
    ensure_enum_opt("executionPlan", &body.execution_plan, EXECUTION_PLANS)?;
    check_project(&state, &p.project_id, &session).await?;
    check_requirement(&state, &p.requirement_id, &p.project_id).await?;

    // 解析 scenarioId：scenarioId > scenarioKey > 已有 case 的 scenarioId。
    // 旧端用 `!scenarioId` 这类真值判断，空串等同缺省，这里用 truthy() 复刻。
    let body_id = truthy(&body.id);
    let mut scenario_id: Option<String> = truthy(&body.scenario_id).map(str::to_string);
    if scenario_id.is_none() {
        if let Some(key) = truthy(&body.scenario_key) {
            let scenarios =
                repo_list_scenarios(&state.pool(), &p.requirement_id, &p.project_id).await?;
            let by_key = scenarios.iter().find(|s| s.key == key);
            match by_key {
                Some(s) => scenario_id = Some(s.id.clone()),
                None => return Err(AppError::not_found("SCENARIO_NOT_FOUND")),
            }
        }
    }
    if scenario_id.is_none() {
        if let Some(id) = body_id {
            let existing = get_case_by_id(&state.pool(), id, &p.requirement_id).await?;
            match existing {
                Some(c) => scenario_id = Some(c.scenario_id),
                None => return Err(AppError::not_found("CASE_NOT_FOUND")),
            }
        }
    }
    let Some(scenario_id) = scenario_id else {
        return Err(AppError::bad_request("SCENARIO_REQUIRED"));
    };
    if body_id.is_none() && truthy(&body.kind).is_none() {
        return Err(AppError::bad_request("KIND_REQUIRED"));
    }
    let sort_order = sort_order_int(body.sort_order)?;
    let input = UpsertCaseInput {
        scenario_id: &scenario_id,
        id: body_id,
        title: &body.title,
        kind: body.kind.as_deref(),
        steps: body.steps,
        expected: body.expected,
        local_path: body.local_path,
        execution_plan: body.execution_plan.as_deref(),
        sort_order,
    };
    let row = upsert_case(&state.pool(), &p.requirement_id, &p.project_id, &input).await?;
    let Some(row) = row else {
        return Err(AppError::not_found("SCENARIO_NOT_FOUND"));
    };
    Ok(ok_val(case_dto(&row)))
}

async fn patch_case_status(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<CaseParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<CaseStatusBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    ensure_enum("status", &body.status, CASE_STATUSES)?;
    ensure_enum_opt("executedBy", &body.executed_by, EXECUTED_BYS)?;
    check_project(&state, &p.project_id, &session).await?;
    let row = set_case_status(
        &state.pool(),
        &p.case_id,
        &p.requirement_id,
        &body.status,
        body.actual_result,
        body.executed_by.as_deref(),
        "manual",
        body.local_path,
    )
    .await?;
    let Some(row) = row else {
        return Err(AppError::not_found("CASE_NOT_FOUND"));
    };
    Ok(ok_val(case_dto(&row)))
}

async fn sync_case_results(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<ReqParams>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<SyncResultsBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    // TypeBox 先整体校验 body 再进 handler，所以脏数据不能写一半——先全量校验。
    for r in &body.results {
        ensure_len("results.id", &r.id, 1, usize::MAX)?;
        ensure_enum("results.status", &r.status, CASE_STATUSES)?;
        ensure_enum_opt("results.executedBy", &r.executed_by, EXECUTED_BYS)?;
    }
    check_project(&state, &p.project_id, &session).await?;
    let mut updated: Vec<CaseRow> = Vec::with_capacity(body.results.len());
    for r in &body.results {
        let row = set_case_status(
            &state.pool(),
            &r.id,
            &p.requirement_id,
            &r.status,
            r.actual_result.clone(),
            r.executed_by.as_deref(),
            "agent",
            r.local_path.clone(),
        )
        .await?;
        if let Some(row) = row {
            updated.push(row);
        }
    }
    let data: Vec<Value> = updated.iter().map(case_dto).collect();
    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok_with_meta(
            json!(data),
            json!({ "updated": updated.len() }),
        )),
    ))
}

async fn delete_case(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(p): Path<CaseParams>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    check_project(&state, &p.project_id, &session).await?;
    let deleted = delete_case_by_id(&state.pool(), &p.case_id, &p.requirement_id).await?;
    let Some(deleted) = deleted else {
        return Err(AppError::not_found("CASE_NOT_FOUND"));
    };
    Ok(ok_val(json!({ "id": deleted.id })))
}

// ---------- router ----------

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/requirements/{requirementId}/runs",
            axum::routing::get(list_runs).post(create_run_handler),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/runs/takeover",
            axum::routing::post(takeover_run),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/runs/{runId}",
            axum::routing::patch(patch_run),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/runs/{runId}/steps",
            axum::routing::get(list_steps).post(create_step_handler),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/memory",
            axum::routing::get(get_memory_handler).put(put_memory),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/reset",
            axum::routing::post(reset_requirement_handler),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/scenarios",
            axum::routing::get(list_scenarios_handler).put(put_scenario),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/scenarios/{scenarioId}/status",
            axum::routing::patch(patch_scenario_status),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/scenarios/{scenarioId}",
            axum::routing::delete(delete_scenario),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/cases",
            axum::routing::get(list_cases).put(put_case),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/cases/sync-results",
            axum::routing::post(sync_case_results),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/cases/{caseId}/status",
            axum::routing::patch(patch_case_status),
        )
        .route(
            "/projects/{projectId}/requirements/{requirementId}/cases/{caseId}",
            axum::routing::delete(delete_case),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // 三态反序列化：本仓最容易翻车的一类 bug。
    // 扁平 `Option<T>` 会把「字段缺失」和「显式 null」压成同一个 None，
    // 于是「清空」被当成「不更新」，旧值残留。harness 有 6 个这类字段。
    // ------------------------------------------------------------------

    #[test]
    fn context_snapshot_is_tri_state() {
        // 缺失 → update 路径跳过该列 / create 路径 500
        let b: MemoryBody = serde_json::from_str("{}").unwrap();
        assert_eq!(b.snapshot, None);
        // 显式 null → 落 jsonb 'null'（列非空，不是 SQL NULL）
        let b: MemoryBody = serde_json::from_str(r#"{"snapshot":null}"#).unwrap();
        assert_eq!(b.snapshot, Some(None));
        // 有值 → 全量覆盖（不是合并）
        let b: MemoryBody = serde_json::from_str(r#"{"snapshot":{"a":1}}"#).unwrap();
        assert_eq!(b.snapshot, Some(Some(serde_json::json!({"a": 1}))));
        // t.Any() 不限类型：标量 / 数组同样合法
        let b: MemoryBody = serde_json::from_str(r#"{"snapshot":"s"}"#).unwrap();
        assert_eq!(b.snapshot, Some(Some(Value::String("s".into()))));
    }

    #[test]
    fn scenario_description_is_tri_state() {
        let b: ScenarioBody = serde_json::from_str(r#"{"key":"k","title":"t"}"#).unwrap();
        assert_eq!(b.description, None); // 保留原值
        let b: ScenarioBody =
            serde_json::from_str(r#"{"key":"k","title":"t","description":null}"#).unwrap();
        assert_eq!(b.description, Some(None)); // 置 NULL
        let b: ScenarioBody =
            serde_json::from_str(r#"{"key":"k","title":"t","description":"d"}"#).unwrap();
        assert_eq!(b.description, Some(Some("d".to_string())));
    }

    #[test]
    fn case_optional_text_fields_are_tri_state() {
        let b: CaseBody = serde_json::from_str(r#"{"title":"t"}"#).unwrap();
        assert_eq!(b.steps, None);
        assert_eq!(b.expected, None);
        assert_eq!(b.local_path, None);
        let b: CaseBody = serde_json::from_str(
            r#"{"title":"t","steps":null,"expected":null,"localPath":null}"#,
        )
        .unwrap();
        assert_eq!(b.steps, Some(None));
        assert_eq!(b.expected, Some(None));
        assert_eq!(b.local_path, Some(None));
    }

    // ------------------------------------------------------------------
    // camelCase 映射：漏一个 rename 就会静默退化成「字段缺失」，
    // 且因为都是可选字段，编译期与运行期都不报错，只有对拍能抓到。
    // ------------------------------------------------------------------

    #[test]
    fn camel_case_fields_are_mapped() {
        let b: RunStatusBody =
            serde_json::from_str(r#"{"status":"finished","endReason":"里程碑收尾"}"#).unwrap();
        assert_eq!(b.end_reason.as_deref(), Some("里程碑收尾"));

        let b: CaseBody = serde_json::from_str(
            r#"{"title":"t","scenarioId":"s1","scenarioKey":"k1","localPath":"a.spec.ts","executionPlan":"manual","sortOrder":7}"#,
        )
        .unwrap();
        assert_eq!(b.scenario_id.as_deref(), Some("s1"));
        assert_eq!(b.scenario_key.as_deref(), Some("k1"));
        assert_eq!(b.local_path, Some(Some("a.spec.ts".to_string())));
        assert_eq!(b.execution_plan.as_deref(), Some("manual"));
        assert_eq!(b.sort_order, Some(7.0));

        let b: CaseStatusBody = serde_json::from_str(
            r#"{"status":"failed","actualResult":"断言失败","executedBy":"agent"}"#,
        )
        .unwrap();
        assert_eq!(b.actual_result, Some(Some("断言失败".to_string())));
        assert_eq!(b.executed_by.as_deref(), Some("agent"));

        let b: SyncResultsBody = serde_json::from_str(
            r#"{"results":[{"id":"c1","status":"passed","actualResult":"ok","executedBy":"manual","localPath":"p"}]}"#,
        )
        .unwrap();
        assert_eq!(b.results[0].actual_result, Some(Some("ok".to_string())));
        assert_eq!(b.results[0].executed_by.as_deref(), Some("manual"));
        assert_eq!(b.results[0].local_path, Some(Some("p".to_string())));
    }

    // ------------------------------------------------------------------
    // 枚举白名单：库里是 PG 原生 enum，脏值直落会撞
    // `invalid input value for enum` → 500，旧端是 422。
    // ------------------------------------------------------------------

    #[test]
    fn enum_whitelists_match_pg_types() {
        assert!(ensure_enum("status", "running", RUN_STATUSES).is_ok());
        assert!(ensure_enum("status", "RUNNING", RUN_STATUSES).is_err()); // 大小写敏感
        assert!(ensure_enum("kind", "ask_user", STEP_KINDS).is_ok());
        assert!(ensure_enum("kind", "askUser", STEP_KINDS).is_err());
        assert!(ensure_enum("status", "waived", SCENARIO_STATUSES).is_ok());
        assert!(ensure_enum("status", "passed", SCENARIO_STATUSES).is_err()); // 场景没有 passed
        assert!(ensure_enum("status", "passing", CASE_STATUSES).is_err()); // 用例没有 passing
        assert!(ensure_enum("kind", "e2e", CASE_KINDS).is_ok());
        assert!(ensure_enum("executionPlan", "auto", EXECUTION_PLANS).is_ok());
        assert!(ensure_enum("executedBy", "agent", EXECUTED_BYS).is_ok());
        // 缺省一律放行（由各 handler 决定是否必填）
        assert!(ensure_enum_opt("status", &None, RUN_STATUSES).is_ok());
        assert!(ensure_enum_opt("status", &Some("x".into()), RUN_STATUSES).is_err());
    }

    /// TypeBox 的 minLength/maxLength 按 UTF-16 code unit 计，不是字节也不是 char。
    #[test]
    fn length_check_uses_utf16_code_units() {
        assert!(ensure_len("title", "", 1, 300).is_err());
        assert!(ensure_len("title", "a", 1, 300).is_ok());
        // 中文 1 char = 1 code unit（3 字节）→ 按字节算会误判
        assert!(ensure_len("k", &"中".repeat(120), 1, 120).is_ok());
        assert!(ensure_len("k", &"中".repeat(121), 1, 120).is_err());
        // emoji 1 char = 2 code unit（代理对）→ 按 char 算会误判
        assert_eq!("🌱".encode_utf16().count(), 2);
        assert!(ensure_len("t", &"🌱".repeat(150), 1, 300).is_ok());
        assert!(ensure_len("t", &"🌱".repeat(151), 1, 300).is_err());
    }

    /// sortOrder 是写库 Int 列：Prisma 向零截断，int4 越界抛异常 → 500。
    /// 注意与分页 `take` 区分（那个实测不校验 Int，见 core/js_number.rs）。
    #[test]
    fn sort_order_truncates_toward_zero() {
        assert_eq!(sort_order_int(None).unwrap(), None);
        assert_eq!(sort_order_int(Some(3.7)).unwrap(), Some(3));
        assert_eq!(sort_order_int(Some(-3.7)).unwrap(), Some(-3));
        assert_eq!(sort_order_int(Some(-0.5)).unwrap(), Some(0));
        assert_eq!(sort_order_int(Some(1.5e3)).unwrap(), Some(1500));
        assert!(sort_order_int(Some(2147483648.0)).is_err());
        assert!(sort_order_int(Some(-2147483649.0)).is_err());
    }

    /// 旧端 `!scenarioId && body.scenarioKey` 是 JS 真值判断：空串等同缺省。
    /// 若把 `""` 当有效值，会拿空 key 去查场景而误报 404。
    #[test]
    fn truthy_treats_empty_string_as_absent() {
        assert_eq!(truthy(&None), None);
        assert_eq!(truthy(&Some(String::new())), None);
        assert_eq!(truthy(&Some(" ".to_string())), Some(" ")); // 纯空格在 JS 里是真值
        assert_eq!(truthy(&Some("x".to_string())), Some("x"));
    }
}
