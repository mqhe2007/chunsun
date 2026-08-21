//! requirement 表访问（对齐 `requirementRepository.ts`）。
//!
//! 兼容要点：
//! - 主键 `nanoid(12)`（Prisma `@default(nanoid(12))`）。
//! - `updated_at` 是 `@updatedAt`（应用层维护），INSERT/UPDATE 都显式写。
//! - 列表 `orderBy: { updatedAt: "desc" }`，且 **owner 关联只取 3 个字段**
//!   （id/nickname/qq），多取会让对拍逐字节比对失败。
//! - `id` 过滤走 Prisma `contains + mode:"insensitive"`，Prisma 生成的是
//!   `ILIKE '%' || $n || '%'` 且**不转义** `%`/`_`——用户传 `%` 就是通配符。
//!   这里照搬这个「不转义」的行为，不擅自收紧。
//! - `create` 不 include owner：调用方拿到的行没有 owner 关联，序列化后
//!   `owner` 恒为 `null`。这个怪癖由 `RequirementRow::owner = None` 表达。

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Executor, FromRow, PgPool, Postgres, QueryBuilder};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 需求负责人摘要（对齐 `RequirementOwner`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementOwner {
    pub id: String,
    pub nickname: Option<String>,
    pub qq: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RequirementRow {
    pub id: String,
    pub project_id: String,
    pub repository_id: Option<String>,
    pub description: String,
    pub source_text: Option<String>,
    pub client_notes: Option<String>,
    pub status: String,
    pub coverage: String,
    pub origin: String,
    pub owner_id: Option<String>,
    pub released_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 仅在 `include: { owner: … }` 的查询里有值；`create` 路径恒为 `None`。
    pub owner: Option<RequirementOwner>,
}

/// sqlx 取行的中间结构：owner 字段用 LEFT JOIN 平铺，再折叠成 `RequirementOwner`。
#[derive(Debug, Clone, FromRow)]
struct RequirementJoinRow {
    id: String,
    project_id: String,
    repository_id: Option<String>,
    description: String,
    source_text: Option<String>,
    client_notes: Option<String>,
    status: String,
    coverage: String,
    origin: String,
    owner_id: Option<String>,
    released_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    o_id: Option<String>,
    o_nickname: Option<String>,
    o_qq: Option<String>,
}

impl RequirementJoinRow {
    fn into_row(self) -> RequirementRow {
        // LEFT JOIN 未命中时 o_id 为 NULL；命中才折叠出 owner 对象。
        let owner = self.o_id.map(|id| RequirementOwner {
            id,
            nickname: self.o_nickname,
            qq: self.o_qq,
        });
        RequirementRow {
            id: self.id,
            project_id: self.project_id,
            repository_id: self.repository_id,
            description: self.description,
            source_text: self.source_text,
            client_notes: self.client_notes,
            status: self.status,
            coverage: self.coverage,
            origin: self.origin,
            owner_id: self.owner_id,
            released_at: self.released_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
            owner,
        }
    }
}

const REQ_COLS: &str = "r.id, r.project_id, r.repository_id, r.description, r.source_text, \
     r.client_notes, r.status::text AS status, r.coverage::text AS coverage, \
     r.origin::text AS origin, r.owner_id, r.released_at, r.created_at, r.updated_at";

const OWNER_COLS: &str =
    "u.id AS o_id, u.nickname AS o_nickname, u.qq AS o_qq";

/// 不带表别名、不带 owner 关联的列（`INSERT … RETURNING` / `DELETE … RETURNING` 用）。
/// owner 四列填 NULL，折叠后 `owner` 就是 `None`——正好复刻旧实现「create/delete 不 include
/// owner」的行为。
const REQ_COLS_RETURNING: &str = "id, project_id, repository_id, description, source_text, \
     client_notes, status::text AS status, coverage::text AS coverage, origin::text AS origin, \
     owner_id, released_at, created_at, updated_at, \
     NULL::text AS o_id, NULL::text AS o_nickname, NULL::text AS o_qq";

pub struct CreateRequirementInput<'a> {
    pub project_id: &'a str,
    pub repository_id: Option<&'a str>,
    pub description: &'a str,
    pub source_text: Option<&'a str>,
    pub client_notes: Option<&'a str>,
    pub status: Option<&'a str>,
    pub coverage: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub owner_id: Option<&'a str>,
}

