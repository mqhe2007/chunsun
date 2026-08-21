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

function makeError(message: string, status = 404, url = "/api/v1/requirements/x/context") {
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

  test("SETUP_REQUIRED 不弹 toast，交给路由去安装页", async () => {
    const err = makeError("SETUP_REQUIRED", 503);
    await expect(rejected(err)).rejects.toBe(err);
    expect(toastError).not.toHaveBeenCalled();
    expect(toastAdd).not.toHaveBeenCalled();
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
});
