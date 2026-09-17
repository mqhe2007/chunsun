//! 项目知识文档 + 项目宪法的表访问
//! （对齐 `projectKnowledgeDocumentRepository.ts` / `projectPolicyRepository.ts`）。
//!
//! 兼容要点：
//! - 两张表主键都是 `nanoid(12)`，`updated_at` 由应用层维护（Prisma `@updatedAt`）。
//! - 列表排序固定 `sortOrder asc, createdAt desc`——**两级排序缺一不可**，
//!   同 sortOrder 时新文档在前。层级展开（前序）在此基础上做：
//!   [`list_knowledge_document_nodes`] 取行后由 [`build_forest`] 组树，父后紧跟其子树；
//!   `sort_order` 只在同级间比较，因此全局递增分配的顺序天然满足同级排序。
//! - [`update_knowledge_document`] 在「没有任何字段要写」时**不发 UPDATE**：
//!   Prisma 对空 `data` 会退化成纯读，`@updatedAt` 不刷新。实测确认
//!   （`PUT {}` 间隔 1.3s 两次，`updatedAt` 逐字节相同），而同值写入
//!   （`PUT {"title":"B"}` 写回原值）**照常刷新**。这个区别必须复刻。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// Prisma 的 `@default(now())` / `@updatedAt` 由应用层生成，统一走这里。
fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, FromRow)]
pub struct KnowledgeDocRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i32,
    pub load_strategy: String,
    pub parent_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// 前序展开后的文档节点：`depth` 从根（0）算起。
