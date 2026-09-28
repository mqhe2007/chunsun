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

/// 乐观锁写入的三态结果。
///
/// **为什么不用 `rows_affected == 0` 直接判冲突**：条件更新落空有两种成因 ——
/// 行不存在（该报 404）与 revision 不匹配（该报 409）。两者在 `rows_affected`
/// 上不可区分，混为一谈会把「文档已被删除」误报成「版本冲突」。
///
/// `Conflict` 带出当前整行：409 响应要附最新正文供调用方合并，而这时再查一次
/// 是同一个查询的重复，不如在落空的那次里一并取回。
#[derive(Debug)]
pub enum WriteOutcome<T> {
    Ok(T),
    NotFound,
    Conflict(Box<T>),
}

/// 纯函数：给定「客户端声称的 revision」与「库里当前行的 revision」，判定是否放行。
///
/// 抽成纯函数是因为 repo 层的 SQL **没有任何测试基础设施**（全仓无 `sqlx::test`、
/// 无 testcontainers），版本比较这种容易写反的逻辑必须能在 `#[test]` 下覆盖到。
///
/// `expected = 0` 是「我认为这行还不存在」的约定，用于首次写宪法 / 首次写记忆。
/// 因此它只在行确实不存在时成立；行已存在但客户端传 0，是真冲突（有人抢先建了）。
pub fn revision_matches(expected: i32, current: i32) -> bool {
    expected == current
}

