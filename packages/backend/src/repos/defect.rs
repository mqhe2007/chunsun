//! 缺陷表访问（对齐 `packages/backend/src/repositories/defectRepository.ts`）。
//!
//! 兼容要点：
//! - 主键 `nanoid(12)`（与 requirement 一致）。
//! - 列表 `orderBy: { updatedAt: "desc" }`，且 **requirement 关联只取 3 个字段**
//!   （id/description/status），多取会让对拍逐字节比对失败。
//! - `q` 过滤是 `OR [id contains q, description contains q insensitive]`：id 用精确 `LIKE`（区分大小写），
//!   description 用 `ILIKE`（不区分）。
//! - `requirementId` 列表过滤是**精确匹配且未 trim**（旧 `filters?.xxx ? {xxx} : {}`）。
//!   注意 JS 里空串是 **falsy**，所以 `?requirementId=` 是「不过滤」而不是「匹配空串」。
//! - `create` / `update` 走 `defectListInclude`，返回行带 requirement 关联；`delete` 同理 RETURNING
//!   带关联占位，但上层只用 `id`。
//! - `get_defect_by_id` 接受泛型 `Executor`，供 convert 事务内复用。

use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow, PgPool, Postgres, QueryBuilder};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 缺陷关联的修复需求摘要（仅列表/详情展示用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefectRequirementLink {
    pub id: String,
    pub description: String,
    pub status: String,
}

/// 缺陷创建人摘要（形状对齐 `RequirementCreator`：id/nickname/qq + email 兜底）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefectCreator {
    pub id: String,
    pub nickname: Option<String>,
    pub qq: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DefectRow {
    pub id: String,
    pub project_id: String,
    pub description: Option<String>,
    pub status: String,
    pub severity: String,
    pub requirement_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 仅在 `include: { requirement: … }` 的查询里有值；新建缺陷恒为 `None`。
    pub requirement: Option<DefectRequirementLink>,
    /// 仅在带 creator 关联的查询里有值；`create` 路径恒为 `None`（同 requirement 行为）。
    pub creator: Option<DefectCreator>,
}

/// sqlx 取行的中间结构：requirement / creator 字段用 LEFT JOIN 平铺，再折叠成对应对象。
#[derive(Debug, Clone, FromRow)]
struct DefectJoinRow {
    id: String,
    project_id: String,
    description: Option<String>,
    status: String,
    severity: String,
    requirement_id: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    rl_id: Option<String>,
    rl_description: Option<String>,
    rl_status: Option<String>,
    c_id: Option<String>,
    c_nickname: Option<String>,
    c_qq: Option<String>,
    c_email: Option<String>,
}

impl DefectJoinRow {
    fn into_row(self) -> DefectRow {
        let requirement = match (self.rl_id, self.rl_description, self.rl_status) {
            (Some(id), Some(description), Some(status)) => Some(DefectRequirementLink {
                id,
                description,
                status,
            }),
            _ => None,
        };
        let creator = self.c_id.map(|id| DefectCreator {
            id,
            nickname: self.c_nickname,
            qq: self.c_qq,
            email: self.c_email,
        });
        DefectRow {
            id: self.id,
            project_id: self.project_id,
            description: self.description,
            status: self.status,
            severity: self.severity,
            requirement_id: self.requirement_id,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            requirement,
            creator,
        }
    }
}

const DEFECT_COLS: &str = "d.id, d.project_id, d.description, \
     d.status::text AS status, d.severity::text AS severity, \
     d.requirement_id, d.created_by, d.created_at, d.updated_at";

const REQ_LINK_COLS: &str =
    "r.id AS rl_id, r.description AS rl_description, r.status::text AS rl_status";

const CREATOR_COLS: &str =
    "u2.id AS c_id, u2.nickname AS c_nickname, u2.qq AS c_qq, u2.email AS c_email";

/// INSERT … RETURNING 投影（无表别名；status/severity 为枚举原值，外层 SELECT 再 ::text）。
const DEFECT_INSERT_RETURNING: &str =
    "id, project_id, description, status, severity, requirement_id, created_by, created_at, updated_at";

/// 不带表别名、不带 requirement/creator 关联的列（`INSERT … RETURNING` / `DELETE … RETURNING` 用）。
/// requirement 三列与 creator 四列填 NULL，折叠后 `requirement`/`creator` 就是 `None`。
const DEFECT_COLS_RETURNING: &str = "id, project_id, description, \
     status::text AS status, severity::text AS severity, \
     requirement_id, created_by, created_at, updated_at, \
     NULL::text AS rl_id, NULL::text AS rl_description, NULL::text AS rl_status, \
     NULL::text AS c_id, NULL::text AS c_nickname, NULL::text AS c_qq, NULL::text AS c_email";