/// 直接子文档数由消费方按 `parentId` 统计（CLI/Console 都拿得到全量列表），不在这里冗余。
#[derive(Debug, Clone)]
pub struct KnowledgeDocNode {
    pub doc: KnowledgeDocRow,
    pub depth: usize,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProjectPolicyRow {
    pub constitution_md: String,
    pub updated_at: DateTime<Utc>,
}

const DOC_COLS: &str =
    "id, title, content, sort_order, load_strategy, parent_id, created_at, updated_at";

/// getProjectPolicy：`findUnique({ where: { projectId } })`，缺行返回 None（**不建行**）。
pub async fn get_project_policy(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<ProjectPolicyRow>, AppError> {
    let row = sqlx::query_as::<_, ProjectPolicyRow>(
        "SELECT constitution_md, updated_at FROM project_policy WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// upsertProjectPolicy：`ON CONFLICT (project_id)` 对齐 Prisma 的 upsert。
///
/// 路由层 `content` 是必填的，所以 create / update 两个分支都会写 `constitution_md`，
/// 不需要复刻 Prisma update 分支里那个 `?? {}` 的条件展开。
pub async fn upsert_project_policy(
    pool: &PgPool,
    project_id: &str,
    constitution_md: &str,
) -> Result<ProjectPolicyRow, AppError> {
    let ts = now();
    let row = sqlx::query_as::<_, ProjectPolicyRow>(
        "INSERT INTO project_policy (id, project_id, constitution_md, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $4) \
         ON CONFLICT (project_id) DO UPDATE SET constitution_md = EXCLUDED.constitution_md, \
           updated_at = EXCLUDED.updated_at \
         RETURNING constitution_md, updated_at",
    )
    .bind(nanoid(12))
    .bind(project_id)
    .bind(constitution_md)
    .bind(ts)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// listContextDocuments：`orderBy: [{ sortOrder: "asc" }, { createdAt: "desc" }]`，
/// 再按 parent_id 组树做**前序展开**（父后紧跟子树）。
///
/// 全量取行后在内存组树（知识文档量级小），好处是策略过滤也不破坏 depth/parentId：
/// 调用方拿到的是绝对层级，过滤只删条目、不改树形。
pub async fn list_knowledge_document_nodes(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<KnowledgeDocNode>, AppError> {
    let sql = format!(
        "SELECT {DOC_COLS} FROM project_knowledge_document WHERE project_id = $1 \
         ORDER BY sort_order ASC, created_at DESC"
    );
    let rows = sqlx::query_as::<_, KnowledgeDocRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(build_forest(rows))
}

/// 前序组树（纯函数，单测覆盖）：
/// - 根 = `parent_id` 为空，或父 id 不在本批数据里（孤儿按根处理，绝不丢文档）；
/// - 同一父下的子保持输入顺序（即 sort_order asc, created_at desc）；
/// - 数据被手改成环时用 visited 兜底：剩余节点按根补在末尾，不挂死、不重复。
fn build_forest(rows: Vec<KnowledgeDocRow>) -> Vec<KnowledgeDocNode> {
    use std::collections::{HashMap, HashSet};

    let ids: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        if let Some(p) = r.parent_id.as_deref() {
            if p != r.id && ids.contains(p) {
                children.entry(p).or_default().push(i);
            }
        }
    }

    fn walk(
        i: usize,
        depth: usize,
        rows: &[KnowledgeDocRow],
        children: &HashMap<&str, Vec<usize>>,
        visited: &mut [bool],
        out: &mut Vec<KnowledgeDocNode>,
    ) {
        if visited[i] {
            return;
        }
        visited[i] = true;
        let kids: &[usize] = children.get(rows[i].id.as_str()).map_or(&[], Vec::as_slice);
        out.push(KnowledgeDocNode {
            doc: rows[i].clone(),
            depth,
        });
        for &k in kids {
            walk(k, depth + 1, rows, children, visited, out);
        }
    }

    let mut visited = vec![false; rows.len()];
    let mut out = Vec::with_capacity(rows.len());
    for i in 0..rows.len() {
        let is_root = match rows[i].parent_id.as_deref() {
            None => true,
            Some(p) => !ids.contains(p),
        };
        if is_root {
            walk(i, 0, &rows, &children, &mut visited, &mut out);
        }
    }
    // 环内节点（无根可达）兜底按根补上，保证不丢文档
    for i in 0..rows.len() {
        if !visited[i] {
            walk(i, 0, &rows, &children, &mut visited, &mut out);
        }
    }
    out
}

/// `findFirst({ where: { id, projectId } })`：**双条件**，防止跨项目拿到别人的文档。
pub async fn find_knowledge_document(
    pool: &PgPool,
    project_id: &str,
    id: &str,
) -> Result<Option<KnowledgeDocRow>, AppError> {
    let sql = format!("SELECT {DOC_COLS} FROM project_knowledge_document WHERE id = $1 AND project_id = $2");
    let row = sqlx::query_as::<_, KnowledgeDocRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// createContextDocument：`sortOrder = (aggregate._max.sortOrder ?? -1) + 1`。
///
/// 注意 max 取的是**当前项目**内的最大值，且可能是负数（旧文档被手工改成 -5 时，
/// 新文档就是 -4）——实测确认过递增基准就是这个 max，不是行数。
///
/// `load_strategy` 为 None 时默认 'eager'；`parent_id` 为 None 时建为根文档。
pub async fn create_knowledge_document(
    pool: &PgPool,
    project_id: &str,
    title: &str,
    content: &str,
    load_strategy: Option<&str>,
    parent_id: Option<&str>,
) -> Result<KnowledgeDocRow, AppError> {
    let max: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(sort_order) FROM project_knowledge_document WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    // JS 里 `max + 1` 不会溢出（IEEE754），Prisma 才在写库时报范围错。
    let sort_order = max.unwrap_or(-1).checked_add(1).ok_or_else(|| {
        AppError::internal("SORT_ORDER_OVERFLOW: max sortOrder is already i32::MAX")
    })?;

    let strategy = load_strategy.unwrap_or("eager");
    let ts = now();
    let sql = format!(
        "INSERT INTO project_knowledge_document \
           (id, project_id, title, content, sort_order, load_strategy, parent_id, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) RETURNING {DOC_COLS}"
    );
    let row = sqlx::query_as::<_, KnowledgeDocRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(title.trim())
        .bind(content)
        .bind(sort_order)
        .bind(strategy)
        .bind(parent_id)
        .bind(ts)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// updateContextDocument 的写入部分（调用方已确认行存在）。
///
/// 各字段都是 `Option`：`None` = 请求里没这个 key = 不写。**全 None 时直接不发
/// UPDATE**，对齐 Prisma 空 `data` 不刷新 `@updatedAt` 的行为。
///
/// `parent_id` 是双层 Option：外层 None = 不改，内层 None = 显式解除关联（写 NULL）。
pub async fn update_knowledge_document(
    pool: &PgPool,
    existing: &KnowledgeDocRow,
    title: Option<&str>,
    content: Option<&str>,
    sort_order: Option<i32>,
    load_strategy: Option<&str>,
    parent_id: Option<Option<&str>>,
) -> Result<KnowledgeDocRow, AppError> {
    if title.is_none()
        && content.is_none()
        && sort_order.is_none()
        && load_strategy.is_none()
        && parent_id.is_none()
    {
        return Ok(existing.clone());
    }

    let mut sets: Vec<String> = Vec::new();
    let mut idx = 1;
    if title.is_some() {
        sets.push(format!("title = ${idx}"));
        idx += 1;
    }
    if content.is_some() {
        sets.push(format!("content = ${idx}"));
        idx += 1;
    }
    if sort_order.is_some() {
        sets.push(format!("sort_order = ${idx}"));
        idx += 1;
    }
    if load_strategy.is_some() {
        sets.push(format!("load_strategy = ${idx}"));
        idx += 1;
    }
    if parent_id.is_some() {
        sets.push(format!("parent_id = ${idx}"));
        idx += 1;
    }
    sets.push(format!("updated_at = ${idx}"));
    idx += 1;

    let sql = format!(
        "UPDATE project_knowledge_document SET {} WHERE id = ${idx} RETURNING {DOC_COLS}",
        sets.join(", ")
    );
    let mut q = sqlx::query_as::<_, KnowledgeDocRow>(&sql);
    if let Some(t) = title {
        // 旧仓储层的 `data.title.trim()`——路由层不 trim，trim 只在这里发生
        q = q.bind(t.trim().to_string());
    }
    if let Some(c) = content {
        q = q.bind(c.to_string());
    }
    if let Some(s) = sort_order {
        q = q.bind(s);
    }
    if let Some(ls) = load_strategy {
        q = q.bind(ls.to_string());
    }
    if let Some(p) = parent_id {
        q = q.bind(p.map(str::to_string));
    }
    let row = q.bind(now()).bind(&existing.id).fetch_one(pool).await?;
    Ok(row)
}

/// deleteContextDocument 的删除部分（调用方已确认行存在）。
pub async fn delete_knowledge_document(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_knowledge_document WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 直接子文档数（删除守卫与树形展示用）。
pub async fn count_children(
    pool: &PgPool,
    project_id: &str,
    doc_id: &str,
) -> Result<i64, AppError> {
    let n = sqlx::query_scalar(
        "SELECT COUNT(*) FROM project_knowledge_document WHERE project_id = $1 AND parent_id = $2",
    )
    .bind(project_id)
    .bind(doc_id)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// 整棵子树的 id（**不含根**），递归 CTE；用 UNION（非 ALL）去重防手改出的环。
pub async fn list_descendant_ids(
    pool: &PgPool,
    project_id: &str,
    doc_id: &str,
) -> Result<Vec<String>, AppError> {
    let ids = sqlx::query_scalar(
        "WITH RECURSIVE sub AS ( \
           SELECT id FROM project_knowledge_document WHERE parent_id = $1 AND project_id = $2 \
           UNION \
           SELECT d.id FROM project_knowledge_document d JOIN sub ON d.parent_id = sub.id \
             WHERE d.project_id = $2 \
         ) SELECT id FROM sub",
    )
    .bind(doc_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

/// `candidate_id` 是否落在 `doc_id` 的子树内（**含 doc 自身**）——环校验用。
pub async fn is_in_subtree(
    pool: &PgPool,
    project_id: &str,
    doc_id: &str,
    candidate_id: &str,
) -> Result<bool, AppError> {
    let exists: bool = sqlx::query_scalar(
        "WITH RECURSIVE sub AS ( \
           SELECT id FROM project_knowledge_document WHERE id = $1 AND project_id = $2 \
           UNION \
           SELECT d.id FROM project_knowledge_document d JOIN sub ON d.parent_id = sub.id \
             WHERE d.project_id = $2 \
         ) SELECT EXISTS(SELECT 1 FROM sub WHERE id = $3)",
    )
    .bind(doc_id)
    .bind(project_id)
    .bind(candidate_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, parent_id: Option<&str>) -> KnowledgeDocRow {
        KnowledgeDocRow {
            id: id.to_string(),
            title: id.to_string(),
            content: String::new(),
            sort_order: 0,
            load_strategy: "eager".to_string(),
            parent_id: parent_id.map(str::to_string),
            updated_at: Utc::now(),
        }
    }

    fn layout(nodes: &[KnowledgeDocNode]) -> Vec<(String, usize)> {
        nodes
            .iter()
            .map(|n| (n.doc.id.clone(), n.depth))
            .collect()
    }

    #[test]
    fn forest_keeps_flat_docs_at_root() {
        let out = build_forest(vec![row("a", None), row("b", None)]);
        assert_eq!(layout(&out), vec![
            ("a".into(), 0),
            ("b".into(), 0),
        ]);
    }

    #[test]
    fn forest_puts_children_right_after_parent_in_input_order() {
        // 输入顺序 = sort_order asc, created_at desc
        let out = build_forest(vec![
            row("p", None),
            row("c1", Some("p")),
            row("c2", Some("p")),
            row("g", Some("c1")),
        ]);
        assert_eq!(layout(&out), vec![
            ("p".into(), 0),
            ("c1".into(), 1),
            ("g".into(), 2),
            ("c2".into(), 1),
        ]);
    }

    #[test]
    fn forest_treats_orphans_as_roots_without_losing_them() {
        let out = build_forest(vec![row("root", None), row("orphan", Some("missing"))]);
        assert_eq!(layout(&out), vec![
            ("root".into(), 0),
            ("orphan".into(), 0),
        ]);
    }

    #[test]
    fn forest_survives_cycles_without_hanging_or_duplicating() {
        // 手改数据成环：a ↔ b；两个节点都必须出现且只出现一次
        let out = build_forest(vec![row("a", Some("b")), row("b", Some("a")), row("solo", None)]);
        let mut ids: Vec<&str> = out.iter().map(|n| n.doc.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["a", "b", "solo"]);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn forest_breaks_self_parent_cycle() {
        let out = build_forest(vec![row("self", Some("self"))]);
        assert_eq!(layout(&out), vec![("self".into(), 0)]);
    }
}