/// createRequirement：默认 `status=pending` / `coverage=none` / `origin=manual`。
///
/// 注意返回行**不带 owner 关联**（旧实现 `prisma.requirement.create` 没有 include），
/// 因此即使 `ownerId` 有值，响应里的 `owner` 也是 `null`。
///
/// 接受泛型 `Executor`：`&PgPool`（普通调用）与 `&mut Transaction`（convert 事务内）都可用。
pub async fn create_requirement<'e, E>(
    executor: E,
    input: CreateRequirementInput<'_>,
) -> Result<RequirementRow, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = nanoid(12);
    let sql = format!(
        r#"INSERT INTO requirement
             (id, project_id, repository_id, description, source_text, client_notes,
              status, coverage, origin, owner_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6,
                   $7::"RequirementStatus", $8::"RequirementCoverage",
                   $9::"RequirementOrigin", $10, NOW(), NOW())
           RETURNING {REQ_COLS_RETURNING}"#
    );
    let row = sqlx::query_as::<_, RequirementJoinRow>(&sql)
        .bind(&id)
        .bind(input.project_id)
        .bind(input.repository_id)
        .bind(input.description)
        .bind(input.source_text)
        .bind(input.client_notes)
        .bind(input.status.unwrap_or("pending"))
        .bind(input.coverage.unwrap_or("none"))
        .bind(input.origin.unwrap_or("manual"))
        .bind(input.owner_id)
        .fetch_one(executor)
        .await?;
    Ok(row.into_row())
}

#[derive(Debug, Default)]
pub struct RequirementListFilters<'a> {
    /// 已解析的合法状态列表；`None` 表示不按状态过滤。
    pub status: Option<Vec<&'a str>>,
    /// 模糊匹配 id（trim 后为空视作不过滤）。
    pub id: Option<&'a str>,
    pub owner_id: Option<&'a str>,
    /// 与 `page_size` 成对出现时启用分页；`None` 返回全量。
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub struct RequirementListResult {
    pub items: Vec<RequirementRow>,
    pub total: i64,
}

fn push_requirement_list_filters<'args>(
    qb: &mut QueryBuilder<'args, Postgres>,
    filters: &RequirementListFilters<'args>,
    id_like: Option<&'args str>,
) {
    if let Some(statuses) = filters.status.as_ref() {
        if !statuses.is_empty() {
            qb.push(" AND r.status::text IN (");
            let mut sep = qb.separated(", ");
            for s in statuses {
                sep.push_bind(*s);
            }
            qb.push(")");
        }
    }

    if let Some(id_like) = id_like {
        qb.push(" AND r.id ILIKE ");
        qb.push_bind(id_like);
    }

    if let Some(owner) = filters.owner_id.filter(|s| !s.is_empty()) {
        qb.push(" AND r.owner_id = ");
        qb.push_bind(owner);
    }
}

/// listRequirementsByProject：`updatedAt` 倒序，带 owner 关联。
///
/// 传入 `page` + `page_size` 时按窗口分页并返回匹配总数；否则返回全量，`total` 为本次条数。
pub async fn list_requirements_by_project(
    pool: &PgPool,
    project_id: &str,
    filters: RequirementListFilters<'_>,
) -> Result<RequirementListResult, AppError> {
    let pagination = match (filters.page, filters.page_size) {
        (Some(page), Some(page_size)) => {
            let page = page.max(1);
            let page_size = page_size.clamp(1, 100);
            Some((page, page_size))
        }
        _ => None,
    };
    let id_like = filters
        .id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let total = if pagination.is_some() {
        let mut count_qb = QueryBuilder::new(
            "SELECT COUNT(*) FROM requirement r WHERE r.project_id = ",
        );
        count_qb.push_bind(project_id);
        push_requirement_list_filters(&mut count_qb, &filters, id_like.as_deref());
        count_qb
            .build_query_scalar::<i64>()
            .fetch_one(pool)
            .await?
    } else {
        -1
    };

    let mut qb = QueryBuilder::new(format!(
        "SELECT {REQ_COLS}, {OWNER_COLS} FROM requirement r \
         LEFT JOIN \"user\" u ON u.id = r.owner_id WHERE r.project_id = "
    ));
    qb.push_bind(project_id);
    push_requirement_list_filters(&mut qb, &filters, id_like.as_deref());
    qb.push(" ORDER BY r.updated_at DESC");

    if let Some((page, page_size)) = pagination {
        let offset = page.saturating_sub(1).saturating_mul(page_size);
        qb.push(" LIMIT ");
        qb.push_bind(page_size);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }

    let rows = qb
        .build_query_as::<RequirementJoinRow>()
        .fetch_all(pool)
        .await?;
    let items: Vec<RequirementRow> = rows.into_iter().map(RequirementJoinRow::into_row).collect();
    let total = if total >= 0 {
        total
    } else {
        items.len() as i64
    };
    Ok(RequirementListResult { items, total })
}

