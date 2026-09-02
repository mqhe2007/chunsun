//! 依赖关系业务服务（需求/缺陷 Blocking / Blocked By）。
//!
//! 语义约定：
//! - 边方向 `source blocks target`（source 不完成，target 无法开始）。
//! - 加边 `source → target` 前做循环检测：若 `target` 到 `source` 已有可达路径，则加边成环，
//!   拒绝并返回 409 `DEPENDENCY_CYCLE`。
//! - 节点必须真实存在（同项目内）：source / target 分别按 kind 校验需求或缺陷是否存在，
//!   不存在返回 404。
//! - 权限档与 requirement/defect 一致：只判项目可见性。

use sqlx::PgPool;

use crate::api::AppError;
use crate::core::dependency_graph::{DependencyGraph, Edge, Node, NodeKind};
use crate::repos::defect;
use crate::repos::dependency::{self, CreateDependencyInput, DependencyRow};
use crate::repos::requirement;
use crate::services::project_access::visible_project_id;

/// 依赖域失败分支。
#[derive(Debug, Clone, Copy)]
pub enum DependencyFailure {
    /// source 节点不存在 → 404
    SourceNotFound,
    /// target 节点不存在 → 404
    TargetNotFound,
    /// 加边会形成循环依赖 → 409
    Cycle,
    /// 边不存在（移除时）→ 404
    DependencyNotFound,
}

impl From<DependencyFailure> for AppError {
    fn from(f: DependencyFailure) -> Self {
        match f {
            DependencyFailure::SourceNotFound => AppError::not_found("DEPENDENCY_SOURCE_NOT_FOUND"),
            DependencyFailure::TargetNotFound => AppError::not_found("DEPENDENCY_TARGET_NOT_FOUND"),
            DependencyFailure::Cycle => AppError::conflict("DEPENDENCY_CYCLE"),
            DependencyFailure::DependencyNotFound => AppError::not_found("DEPENDENCY_NOT_FOUND"),
        }
    }
}

/// 节点类型字符串 → NodeKind，未知值由路由层已校验，这里兜底 None。
fn parse_kind(s: &str) -> Option<NodeKind> {
    NodeKind::parse(s)
}

/// 校验节点是否真实存在于该项目（按 kind 分派到需求/缺陷查询）。
async fn node_exists(
    pool: &PgPool,
    project_id: &str,
    kind: NodeKind,
    id: &str,
) -> Result<bool, AppError> {
    match kind {
        NodeKind::Requirement => {
            Ok(requirement::get_requirement_by_id(pool, id, project_id)
                .await?
                .is_some())
        }
        NodeKind::Defect => {
            Ok(defect::get_defect_by_id(pool, id, project_id).await?.is_some())
        }
    }
}

/// 把项目内全部依赖边构建成图。
async fn build_graph(pool: &PgPool, project_id: &str) -> Result<DependencyGraph, AppError> {
    let rows = dependency::list_all_in_project(pool, project_id).await?;
    let edges = rows.into_iter().map(|r| Edge {
        source: Node {
            kind: parse_kind(&r.source_type).expect("db kind valid"),
            id: r.source_id,
        },
        target: Node {
            kind: parse_kind(&r.target_type).expect("db kind valid"),
            id: r.target_id,
        },
    });
    Ok(DependencyGraph::from_edges(edges))
}

/// 新建需求/缺陷时携带的"被谁阻塞"引用列表（每条引用 = 一个上游节点）。
#[derive(Debug, Clone, Copy)]
pub struct BlockedByRef<'a> {
    pub kind: &'a str,
    pub id: &'a str,
}

