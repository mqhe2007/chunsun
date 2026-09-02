//! 依赖关系图的纯函数：DAG 有向边建模与循环检测。
//!
//! 依赖边的语义是 `source blocks target`（source 不完成，target 无法开始），
//! 即有向边 source → target。加边 `source → target` 前必须确认：target 到 source
//! 不存在可达路径（否则加边后形成环）。
//!
//! 节点用 `(kind, id)` 二元组标识，kind ∈ {"requirement", "defect"}。

/// 节点类型（需求 / 缺陷）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Requirement,
    Defect,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Requirement => "requirement",
            NodeKind::Defect => "defect",
        }
    }

    /// 从字符串解析，未知值返回 `None`。
    pub fn parse(s: &str) -> Option<NodeKind> {
        match s {
            "requirement" => Some(NodeKind::Requirement),
            "defect" => Some(NodeKind::Defect),
            _ => None,
        }
    }
}

/// 图节点 `(kind, id)`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub kind: NodeKind,
    pub id: String,
}

impl Node {
    #[allow(dead_code)]
    pub fn new(kind: NodeKind, id: impl Into<String>) -> Self {
        Node { kind, id: id.into() }
    }
}

/// 有向边 `source blocks target`（source → target）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub source: Node,
    pub target: Node,
}

/// 邻接表表示的依赖图：`outgoing.get(node)` = node 直接阻塞的所有节点。
#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    outgoing: std::collections::HashMap<Node, Vec<Node>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// 加入一条边 source → target。调用方须保证没有重复边（唯一约束在 DB 兜底）。
    pub fn add_edge(&mut self, source: Node, target: Node) {
        self.outgoing.entry(source).or_default().push(target);
    }

    /// 从边集合构建图。
    pub fn from_edges(edges: impl IntoIterator<Item = Edge>) -> Self {
        let mut g = Self::new();
        for e in edges {
            g.add_edge(e.source, e.target);
        }
        g
    }

    /// 判断 `from` 到 `to` 是否存在有向可达路径（DFS）。
    ///
    /// 用于加边 `from → to` 前的循环检测：若 `to` 已可达 `from`，则加边会成环。
    pub fn reaches(&self, from: &Node, to: &Node) -> bool {
        if from == to {
            return true;
        }
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<&Node> = vec![from];
        while let Some(node) = stack.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            if let Some(nexts) = self.outgoing.get(node) {
                for n in nexts {
                    if n == to {
                        return true;
                    }
                    stack.push(n);
                }
            }
        }
        false
    }

    /// 从 `start` 出发做 BFS，返回所有可达节点（不含 `start` 自身）。
    /// 用于「传递依赖」查询：source 的传递阻塞集 / target 的传递被阻塞集。
    pub fn descendants(&self, start: &Node) -> Vec<Node> {
        let mut visited = std::collections::HashSet::new();
        let mut order: Vec<Node> = Vec::new();
        let mut queue: std::collections::VecDeque<Node> =
            std::collections::VecDeque::new();
        queue.push_back(start.clone());
        visited.insert(start.clone());
        while let Some(node) = queue.pop_front() {
            if let Some(nexts) = self.outgoing.get(&node) {
                for n in nexts {
                    if visited.insert(n.clone()) {
                        order.push(n.clone());
                        queue.push_back(n.clone());
                    }
                }
            }
        }
        order
    }

    /// 返回 `node` 的所有直接后继（直接阻塞的对象）。
    #[allow(dead_code)]
    pub fn direct_successors(&self, node: &Node) -> Vec<Node> {
        self.outgoing.get(node).cloned().unwrap_or_default()
    }

    /// 返回 `node` 的所有直接前驱（直接阻塞它的对象）。
    /// 由反向遍历整图得出，适合小规模图；数据量大时应走 DB 的 `idx_dependency_target` 索引。
    #[allow(dead_code)]
    pub fn direct_predecessors(&self, node: &Node) -> Vec<Node> {
        self.outgoing
            .iter()
            .filter_map(|(src, targets)| {
                targets.iter().any(|t| t == node).then(|| src.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str) -> Node {
        Node::new(NodeKind::Requirement, id)
    }

    #[test]
    fn reaches_detects_cycle_a_b_a() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        // B 可达 A？A→B，B 无后继 → 否
        assert!(!g.reaches(&req("B"), &req("A")));
        // A 可达 B → 是
        assert!(g.reaches(&req("A"), &req("B")));
        // 自环：A 可达自身恒真
        assert!(g.reaches(&req("A"), &req("A")));
    }

    #[test]
    fn reaches_transitive_chain() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        // A 可传递到 C
        assert!(g.reaches(&req("A"), &req("C")));
        // C 不可回到 A
        assert!(!g.reaches(&req("C"), &req("A")));
        // 加 C→A 会成环：C 已可达 A
        assert!(g.reaches(&req("C"), &req("A")) == false);
    }

    #[test]
    fn descendants_returns_transitive_closure_without_self() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        g.add_edge(req("A"), req("D"));
        let mut ds = g.descendants(&req("A"));
        ds.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            ds,
            vec![req("B"), req("C"), req("D")]
        );
    }

    #[test]
    fn direct_predecessors_reverse_lookup() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("C"));
        g.add_edge(req("B"), req("C"));
        let mut preds = g.direct_predecessors(&req("C"));
        preds.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(preds, vec![req("A"), req("B")]);
    }

    #[test]
    fn cross_kind_nodes_are_distinct() {
        let mut g = DependencyGraph::new();
        g.add_edge(
            Node::new(NodeKind::Requirement, "X"),
            Node::new(NodeKind::Defect, "X"),
        );
        let src = Node::new(NodeKind::Requirement, "X");
        let tgt = Node::new(NodeKind::Defect, "X");
        assert!(g.reaches(&src, &tgt));
        // 同 id 不同 kind 是不同节点，requirement X 不可达 requirement X 自身之外
        assert!(!g.reaches(&tgt, &src));
    }

    #[test]
    fn node_kind_parse() {
        assert_eq!(NodeKind::parse("requirement"), Some(NodeKind::Requirement));
        assert_eq!(NodeKind::parse("defect"), Some(NodeKind::Defect));
        assert_eq!(NodeKind::parse("bogus"), None);
        assert_eq!(NodeKind::Requirement.as_str(), "requirement");
    }
}