/// 纯函数：条件更新落空后，凭「重新查到的当前行」判定结果。
///
/// `None` 表示落空重查也没查到 —— 行确实没了，是 `NotFound`。
pub fn classify_miss<T>(current: Option<T>) -> WriteOutcome<T> {
    match current {
        Some(row) => WriteOutcome::Conflict(Box::new(row)),
        None => WriteOutcome::NotFound,
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct KnowledgeDocRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i32,
    pub load_strategy: String,
    pub parent_id: Option<String>,
    pub revision: i32,
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
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

const DOC_COLS: &str =
    "id, title, content, sort_order, load_strategy, parent_id, revision, created_at, updated_at";

/// getProjectPolicy：`findUnique({ where: { projectId } })`，缺行返回 None（**不建行**）。
pub async fn get_project_policy(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<ProjectPolicyRow>, AppError> {
    let row = sqlx::query_as::<_, ProjectPolicyRow>(
        "SELECT constitution_md, revision, updated_at FROM project_policy WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

const POLICY_COLS: &str = "constitution_md, revision, updated_at";

/// upsertProjectPolicy，带乐观锁。
///
/// **从单条 `ON CONFLICT DO UPDATE` 改成两段式**，理由不是性能而是可判定性：
/// 把 revision 谓词写在 `DO UPDATE ... WHERE` 上时，谓词不成立会**静默不更新并返回 0 行**，
/// 与「IN 的表压根没这行」在 `rows_affected` 上完全同形，无法区分 404 与 409。
/// 拆成「先条件 UPDATE，落空再决定插入/冲突」后，两条分支的下场各自明确，
/// 且与另外三个写入点形态一致 —— 形态一致比省一次查询重要。
///
/// `expected_revision = 0` 表示调用方认为宪法尚不存在（首次写入）。
pub async fn upsert_project_policy(
    pool: &PgPool,
    project_id: &str,
    constitution_md: &str,
    expected_revision: i32,
) -> Result<WriteOutcome<ProjectPolicyRow>, AppError> {
    let ts = now();

    // 第一段：按 revision 条件更新。命中即返回。
    let updated = sqlx::query_as::<_, ProjectPolicyRow>(&format!(
        "UPDATE project_policy SET constitution_md = $1, revision = revision + 1, updated_at = $2 \
         WHERE project_id = $3 AND revision = $4 RETURNING {POLICY_COLS}"
    ))
    .bind(constitution_md)
    .bind(ts)
    .bind(project_id)
    .bind(expected_revision)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = updated {
        return Ok(WriteOutcome::Ok(row));
    }

    // 第二段：落空。先看行在不在，再决定是冲突还是需要插入。
    let current = get_project_policy(pool, project_id).await?;
    if let Some(row) = current {
        // 行存在但 revision 不符 —— 有人抢先写了（或调用方拿的是旧版本号）。
        return Ok(WriteOutcome::Conflict(Box::new(row)));
    }

    // 行不存在。只有 `expected_revision = 0` 才允许建，否则是调用方声称的版本
    // 与「不存在」这一事实矛盾，属于冲突而不是创建。
    if !revision_matches(expected_revision, 0) {
        return Ok(WriteOutcome::NotFound);
    }

    // 并发下两个请求可能同时走到这里，`ON CONFLICT` 兜住后者：它撞上唯一约束，
    // 转成条件更新重新判定（此时 expected_revision 是 0，而库里已是 1，必然是冲突）。
    let inserted = sqlx::query_as::<_, ProjectPolicyRow>(&format!(
        "INSERT INTO project_policy (id, project_id, constitution_md, revision, created_at, updated_at) \
         VALUES ($1, $2, $3, 1, $4, $4) \
         ON CONFLICT (project_id) DO NOTHING \
         RETURNING {POLICY_COLS}"
    ))
    .bind(nanoid(12))
    .bind(project_id)
    .bind(constitution_md)
    .bind(ts)
    .fetch_optional(pool)
    .await?;

    match inserted {
        Some(row) => Ok(WriteOutcome::Ok(row)),
        // 撞上并发插入：别人先建了，本次是丢失更新，报冲突并带出当前内容。
        None => Ok(classify_miss(get_project_policy(pool, project_id).await?)),
    }
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
           (id, project_id, title, content, sort_order, load_strategy, parent_id, revision, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8, $8) RETURNING {DOC_COLS}"
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

/// updateContextDocument 的写入部分，带乐观锁。
///
/// 各字段都是 `Option`：`None` = 请求里没这个 key = 不写。
///
/// **空补丁不再在这里短路**。旧实现对「全 None」直接 `Ok(existing.clone())`，那是复刻
/// Prisma 空 `data` 不刷新 `@updatedAt` 的行为。但新协议要求写入必须携带 revision，
/// 而空补丁 + 带 revision 仍然不是一个有意义的写请求 —— 该由 handler 层拒绝（400），
/// 而不是在这里变成一个「看起来成功、其实什么都没做」的响应，那会掩盖调用方的 bug。
///
/// `parent_id` 是双层 Option：外层 None = 不改，内层 None = 显式解除关联（写 NULL）。
///
/// `expected_revision` 参与 WHERE 谓词：命中则写并 `revision + 1`，落空则重查一次
/// 判定 `Conflict` / `NotFound`。
pub async fn update_knowledge_document(
    pool: &PgPool,
    doc_id: &str,
    expected_revision: i32,
    title: Option<&str>,
    content: Option<&str>,
    sort_order: Option<i32>,
    load_strategy: Option<&str>,
    parent_id: Option<Option<&str>>,
) -> Result<WriteOutcome<KnowledgeDocRow>, AppError> {
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
    sets.push(format!("revision = revision + 1"));
    sets.push(format!("updated_at = ${idx}"));
    idx += 1;

    let id_placeholder = idx;
    idx += 1;
    let revision_placeholder = idx;

    let sql = format!(
        "UPDATE project_knowledge_document SET {} \
         WHERE id = ${id_placeholder} AND revision = ${revision_placeholder} \
         RETURNING {DOC_COLS}",
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
    let updated = q
        .bind(now())
        .bind(doc_id)
        .bind(expected_revision)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = updated {
        return Ok(WriteOutcome::Ok(row));
    }

    // 落空：重查一次区分「行没了」与「版本不符」。
    let current = get_knowledge_doc_by_id(pool, doc_id).await?;
    Ok(classify_miss(current))
}

/// 按 id 取单行（乐观锁落空后的重查，以及 409 组装时取当前正文）。
pub async fn get_knowledge_doc_by_id(
    pool: &PgPool,
    doc_id: &str,
) -> Result<Option<KnowledgeDocRow>, AppError> {
    let sql = format!("SELECT {DOC_COLS} FROM project_knowledge_document WHERE id = $1");
    let row = sqlx::query_as::<_, KnowledgeDocRow>(&sql)
        .bind(doc_id)
        .fetch_optional(pool)
        .await?;
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
            revision: 1,
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

    // ---------- 乐观锁判定 ----------
    //
    // 这四个写入点（知识文档 / 宪法 / 项目记忆 / 需求记忆）的行为**完全依赖**
    // 下面两个纯函数，而 repo 层没有连库测试基础设施（无 `sqlx::test`、
    // 无 testcontainers），SQL 本身只能在真实 Postgres 上手测。
    // 所以把「拿到什么结果该判成什么」抽出来钉死在这里——它是这套保护里唯一
    // 可以在 CI 上验证的一环。

    #[test]
    fn revision_matches_only_on_exact_equality() {
        assert!(revision_matches(7, 7));
        assert!(revision_matches(0, 0));
        // 只增不减：并发写入让库里版本更高时，旧版本号必须落空
        assert!(!revision_matches(6, 7));
        // 库里版本更低同样落空 —— 客户端拿的是未来版本号，说明它读到了别处的数据
        assert!(!revision_matches(8, 7));
    }

    #[test]
    fn classify_miss_with_a_current_row_is_a_conflict() {
        let outcome = classify_miss(Some(row("d1", None)));
        match outcome {
            WriteOutcome::Conflict(current) => assert_eq!(current.id, "d1"),
            other => panic!("期望 Conflict，得到 {other:?}"),
        }
    }

    /// 判定的**关键分界**：条件 UPDATE 落空后重查，查不到就是行没了。
    /// 若这里错判成 Conflict，删掉的文档会变成 409「版本冲突」，
    /// 客户端会拿 `currentContent` 去合并一份根本不存在的正文。
    #[test]
    fn classify_miss_without_a_current_row_is_not_found() {
        let outcome: WriteOutcome<KnowledgeDocRow> = classify_miss(None);
        assert!(matches!(outcome, WriteOutcome::NotFound));
    }

    /// `revision = 0` 的「我认为这行还不存在」语义。
    /// 它只在**行确实不存在**时才允许走到建行分支；行存在时是冲突（已经在别处建过）。
    #[test]
    fn revision_zero_is_the_create_intent_not_a_valid_existing_revision() {
        assert!(revision_matches(0, 0), "首次创建：声称不存在，库里也不存在");
        assert!(
            !revision_matches(0, 1),
            "行已存在（revision=1）时，声称「不存在」的写必须落空"
        );
    }
}