/// 在**事务内**为新建节点 `target_id`（kind 已知）批量建立依赖边：
/// 对每个 `ref` 插入 `source = ref.kind/ref.id` → `target = (target_kind, target_id)`。
///
/// 调用方前置条件：
/// 1. 已对项目做过 `visible_project_id` 校验；
/// 2. 已对每个 `ref.kind` 校验是合法 NodeKind 且节点存在；
/// 3. 不要做循环检测——新建节点 id 不在图里，没环可成。
///
/// 重复引用由数据库唯一约束 `(source_type, source_id, target_type, target_id)` 兜底，
/// 命中时整个事务回滚（由调用方控制 tx 生命周期）。
///
/// `executor` 是 `&mut PgConnection`（sqlx `Transaction` 解引用后的连接）。
/// 在同一次事务里多次 `create_dependency` 复用同一个 `&mut PgConnection`，
/// 复用 `Executor` 的 `&mut PgConnection` 实现（`PgConnection` 本身不实现 `Executor`）。
pub async fn link_blocked_by_in_tx<'c>(
    executor: &'c mut sqlx::PgConnection,
    project_id: &str,
    target_kind: NodeKind,
    target_id: &str,
    refs: &[BlockedByRef<'_>],
) -> Result<Vec<DependencyRow>, AppError> {
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let row = dependency::create_dependency(
            &mut *executor,
            CreateDependencyInput {
                project_id,
                source_type: r.kind,
                source_id: r.id,
                target_type: target_kind.as_str(),
                target_id,
            },
        )
        .await?;
        out.push(row);
    }
    Ok(out)
}

/// 创建节点（需求/缺陷）携带的 `blockedBy` 引用前置校验：
/// - 每个 `ref.kind` 必须是合法 NodeKind（`requirement` / `defect`）；
/// - 对应节点必须真实存在于该项目；
/// - 重复 `(kind, id)` 视作同一上游，自动去重（不抛错）。
///
/// 任一引用节点不存在返回 404 `DEPENDENCY_TARGET_NOT_FOUND`。
pub async fn assert_blocked_by_refs_exist(
    pool: &PgPool,
    project_id: &str,
    refs: &[BlockedByRef<'_>],
) -> Result<(), AppError> {
    // 去重 + 过滤空 id
    let mut seen = std::collections::HashSet::new();
    let mut uniq: Vec<BlockedByRef<'_>> = Vec::new();
    for r in refs {
        if r.id.is_empty() {
            continue;
        }
        if seen.insert((r.kind.to_string(), r.id.to_string())) {
            uniq.push(*r);
        }
    }

    for r in &uniq {
        let kind = NodeKind::parse(r.kind).ok_or(DependencyFailure::TargetNotFound)?;
        if !node_exists(pool, project_id, kind, r.id).await? {
            return Err(DependencyFailure::TargetNotFound.into());
        }
    }
    Ok(())
}

pub struct AddDependencyArgs<'a> {
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
}

/// POST 添加依赖边 `source → target`（source blocks target）。
///
/// 顺序：项目可见性 → 两个节点存在性 → 循环检测 → 落库（事务）。
/// 循环检测在写入前基于当前图判断 `target` 是否可达 `source`。
pub async fn add_dependency(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    args: AddDependencyArgs<'_>,
) -> Result<DependencyRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let source_kind = parse_kind(args.source_type).ok_or(DependencyFailure::SourceNotFound)?;
    let target_kind = parse_kind(args.target_type).ok_or(DependencyFailure::TargetNotFound)?;

    // 自引用直接拒绝（source == target 且同类型）
    if source_kind == target_kind && args.source_id == args.target_id {
        return Err(DependencyFailure::Cycle.into());
    }

    if !node_exists(pool, &project_id, source_kind, args.source_id).await? {
        return Err(DependencyFailure::SourceNotFound.into());
    }
    if !node_exists(pool, &project_id, target_kind, args.target_id).await? {
        return Err(DependencyFailure::TargetNotFound.into());
    }

    // 循环检测：若 target 已可达 source，则加 source→target 会成环
    let graph = build_graph(pool, &project_id).await?;
    let source_node = Node { kind: source_kind, id: args.source_id.to_string() };
    let target_node = Node { kind: target_kind, id: args.target_id.to_string() };
    if graph.reaches(&target_node, &source_node) {
        return Err(DependencyFailure::Cycle.into());
    }

    let row = dependency::create_dependency(
        pool,
        CreateDependencyInput {
            project_id: &project_id,
            source_type: args.source_type,
            source_id: args.source_id,
            target_type: args.target_type,
            target_id: args.target_id,
        },
    )
    .await?;

    Ok(row)
}

