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
