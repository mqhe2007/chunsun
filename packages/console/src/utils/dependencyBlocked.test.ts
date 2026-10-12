import { describe, expect, it } from "vitest";

import { collectBlockedIds, type DependencyEdge } from "./dependencyBlocked";

function edge(
  overrides: Partial<DependencyEdge> & Pick<DependencyEdge, "sourceId" | "targetId">,
): DependencyEdge {
  return {
    sourceType: "requirement",
    targetType: "requirement",
    ...overrides,
  };
}

describe("collectBlockedIds", () => {
  it("仅未完成 source 的边计入被阻塞（部分前置完成不误隐）", () => {
    const set = collectBlockedIds(
      [
        edge({ sourceId: "a", targetId: "b", sourceDone: false }),
        edge({ sourceId: "c", targetId: "b", sourceDone: true }),
        edge({ sourceId: "d", targetId: "e", sourceDone: true }),
      ],
      "requirement",
    );
    expect([...set]).toEqual(["b"]);
  });

  it("前置全部完成后集合为空", () => {
    const set = collectBlockedIds(
      [
        edge({ sourceId: "a", targetId: "b", sourceDone: true }),
        edge({ sourceId: "c", targetId: "b", sourceDone: true, sourceType: "defect" }),
      ],
      "requirement",
    );
    expect(set.size).toBe(0);
  });

  it("跨类型：缺陷作为 source（resolved/closed 由后端折算 sourceDone）不阻塞需求", () => {
    const set = collectBlockedIds(
      [edge({ sourceId: "d1", targetId: "b", sourceType: "defect", sourceDone: true })],
      "requirement",
    );
    expect(set.size).toBe(0);
  });

  it("按 targetType 过滤，不混入另一类节点", () => {
    const set = collectBlockedIds(
      [
        edge({ sourceId: "a", targetId: "b", sourceDone: false }),
        edge({ sourceId: "a", targetId: "d", targetType: "defect", sourceDone: false }),
      ],
      "requirement",
    );
    expect([...set]).toEqual(["b"]);
  });

  it("sourceDone 缺省（旧后端）按未完成处理", () => {
    const set = collectBlockedIds(
      [edge({ sourceId: "a", targetId: "b" })],
      "requirement",
    );
    expect([...set]).toEqual(["b"]);
  });

  it("abandoned 等非完成状态由后端折算 sourceDone=false，仍显示被阻塞", () => {
    const set = collectBlockedIds(
      [edge({ sourceId: "a", targetId: "b", sourceDone: false })],
      "requirement",
    );
    expect([...set]).toEqual(["b"]);
  });
});
