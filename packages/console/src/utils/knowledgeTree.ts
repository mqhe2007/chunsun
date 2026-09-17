/**
 * 知识库文档树（主文档 ↔ 分册）的纯计算辅助。
 *
 * 后端 `/knowledge/documents` 与 `/knowledge/index` 都返回**前序**（父后紧跟其子树）
 * 并携带 `parentId` / `depth`，这里只做「折叠过滤 + 子文档计数」，不重复组树：
 * 过滤只删行，不改 depth，父被折叠时其所有层级后代一并隐藏。
 */

export type KnowledgeRelItem = {
  key: string;
  parentId?: string | null;
  depth?: number;
};

export type KnowledgeTreeRow<T extends KnowledgeRelItem> = {
  item: T;
  depth: number;
  /** 直接子文档数（不含孙辈） */
  childCount: number;
  /** 该行的子级当前是否被折叠 */
  collapsed: boolean;
};

/** parent key → 直接子项列表（保持输入顺序）。 */
export function buildChildrenIndex<T extends KnowledgeRelItem>(
  items: T[],
): Map<string, T[]> {
  const index = new Map<string, T[]>();
  for (const item of items) {
    const parent = item.parentId;
    if (!parent) continue;
    const bucket = index.get(parent);
    if (bucket) bucket.push(item);
    else index.set(parent, [item]);
  }
  return index;
}

/** 某文档的全部后代 key（任意层级），用于「所属主文档」选择器排除自身子树；环数据兜底。 */
export function collectDescendantKeys<T extends KnowledgeRelItem>(
  items: T[],
  key: string,
): Set<string> {
  const index = buildChildrenIndex(items);
  const out = new Set<string>();
  const stack = [...(index.get(key) ?? [])];
  while (stack.length) {
    const node = stack.pop()!;
    if (out.has(node.key)) continue;
    out.add(node.key);
    stack.push(...(index.get(node.key) ?? []));
  }
  return out;
}

/**
 * 按折叠状态过滤出要渲染的行（保持前序）。
 * - `collapsedKeys` 中的节点的所有层级后代都会被隐藏；
 * - 父不在列表里的条目（策略过滤视图/脏数据）视为根，不隐藏、不丢失；
 * - parentId 成环时用 guard 截断，避免死循环。
 */
export function visibleKnowledgeRows<T extends KnowledgeRelItem>(
  items: T[],
  collapsedKeys: ReadonlySet<string> = new Set(),
): KnowledgeTreeRow<T>[] {
  const byKey = new Map(items.map(item => [item.key, item] as const));
  const children = buildChildrenIndex(items);

  const isHidden = (item: T): boolean => {
    let cursor = item.parentId ?? null;
    let guard = 0;
    while (cursor && guard < 64) {
      const parent = byKey.get(cursor);
      // 父不在列表里（策略过滤视图/脏数据）就不可见地折叠，按根处理
      if (!parent) return false;
      if (collapsedKeys.has(cursor)) return true;
      cursor = parent.parentId ?? null;
      guard += 1;
    }
    return false;
  };

  return items
    .filter(item => !isHidden(item))
    .map(item => ({
      item,
      depth: item.depth ?? 0,
      childCount: (children.get(item.key) ?? []).length,
      collapsed: collapsedKeys.has(item.key),
    }));
}