/// getRequirementById：**id + projectId 双条件**，带 owner 关联。
/// 接受泛型 `Executor`（事务内查既有 requirement 用）。
pub async fn get_requirement_by_id<'e, E>(
    executor: E,
    id: &str,
    project_id: &str,
) -> Result<Option<RequirementRow>, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {REQ_COLS}, {OWNER_COLS} FROM requirement r \
         LEFT JOIN \"user\" u ON u.id = r.owner_id \
         WHERE r.id = $1 AND r.project_id = $2"
    );
    let row = sqlx::query_as::<_, RequirementJoinRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(RequirementJoinRow::into_row))
}

/// 按主键全局查需求，**不限项目**（对齐旧实现 `tx.requirement.findUnique({ where: { id } })`）。
///
/// 只有 defect 域的 `convert-to-requirement` 幂等分支用它：缺陷的 `requirementId` 可以被
/// create/patch 直接写成**别的项目**的需求 ID（那两个端点只有 FK 约束、不校验项目归属），
/// 旧实现的 findUnique 会照样命中并幂等返回。若这里改成带 projectId 的查询，跨项目关联的
/// 缺陷就会走到「重新创建需求」分支，行为与旧后端不同。
pub async fn find_requirement_by_id_global<'e, E>(
    executor: E,
    id: &str,
) -> Result<Option<RequirementRow>, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "SELECT {REQ_COLS}, {OWNER_COLS} FROM requirement r \
         LEFT JOIN \"user\" u ON u.id = r.owner_id \
         WHERE r.id = $1"
    );
    let row = sqlx::query_as::<_, RequirementJoinRow>(&sql)
        .bind(id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(RequirementJoinRow::into_row))
}

/// PATCH 的字段补丁：`None` = 不动该列，`Some(v)` = 写入 v。
///
/// `released_at` / `owner_id` 用 `Option<Option<…>>` 表达「不动 / 置 NULL / 赋值」三态。
#[derive(Debug, Default)]
pub struct UpdateRequirementPatch<'a> {
    pub description: Option<&'a str>,
    pub source_text: Option<Option<&'a str>>,
    pub client_notes: Option<Option<&'a str>>,
    pub status: Option<&'a str>,
    pub coverage: Option<&'a str>,
    pub released_at: Option<Option<String>>,
    pub owner_id: Option<Option<&'a str>>,
}

impl UpdateRequirementPatch<'_> {
    fn is_empty(&self) -> bool {
        self.description.is_none()
            && self.source_text.is_none()
            && self.client_notes.is_none()
            && self.status.is_none()
            && self.coverage.is_none()
            && self.released_at.is_none()
            && self.owner_id.is_none()
    }
}

/// 对齐 `releasedAt ? new Date(releasedAt) : null`：JS `new Date()` **不抛错**，非法串得到
/// `Invalid Date`，要等 Prisma 落库才报 500；且 `""` 是 falsy → 走 `null` 分支（清空白）。
///
/// 解析策略：
/// - RFC3339 优先（`2026-01-02T03:04:05Z`、`2026-01-02T03:04:05.123+08:00`）；
/// - 其次兼容 JS 能接受的裸日期 `YYYY-MM-DD`（`new Date("2026-01-02")` 按 **UTC 零点**，
///   这是 ES 规范对 date-only 的规定，与机器本地时区无关）；
/// - 其余（含 `garbage`）按 `Invalid Date` 处理，由调用方返回 500，与旧实现落库报错一致。
///
/// 已知未对齐的边界（刻意不处理）：无时区的日期时间 `YYYY-MM-DDTHH:MM:SS`（JS 按**本地**
/// 时区解析，而 chrono 未启用 `clock` 特性拿不到本地偏移）。真实前端一律发带时标的 RFC3339，
/// 对拍脚本也不构造该输入，故归为「不测」而非「已知差异」。
///
/// **必须在「需求存在性校验」之后调用**：不存在的需求即便日期非法也应先 404。
fn parse_released_at(raw: &str) -> Result<DateTime<Utc>, AppError> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        if let Some(naive) = d.and_hms_opt(0, 0, 0) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }
    }
    Err(AppError::internal(format!("Invalid Date: {raw}")))
}

