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
    pub fn direct_successors(&self, node: &Node) -> Vec<Node> {
        self.outgoing.get(node).cloned().unwrap_or_default()
    }

    /// 返回 `node` 的所有直接前驱（直接阻塞它的对象）。
    /// 由反向遍历整图得出，适合小规模图；数据量大时应走 DB 的 `idx_dependency_target` 索引。
    pub fn direct_predecessors(&self, node: &Node) -> Vec<Node> {
        self.outgoing
            .iter()
            .filter_map(|(src, targets)| {
                targets.iter().any(|t| t == node).then(|| src.clone())
            })
            .collect()
    }

    /// 图中出现的全部节点（出边源 + 出边目标，去重）。
    pub fn all_nodes(&self) -> Vec<Node> {
        let mut set = std::collections::HashSet::new();
        for (src, targets) in &self.outgoing {
            set.insert(src.clone());
            for t in targets {
                set.insert(t.clone());
            }
        }
        set.into_iter().collect()
    }

    /// 未完成节点的拓扑分层调度（Kahn 算法）。
    ///
    /// `done` = 已完成节点集合（视为已满足的前置，不再阻塞）。
    /// 返回的每层节点互无阻塞关系、可并行执行；层内顺序任意，层间按序串行。
    /// 已完成的节点不出现在输出中。
    pub fn schedule_levels(&self, done: &std::collections::HashSet<Node>) -> Vec<Vec<Node>> {
        let all = self.all_nodes();
        // 阻塞计数：每个未完成节点 = 其「未完成」直接前驱的数量
        let mut indegree: std::collections::HashMap<Node, usize> = all
            .iter()
            .filter(|n| !done.contains(*n))
            .map(|n| (n.clone(), self.direct_predecessors(n).iter().filter(|p| !done.contains(*p)).count()))
            .collect();

        let mut levels: Vec<Vec<Node>> = Vec::new();
        loop {
            let ready: Vec<Node> = indegree
                .iter()
                .filter(|(_, deg)| **deg == 0)
                .map(|(n, _)| n.clone())
                .collect();
            if ready.is_empty() {
                break;
            }
            for n in &ready {
                indegree.remove(n);
                // 减少其后继的入度
                if let Some(nexts) = self.outgoing.get(n) {
                    for succ in nexts {
                        if let Some(deg) = indegree.get_mut(succ) {
                            if *deg > 0 {
                                *deg -= 1;
                            }
                        }
                    }
                }
            }
            levels.push(ready);
        }
        levels
    }

    /// 未完成节点中的关键路径（最长依赖链）。
    ///
    /// 返回该路径上的节点序列（从无未完成前驱的起点到无未完成后继的终点）。
    /// `done` = 已完成节点（不参与关键路径）。
    /// 无未完成节点时返回空；多路径同长时取其中一条（按 id 排序取稳定结果）。
    pub fn critical_path(&self, done: &std::collections::HashSet<Node>) -> Vec<Node> {
        let all: Vec<Node> = self
            .all_nodes()
            .into_iter()
            .filter(|n| !done.contains(n))
            .collect();
        if all.is_empty() {
            return Vec::new();
        }

        // 记忆化 DFS：从 node 出发的最长链（含 node 自身）
        fn longest(
            g: &DependencyGraph,
            node: &Node,
            done: &std::collections::HashSet<Node>,
            memo: &mut std::collections::HashMap<Node, Vec<Node>>,
        ) -> Vec<Node> {
            if let Some(cached) = memo.get(node) {
                return cached.clone();
            }
            let nexts: Vec<&Node> = g
                .outgoing
                .get(node)
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let mut best: Vec<Node> = Vec::new();
            for n in nexts {
                if done.contains(n) {
                    continue;
                }
                let chain = longest(g, n, done, memo);
                if chain.len() > best.len()
                    || (chain.len() == best.len()
                        && chain.iter().map(|x| x.id.as_str()).lt(best.iter().map(|x| x.id.as_str())))
                {
                    best = chain;
                }
            }
            let mut path = vec![node.clone()];
            path.extend(best);
            memo.insert(node.clone(), path.clone());
            path
        }

        let mut memo = std::collections::HashMap::new();
        let mut best_path: Vec<Node> = Vec::new();
        for n in &all {
            let chain = longest(self, n, done, &mut memo);
            if chain.len() > best_path.len()
                || (chain.len() == best_path.len()
                    && chain.iter().map(|x| x.id.as_str()).lt(best_path.iter().map(|x| x.id.as_str())))
            {
                best_path = chain;
            }
        }
        best_path
    }

    /// 模拟某节点完成后的解锁计算。
    ///
    /// `completed` = 刚完成的节点。返回其**直接下游**中，所有直接前驱都已完成的节点
    /// （即从「被阻塞」解锁、可进入执行状态的任务）。
    /// `done` 需已包含 `completed`。
    pub fn unlockable_after(
        &self,
        completed: &Node,
        done: &std::collections::HashSet<Node>,
    ) -> Vec<Node> {
        let mut out = Vec::new();
        if let Some(nexts) = self.outgoing.get(completed) {
            for succ in nexts {
                if done.contains(succ) {
                    continue;
                }
                let all_preds_done = self
                    .direct_predecessors(succ)
                    .iter()
                    .all(|p| done.contains(p));
                if all_preds_done {
                    out.push(succ.clone());
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
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

    fn done_set(nodes: Vec<Node>) -> std::collections::HashSet<Node> {
        nodes.into_iter().collect()
    }

    #[test]
    fn schedule_levels_basic_dag() {
        // A → B → C；A → D
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        g.add_edge(req("A"), req("D"));
        let done = done_set(vec![]);
        let levels = g.schedule_levels(&done);
        // 层1: A；层2: B, D；层3: C
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec![req("A")]);
        let mut l1 = levels[1].clone();
        l1.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(l1, vec![req("B"), req("D")]);
        assert_eq!(levels[2], vec![req("C")]);
    }

    #[test]
    fn schedule_levels_skips_done_nodes() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        // A 已完成 → 其不再阻塞 B，B 直接可执行
        let done = done_set(vec![req("A")]);
        let levels = g.schedule_levels(&done);
        assert_eq!(levels, vec![vec![req("B")], vec![req("C")]]);
    }

    #[test]
    fn schedule_levels_parallel_sources() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("C"));
        g.add_edge(req("B"), req("C"));
        let done = done_set(vec![]);
        let levels = g.schedule_levels(&done);
        assert_eq!(levels.len(), 2);
        let mut l0 = levels[0].clone();
        l0.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(l0, vec![req("A"), req("B")]);
        assert_eq!(levels[1], vec![req("C")]);
    }

    #[test]
    fn critical_path_returns_longest_chain() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        g.add_edge(req("A"), req("D"));
        let done = done_set(vec![]);
        let path = g.critical_path(&done);
        // 最长链 A→B→C（3 个）
        assert_eq!(path, vec![req("A"), req("B"), req("C")]);
    }

    #[test]
    fn critical_path_skips_done() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        g.add_edge(req("A"), req("D"));
        // B 已完成 → 最长未完成链 A→D（2 个）或 C 自身
        let done = done_set(vec![req("B")]);
        let path = g.critical_path(&done);
        assert_eq!(path, vec![req("A"), req("D")]);
    }

    #[test]
    fn critical_path_empty_when_all_done() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("B"));
        let done = done_set(vec![req("A"), req("B")]);
        assert!(g.critical_path(&done).is_empty());
    }

    #[test]
    fn unlockable_after_releases_downstream() {
        let mut g = DependencyGraph::new();
        // A → B → C；A → D
        g.add_edge(req("A"), req("B"));
        g.add_edge(req("B"), req("C"));
        g.add_edge(req("A"), req("D"));
        // A 完成后：B、D 均解锁（其唯一前置 A 已完成）
        let mut done = done_set(vec![req("A")]);
        let mut unlocked = g.unlockable_after(&req("A"), &done);
        unlocked.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(unlocked, vec![req("B"), req("D")]);
        // 再完成 B：C 解锁
        done.insert(req("B"));
        assert_eq!(g.unlockable_after(&req("B"), &done), vec![req("C")]);
    }

    #[test]
    fn unlockable_requires_all_predecessors() {
        let mut g = DependencyGraph::new();
        g.add_edge(req("A"), req("C"));
        g.add_edge(req("B"), req("C"));
        // 只完成 A，C 的前置 B 未完成 → 不解锁
        let done = done_set(vec![req("A")]);
        assert!(g.unlockable_after(&req("A"), &done).is_empty());
        // A、B 都完成 → C 解锁
        let mut done = done_set(vec![req("A"), req("B")]);
        done.insert(req("B"));
        let _ = done;
        let done = done_set(vec![req("A"), req("B")]);
        assert_eq!(g.unlockable_after(&req("B"), &done), vec![req("C")]);
    }
}
