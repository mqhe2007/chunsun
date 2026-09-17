import { describe, expect, it } from "vitest";
import {
  buildChildrenIndex,
  collectDescendantKeys,
  visibleKnowledgeRows,
  type KnowledgeRelItem,
} from "./knowledgeTree";

type Item = KnowledgeRelItem & { title?: string };

// 输入约定与后端一致：前序（父后紧跟其子树）+ 绝对 depth
const items: Item[] = [
  { key: "master", parentId: null, depth: 0 },
  { key: "vol1", parentId: "master", depth: 1 },
  { key: "sub1", parentId: "vol1", depth: 2 },
  { key: "vol2", parentId: "master", depth: 1 },
  { key: "solo", parentId: null, depth: 0 },
];

describe("buildChildrenIndex", () => {
  it("indexes direct children by parent key", () => {
    const index = buildChildrenIndex(items);
    expect(index.get("master")?.map(i => i.key)).toEqual(["vol1", "vol2"]);
    expect(index.get("vol1")?.map(i => i.key)).toEqual(["sub1"]);
    expect(index.get("solo")).toBeUndefined();
  });
});

describe("collectDescendantKeys", () => {
  it("collects all levels for parent picker exclusion", () => {
    expect([...collectDescendantKeys(items, "master")].sort()).toEqual([
      "sub1",
      "vol1",
      "vol2",
    ]);
  });

  it("returns empty set for leaf docs", () => {
    expect(collectDescendantKeys(items, "solo").size).toBe(0);
  });

  it("survives cyclic data without hanging", () => {
    const cyclic: Item[] = [
      { key: "a", parentId: "b", depth: 0 },
      { key: "b", parentId: "a", depth: 0 },
    ];
    expect([...collectDescendantKeys(cyclic, "a")].sort()).toEqual(["a", "b"]);
  });
});

describe("visibleKnowledgeRows", () => {
  it("keeps pre-order and computes direct child counts", () => {
    const rows = visibleKnowledgeRows(items);
    expect(rows.map(r => r.item.key)).toEqual([
      "master",
      "vol1",
      "sub1",
      "vol2",
      "solo",
    ]);
    expect(rows.map(r => r.childCount)).toEqual([2, 1, 0, 0, 0]);
    expect(rows.map(r => r.depth)).toEqual([0, 1, 2, 1, 0]);
  });

  it("hides all levels of descendants when a parent is collapsed", () => {
    const rows = visibleKnowledgeRows(items, new Set(["master"]));
    expect(rows.map(r => r.item.key)).toEqual(["master", "solo"]);
    expect(rows[0].collapsed).toBe(true);
  });

  it("hides grandchildren when only the middle node is collapsed", () => {
    const rows = visibleKnowledgeRows(items, new Set(["vol1"]));
    expect(rows.map(r => r.item.key)).toEqual(["master", "vol1", "vol2", "solo"]);
  });

  it("treats items with absent parents as roots, never dropping them", () => {
    const filtered: Item[] = [
      { key: "orphan", parentId: "missing", depth: 1 },
      { key: "alone", parentId: null, depth: 0 },
    ];
    const rows = visibleKnowledgeRows(filtered, new Set(["missing"]));
    expect(rows.map(r => r.item.key)).toEqual(["orphan", "alone"]);
  });

  it("survives cyclic parent chains", () => {
    const cyclic: Item[] = [
      { key: "a", parentId: "b", depth: 0 },
      { key: "b", parentId: "a", depth: 0 },
    ];
    const rows = visibleKnowledgeRows(cyclic);
    expect(rows.map(r => r.item.key)).toEqual(["a", "b"]);
  });
});
