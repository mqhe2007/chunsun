//! 春笋 harness 域仓储层（1:1 移植自 `harnessRepository.ts`）。
//!
//! 覆盖 Run / Step / Context / Scenario / Case 的 CRUD，以及撞锁、完成硬条件门禁、
//! reset（幂等）等编排逻辑。所有实体主键用 `nanoid(12)`（对齐 Prisma `@default(nanoid(12))`）。
//!
//! **Prisma @updatedAt 客户端层陷阱**：`updated_at` 列在 DDL 里没有默认值，所有 INSERT/UPDATE
//! 都必须显式写 `NOW()`，否则违反 NOT NULL 或陈旧（projectContexts 域已踩过相反方向的坑——那里是
//! 空补丁不刷新；harness 这里走 Prisma 默认行为，每次更新都刷新 updatedAt）。

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::ids::nanoid;

// ⚠️ harness 五张表的 status / kind / execution_plan / executed_by 是 **PostgreSQL 原生
// enum**（"RunStatus" / "StepKind" / "ScenarioStatus" / "TestCaseKind" /
// "TestCaseExecutionPlan" / "TestCaseStatus" / "TestCaseExecutedBy"），不是 TEXT。
// sqlx 不会把 pg enum 隐式解码成 String，读侧必须 `col::text AS col`，
// 写侧必须 `$n::"EnumType"`，否则运行期 500（decode/encode mismatched types）。
const RUN_COLS: &str = "id, requirement_id, project_id, index, status::text AS status, end_reason, started_at, ended_at, created_at, updated_at";
const STEP_COLS: &str =
    "id, run_id, seq, kind::text AS kind, summary, detail, artifacts, created_at";
const SCENARIO_COLS: &str =
    "id, requirement_id, project_id, key, title, description, status::text AS status, sort_order, created_at, updated_at";
const CASE_COLS: &str = "id, requirement_id, project_id, scenario_id, title, kind::text AS kind, steps, expected, local_path, execution_plan::text AS execution_plan, status::text AS status, actual_result, executed_at, executed_by::text AS executed_by, sort_order, created_at, updated_at";
const CONTEXT_COLS: &str = "id, requirement_id, project_id, snapshot, updated_at";

