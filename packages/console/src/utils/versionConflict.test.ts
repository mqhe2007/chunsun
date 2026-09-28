import { describe, expect, it } from "vitest";
import {
  describeVersionConflict,
  errorCodeOf,
  isConflictErrorCode,
  parseVersionConflict,
} from "./versionConflict";

function conflictError(
  code: string,
  data?: unknown,
  status = 409,
): unknown {
  return { response: { status, data: { success: false, error: code, ...(data === undefined ? {} : { data }) } } };
}

describe("isConflictErrorCode", () => {
  it("accepts the three write sites' codes", () => {
    expect(isConflictErrorCode("KNOWLEDGE_DOC_CONFLICT")).toBe(true);
    expect(isConflictErrorCode("CONSTITUTION_CONFLICT")).toBe(true);
    expect(isConflictErrorCode("MEMORY_CONFLICT")).toBe(true);
  });

  it("rejects the run lock and unrelated codes", () => {
    // RUN_ALREADY_RUNNING 也是 409，但它没有 revision，语义是「换个端点」而非「合并重写」
    expect(isConflictErrorCode("RUN_ALREADY_RUNNING")).toBe(false);
    expect(isConflictErrorCode("MISSING_REVISION")).toBe(false);
    expect(isConflictErrorCode(null)).toBe(false);
  });
});

describe("errorCodeOf", () => {
  it("reads the error code from an axios-shaped error", () => {
    expect(errorCodeOf(conflictError("MEMORY_CONFLICT"))).toBe("MEMORY_CONFLICT");
  });

  it("returns null for non-axios errors and malformed bodies", () => {
    expect(errorCodeOf(new Error("boom"))).toBeNull();
    expect(errorCodeOf(null)).toBeNull();
    expect(errorCodeOf(undefined)).toBeNull();
    expect(errorCodeOf({ response: { status: 409 } })).toBeNull();
    expect(errorCodeOf({ response: { status: 409, data: { error: 42 } } })).toBeNull();
  });
});

describe("parseVersionConflict", () => {
  it("reads the knowledge document shape (currentContent is the body)", () => {
    const c = parseVersionConflict(
      conflictError("KNOWLEDGE_DOC_CONFLICT", {
        docId: "d1",
        yourRevision: 7,
        currentRevision: 9,
        currentContent: "原来的正文",
        currentTitle: "标题",
        updatedAt: "2026-09-28T10:11:12.123Z",
      }),
    );
    expect(c).not.toBeNull();
    expect(c!.yourRevision).toBe(7);
    expect(c!.currentRevision).toBe(9);
    // 合并重试要用 currentRevision，不是 yourRevision
    expect(c!.currentContent).toBe("原来的正文");
    expect(c!.isSnapshot).toBe(false);
  });

  it("reads the memory shape and flags the snapshot", () => {
    const c = parseVersionConflict(
      conflictError("MEMORY_CONFLICT", {
        projectId: "p1",
        yourRevision: 0,
        currentRevision: 3,
        currentSnapshot: "## 项目记忆",
      }),
    );
    expect(c!.currentContent).toBe("## 项目记忆");
    expect(c!.isSnapshot).toBe(true);
    expect(c!.currentRevision).toBe(3);
  });

  it("treats revision 0 as a real version, not as missing", () => {
    // 首次写传 0；若把 0 当 falsy 丢掉，提示会退化成「已被他人修改」而不说版本
    const c = parseVersionConflict(
      conflictError("CONSTITUTION_CONFLICT", { yourRevision: 0, currentRevision: 1, currentContent: "x" }),
    );
    expect(c!.yourRevision).toBe(0);
  });

  it("ignores non-409 responses even with a conflict-shaped body", () => {
    expect(
      parseVersionConflict(conflictError("KNOWLEDGE_DOC_CONFLICT", {}, 500)),
    ).toBeNull();
  });

  it("ignores 409s from other features", () => {
    // Run 撞锁：同一个 409 状态码，但走的是「接管」而不是「合并重写」
    expect(parseVersionConflict(conflictError("RUN_ALREADY_RUNNING", { runId: "r1" }))).toBeNull();
  });

  it("still recognizes the conflict when the body is thin", () => {
    // 状态码 + 错误码已足以确认是冲突；缺正文只该让提示退化成一句话，
    // 不该把冲突降级成「错误：KNOWLEDGE_DOC_CONFLICT」这种五个字母的 toast
    const c = parseVersionConflict(conflictError("KNOWLEDGE_DOC_CONFLICT"));
    expect(c).not.toBeNull();
    expect(c!.currentContent).toBeNull();
    expect(c!.currentRevision).toBeNull();
  });

  it("survives malformed payloads without throwing", () => {
    expect(parseVersionConflict(undefined)).toBeNull();
    expect(parseVersionConflict(null)).toBeNull();
    expect(parseVersionConflict(new Error("boom"))).toBeNull();
    expect(parseVersionConflict({ response: { status: 409, data: null } })).toBeNull();
    // data 段类型不对：revision 是字符串 → null，正文仍是字符串 → 保留
    const c = parseVersionConflict(
      conflictError("MEMORY_CONFLICT", { yourRevision: "7", currentRevision: null, currentContent: "正文" }),
    );
    expect(c!.yourRevision).toBeNull();
    expect(c!.currentRevision).toBeNull();
    expect(c!.currentContent).toBe("正文");
  });
});

describe("describeVersionConflict", () => {
  it("names both revisions in the message", () => {
    expect(
      describeVersionConflict({
        code: "KNOWLEDGE_DOC_CONFLICT",
        yourRevision: 7,
        currentRevision: 9,
        currentContent: "x",
        isSnapshot: false,
      }),
    ).toContain("你的版本 7，当前版本 9");
  });

  it("degrades to a version-less sentence when revisions are absent", () => {
    const text = describeVersionConflict({
      code: "MEMORY_CONFLICT",
      yourRevision: null,
      currentRevision: null,
      currentContent: null,
      isSnapshot: true,
    });
    expect(text).toContain("已被他人修改");
    expect(text).not.toContain("null");
  });

  it("says 记忆 for snapshots and 文档 otherwise", () => {
    const base = { yourRevision: null, currentRevision: null, currentContent: null } as const;
    expect(describeVersionConflict({ ...base, code: "MEMORY_CONFLICT", isSnapshot: true })).toContain("记忆");
    expect(describeVersionConflict({ ...base, code: "KNOWLEDGE_DOC_CONFLICT", isSnapshot: false })).toContain("文档");
  });
});
