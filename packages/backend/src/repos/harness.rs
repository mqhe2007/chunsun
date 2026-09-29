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
use crate::repos::project_knowledge::{classify_miss, revision_matches, WriteOutcome};

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
const MEMORY_COLS: &str = "id, requirement_id, project_id, snapshot, revision, updated_at";

/// `upsert_memory` 建行时的插入语句。
///
/// 提成常量是为了让 `memory_insert_sql_is_pinned` 能直接断言它：这条 SQL 曾经把
/// 仲裁目标写成 `(requirement_id, project_id)`，而表上唯一的唯一索引
/// （baseline 的 `context_requirement_id_key`，表从 `context` 改名而来）是
/// **单列 requirement_id**。Postgres 的仲裁索引推断要求列集合与某个唯一索引
/// 完全一致，多列少列都不认，于是首次写入直接抛 42P10
/// 「no unique or exclusion constraint matching the ON CONFLICT specification」，
/// 在路由层就是 500。纯函数测试挡不住这种错，只能把 SQL 本身钉住。
const MEMORY_INSERT_SQL: &str = "INSERT INTO requirement_memory \
     (id, requirement_id, project_id, snapshot, revision, updated_at) \
     VALUES ($1, $2, $3, $4, 1, NOW()) \
     ON CONFLICT (requirement_id) DO NOTHING \
     RETURNING id, requirement_id, project_id, snapshot, revision, updated_at";

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
pub struct MemoryRow {
    pub id: String,
    pub requirement_id: String,
    pub project_id: String,
    pub snapshot: Option<String>,
    pub revision: i32,
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

/// `revision` 是新增的乐观锁版本载体，客户端写回时必须带上。
pub fn memory_dto(c: &MemoryRow) -> Value {
    json!({
        "id": c.id,
        "requirementId": c.requirement_id,
        "projectId": c.project_id,
        "snapshot": c.snapshot,
        "revision": c.revision,
        "updatedAt": c.updated_at,
    })
}

// ---------- Run 并发保护 ----------
//
// `20260928120000_concurrency_guard.sql` 给 `run` 表加了两个约束：
//
// | 约束 | 名字 | 撞上说明 |
// | --- | --- | --- |
// | `UNIQUE (requirement_id) WHERE status = 'running'` | `uq_run_one_running_per_requirement` | 该需求已有 Run 在跑 |
// | `UNIQUE (requirement_id, index)` | `run_requirement_id_index_key` | `MAX(index)+1` 算重了 |
//
// 这两个的**处置完全不同**，而 PG 只给一个错误码（`23505`）。靠 `constraint()` 拿到
// 约束名来分流是唯一可靠的办法——不能只看 `ErrorKind::UniqueViolation`：
// 那是「唯一的索引撞了」，但撞的是哪一个决定了是「报 409 让用户接管」还是「默默重试一次」。

/// 撞上的是哪个唯一约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunUniqueConflict {
    /// 同需求已有 running —— 业务冲突，转 409。
    AlreadyRunning,
    /// `(requirement_id, index)` 撞了 —— 纯粹的计算竞态，重试即可。
    IndexRace,
    /// 别的唯一约束（或拿不到约束名）。调用方原样上抛成 500，不要吞。
    Other,
}

/// 纯函数：把「约束名」映射成处置方式。
///
/// 抽出来是为了可测——`Other` 分支尤其重要：约束名解析不出来时**必须**保守上抛，
/// 若默认当成 `IndexRace` 重试，一个真正的 bug（比如插入违反别的唯一约束）会被
/// 静默重试两次然后变成一个语焉不详的 500，现场线索全丢。
pub fn classify_run_unique(constraint: Option<&str>) -> RunUniqueConflict {
    match constraint {
        Some("uq_run_one_running_per_requirement") => RunUniqueConflict::AlreadyRunning,
        Some("run_requirement_id_index_key") => RunUniqueConflict::IndexRace,
        _ => RunUniqueConflict::Other,
    }
}

/// 从 sqlx 错误里读约束名。
fn unique_constraint(e: &sqlx::Error) -> Option<&str> {
    match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => db.constraint(),
        _ => None,
    }
}