// ---------- Row structs ----------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RunRow {
    pub id: String,
    pub requirement_id: String,
    pub project_id: String,
    pub index: i32,
    pub status: String,
    pub end_reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StepRow {
    pub id: String,
    pub run_id: String,
    pub seq: i32,
    pub kind: String,
    pub summary: String,
    pub detail: Option<String>,
    pub artifacts: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScenarioRow {
    pub id: String,
    pub requirement_id: String,
    pub project_id: String,
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CaseRow {
    pub id: String,
    pub requirement_id: String,
    pub project_id: String,
    pub scenario_id: String,
    pub title: String,
    pub kind: String,
    pub steps: Option<String>,
    pub expected: Option<String>,
    pub local_path: Option<String>,
    pub execution_plan: String,
    pub status: String,
    pub actual_result: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
    pub executed_by: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ContextRow {
    pub id: String,
    pub requirement_id: String,
    pub project_id: String,
    pub snapshot: Value,
    pub updated_at: DateTime<Utc>,
}

// ---------- DTO（对齐旧端 serializeRun/Step/Scenario/Case） ----------

pub fn run_dto(r: &RunRow) -> Value {
    json!({
        "id": r.id,
        "requirementId": r.requirement_id,
        "projectId": r.project_id,
        "index": r.index,
        "status": r.status,
        "endReason": r.end_reason,
        "startedAt": r.started_at,
        "endedAt": r.ended_at,
        "createdAt": r.created_at,
        "updatedAt": r.updated_at,
    })
}

pub fn step_dto(s: &StepRow) -> Value {
    json!({
        "id": s.id,
        "runId": s.run_id,
        "seq": s.seq,
        "kind": s.kind,
        "summary": s.summary,
        "detail": s.detail,
        "artifacts": s.artifacts,
        "createdAt": s.created_at,
    })
}

pub fn scenario_dto(s: &ScenarioRow) -> Value {
    json!({
        "id": s.id,
        "requirementId": s.requirement_id,
        "projectId": s.project_id,
        "key": s.key,
        "title": s.title,
        "description": s.description,
        "status": s.status,
        "sortOrder": s.sort_order,
        "createdAt": s.created_at,
        "updatedAt": s.updated_at,
    })
}

pub fn case_dto(c: &CaseRow) -> Value {
    json!({
        "id": c.id,
        "requirementId": c.requirement_id,
        "projectId": c.project_id,
        "scenarioId": c.scenario_id,
        "title": c.title,
        "kind": c.kind,
        "steps": c.steps,
        "expected": c.expected,
        "localPath": c.local_path,
        "executionPlan": c.execution_plan,
        "status": c.status,
        "actualResult": c.actual_result,
        "executedAt": c.executed_at,
        "executedBy": c.executed_by,
        "sortOrder": c.sort_order,
        "createdAt": c.created_at,
        "updatedAt": c.updated_at,
    })
}

pub fn context_dto(c: &ContextRow) -> Value {
    json!({
        "id": c.id,
        "requirementId": c.requirement_id,
        "projectId": c.project_id,
        "snapshot": c.snapshot,
        "updatedAt": c.updated_at,
    })
}

// ---------- Run ----------

/// 开新 Run：需求内 index 递增，状态 running；同事务把 Requirement.status 投影为 running。
pub async fn create_run(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<RunRow, AppError> {
    let mut tx = pool.begin().await?;
    let last: Option<(i32,)> =
        sqlx::query_as("SELECT COALESCE(MAX(index), 0) AS m FROM run WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    let next_index = last.map(|(m,)| m).unwrap_or(0) + 1;
    let id = nanoid(12);
    let row = sqlx::query_as::<_, RunRow>(&format!(
        "INSERT INTO run (id, requirement_id, project_id, index, status, started_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'running'::\"RunStatus\", NOW(), NOW()) \
         RETURNING {RUN_COLS}"
    ))
    .bind(&id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(next_index)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE requirement SET status = 'running'::\"RequirementStatus\", updated_at = NOW() WHERE id = $1",
    )
    .bind(requirement_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn list_runs_by_requirement(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Vec<RunRow>, AppError> {
    let rows = sqlx::query_as::<_, RunRow>(&format!(
        "SELECT {RUN_COLS} FROM run WHERE requirement_id = $1 AND project_id = $2 ORDER BY index ASC"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_run_by_id(
    pool: &PgPool,
    run_id: &str,
    requirement_id: &str,
) -> Result<Option<RunRow>, AppError> {
    let row = sqlx::query_as::<_, RunRow>(&format!(
        "SELECT {RUN_COLS} FROM run WHERE id = $1 AND requirement_id = $2"
    ))
    .bind(run_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 撞锁接管：把该需求当前 running 的 Run 置 finished（end_reason=接管），返回被接管的 Run（无则 null）。
pub async fn takeover_running_run(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Option<RunRow>, AppError> {
    let running = sqlx::query_as::<_, RunRow>(&format!(
        "SELECT {RUN_COLS} FROM run WHERE requirement_id = $1 AND project_id = $2 AND status = 'running'::\"RunStatus\" LIMIT 1"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    let Some(running) = running else {
        return Ok(None);
    };
    let updated = sqlx::query_as::<_, RunRow>(&format!(
        "UPDATE run SET status = 'finished'::\"RunStatus\", end_reason = COALESCE(end_reason, '被接管'), ended_at = NOW(), updated_at = NOW() \
         WHERE id = $1 RETURNING {RUN_COLS}"
    ))
    .bind(&running.id)
    .fetch_one(pool)
    .await?;
    Ok(Some(updated))
}

/// Run 状态迁移（completed / finished / abandoned），同事务回写 Requirement.status 投影。
/// 投影映射：finished → running（需求仍在推进，等待下一轮）；abandoned → abandoned；completed → completed。
/// completed 联动缺陷 open/processing → resolved。
pub async fn set_run_status(
    pool: &PgPool,
    run_id: &str,
    requirement_id: &str,
    status: &str,
    end_reason: Option<&str>,
) -> Result<Option<RunRow>, AppError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<_, RunRow>(&format!(
        "SELECT {RUN_COLS} FROM run WHERE id = $1 AND requirement_id = $2"
    ))
    .bind(run_id)
    .bind(requirement_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(existing) = existing else {
        // run 不存在：无任何写入；tx 随函数返回而 Drop 自动回滚。
        return Ok(None);
    };
    let row = sqlx::query_as::<_, RunRow>(&format!(
        "UPDATE run SET \
           status = $3::\"RunStatus\", \
           ended_at = CASE WHEN $3 IN ('completed', 'finished', 'abandoned') THEN NOW() ELSE ended_at END, \
           end_reason = CASE WHEN $3 IN ('finished', 'abandoned') THEN $4 ELSE end_reason END, \
           updated_at = NOW() \
         WHERE id = $1 RETURNING {RUN_COLS}"
    ))
    .bind(run_id)
    .bind(requirement_id)
    .bind(status)
    .bind(end_reason)
    .fetch_one(&mut *tx)
    .await?;
    let projected = match status {
        "running" => "running",
        "completed" => "completed",
        "finished" => "running",
        "abandoned" => "abandoned",
        _ => status,
    };
    sqlx::query(
        "UPDATE requirement SET status = $1::\"RequirementStatus\", updated_at = NOW() WHERE id = $2",
    )
    .bind(projected)
    .bind(requirement_id)
    .execute(&mut *tx)
    .await?;
    if status == "completed" {
        sqlx::query(
            "UPDATE defect SET status = 'resolved'::\"DefectStatus\", updated_at = NOW() \
             WHERE requirement_id = $1 AND status::text IN ('open', 'processing')",
        )
        .bind(requirement_id)
        .execute(&mut *tx)
        .await?;
    }
    let _ = existing;
    tx.commit().await?;
    Ok(Some(row))
}

// ---------- Step ----------

/// 追加 Step：seq 自动 = run 内 max+1；run 不存在返回 None。
pub async fn create_step(
    pool: &PgPool,
    run_id: &str,
    requirement_id: &str,
    kind: &str,
    summary: &str,
    detail: Option<&str>,
    artifacts: &Option<Value>,
) -> Result<Option<StepRow>, AppError> {
    let mut tx = pool.begin().await?;
    let run: Option<(String,)> =
        sqlx::query_as("SELECT id FROM run WHERE id = $1 AND requirement_id = $2")
            .bind(run_id)
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(_) = run else {
        // run 不存在：无写入；tx 随函数返回而 Drop 自动回滚。
        return Ok(None);
    };
    let last: Option<(i32,)> = sqlx::query_as("SELECT COALESCE(MAX(seq), 0) AS m FROM step WHERE run_id = $1")
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?;
    let next_seq = last.map(|(m,)| m).unwrap_or(0) + 1;
    let id = nanoid(12);
    let row = sqlx::query_as::<_, StepRow>(&format!(
        "INSERT INTO step (id, run_id, seq, kind, summary, detail, artifacts, created_at) \
         VALUES ($1, $2, $3, $4::\"StepKind\", $5, $6, $7, NOW()) \
         RETURNING {STEP_COLS}"
    ))
    .bind(&id)
    .bind(run_id)
    .bind(next_seq)
    .bind(kind)
    .bind(summary)
    .bind(detail)
    .bind(artifacts)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(row))
}

pub async fn list_steps_by_run(pool: &PgPool, run_id: &str) -> Result<Vec<StepRow>, AppError> {
    let rows = sqlx::query_as::<_, StepRow>(&format!(
        "SELECT {STEP_COLS} FROM step WHERE run_id = $1 ORDER BY seq ASC"
    ))
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------- Context ----------

pub async fn get_context(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Option<ContextRow>, AppError> {
    let row = sqlx::query_as::<_, ContextRow>(&format!(
        "SELECT {CONTEXT_COLS} FROM context WHERE requirement_id = $1 AND project_id = $2"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// upsert Context：snapshot 全量覆盖（客户端传 §4 结构全量）。
///
/// `snapshot` 三态复刻旧端 `prisma.context.update/create({ data: { snapshot } })`：
/// - `None`（字段缺失 → JS undefined）：update 路径退化为空 data，Prisma 不发 UPDATE，
///   连 `@updatedAt` 都不动 → 这里直接回读原行；create 路径必填缺参 → 500。
/// - `Some(None)`（显式 null）：列是 `Json` 非空，落 jsonb `'null'`。
/// - `Some(Some(v))`：正常写入。
pub async fn upsert_context(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
    snapshot: Option<Option<&Value>>,
) -> Result<ContextRow, AppError> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM context WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    // 显式 null 与「有值」都要落库，缺失才走各自的退化分支。
    let null_json = Value::Null;
    if let Some((id,)) = existing {
        let Some(value) = snapshot else {
            // 空 data：Prisma 退化纯读，不刷 updated_at。
            let row = sqlx::query_as::<_, ContextRow>(&format!(
                "SELECT {CONTEXT_COLS} FROM context WHERE id = $1"
            ))
            .bind(&id)
            .fetch_one(pool)
            .await?;
            return Ok(row);
        };
        let row = sqlx::query_as::<_, ContextRow>(&format!(
            "UPDATE context SET snapshot = $2, updated_at = NOW() WHERE id = $1 RETURNING {CONTEXT_COLS}"
        ))
        .bind(&id)
        .bind(value.unwrap_or(&null_json))
        .fetch_one(pool)
        .await?;
        Ok(row)
    } else {
        let Some(value) = snapshot else {
            return Err(AppError::internal(
                "Argument `snapshot` is missing.".to_string(),
            ));
        };
        let id = nanoid(12);
        let row = sqlx::query_as::<_, ContextRow>(&format!(
            "INSERT INTO context (id, requirement_id, project_id, snapshot, updated_at) \
             VALUES ($1, $2, $3, $4, NOW()) RETURNING {CONTEXT_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(value.unwrap_or(&null_json))
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}

// ---------- Scenario ----------

#[derive(Debug, Default)]
pub struct UpsertScenarioInput<'a> {
    pub key: &'a str,
    pub title: &'a str,
    /// 三态：None=key 缺失（更新时保留原值，Prisma 跳过该列）；
    /// Some(None)=显式 null（置 NULL）；Some(Some(v))=设置。
    pub description: Option<Option<&'a str>>,
    pub sort_order: Option<i32>,
    pub status: Option<&'a str>,
}

pub async fn upsert_scenario(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
    input: &UpsertScenarioInput<'_>,
) -> Result<ScenarioRow, AppError> {
    let existing: Option<(String, i32, String)> = sqlx::query_as(
        "SELECT id, sort_order, status::text AS status FROM scenario WHERE requirement_id = $1 AND key = $2",
    )
    .bind(requirement_id)
    .bind(input.key)
    .fetch_optional(pool)
    .await?;
    if let Some((id, existing_sort, existing_status)) = existing {
        // description 三态：`$9` 为 false 时整列不动（对齐 Prisma 对 undefined 的跳过语义）。
        let desc_provided = input.description.is_some();
        let desc_value = input.description.flatten();
        let row = sqlx::query_as::<_, ScenarioRow>(&format!(
            "UPDATE scenario SET \
               title = $3, \
               description = CASE WHEN $9 THEN $4 ELSE description END, \
               sort_order = COALESCE($5, $6), \
               status = COALESCE($7::\"ScenarioStatus\", $8::\"ScenarioStatus\"), \
               updated_at = NOW() \
             WHERE id = $1 RETURNING {SCENARIO_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(input.title)
        .bind(desc_value)
        .bind(input.sort_order)
        .bind(existing_sort)
        .bind(input.status)
        .bind(&existing_status)
        .bind(desc_provided)
        .fetch_one(pool)
        .await?;
        Ok(row)
    } else {
        let id = nanoid(12);
        let row = sqlx::query_as::<_, ScenarioRow>(&format!(
            "INSERT INTO scenario (id, requirement_id, project_id, key, title, description, sort_order, status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 0), COALESCE($8::\"ScenarioStatus\", 'pending'::\"ScenarioStatus\"), NOW(), NOW()) \
             RETURNING {SCENARIO_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(input.key)
        .bind(input.title)
        .bind(input.description.flatten())
        .bind(input.sort_order)
        .bind(input.status)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}

pub async fn list_scenarios(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Vec<ScenarioRow>, AppError> {
    let rows = sqlx::query_as::<_, ScenarioRow>(&format!(
        "SELECT {SCENARIO_COLS} FROM scenario WHERE requirement_id = $1 AND project_id = $2 ORDER BY sort_order ASC"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn set_scenario_status(
    pool: &PgPool,
    scenario_id: &str,
    requirement_id: &str,
    status: &str,
) -> Result<Option<ScenarioRow>, AppError> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM scenario WHERE id = $1 AND requirement_id = $2")
            .bind(scenario_id)
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let Some((id,)) = existing else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, ScenarioRow>(&format!(
        "UPDATE scenario SET status = $3::\"ScenarioStatus\", updated_at = NOW() WHERE id = $1 RETURNING {SCENARIO_COLS}"
    ))
    .bind(&id)
    .bind(requirement_id)
    .bind(status)
    .fetch_one(pool)
    .await?;
    Ok(Some(row))
}

pub async fn delete_scenario_by_id(
    pool: &PgPool,
    scenario_id: &str,
    requirement_id: &str,
) -> Result<Option<ScenarioRow>, AppError> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM scenario WHERE id = $1 AND requirement_id = $2")
            .bind(scenario_id)
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let Some((id,)) = existing else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, ScenarioRow>(&format!(
        "DELETE FROM scenario WHERE id = $1 RETURNING {SCENARIO_COLS}"
    ))
    .bind(&id)
    .fetch_one(pool)
    .await?;
    Ok(Some(row))
}

// ---------- Case ----------

#[derive(Debug, Default)]
pub struct UpsertCaseInput<'a> {
    pub scenario_id: &'a str,
    pub id: Option<&'a str>,
    pub title: &'a str,
    pub kind: Option<&'a str>,
    /// 三态：None=key 缺失（更新时保留原值）；Some(None)=显式 null（置空）；Some(Some(v))=设置。
    pub steps: Option<Option<String>>,
    pub expected: Option<Option<String>>,
    pub local_path: Option<Option<String>>,
    pub execution_plan: Option<&'a str>,
    pub sort_order: Option<i32>,
}

/// 场景必须属于同一需求；id 存在则更新否则创建。返回 None 表示场景不存在。
pub async fn upsert_case(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
    input: &UpsertCaseInput<'_>,
) -> Result<Option<CaseRow>, AppError> {
    let scenario: Option<(String,)> =
        sqlx::query_as("SELECT id FROM scenario WHERE id = $1 AND requirement_id = $2")
            .bind(input.scenario_id)
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let Some(_) = scenario else {
        return Ok(None);
    };
    if let Some(case_id) = input.id {
        let existing: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, steps, expected, local_path FROM test_case WHERE id = $1 AND requirement_id = $2",
        )
        .bind(case_id)
        .bind(requirement_id)
        .fetch_optional(pool)
        .await?;
        let Some((id, ex_steps, ex_expected, ex_local)) = existing else {
            return Ok(None);
        };
        let steps = match &input.steps {
            None => ex_steps,
            Some(v) => v.clone(),
        };
        let expected = match &input.expected {
            None => ex_expected,
            Some(v) => v.clone(),
        };
        let local_path = match &input.local_path {
            None => ex_local,
            Some(v) => v.clone(),
        };
        let row = sqlx::query_as::<_, CaseRow>(&format!(
            "UPDATE test_case SET \
               scenario_id = $3, title = $4, \
               kind = COALESCE($5::\"TestCaseKind\", kind), \
               steps = $6, expected = $7, local_path = $8, \
               execution_plan = COALESCE($9::\"TestCaseExecutionPlan\", execution_plan), \
               sort_order = COALESCE($10, sort_order), \
               updated_at = NOW() \
             WHERE id = $1 RETURNING {CASE_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(input.scenario_id)
        .bind(input.title)
        .bind(input.kind)
        .bind(steps)
        .bind(expected)
        .bind(local_path)
        .bind(input.execution_plan)
        .bind(input.sort_order)
        .fetch_one(pool)
        .await?;
        Ok(Some(row))
    } else {
        let id = nanoid(12);
        let steps = input.steps.clone().flatten();
        let expected = input.expected.clone().flatten();
        let local_path = input.local_path.clone().flatten();
        let row = sqlx::query_as::<_, CaseRow>(&format!(
            "INSERT INTO test_case \
               (id, requirement_id, project_id, scenario_id, title, kind, steps, expected, local_path, execution_plan, status, sort_order, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, COALESCE($6::\"TestCaseKind\", 'e2e'::\"TestCaseKind\"), $7, $8, $9, COALESCE($10::\"TestCaseExecutionPlan\", 'auto'::\"TestCaseExecutionPlan\"), 'pending'::\"TestCaseStatus\", COALESCE($11, 0), NOW(), NOW()) \
             RETURNING {CASE_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(input.scenario_id)
        .bind(input.title)
        .bind(input.kind)
        .bind(steps)
        .bind(expected)
        .bind(local_path)
        .bind(input.execution_plan)
        .bind(input.sort_order)
        .fetch_one(pool)
        .await?;
        Ok(Some(row))
    }
}

pub async fn list_cases_by_requirement(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Vec<CaseRow>, AppError> {
    let rows = sqlx::query_as::<_, CaseRow>(&format!(
        "SELECT {CASE_COLS} FROM test_case WHERE requirement_id = $1 AND project_id = $2 ORDER BY sort_order ASC"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_case_by_id(
    pool: &PgPool,
    case_id: &str,
    requirement_id: &str,
) -> Result<Option<CaseRow>, AppError> {
    let row = sqlx::query_as::<_, CaseRow>(&format!(
        "SELECT {CASE_COLS} FROM test_case WHERE id = $1 AND requirement_id = $2"
    ))
    .bind(case_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_case_status(
    pool: &PgPool,
    case_id: &str,
    requirement_id: &str,
    status: &str,
    // 三态：None=未传（保留原值）；Some(None)=显式 null；Some(Some(v))=设置。
    actual_result: Option<Option<String>>,
    // 未传时回落到 default_executed_by（PATCH 用 manual，sync 用 agent）。
    executed_by: Option<&str>,
    default_executed_by: &str,
    // 三态：None=未传（保留原值）；Some(None)=显式 null；Some(Some(v))=设置。
    local_path: Option<Option<String>>,
) -> Result<Option<CaseRow>, AppError> {
    let existing: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, actual_result, local_path FROM test_case WHERE id = $1 AND requirement_id = $2",
    )
    .bind(case_id)
    .bind(requirement_id)
    .fetch_optional(pool)
    .await?;
    let Some((id, ex_actual, ex_local)) = existing else {
        return Ok(None);
    };
    let actual = match actual_result {
        None => ex_actual,
        Some(v) => v,
    };
    let local = match local_path {
        None => ex_local,
        Some(v) => v,
    };
    let row = sqlx::query_as::<_, CaseRow>(&format!(
        "UPDATE test_case SET \
           status = $3::\"TestCaseStatus\", \
           actual_result = $4, \
           executed_by = COALESCE($5, $6)::\"TestCaseExecutedBy\", \
           executed_at = NOW(), \
           local_path = $7, \
           updated_at = NOW() \
         WHERE id = $1 RETURNING {CASE_COLS}"
    ))
    .bind(&id)
    .bind(requirement_id)
    .bind(status)
    .bind(actual)
    .bind(executed_by)
    .bind(default_executed_by)
    .bind(local)
    .fetch_one(pool)
    .await?;
    Ok(Some(row))
}

pub async fn delete_case_by_id(
    pool: &PgPool,
    case_id: &str,
    requirement_id: &str,
) -> Result<Option<CaseRow>, AppError> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM test_case WHERE id = $1 AND requirement_id = $2")
            .bind(case_id)
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let Some((id,)) = existing else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, CaseRow>(&format!(
        "DELETE FROM test_case WHERE id = $1 RETURNING {CASE_COLS}"
    ))
    .bind(&id)
    .fetch_one(pool)
    .await?;
    Ok(Some(row))
}

// ---------- 完成硬条件 / reset ----------

/// completed 硬条件：所有场景 passing/waived 且无 open decisions。
pub async fn check_completion_gate(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<(bool, Vec<Value>), AppError> {
    let scenarios: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT key, title, status::text AS status FROM scenario WHERE requirement_id = $1 AND project_id = $2",
    )
    .bind(requirement_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let ctx: Option<(Value,)> =
        sqlx::query_as("SELECT snapshot FROM context WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let mut blockers: Vec<Value> = Vec::new();
    for (key, title, status) in &scenarios {
        if status != "passing" && status != "waived" {
            blockers.push(json!({
                "code": "SCENARIO_NOT_PASSING",
                "detail": format!("场景「{title}」状态为 {status}，需 passing 或 waived"),
            }));
            let _ = key;
        }
    }
    if let Some((snapshot,)) = ctx {
        let open_decisions = snapshot
            .get("openDecisions")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if open_decisions > 0 {
            blockers.push(json!({
                "code": "OPEN_DECISIONS",
                "detail": format!("存在 {open_decisions} 个未决 open decision"),
            }));
        }
    }
    Ok((blockers.is_empty(), blockers))
}

/// reset（幂等）：清 Context 工作记忆（保留 requirementSnapshot）+ Scenario/Case 全部重置 pending + 开新 Run。
pub async fn reset_requirement(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<RunRow, AppError> {
    let mut tx = pool.begin().await?;
    // 取 id 与 snapshot 两列：id 用于 UPDATE 定位，snapshot 用于保留 requirementSnapshot。
    let ctx: Option<(String, Value)> =
        sqlx::query_as("SELECT id, snapshot FROM context WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    let requirement_snapshot = ctx
        .as_ref()
        .and_then(|(_, s)| s.get("requirementSnapshot"))
        .cloned();
    let new_snapshot = requirement_snapshot
        .map(|rs| json!({ "requirementSnapshot": rs }))
        .unwrap_or_else(|| json!({}));
    if let Some((id, _)) = ctx {
        sqlx::query("UPDATE context SET snapshot = $2, updated_at = NOW() WHERE id = $1")
            .bind(&id)
            .bind(&new_snapshot)
            .execute(&mut *tx)
            .await?;
    } else {
        let id = nanoid(12);
        sqlx::query(
            "INSERT INTO context (id, requirement_id, project_id, snapshot, updated_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(&new_snapshot)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE scenario SET status = 'pending'::\"ScenarioStatus\", updated_at = NOW() WHERE requirement_id = $1")
        .bind(requirement_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE test_case SET status = 'pending'::\"TestCaseStatus\", actual_result = NULL, executed_at = NULL, executed_by = NULL, updated_at = NOW() WHERE requirement_id = $1",
    )
    .bind(requirement_id)
    .execute(&mut *tx)
    .await?;
    let last: Option<(i32,)> =
        sqlx::query_as("SELECT COALESCE(MAX(index), 0) AS m FROM run WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    let next_index = last.map(|(m,)| m).unwrap_or(0) + 1;
    let run_id = nanoid(12);
    let run = sqlx::query_as::<_, RunRow>(&format!(
        "INSERT INTO run (id, requirement_id, project_id, index, status, started_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'running'::\"RunStatus\", NOW(), NOW()) \
         RETURNING {RUN_COLS}"
    ))
    .bind(&run_id)
    .bind(requirement_id)
    .bind(project_id)
    .bind(next_index)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE requirement SET status = 'running'::\"RequirementStatus\", updated_at = NOW() WHERE id = $1",
    )
    .bind(requirement_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(run)
}
