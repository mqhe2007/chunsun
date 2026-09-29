import { describe, expect, it } from "vitest";
import {
  ABSENT_ROW_ERROR_CODES,
  isAbsentRowError,
  isAbsentRowErrorCode,
} from "./absentRow";

function httpError(status: number, code?: unknown): unknown {
  return { response: { status, data: code === undefined ? {} : { error: code } } };
}

describe("isAbsentRowErrorCode", () => {
  it("accepts the two benign-empty codes", () => {
    expect(ABSENT_ROW_ERROR_CODES).toEqual(["MEMORY_NOT_FOUND", "CONTEXT_NOT_FOUND"]);
    expect(isAbsentRowErrorCode("MEMORY_NOT_FOUND")).toBe(true);
    expect(isAbsentRowErrorCode("CONTEXT_NOT_FOUND")).toBe(true);
  });

  it("rejects conflict codes and non-string codes", () => {
    // 冲突不是空状态：那行存在，只是被别人改过，必须走合并分支而不是清空编辑器
    expect(isAbsentRowErrorCode("MEMORY_CONFLICT")).toBe(false);
    expect(isAbsentRowErrorCode("KNOWLEDGE_DOC_CONFLICT")).toBe(false);
    expect(isAbsentRowErrorCode("MISSING_REVISION")).toBe(false);
    expect(isAbsentRowErrorCode(null)).toBe(false);
  });
});

describe("isAbsentRowError", () => {
  it("requires 404 alongside the code", () => {
    expect(isAbsentRowError(httpError(404, "MEMORY_NOT_FOUND"))).toBe(true);
    expect(isAbsentRowError(httpError(404, "CONTEXT_NOT_FOUND"))).toBe(true);
  });

  it("refuses the same code on a non-404 status", () => {
    // 码对但状态码不对 —— 宁可让调用方走失败分支，也不要静默清空编辑器
    expect(isAbsentRowError(httpError(500, "MEMORY_NOT_FOUND"))).toBe(false);
    expect(isAbsentRowError(httpError(409, "MEMORY_NOT_FOUND"))).toBe(false);
  });

  it("never treats a transport failure as an absent row", () => {
    // 这条是整个模块存在的理由：网络异常意味着「我们不知道服务端有什么」，
    // 按空文档放行会让用户带着 revision 0 覆盖掉真实内容
    expect(isAbsentRowError(new Error("Network Error"))).toBe(false);
    expect(isAbsentRowError(null)).toBe(false);
    expect(isAbsentRowError(undefined)).toBe(false);
    expect(isAbsentRowError(httpError(404))).toBe(false);
    expect(isAbsentRowError(httpError(404, 42))).toBe(false);
  });
});