/// updateRequirementById：先按 id+projectId 查存在性（不存在返回 None，由上层 404），
/// 再按 id 更新并 include owner。
///
/// 旧实现即使补丁为空也会走一次 `prisma.update`，从而**刷新 `updatedAt`**。
/// 这里照搬：空补丁也执行 `SET updated_at = NOW()`。
pub async fn update_requirement_by_id(
    pool: &PgPool,
    id: &str,
    project_id: &str,
    patch: UpdateRequirementPatch<'_>,
) -> Result<Option<RequirementRow>, AppError> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM requirement WHERE id = $1 AND project_id = $2")
            .bind(id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Ok(None);
    }

    if patch.is_empty() {
        // 空补丁也要刷新 updatedAt，与 Prisma 行为一致
        sqlx::query("UPDATE requirement SET updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return get_requirement_by_id(pool, id, project_id).await;
    }

    let mut qb = QueryBuilder::new("UPDATE requirement SET updated_at = NOW()");
    if let Some(v) = patch.description {
        qb.push(", description = ");
        qb.push_bind(v);
    }
    if let Some(v) = patch.source_text {
        qb.push(", source_text = ");
        qb.push_bind(v);
    }
    if let Some(v) = patch.client_notes {
        qb.push(", client_notes = ");
        qb.push_bind(v);
    }
    if let Some(v) = patch.status {
        qb.push(", status = ");
        qb.push_bind(v);
        qb.push("::\"RequirementStatus\"");
    }
    if let Some(v) = patch.coverage {
        qb.push(", coverage = ");
        qb.push_bind(v);
        qb.push("::\"RequirementCoverage\"");
    }
    if let Some(v) = &patch.released_at {
        qb.push(", released_at = ");
        match v {
            // 显式 null 或空串（JS falsy → `: null` 分支）→ 清空
            None => {
                qb.push("NULL");
            }
            Some(s) if s.is_empty() => {
                qb.push("NULL");
            }
            // 有值：存在性已确认，此时再解析；垃圾串 → 500（对齐 Invalid Date 落库报错）
            Some(s) => {
                qb.push_bind(parse_released_at(s)?);
            }
        }
    }
    if let Some(v) = patch.owner_id {
        qb.push(", owner_id = ");
        qb.push_bind(v);
    }
    qb.push(" WHERE id = ");
    qb.push_bind(id);

    qb.build().execute(pool).await?;
    get_requirement_by_id(pool, id, project_id).await
}

/// deleteRequirementById：先查存在性再删，返回被删的行（不带 owner）。
pub async fn delete_requirement_by_id(
    pool: &PgPool,
    id: &str,
    project_id: &str,
) -> Result<Option<RequirementRow>, AppError> {
    let sql = format!(
        "DELETE FROM requirement WHERE id = $1 AND project_id = $2 \
         RETURNING {REQ_COLS_RETURNING}"
    );
    let row = sqlx::query_as::<_, RequirementJoinRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(RequirementJoinRow::into_row))
}

/// countRequirementsByStatus：`groupBy({ by: ["status"], _count: true })`。
///
/// 调用方把它摊成 `{ total, by_status }`。**不补零**——某个状态没有行时
/// 它就不出现在 map 里，而不是 `status: 0`（空项目的 `by_status` 是 `{}`）。
pub async fn count_requirements_by_status(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<(String, i64)>, AppError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status::text, COUNT(*)::bigint FROM requirement WHERE project_id = $1 \
         GROUP BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_released_at_accepts_rfc3339_and_bare_date() {
        // RFC3339 全形态
        assert!(parse_released_at("2026-01-02T03:04:05Z").is_ok());
        assert!(parse_released_at("2026-01-02T03:04:05.123+08:00").is_ok());
        // JS new Date("YYYY-MM-DD") 接受，按 UTC 零点
        assert!(parse_released_at("2026-01-02").is_ok());
    }

    #[test]
    fn parse_released_at_rejects_garbage_like_js_invalid_date() {
        // 非法串对齐 Invalid Date：交给调用方 500（而非 422），保持状态码一致
        assert!(parse_released_at("garbage").is_err());
        assert!(parse_released_at("2026-13-45").is_err());
    }
}