pub struct RemoveDependencyArgs<'a> {
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
}

/// DELETE 移除依赖边。边不存在返回 404 `DEPENDENCY_NOT_FOUND`。
pub async fn remove_dependency(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    args: RemoveDependencyArgs<'_>,
) -> Result<DependencyRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let row = dependency::delete_dependency(
        pool,
        &project_id,
        args.source_type,
        args.source_id,
        args.target_type,
        args.target_id,
    )
    .await?
    .ok_or::<AppError>(DependencyFailure::DependencyNotFound.into())?;
    Ok(row)
}

/// 依赖列表（Blocking + Blocked By），带传递依赖。
#[derive(Debug, Clone)]
pub struct DependencySummary {
    pub blocking: Vec<DependencyNode>,
    pub blocked_by: Vec<DependencyNode>,
    pub transitive_blocking: Vec<DependencyNode>,
    pub transitive_blocked_by: Vec<DependencyNode>,
}

/// 依赖节点摘要（id / 类型 / 描述 / 状态，供前端展示）。
#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub id: String,
    pub kind: String,
    pub description: Option<String>,
    pub status: Option<String>,
}

/// GET 查询某节点的直接依赖 + 传递依赖。
///
/// - `blocking`：直接出边指向的节点（该节点阻塞了谁）。
/// - `blocked_by`：直接入边来源节点（谁阻塞了该节点）。
/// - `transitive_blocking`：从该节点出发可达的所有节点（传递阻塞集，BFS，不含自身）。
/// - `transitive_blocked_by`：反向图 BFS，能到达该节点的所有节点（传递被阻塞集，不含自身）。
pub async fn get_dependencies(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    node_type: &str,
    node_id: &str,
) -> Result<DependencySummary, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let kind = parse_kind(node_type).ok_or(DependencyFailure::SourceNotFound)?;

    if !node_exists(pool, &project_id, kind, node_id).await? {
        return Err(DependencyFailure::SourceNotFound.into());
    }

    let outgoing = dependency::list_outgoing(pool, &project_id, node_type, node_id).await?;
    let incoming = dependency::list_incoming(pool, &project_id, node_type, node_id).await?;
    let graph = build_graph(pool, &project_id).await?;

    let self_node = Node { kind, id: node_id.to_string() };

    // 直接依赖：从出边/入边的另一端解析节点摘要（先摊成 owned Vec，规避闭包 HRTB）
    let blocking_pairs: Vec<(String, String)> = outgoing
        .iter()
        .map(|r| (r.target_type.clone(), r.target_id.clone()))
        .collect();
    let blocked_by_pairs: Vec<(String, String)> = incoming
        .iter()
        .map(|r| (r.source_type.clone(), r.source_id.clone()))
        .collect();
    let blocking = summarize_nodes(pool, &project_id, blocking_pairs).await?;
    let blocked_by = summarize_nodes(pool, &project_id, blocked_by_pairs).await?;

    // 传递依赖：正向 BFS（阻塞谁）
    let transitive_blocking = {
        let nodes = graph.descendants(&self_node);
        let pairs: Vec<(String, String)> = nodes
            .iter()
            .map(|n| (n.kind.as_str().to_string(), n.id.clone()))
            .collect();
        summarize_nodes(pool, &project_id, pairs).await?
    };

    // 反向图：反向边 target → source，BFS 找「能到达 self 的节点」（被谁阻塞）
    let transitive_blocked_by = {
        let all = dependency::list_all_in_project(pool, &project_id).await?;
        let mut rev = DependencyGraph::new();
        for r in &all {
            let sn = Node { kind: parse_kind(&r.source_type).expect("valid"), id: r.source_id.clone() };
            let tn = Node { kind: parse_kind(&r.target_type).expect("valid"), id: r.target_id.clone() };
            rev.add_edge(tn, sn); // 反向：target → source
        }
        let nodes = rev.descendants(&self_node);
        let pairs: Vec<(String, String)> = nodes
            .iter()
            .map(|n| (n.kind.as_str().to_string(), n.id.clone()))
            .collect();
        summarize_nodes(pool, &project_id, pairs).await?
    };

    Ok(DependencySummary {
        blocking,
        blocked_by,
        transitive_blocking,
        transitive_blocked_by,
    })
}

