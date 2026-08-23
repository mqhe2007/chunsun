import axios from "axios";
import { appToast } from "@/ui";
import { router } from "../router";

/** 实例始终挂在 origin 根路径；Vite base（/console/）不能带进 API。 */
function resolveApiBaseUrl(): string {
  return "/api/v1";
}

export const api = axios.create({
  baseURL: resolveApiBaseUrl(),
});

function isPublicApiUrl(url: string | undefined) {
  if (!url) return false;
  return (
    url.includes("/auth/login") ||
    url.includes("/auth/register") ||
    url.includes("/auth/registration-config") ||
    url.includes("/auth/verify-email") ||
    url.includes("/auth/resend-verification") ||
    url.includes("/auth/forgot-password") ||
    url.includes("/auth/reset-password") ||
    url.includes("/setup/")
  );
}

function isAuthOrSetupPath(path: string) {
  return path === "/auth" || path.startsWith("/auth/") || path === "/setup" || path.startsWith("/setup/");
}

api.interceptors.request.use(config => {
  if (isPublicApiUrl(config.url)) return config;
  const token = localStorage.getItem("token");
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

api.interceptors.response.use(
  response => response,
  error => {
    const status = error?.response?.status as number | undefined;
    const message = error?.response?.data?.error as string | undefined;
    const requestUrl = error?.config?.url as string | undefined;
    const now = Date.now();

    const state = (
      api as unknown as { __toastState?: { last?: string; time?: number } }
    ).__toastState;
    const lastMessage = state?.last;
    const lastTime = state?.time ?? 0;

    const shouldShow = (text: string) => {
      if (text === lastMessage && now - lastTime < 3000) return false;
      (
        api as unknown as { __toastState?: { last?: string; time?: number } }
      ).__toastState = {
        last: text,
        time: now,
      };
      return true;
    };

    if (isPublicApiUrl(requestUrl)) {
      return Promise.reject(error);
    }

    if (message === "CONTEXT_NOT_FOUND") {
      return Promise.reject(error);
    }

    if (message === "SETUP_REQUIRED") {
      void router.push("/setup");
      return Promise.reject(error);
    }

    if (status === 401) {
      localStorage.removeItem("token");
      const currentPath = router.currentRoute.value?.path ?? "";
      // 认证页上的过期会话只清 token，不弹窗、不改路由（否则「开始生长」会被拽去登录）。
      if (!isAuthOrSetupPath(currentPath)) {
        if (shouldShow("登录已过期，请重新登录")) {
          appToast.warn("登录已过期", "请重新登录");
        }
        void router.push("/auth/login");
      }
    } else if (message) {
      if (shouldShow(message)) {
        appToast.error("错误", message);
      }
    } else if (status && shouldShow("请求失败，请稍后重试")) {
      appToast.error("请求失败", "请稍后重试");
    }

    return Promise.reject(error);
  },
);