/// 开新 Run：需求内 index 递增，状态 running；同事务把 Requirement.status 投影为 running。
///
/// **并发安全由数据库保证，不再是「先查后插」**：应用层的预检查挪到了 handler
/// （为的是能给出带 `lastActiveAt` 的友好 409），但真正的裁决在这里——
/// 撞 `uq_run_one_running_per_requirement` 时**整个事务已回滚**，
/// 调用方据此报 409；撞 `(requirement_id, index)` 时 index 是本地算出来的、
/// 重算一次即可，故原地重试。
///
/// 「算下一个 index」的串行化锚点是**父行 `requirement`，不是 run 行**。
///
/// 这里曾写成 `SELECT COALESCE(MAX(index), 0) FROM run WHERE requirement_id = $1 FOR UPDATE`，
/// 在 PG 上**每次调用都必挂**（不是并发才暴露）：
///     FOR UPDATE is not allowed with aggregate functions
/// 行锁只能加在表里的实体行上，而 `MAX(...)` 的输出是一行算出来的聚合结果，没有行可锁。
///
/// 语义上也本来就是空的：该需求**一条 run 都没有**时（正是并发首次开 Run 的场景），
/// 「锁住已有的 run 行」锁的是空集，两个事务双双读到空集、双双算出 `index = 1`。
/// 锚点必须换成必然存在的那一行 —— FK `run.requirement_id → requirement.id`
/// （baseline `:900`）保证父行存在，锁住它即可让同需求的开 Run 在此串行化，
/// 后来者读到的是前者**已提交**的结果，`MAX` 于是算出正确的下一个序号。
///
/// 因此 `MAX` 那一句**不带任何锁修饰**：它已经处在父行锁的保护下，
/// 再挂 `FOR UPDATE` 既非法（上面）又多余。
///
/// 锁序（全文件一致，见 `20260928120000_concurrency_guard.sql` 之外的仓库约定）：
/// **先 requirement、后 run**。任何同时动这两张表的事务都必须按这个顺序取锁，
/// 否则与 `set_run_status` 反向成环。
///
/// `index` 重试上限定为 1 次：有了父行锁，`MAX(index)+1` 的竞态窗口已经关闭，
/// 走到重试只可能是「有别的写入路径在动 index」这类异常，多试几次也只是掩盖问题。
pub async fn create_run(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<RunRow, AppError> {
    for attempt in 0..2 {
        let mut tx = pool.begin().await?;
        // 序列化锚点：锁父行 requirement。用 `fetch_one` 而非 `fetch_optional`——
        // 父行不存在说明调用方越过了 `check_requirement`，那是编程错误，
        // 应当显式失败（RowNotFound），而不是让后面那句 INSERT 去撞 FK 报一个更难懂的错。
        sqlx::query("SELECT id FROM requirement WHERE id = $1 FOR UPDATE")
            .bind(requirement_id)
            .fetch_one(&mut *tx)
            .await?;
        // 已在父行锁下，普通聚合即可；**不能**加 FOR UPDATE。
        let last: Option<(i32,)> = sqlx::query_as(
            "SELECT COALESCE(MAX(index), 0) AS m FROM run WHERE requirement_id = $1",
        )
        .bind(requirement_id)
        .fetch_optional(&mut *tx)
        .await?;
        let next_index = last.map(|(m,)| m).unwrap_or(0) + 1;
        let id = nanoid(12);
        let inserted = sqlx::query_as::<_, RunRow>(&format!(
            "INSERT INTO run (id, requirement_id, project_id, index, status, started_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'running'::\"RunStatus\", NOW(), NOW()) \
             RETURNING {RUN_COLS}"
        ))
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(next_index)
        .fetch_one(&mut *tx)
        .await;

        let row = match inserted {
            Ok(row) => row,
            Err(e) => {
                let kind = classify_run_unique(unique_constraint(&e));
                // 事务随 Drop 回滚 —— 无需显式 rollback
                drop(tx);
                return match (kind, attempt) {
                    // index 撞了且还能重试：重算一次
                    (RunUniqueConflict::IndexRace, 0) => continue,
                    // 已有 running：业务冲突，交给调用方转 409
                    (RunUniqueConflict::AlreadyRunning, _) => Err(run_already_running()),
                    (RunUniqueConflict::IndexRace, _) => {
                        Err(AppError::conflict("RUN_ALREADY_RUNNING")
                            .with_message("同一需求并发开 Run，index 分配冲突且重试后仍失败")
                            .with_hint(
                                "稍后重试；若持续出现请检查是否有绕过 create_run 的写入路径",
                            ))
                    }
                    (RunUniqueConflict::Other, _) => Err(e.into()),
                };
            }
        };

        sqlx::query(
            "UPDATE requirement SET status = 'running'::\"RequirementStatus\", updated_at = NOW() WHERE id = $1",
        )
        .bind(requirement_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(row);
    }
    unreachable!("循环要么 return 要么 continue，最多两轮")
}

/// 409 的**兜底构造**（数据库层撞锁时用）。
///
/// 正常路径上 handler 的预检查会先一步拦下并给出 `runId` + `lastActiveAt`，
/// 走到这里说明是两个请求同一刻穿过预检查的竞态——那种情况下拿不到具体是哪条
/// running（它可能正要结束），所以 data 里只给 requirementId，不给 runId。
/// 形状与预检查那条**刻意保持一致**（同码同状态），CLI 只按 `code` 分流。
pub fn run_already_running() -> AppError {
    AppError::conflict("RUN_ALREADY_RUNNING")
        .with_hint("该需求已有 Run 在跑；如需接管（僵尸 Run 人工接管），请调用 takeover 端点")
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

/// 接管并开新 Run —— **必须是一个事务**。
///
/// 这里原本是一个独立的 `takeover_running_run`，由 handler 连着 `create_run`
/// 顺序调用。那留下了两个窗口，都在 2026-09-28 的并发改造里被合掉：
///
/// 1. 接管提交后、新 Run 插入前是一个空档，此时若另一个请求也来接管，它会读到
///    「没有 running」从而直接开新 Run，于是两条并存。虽然
///    `uq_run_one_running_per_requirement` 仍能拦住最终的两条 running，但那条
///    请求会拿到 409 而不是完成接管 —— 对一个**明确要求接管**的调用方来说，
///    409 是错的答案。
/// 2. 原实现自己也没有事务：「SELECT running → UPDATE 它」跨两条语句且无行锁，
///    两个并发 takeover 会同时读到同一条 running、双双把它置 finished，
///    然后各自去开新 Run。
///
/// 这里把两步放进同一事务：行锁一直持有到新 Run 插入并提交，
/// 并发调用方要么看到旧 Run、要么看到新 Run，永远看不到中间的「空档」。
///
/// 锁序与 `create_run` 一致：**先锁父行 requirement，再动 run 行**。
/// 父行锁是主序列化手段（同需求的开 Run / 接管在此排队），
/// 下面第 1) 步那条 `FOR UPDATE` 退化为兜底 —— 它锁的是**已有的 run 行**，
/// 在「一条 run 都没有」时锁不到任何东西，单靠它序列化不了首次并发。
pub async fn takeover_and_create_run(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<(Option<RunRow>, RunRow), AppError> {
    let mut tx = pool.begin().await?;

    // 0) 序列化锚点：锁父行。语义同 `create_run` 里那一句。
    sqlx::query("SELECT id FROM requirement WHERE id = $1 FOR UPDATE")
        .bind(requirement_id)
        .fetch_one(&mut *tx)
        .await?;

    // 1) 找到并接管当前 running（若有）
    let running = sqlx::query_as::<_, RunRow>(&format!(
        "SELECT {RUN_COLS} FROM run WHERE requirement_id = $1 AND project_id = $2 AND status = 'running'::\"RunStatus\" \
         ORDER BY index DESC LIMIT 1 FOR UPDATE"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_optional(&mut *tx)
    .await?;

    let taken = match &running {
        Some(r) => Some(
            sqlx::query_as::<_, RunRow>(&format!(
                "UPDATE run SET status = 'finished'::\"RunStatus\", end_reason = COALESCE(end_reason, '被接管'), ended_at = NOW(), updated_at = NOW() \
                 WHERE id = $1 RETURNING {RUN_COLS}"
            ))
            .bind(&r.id)
            .fetch_one(&mut *tx)
            .await?,
        ),
        None => None,
    };

    // 2) 同一事务内开新 Run。index 在父行锁下重算 —— **不带** `FOR UPDATE`
    //    （聚合上不允许加锁，且已有父行锁保护），此刻「同需求至多一条 running」
    //    已被上一步保证。
    let last: Option<(i32,)> =
        sqlx::query_as("SELECT COALESCE(MAX(index), 0) AS m FROM run WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    let next_index = last.map(|(m,)| m).unwrap_or(0) + 1;

    let run = sqlx::query_as::<_, RunRow>(&format!(
        "INSERT INTO run (id, requirement_id, project_id, index, status, started_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'running'::\"RunStatus\", NOW(), NOW()) \
         RETURNING {RUN_COLS}"
    ))
    .bind(nanoid(12))
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
    Ok((taken, run))
}

/// Run 状态迁移（completed / finished / abandoned），同事务回写 Requirement.status 投影。
/// 投影映射：finished → running（需求仍在推进，等待下一轮）；abandoned → abandoned；completed → completed。
/// completed 联动缺陷 open/processing → resolved。
///
/// **`running` 是非法目标态，在这里被堵死**（不是靠 `uq_run_one_running_per_requirement`
/// 去撞）。理由：那个索引是「每需求至多一条 running」的**全局**约束，而
/// `set_run_status` 要让一个旧的 finished Run 复活，跟「别处已经有在跑的」是同一个
/// 冲突的两面 —— 前者返回一个 500 样的数据库错误，后者给出「已经有 Run 在跑」这种
/// 调用方能理解并据此决策的信息，差别全在错误码上。
///
/// 复活一条已结束的 Run 在协议里本来也没有位置：续跑的正确动作是 `POST /runs` 开一条
/// 新的（或 `takeover`），而不是把旧的那条原地改回去 —— 它的 `ended_at` 已经写过、
/// `step` 序号已经排到底，复活只会得到一个状态自相矛盾的记录。
///
/// 用 `CONFLICT` 而**不是** `BAD_REQUEST`：调用方是拿着其它客户端读过的一行来发指令的，
/// 这是典型的「你手上的状态过期了」，不是「你这份请求体写错了」。
/// 沿用同码的同时把 `status` 一并放进 data，因为这条路径的歧义点在于目标态。
pub async fn set_run_status(
    pool: &PgPool,
    run_id: &str,
    requirement_id: &str,
    status: &str,
    end_reason: Option<&str>,
) -> Result<Option<RunRow>, AppError> {
    let mut tx = pool.begin().await?;
    // 锁序：**先 requirement、后 run**（全文件一致约定，见 `create_run` 的说明）。
    // 本函数两张表都写（run 改状态、requirement 回写投影），必须按同一顺序取锁，
    // 否则与开 Run 的三条路径反向成环。
    // 父行锁同时把「复活成 running」的判定串行化：并发的复活请求在这里排队，
    // 后来者读到的是前者已提交的结果。
    sqlx::query("SELECT id FROM requirement WHERE id = $1 FOR UPDATE")
        .bind(requirement_id)
        .fetch_one(&mut *tx)
        .await?;
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM run WHERE id = $1 AND requirement_id = $2")
            .bind(run_id)
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        // run 不存在：无任何写入；tx 随函数返回而 Drop 自动回滚。
        return Ok(None);
    }

    if status == "running" {
        // 下面是**兜底**，主序列化已由上面那句父行锁完成。
        //
        // 它挡的场景是：两个并发请求各自去复活**不同**的旧 Run。只做一次普通 SELECT
        // 会双双读到「没有 running」从而双双通过，然后由唯一索引拦下第二个 ——
        // 那是一个 500 样的数据库错误。行锁让第二个事务阻塞到第一个提交，
        // 再读时就看得见对方刚复活的 running 了。
        //
        // 锁的范围是「该需求的 run 行」而不是某一行：要判断的是这条需求上有没有
        // 别的 running，单行锁答不了这个问题。注意它在「该需求一条 run 都没有」时
        // 锁的是空集 —— 所以它不能充当唯一防线，锚点只能是父行。
        let running_elsewhere: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM run \
             WHERE requirement_id = $1 AND status = 'running'::\"RunStatus\" AND id <> $2 \
             ORDER BY index DESC LIMIT 1 FOR UPDATE",
        )
        .bind(requirement_id)
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((other_id,)) = running_elsewhere {
            return Err(AppError::conflict("RUN_ALREADY_RUNNING")
                .with_message("该需求已有 Run 在跑，不能把另一条 Run 改回 running")
                .with_hint(
                    "续跑请新开一条 Run（POST /runs）或接管（takeover），不要复活已结束的 Run",
                )
                .with_data(json!({ "requirementId": requirement_id, "runningRunId": other_id })));
        }
    }

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

pub async fn get_memory(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Option<MemoryRow>, AppError> {
    let row = sqlx::query_as::<_, MemoryRow>(&format!(
        "SELECT {MEMORY_COLS} FROM requirement_memory WHERE requirement_id = $1 AND project_id = $2"
    ))
    .bind(requirement_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// upsert Memory：snapshot 全量覆盖（Markdown 文本），带乐观锁。
///
/// `snapshot` 三态：
/// - `None`（字段缺失 → JS undefined）：**不再走空 data 短路**——路由层已把
///   「没有 snapshot」判成 400 SNAPSHOT_REQUIRED，到不了这里。
/// - `Some(None)`（显式 null）：置 NULL（清空记忆）。
/// - `Some(Some(v))`：正常写入 Markdown 文本。
///
/// `expected_revision = 0` 表示调用方认为该需求记忆尚不存在（首次写入）。
///
/// **形态与 `upsert_project_memory` / `update_knowledge_document` 统一**：
/// 原先是「先 SELECT id 再 UPDATE ... WHERE id」的 check-then-act，
/// 两次查询之间发生的任何写入都会被静默覆盖。改为「条件 UPDATE → 落空后重查分类」。
pub async fn upsert_memory(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
    snapshot: Option<&str>,
    expected_revision: i32,
) -> Result<WriteOutcome<MemoryRow>, AppError> {
    // 第一段：按 revision 条件更新。
    let updated = sqlx::query_as::<_, MemoryRow>(&format!(
        "UPDATE requirement_memory SET snapshot = $1, revision = revision + 1, updated_at = NOW() \
         WHERE requirement_id = $2 AND project_id = $3 AND revision = $4 RETURNING {MEMORY_COLS}"
    ))
    .bind(snapshot)
    .bind(requirement_id)
    .bind(project_id)
    .bind(expected_revision)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = updated {
        return Ok(WriteOutcome::Ok(row));
    }

    // 第二段：落空。行在 → 版本不符；行不在 → 看调用方声称的版本是否为 0。
    let current = get_memory(pool, requirement_id, project_id).await?;
    if let Some(row) = current {
        return Ok(WriteOutcome::Conflict(Box::new(row)));
    }
    if !revision_matches(expected_revision, 0) {
        return Ok(WriteOutcome::NotFound);
    }

    // 并发下两个请求可能同时走到这里，`ON CONFLICT` 兜住后者。
    {
        let id = nanoid(12);
        let inserted = sqlx::query_as::<_, MemoryRow>(MEMORY_INSERT_SQL)
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
        .bind(snapshot)
        .fetch_optional(pool)
        .await?;

        match inserted {
            Some(row) => Ok(WriteOutcome::Ok(row)),
            None => Ok(classify_miss(
                get_memory(pool, requirement_id, project_id).await?,
            )),
        }
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
    scenario_ref: &str,
    requirement_id: &str,
    status: &str,
) -> Result<Option<ScenarioRow>, AppError> {
    // scenario_ref 支持内部 ID 或 key：先按 id 查，未命中再按 (requirement_id, key) 查，
    // 对齐 put_case 的 scenarioId > scenarioKey 解析；两者都未命中返回 None → 404。
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM scenario WHERE id = $1 AND requirement_id = $2")
            .bind(scenario_ref)
            .bind(requirement_id)
            .fetch_optional(pool)
            .await?;
    let id = match existing {
        Some((id,)) => id,
        None => {
            let by_key: Option<(String,)> =
                sqlx::query_as("SELECT id FROM scenario WHERE key = $1 AND requirement_id = $2")
                    .bind(scenario_ref)
                    .bind(requirement_id)
                    .fetch_optional(pool)
                    .await?;
            match by_key {
                Some((id,)) => id,
                None => return Ok(None),
            }
        }
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

/// completed 硬条件：所有场景 passing/waived。
/// （2026-09-09 起去掉 openDecisions 检查——记忆改为 Markdown 后无法可靠解析未决决策项，
///  决策由 AI 在会话中主动提出，不再作为 completed 硬门禁。）
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
    Ok((blockers.is_empty(), blockers))
}

/// reset（幂等）：清工作记忆 + Scenario/Case 全部重置 pending + 开新 Run。
/// （2026-09-09 起记忆改为 Markdown，reset 时直接清空 snapshot，不再保留 requirementSnapshot。）
///
/// 它也是「开 Run」的第三条路径（见 `create_run` 的锁序说明），
/// 因此同样**先锁父行 requirement**，再动 run 行。
pub async fn reset_requirement(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<RunRow, AppError> {
    let mut tx = pool.begin().await?;
    // 序列化锚点：锁父行。理由同 `create_run`——两条并发 reset、或 reset 撞上
    // 并发的 create_run，都在这里排队，后面那条 `UPDATE run ... SET status='finished'`
    // 与 `MAX(index)` 才看得到前者的结果。
    sqlx::query("SELECT id FROM requirement WHERE id = $1 FOR UPDATE")
        .bind(requirement_id)
        .fetch_one(&mut *tx)
        .await?;
    // 清空工作记忆（snapshot 置 NULL）
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM requirement_memory WHERE requirement_id = $1")
            .bind(requirement_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some((id,)) = existing {
        // reset 也是一次真实写入，必须推进 revision —— 否则任何在此之前读到该记忆的
        // Agent，会拿着一个「已经开始匹配的」旧版本号把 reset 掉的快照又写回去。
        // 清空记忆却让版本号停滞，等于给并发写入开了一道后门。
        sqlx::query(
            "UPDATE requirement_memory SET snapshot = NULL, revision = revision + 1, updated_at = NOW() WHERE id = $1",
        )
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    } else {
        let id = nanoid(12);
        // 新建行显式写 revision = 1，与 `upsert_memory` 的 INSERT 一致。
        sqlx::query(
            "INSERT INTO requirement_memory (id, requirement_id, project_id, snapshot, revision, updated_at) VALUES ($1, $2, $3, NULL, 1, NOW())",
        )
        .bind(&id)
        .bind(requirement_id)
        .bind(project_id)
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
    // reset 语义是「把这个需求推倒重来」：它**先结束掉当前 running**（若有），
    // 再开新的一轮。不这么做的话，下面的 INSERT 会直接撞
    // `uq_run_one_running_per_requirement` —— 而这个函数已经清空了工作记忆、
    // 重置了场景与用例状态，事务回滚会让整次 reset 白做。
    //
    // 接管用的 `end_reason` 与 takeover 区分开：两者的前因不同（一个是人工接管僵尸
    // Run，一个是 reset 推倒重来），排查时要能分辨。
    sqlx::query(
        "UPDATE run SET status = 'finished'::\"RunStatus\", \
           end_reason = COALESCE(end_reason, '被 reset 结束'), \
           ended_at = COALESCE(ended_at, NOW()), updated_at = NOW() \
         WHERE requirement_id = $1 AND status = 'running'::\"RunStatus\"",
    )
    .bind(requirement_id)
    .execute(&mut *tx)
    .await?;

    // 父行锁下重算，**不带** `FOR UPDATE`（聚合上不允许，且已无必要）。
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 三种 23505 的来源必须分清：它们**都会**以 `unique_violation` 出现，
    /// 但处理方式相反 —— 一个是业务冲突（409，就此结束），另一个是自愈性的
    /// 序号竞争（重试一次即可），混起来会把「有人已经在跑」静默重试成第二次写入。
    ///
    /// 这里的断言是**字符串字面量**，不是从迁移文件里解析出来的 —— 索引名写错
    /// 不会在这里红，而是会在真库上退化成 `Other`（500）。这是权衡后的取舍：
    /// 迁移的 SQL 无法在单测里跑（repo 层没有连库测试基础设施），
    /// 所以两个名字必须与 `20260928120000_concurrency_guard.sql` 人工对齐。
    /// 下面这条断言就是那个对齐点，改名字时两处一起改。
    #[test]
    fn classify_run_unique_splits_business_conflict_from_index_race() {
        assert!(matches!(
            classify_run_unique(Some("uq_run_one_running_per_requirement")),
            RunUniqueConflict::AlreadyRunning
        ));
        assert!(matches!(
            classify_run_unique(Some("run_requirement_id_index_key")),
            RunUniqueConflict::IndexRace
        ));
    }

    /// 认不出来的约束**必须**落到 `Other` 而不是 `IndexRace`。
    ///
    /// 这个方向的错误代价不对称：若默认成 `IndexRace`，`create_run` 会再试一次，
    /// 而重试后仍然失败就返回一个措辞含糊的 409「并发开 Run，index 分配冲突」——
    /// 真实的 bug（比如别人新加了一条我们不知道的唯一约束）会被这句话盖住，
    /// 排查时以为只是高了并发。落到 `Other` 则原样冒泡成 500 并带上原始错误，
    /// 虽然对调用方不友好，但至少把真相留在了日志里。
    #[test]
    fn classify_run_unique_defaults_to_other_not_index_race() {
        assert!(matches!(
            classify_run_unique(None),
            RunUniqueConflict::Other
        ));
        assert!(matches!(
            classify_run_unique(Some("run_pkey")),
            RunUniqueConflict::Other
        ));
        // 前缀相近但不相等 —— 必须走精确匹配，不能 startswith
        assert!(matches!(
            classify_run_unique(Some("uq_run_one_running_per_requirement_x")),
            RunUniqueConflict::Other
        ));
    }

    /// 非 23505 的错误拿不到约束名，`unique_constraint` 应返回 None。
    /// 用 `RowNotFound` 是随手取的一个「肯定不是唯一冲突」的变体。
    #[test]
    fn unique_constraint_ignores_non_unique_violations() {
        assert!(unique_constraint(&sqlx::Error::RowNotFound).is_none());
    }

    /// 兜底 409 的形状必须与 handler 预检查那条**同码**，否则 CLI 的
    /// `contains("RUN_ALREADY_RUNNING")` 分流会在并发这条路径上失效 ——
    /// 而并发恰恰是它最需要工作的场景。
    #[test]
    fn run_already_running_matches_the_handler_precheck_code() {
        let e = run_already_running();
        assert_eq!(e.code, "RUN_ALREADY_RUNNING");
        assert_eq!(e.status, 409);
    }

    /// 建行 SQL 的两处硬约束，一次钉死。
    ///
    /// **一、仲裁目标必须是单列 `requirement_id`。** 表上唯一的唯一索引是 baseline
    /// 建的 `context_requirement_id_key`（表从 `context` 改名，索引名跟着留下），
    /// 单列。Postgres 的仲裁索引推断要求列集合与某个唯一索引**完全一致**，多一列
    /// 就够不上。曾经这里写的是 `(requirement_id, project_id)`，于是
    /// 「行不存在 + revision=0」这条**首次写入**路径稳定抛 42P10 → 500，
    /// 而更新已有行的路径完全正常 —— 故障因此长得像「某些需求写不进去」，
    /// 而不是「这个接口坏了」。
    ///
    /// **二、revision 必须硬编码为 1。** `0` 在客户端侧是「我认为这行还不存在」
    /// 的意图表达，只用于触发建行；落进库里会让下一个用 0 建行的调用方与真实
    /// 版本号错开一格，乐观锁第一次比较就误判。
    #[test]
    fn memory_insert_sql_is_pinned() {
        assert!(
            MEMORY_INSERT_SQL.contains("ON CONFLICT (requirement_id) DO NOTHING"),
            "仲裁目标必须是单列 requirement_id，与 context_requirement_id_key 对齐"
        );
        // 反向确认：两列写法必须不在其中 —— 这正是本次回归的原始形态
        assert!(
            !MEMORY_INSERT_SQL.contains("requirement_id, project_id)"),
            "两列仲裁目标够不上任何唯一索引，会在首次写入时抛 42P10"
        );
        assert!(
            MEMORY_INSERT_SQL.contains("VALUES ($1, $2, $3, $4, 1, NOW())"),
            "建行必须显式把 revision 写成 1"
        );
    }
}