/// 把一批 (type, id) 解析成节点摘要（查需求/缺陷拿到 description/status）。
/// 接受 owned `(String, String)` 元组，避免借用生命周期在多个调用点之间打架。
async fn summarize_nodes<I>(
    pool: &PgPool,
    project_id: &str,
    iter: I,
) -> Result<Vec<DependencyNode>, AppError>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut out = Vec::new();
    for (t, id) in iter {
        let kind = parse_kind(&t);
        let Some(kind) = kind else { continue };
        let node = match kind {
            NodeKind::Requirement => {
                requirement::get_requirement_by_id(pool, &id, project_id)
                    .await?
                    .map(|r| DependencyNode {
                        id: r.id,
                        kind: "requirement".into(),
                        description: Some(r.description),
                        status: Some(r.status),
                    })
            }
            NodeKind::Defect => defect::get_defect_by_id(pool, &id, project_id)
                .await?
                .map(|d| DependencyNode {
                    id: d.id,
                    kind: "defect".into(),
                    description: d.description,
                    status: Some(d.status),
                }),
        };
        if let Some(n) = node {
            out.push(n);
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────
// Agent 依赖感知与调度（DAG 任务执行编排）
//
// 语义约定（与 dependency_graph.rs 一致，source → target = source blocks target）：
// - 「完成」判定：requirement status == "completed"；defect status ∈ {resolved, closed}。
// - 「被阻塞」：节点存在任一未完成的直接前置（Blocked By 中存在非完成节点）。
// - 「可执行」：节点的所有直接前置均已完成。
// ─────────────────────────────────────────────────────────────

/// 节点「是否已完成」判定（需求 completed；缺陷 resolved/closed）。
pub fn node_is_done(kind: &NodeKind, status: &str) -> bool {
    match kind {
        NodeKind::Requirement => status == "completed",
        NodeKind::Defect => matches!(status, "resolved" | "closed"),
    }
}

/// 调度分析用节点（含状态）。
#[derive(Debug, Clone)]
pub struct ScheduleNode {
    pub id: String,
    pub kind: NodeKind,
    pub description: Option<String>,
    pub status: String,
}

impl ScheduleNode {
    pub fn done(&self) -> bool {
        node_is_done(&self.kind, &self.status)
    }

    pub fn node(&self) -> Node {
        Node {
            kind: self.kind,
            id: self.id.clone(),
        }
    }
}

/// 单节点阻塞状态（供 Agent 判断「是否被阻塞」「阻塞原因」「是否可执行」）。
#[derive(Debug, Clone)]
pub struct BlockedStatus {
    pub node: ScheduleNode,
    /// 是否被阻塞（存在未完成前置）。
    pub blocked: bool,
    /// 未完成前置（Blocked By 中非完成的直接前置）——阻塞原因。
    pub blockers: Vec<ScheduleNode>,
    /// 已完成前置（不阻塞）。
    pub completed_blockers: Vec<ScheduleNode>,
}

/// 全项目调度分析结果。
#[derive(Debug, Clone)]
pub struct ScheduleAnalysis {
    /// 拓扑分层：每层内的节点互无依赖、可并行；层间按序串行。不含已完成节点。
    pub levels: Vec<Vec<ScheduleNode>>,
    /// 未完成节点中的关键路径（最长依赖链）。
    pub critical_path: Vec<ScheduleNode>,
    /// 各未完成节点的阻塞状态。
    pub blocked: Vec<BlockedStatus>,
    /// 当前可执行（无未完成前置）的未完成节点。
    pub ready: Vec<ScheduleNode>,
    /// 统计。
    pub stats: ScheduleStats,
}

#[derive(Debug, Clone, Default)]
pub struct ScheduleStats {
    pub total: usize,
    pub done: usize,
    pub pending: usize,
    pub blocked: usize,
    pub ready: usize,
}

/// 解锁分析：模拟某节点完成后，其直接下游中哪些解锁。
#[derive(Debug, Clone)]
pub struct UnlockAnalysis {
    /// 刚完成的节点。
    pub node: ScheduleNode,
    /// 解锁的直接下游（所有前置已完成，进入可执行状态）。
    pub unlocked: Vec<ScheduleNode>,
    /// 仍被阻塞的直接下游（存在其他未完成前置）。
    pub still_blocked: Vec<ScheduleNode>,
}

/// 拉取项目内全部节点（需求 + 缺陷），统一成 `ScheduleNode`。
async fn load_all_nodes(pool: &PgPool, project_id: &str) -> Result<Vec<ScheduleNode>, AppError> {
    let reqs = requirement::list_requirements_by_project(
        pool,
        project_id,
        requirement::RequirementListFilters::default(),
    )
    .await?;
    let defects = defect::list_defects_by_project(pool, project_id, defect::DefectListFilters::default())
        .await?;

    let mut out = Vec::with_capacity(reqs.items.len() + defects.len());
    for r in reqs.items {
        out.push(ScheduleNode {
            id: r.id,
            kind: NodeKind::Requirement,
            description: Some(r.description),
            status: r.status,
        });
    }
    for d in defects {
        out.push(ScheduleNode {
            id: d.id,
            kind: NodeKind::Defect,
            description: d.description,
            status: d.status,
        });
    }
    Ok(out)
}

fn build_graph_from_rows(rows: &[dependency::DependencyRow]) -> DependencyGraph {
    DependencyGraph::from_edges(rows.iter().map(|r| Edge {
        source: Node {
            kind: parse_kind(&r.source_type).expect("db kind valid"),
            id: r.source_id.clone(),
        },
        target: Node {
            kind: parse_kind(&r.target_type).expect("db kind valid"),
            id: r.target_id.clone(),
        },
    }))
}

/// 已完成节点集合（HashSet<Node>）。
fn done_set(nodes: &[ScheduleNode]) -> std::collections::HashSet<Node> {
    nodes.iter().filter(|n| n.done()).map(|n| n.node()).collect()
}

fn node_map(nodes: &[ScheduleNode]) -> std::collections::HashMap<Node, ScheduleNode> {
    nodes.iter().map(|n| (n.node(), n.clone())).collect()
}

/// 计算单节点阻塞状态。
fn compute_blocked_status(
    node: &ScheduleNode,
    graph: &DependencyGraph,
    by_node: &std::collections::HashMap<Node, ScheduleNode>,
) -> BlockedStatus {
    let mut blockers = Vec::new();
    let mut completed_blockers = Vec::new();
    for pred in graph.direct_predecessors(&node.node()) {
        if let Some(p) = by_node.get(&pred) {
            if p.done() {
                completed_blockers.push(p.clone());
            } else {
                blockers.push(p.clone());
            }
        }
    }
    BlockedStatus {
        node: node.clone(),
        blocked: !blockers.is_empty(),
        blockers,
        completed_blockers,
    }
}

/// GET 全项目调度分析（拓扑分层 / 关键路径 / 阻塞状态 / 可执行集合）。
pub async fn analyze_schedule(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<ScheduleAnalysis, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let rows = dependency::list_all_in_project(pool, &project_id).await?;
    let graph = build_graph_from_rows(&rows);
    let all_nodes = load_all_nodes(pool, &project_id).await?;
    let done = done_set(&all_nodes);
    let by_node = node_map(&all_nodes);

    // 拓扑分层（未完成节点）
    let levels: Vec<Vec<ScheduleNode>> = graph
        .schedule_levels(&done)
        .into_iter()
        .map(|level| {
            let mut nodes: Vec<ScheduleNode> = level
                .into_iter()
                .filter_map(|n| by_node.get(&n).cloned())
                .collect();
            nodes.sort_by(|a, b| a.id.cmp(&b.id));
            nodes
        })
        .collect();

    // 关键路径（未完成节点）
    let critical_path: Vec<ScheduleNode> = graph
        .critical_path(&done)
        .into_iter()
        .filter_map(|n| by_node.get(&n).cloned())
        .collect();

    // 阻塞状态：所有未完成节点
    let mut blocked: Vec<BlockedStatus> = all_nodes
        .iter()
        .filter(|n| !n.done())
        .map(|n| compute_blocked_status(n, &graph, &by_node))
        .collect();
    blocked.sort_by(|a, b| a.node.id.cmp(&b.node.id));

    let ready: Vec<ScheduleNode> = blocked
        .iter()
        .filter(|b| !b.blocked)
        .map(|b| b.node.clone())
        .collect();

    let stats = ScheduleStats {
        total: all_nodes.len(),
        done: done.len(),
        pending: all_nodes.len() - done.len(),
        blocked: blocked.iter().filter(|b| b.blocked).count(),
        ready: ready.len(),
    };

    Ok(ScheduleAnalysis {
        levels,
        critical_path,
        blocked,
        ready,
        stats,
    })
}

/// GET 单节点阻塞状态：是否被阻塞 + 阻塞原因（未完成前置）+ 是否可执行。
pub async fn get_node_blocked_status(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    node_type: &str,
    node_id: &str,
) -> Result<BlockedStatus, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let kind = parse_kind(node_type).ok_or(DependencyFailure::SourceNotFound)?;

    if !node_exists(pool, &project_id, kind, node_id).await? {
        return Err(DependencyFailure::SourceNotFound.into());
    }

    let rows = dependency::list_all_in_project(pool, &project_id).await?;
    let graph = build_graph_from_rows(&rows);
    let all_nodes = load_all_nodes(pool, &project_id).await?;
    let by_node = node_map(&all_nodes);

    let node = by_node
        .get(&Node { kind, id: node_id.to_string() })
        .cloned()
        .ok_or(DependencyFailure::SourceNotFound)?;

    Ok(compute_blocked_status(&node, &graph, &by_node))
}

/// GET 解锁分析：模拟某节点完成后，其直接下游中哪些解锁、哪些仍被阻塞。
pub async fn analyze_unlock(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    node_type: &str,
    node_id: &str,
) -> Result<UnlockAnalysis, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let kind = parse_kind(node_type).ok_or(DependencyFailure::SourceNotFound)?;

    if !node_exists(pool, &project_id, kind, node_id).await? {
        return Err(DependencyFailure::SourceNotFound.into());
    }

    let rows = dependency::list_all_in_project(pool, &project_id).await?;
    let graph = build_graph_from_rows(&rows);
    let all_nodes = load_all_nodes(pool, &project_id).await?;
    let mut done = done_set(&all_nodes);
    done.insert(Node { kind, id: node_id.to_string() });

    let by_node = node_map(&all_nodes);
    let node = by_node
        .get(&Node { kind, id: node_id.to_string() })
        .cloned()
        .ok_or(DependencyFailure::SourceNotFound)?;

    let unlocked_nodes = graph.unlockable_after(&node.node(), &done);
    let mut unlocked: Vec<ScheduleNode> = unlocked_nodes
        .into_iter()
        .filter_map(|n| by_node.get(&n).cloned())
        .collect();
    unlocked.sort_by(|a, b| a.id.cmp(&b.id));

    // 仍被阻塞的直接下游：未被解锁但存在该节点出边指向的未完成节点
    let mut still_blocked: Vec<ScheduleNode> = graph
        .direct_successors(&node.node())
        .into_iter()
        .filter(|n| !done.contains(n))
        .filter(|n| !unlocked.iter().any(|u| &u.node() == n))
        .filter_map(|n| by_node.get(&n).cloned())
        .collect();
    still_blocked.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(UnlockAnalysis {
        node,
        unlocked,
        still_blocked,
    })
}
