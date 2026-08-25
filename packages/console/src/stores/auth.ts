import { defineStore } from "pinia";
import { router } from "../router";
import { api } from "../utils/api";

type AuthToken = {
  token: string;
  expiresIn: string;
};

type RegisterPayload = {
  email: string;
  password: string;
  inviteCode?: string;
  nickname?: string;
  qq?: string;
};

type AuthErrorCode =
  | "INVALID_CREDENTIALS"
  | "EMAIL_EXISTS"
  | "INVITE_CODE_REQUIRED"
  | "INVALID_INVITE_CODE"
  | "WEAK_PASSWORD"
  | "EMAIL_NOT_VERIFIED"
  | "EMAIL_SEND_FAILED"
  | "ACCOUNT_INACTIVE"
  | "ACCOUNT_LOCKED"
  | "INVALID_OR_EXPIRED_TOKEN"
  | "INTERNAL_ERROR";

const getErrorCode = (error: unknown): AuthErrorCode | undefined => {
  return (error as { response?: { data?: { error?: AuthErrorCode; message?: string } } })
    ?.response?.data?.error;
};

const getErrorMessage = (error: unknown): string | undefined => {
  return (error as { response?: { data?: { message?: string } } })?.response?.data?.message;
};

const mapAuthError = (
  code: AuthErrorCode | undefined,
  fallback: string,
): string => {
  switch (code) {
    case "INVALID_CREDENTIALS":
      return "邮箱或密码错误";
    case "EMAIL_EXISTS":
      return "邮箱已被注册";
    case "INVITE_CODE_REQUIRED":
      return "当前仅支持邀请注册，请输入邀请码";
    case "INVALID_INVITE_CODE":
      return "邀请码无效或已用完";
    case "WEAK_PASSWORD":
      return "密码强度不足";
    case "EMAIL_NOT_VERIFIED":
      return "邮箱尚未验证，请查收验证邮件";
    case "EMAIL_SEND_FAILED":
      return "验证邮件发送失败，请检查邮箱或稍后重试";
    case "ACCOUNT_INACTIVE":
      return "账户已被停用";
    case "ACCOUNT_LOCKED":
      return "登录尝试过多，账户已锁定，请稍后再试";
    case "INVALID_OR_EXPIRED_TOKEN":
      return "链接无效或已过期";
    case "INTERNAL_ERROR":
      return "服务异常，请稍后重试";
    default:
      return fallback;
  }
};

const getAuthFailureMessage = (error: unknown, fallback: string): string => {
  const code = getErrorCode(error);
  if (code === "WEAK_PASSWORD" || code === "EMAIL_SEND_FAILED") {
    return getErrorMessage(error) || mapAuthError(code, fallback);
  }
  if (code) return mapAuthError(code, fallback);

  const axiosMessage = (error as { message?: string })?.message;
  if (axiosMessage === "Network Error" || !(error as { response?: unknown })?.response) {
    return "无法连接服务器，请确认后端已启动";
  }

  return fallback;
};

export const useAuthStore = defineStore("auth", {
  state: () => ({
    token: localStorage.getItem("token") ?? "",
  }),
  getters: {
    userId(): string | null {
      if (!this.token) return null;
      try {
        const payload = JSON.parse(atob(this.token.split(".")[1]));
        return payload.userId ?? null;
      } catch {
        return null;
      }
    },
    userRole(): string | null {
      if (!this.token) return null;
      try {
        const payload = JSON.parse(atob(this.token.split(".")[1]));
        return payload.role ?? null;
      } catch {
        return null;
      }
    },
    isAdmin(): boolean {
      return this.userRole === "ADMIN";
    },
  },
  actions: {
    async login(email: string, password: string) {
      try {
        const { data } = await api.post<{ success: boolean; data: AuthToken }>(
          "/auth/login",
          { email, password },
        );

        if (!data.success) {
          throw new Error(mapAuthError(undefined, "登录失败"));
        }

        this.token = data.data.token;
        localStorage.setItem("token", data.data.token);
        router.push("/projects");
      } catch (error) {
        throw new Error(getAuthFailureMessage(error, "登录失败"));
      }
    },
    async register(payload: RegisterPayload) {
      try {
        const { data } = await api.post<{
          success: boolean;
          data: { userId: string; email: string };
        }>("/auth/register", payload);

        if (!data.success) {
          throw new Error(mapAuthError(undefined, "注册失败"));
        }

        return data.data;
      } catch (error) {
        throw new Error(getAuthFailureMessage(error, "注册失败"));
      }
    },
    logout() {
      this.token = "";
      localStorage.removeItem("token");
    },
  },
});
