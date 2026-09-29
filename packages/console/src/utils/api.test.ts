import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const memoryStore = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => memoryStore.get(key) ?? null,
  setItem: (key: string, value: string) => {
    memoryStore.set(key, value);
  },
  removeItem: (key: string) => {
    memoryStore.delete(key);
  },
  clear: () => {
    memoryStore.clear();
  },
  key: (index: number) => [...memoryStore.keys()][index] ?? null,
  get length() {
    return memoryStore.size;
  },
});

vi.mock("../router", () => ({
  router: {
    push: vi.fn(),
    currentRoute: { value: { path: "/projects" } },
  },
}));

const { toastAdd, toastWarn, toastError } = vi.hoisted(() => ({
  toastAdd: vi.fn(),
  toastWarn: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/ui", () => ({
  appToast: {
    add: toastAdd,
    warn: toastWarn,
    error: toastError,
    success: vi.fn(),
    info: vi.fn(),
  },
  useToast: () => ({
    add: toastAdd,
    warn: toastWarn,
    error: toastError,
  }),
}));

import { api } from "./api";
import { router } from "../router";

function makeError(message: string, status = 404, url = "/api/v1/requirements/x/memory") {
  return {
    response: {
      status,
      data: { error: message },
    },
    message,
    isAxiosError: true,
    config: { url },
    toJSON: () => ({}),
  } as unknown as import("axios").AxiosError;
}

describe("api response interceptor", () => {
  beforeEach(() => {
    toastAdd.mockClear();
    toastWarn.mockClear();
    toastError.mockClear();
    vi.mocked(router.push).mockClear();
    router.currentRoute.value.path = "/projects";
    localStorage.clear();
  });

  afterEach(() => {
    toastAdd.mockClear();
    toastWarn.mockClear();
    toastError.mockClear();
    localStorage.clear();
  });

  const rejected = api.interceptors.response.handlers[0].rejected!;

  test("CONTEXT_NOT_FOUND 不弹错误 toast（良性『尚无工作记忆』状态）", async () => {
    const err = makeError("CONTEXT_NOT_FOUND");
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).not.toHaveBeenCalled();
    expect(toastAdd).not.toHaveBeenCalled();
  });

  test("MEMORY_NOT_FOUND 不弹错误 toast（重命名后的良性空状态）", async () => {
    const err = makeError("MEMORY_NOT_FOUND");
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).not.toHaveBeenCalled();
    expect(toastAdd).not.toHaveBeenCalled();
  });

  test("SETUP_REQUIRED 不弹 toast，交给路由去安装页", async () => {
    const err = makeError("SETUP_REQUIRED", 503);
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).not.toHaveBeenCalled();
    expect(toastAdd).not.toHaveBeenCalled();
  });

  test("乐观锁冲突不弹裸错误码 toast，交给页面渲染", async () => {
    // 通用分支会 toast 出 "KNOWLEDGE_DOC_CONFLICT" 这五个字母，
    // 而用户需要的是「谁的版本更新」和一个留在页面上的合并入口
    const err = makeError("KNOWLEDGE_DOC_CONFLICT", 409, "/projects/p1/knowledge/documents/d1");
    (err.response as { data: unknown }).data = {
      error: "KNOWLEDGE_DOC_CONFLICT",
      data: { yourRevision: 3, currentRevision: 5, currentContent: "对方的正文" },
    };
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).not.toHaveBeenCalled();
    expect(toastAdd).not.toHaveBeenCalled();
  });

  test("冲突提示不被 3 秒去重压掉（连续两次保存失败都要有反馈）", async () => {
    // 去重是按文案 + 3 秒窗口做的；冲突若走那条路，「刚失败又点了一次」
    // 会看起来像没反应，用户会以为按钮坏了
    const mk = () => {
      const e = makeError("MEMORY_CONFLICT", 409, "/projects/p1/memory");
      (e.response as { data: unknown }).data = {
        error: "MEMORY_CONFLICT",
        data: { yourRevision: 1, currentRevision: 2, currentSnapshot: "x" },
      };
      return e;
    };
    await expect(rejected(mk())).rejects.toBeDefined();
    await expect(rejected(mk())).rejects.toBeDefined();
    expect(toastError).not.toHaveBeenCalled();
    expect(toastWarn).not.toHaveBeenCalled();
  });

  test("Run 撞锁（同是 409 但不是版本冲突）仍走通用分支", async () => {
    // RUN_ALREADY_RUNNING 的出路是「接管」，不是「合并重写」，
    // 不能被 conflict 分支吞掉而失去提示
    const err = makeError("RUN_ALREADY_RUNNING", 409);
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).toHaveBeenCalledTimes(1);
    expect(toastError.mock.calls[0]).toEqual(["错误", "RUN_ALREADY_RUNNING"]);
  });

  test("其他错误仍弹 toast 并 reject", async () => {
    const err = makeError("REQUIREMENT_NOT_FOUND", 404);
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).toHaveBeenCalledTimes(1);
    expect(toastError.mock.calls[0]).toEqual(["错误", "REQUIREMENT_NOT_FOUND"]);
  });

  test("应用页 401 弹过期提示并跳转登录", async () => {
    localStorage.setItem("token", "stale");
    const err = makeError("UNAUTHORIZED", 401, "/users/me");
    await expect(rejected(err)).rejects.toBe(err);
    expect(localStorage.getItem("token")).toBeNull();
    expect(toastWarn).toHaveBeenCalledWith("登录已过期", "请重新登录");
    expect(router.push).toHaveBeenCalledWith("/auth/login");
  });

  test("注册页 401 只清 token，不弹窗、不跳登录", async () => {
    router.currentRoute.value.path = "/auth/register";
    localStorage.setItem("token", "stale");
    const err = makeError("UNAUTHORIZED", 401, "/users/me");
    await expect(rejected(err)).rejects.toBe(err);
    expect(localStorage.getItem("token")).toBeNull();
    expect(toastWarn).not.toHaveBeenCalled();
    expect(router.push).not.toHaveBeenCalled();
  });

  test("registration-config 公开路由不附加 Authorization", async () => {
    localStorage.setItem("token", "fake");
    const fulfilled = api.interceptors.request.handlers[0].fulfilled!;
    const result = await fulfilled({
      url: "/auth/registration-config",
      headers: {} as import("axios").AxiosRequestHeaders,
    });
    expect(result.headers.Authorization).toBeUndefined();
  });
});
