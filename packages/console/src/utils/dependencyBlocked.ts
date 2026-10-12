/** 依赖边（GET /projects/:id/dependencies 返回项）。 */
export type DependencyEdge = {
  sourceType: "requirement" | "defect";
  sourceId: string;
  targetType: "requirement" | "defect";
  targetId: string;
  /** source（阻塞方）是否已完成：需求 completed；缺陷 resolved / closed。旧后端可能缺省。 */
  sourceDone?: boolean;
};

/**
 * 计算「被阻塞」节点 ID 集合：仅当入边 source 未完成时，target 才算被阻塞。
 * 口径与后端 `node_is_done` / `dependency blocked` 一致（abandoned 不视为完成）。
 * `sourceDone` 缺省（旧后端）按未完成处理，保持原展示行为。
 */
export function collectBlockedIds(
  edges: DependencyEdge[],
  targetType: "requirement" | "defect",
): Set<string> {
  const set = new Set<string>();
  for (const e of edges) {
    if (e.targetType === targetType && !e.sourceDone) set.add(e.targetId);
  }
  return set;
}