pub struct CreateDefectInput<'a> {
    pub project_id: &'a str,
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub requirement_id: Option<&'a str>,
    /// 创建人用户 id（由服务层写当前登录用户；历史数据迁移后为 NULL）。
    pub created_by: Option<&'a str>,
}

/// createDefect：默认 `status=open` / `severity=minor`。
///
/// 旧实现带 `include: defectListInclude`，所以**传了合法 requirementId 时响应里的
/// `requirement` 是关联对象而不是 null**。这里用 CTE 一次往返完成「插入 + 关联查询」，
/// 避免退化成恒 null。
pub async fn create_defect<'e, E>(
    executor: E,
    input: CreateDefectInput<'_>,
) -> Result<DefectRow, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = nanoid(12);
    let sql = format!(
        "WITH ins AS ( \
           INSERT INTO defect \
             (id, project_id, description, status, severity, requirement_id, \
              created_by, created_at, updated_at) \
           VALUES ($1, $2, $3, $4::\"DefectStatus\", $5::\"DefectSeverity\", $6, $7, NOW(), NOW()) \
           RETURNING {DEFECT_INSERT_RETURNING} \
         ) \
         SELECT {DEFECT_COLS}, {REQ_LINK_COLS}, {CREATOR_COLS} FROM ins d \
         LEFT JOIN \"requirement\" r ON r.id = d.requirement_id \
         LEFT JOIN \"user\" u2 ON u2.id = d.created_by"
    );
    let row = sqlx::query_as::<_, DefectJoinRow>(&sql)
        .bind(&id)
        .bind(input.project_id)
        .bind(input.description)
        .bind(input.status.unwrap_or("open"))
        .bind(input.severity.unwrap_or("minor"))
        .bind(input.requirement_id)
        .bind(input.created_by)
        .fetch_one(executor)
        .await?;
    Ok(row.into_row())
}

#[derive(Debug, Default)]
pub struct DefectListFilters<'a> {
    /// 已解析的合法状态列表；`None` 表示不按状态过滤。
    pub status: Option<Vec<&'a str>>,
    /// 精确匹配 severity；`None` 表示不过滤。
    pub severity: Option<&'a str>,
    /// 精确匹配 requirementId（未 trim）；`None` 或**空串**都不过滤（JS 空串 falsy）。
    pub requirement_id: Option<&'a str>,
    /// 模糊匹配 id/description；`None` 或 trim 后为空不过滤。
    pub q: Option<&'a str>,
}

/// listDefectsByProject：`updatedAt` 倒序，带 requirement 关联。
pub async fn list_defects_by_project(
    pool: &PgPool,
    project_id: &str,
    filters: DefectListFilters<'_>,
) -> Result<Vec<DefectRow>, AppError> {
    let mut qb = QueryBuilder::new(format!(
        "SELECT {DEFECT_COLS}, {REQ_LINK_COLS}, {CREATOR_COLS} FROM defect d \
         LEFT JOIN \"requirement\" r ON r.id = d.requirement_id \
         LEFT JOIN \"user\" u2 ON u2.id = d.created_by WHERE d.project_id = "
    ));
    qb.push_bind(project_id);

    if let Some(statuses) = filters.status.as_ref() {
        if !statuses.is_empty() {
            qb.push(" AND d.status::text IN (");
            let mut sep = qb.separated(", ");
            for s in statuses {
                sep.push_bind(*s);
            }
            qb.push(")");
        }
    }

    // severity 精确匹配
    if let Some(sev) = filters.severity {
        qb.push(" AND d.severity::text = ");
        qb.push_bind(sev);
    }

    // requirementId：精确匹配、**不 trim**，但空串按 JS falsy 跳过
    // （旧 `filters?.xxx ? { xxx } : {}`——`"" ? A : B` 走 B，即不过滤）。
    if let Some(rid) = filters.requirement_id.filter(|s| !s.is_empty()) {
        qb.push(" AND d.requirement_id = ");
        qb.push_bind(rid);
    }

    // q：OR[id contains q, description contains q insensitive]；id 用 LIKE（区分大小写），description 用 ILIKE
    if let Some(q) = filters.q.map(str::trim).filter(|s| !s.is_empty()) {
        qb.push(" AND (d.id LIKE ");
        qb.push_bind(format!("%{q}%"));
        qb.push(" OR d.description ILIKE ");
        qb.push_bind(format!("%{q}%"));
        qb.push(")");
    }

    qb.push(" ORDER BY d.updated_at DESC");

    let rows = qb
        .build_query_as::<DefectJoinRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(DefectJoinRow::into_row).collect())
}

/// getDefectById：**id + projectId 双条件**，带 requirement 关联。
/// 接受泛型 `Executor`（convert 事务内查缺陷用）。
pub async fn get_defect_by_id<'e, E>(
    executor: E,
    id: &str,
    project_id: &str,
) -> Result<Option<DefectRow>, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {DEFECT_COLS}, {REQ_LINK_COLS}, {CREATOR_COLS} FROM defect d \
         LEFT JOIN \"requirement\" r ON r.id = d.requirement_id \
         LEFT JOIN \"user\" u2 ON u2.id = d.created_by \
         WHERE d.id = $1 AND d.project_id = $2"
    );
    let row = sqlx::query_as::<_, DefectJoinRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(DefectJoinRow::into_row))
}

/// PATCH 的字段补丁：`None` = 不动该列，`Some(v)` = 写入 v。
///
/// `requirement_id` 用 `Option<Option<&str>>` 表达「不传 / 置 NULL / 赋值」三态；
/// 空串在路由层已摊平成内层 `None`（对齐 JS `|| null`）。
#[derive(Debug, Default)]
pub struct UpdateDefectPatch<'a> {
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub requirement_id: Option<Option<&'a str>>,
}

impl UpdateDefectPatch<'_> {
    fn is_empty(&self) -> bool {
        self.description.is_none()
            && self.status.is_none()
            && self.severity.is_none()
            && self.requirement_id.is_none()
    }
}

/// updateDefectById：先按 id+projectId 查存在性（不存在返回 None，由上层 404），
/// 再按 id 更新并 include requirement。
///
/// 旧实现即使补丁为空也会走一次 `prisma.update`，从而**刷新 `updatedAt`**。
/// 这里照搬：空补丁也执行 `SET updated_at = NOW()`。
pub async fn update_defect_by_id(
    pool: &PgPool,
    id: &str,
    project_id: &str,
    patch: UpdateDefectPatch<'_>,
) -> Result<Option<DefectRow>, AppError> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM defect WHERE id = $1 AND project_id = $2")
            .bind(id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Ok(None);
    }

    if patch.is_empty() {
        sqlx::query("UPDATE defect SET updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return get_defect_by_id(pool, id, project_id).await;
    }

    let mut qb = QueryBuilder::new("UPDATE defect SET updated_at = NOW()");
    if let Some(v) = patch.description {
        // description 不 trim，空串也原样存（对齐 `body.description`，PATCH 无 minLength）
        qb.push(", description = ");
        qb.push_bind(v);
    }
    if let Some(v) = patch.status {
        qb.push(", status = ");
        qb.push_bind(v);
        qb.push("::\"DefectStatus\"");
    }
    if let Some(v) = patch.severity {
        qb.push(", severity = ");
        qb.push_bind(v);
        qb.push("::\"DefectSeverity\"");
    }
    if let Some(v) = patch.requirement_id {
        qb.push(", requirement_id = ");
        qb.push_bind(v);
    }
    qb.push(" WHERE id = ");
    qb.push_bind(id);

    qb.build().execute(pool).await?;
    get_defect_by_id(pool, id, project_id).await
}

/// deleteDefectById：先查存在性再删，返回被删的行（带 requirement 关联占位）。
pub async fn delete_defect_by_id(
    pool: &PgPool,
    id: &str,
    project_id: &str,
) -> Result<Option<DefectRow>, AppError> {
    let sql = format!(
        "DELETE FROM defect WHERE id = $1 AND project_id = $2 \
         RETURNING {}",
        DEFECT_COLS_RETURNING
    );
    let row = sqlx::query_as::<_, DefectJoinRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(DefectJoinRow::into_row))
}

/// 在事务内把缺陷回链到修复需求并置 processing（convert 专用）。
pub async fn link_defect_to_requirement<'e, E>(
    executor: E,
    defect_id: &str,
    requirement_id: &str,
) -> Result<(), AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "UPDATE defect SET requirement_id = $1, status = 'processing', updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(requirement_id)
    .bind(defect_id)
    .execute(executor)
    .await?;
    Ok(())
}
