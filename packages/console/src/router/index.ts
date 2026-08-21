import { createRouter, createWebHistory } from "vue-router";
import { routes } from "vue-router/auto-routes";
import { useAuthStore } from "../stores/auth";
import { useSetupStore } from "../stores/setup";

function isSetupPath(path: string) {
  return path === "/setup" || path.startsWith("/setup/");
}

function isAuthPath(path: string) {
  return path === "/auth" || path.startsWith("/auth/");
}

export const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    { path: "/admin", redirect: "/admin/users" },
    ...routes,
  ],
});

router.beforeEach(async to => {
  if (to.path === "/help") {
    window.location.replace("/docs");
    return false;
  }

  const setup = useSetupStore();
  if (!setup.loaded) {
    try {
      await setup.refresh();
    } catch {
      if (!isSetupPath(to.path)) {
        return "/setup";
      }
      return true;
    }
  }

  if (setup.needed) {
    if (!isSetupPath(to.path)) {
      return "/setup";
    }
    return true;
  }

  if (isSetupPath(to.path)) {
    return "/auth/login";
  }

  const token = localStorage.getItem("token");
  const isAuthPage = isAuthPath(to.path);
  const isAppPage = !isAuthPage && !isSetupPath(to.path);

  // 注册/登录等认证页始终可进。仅凭 localStorage 有 token 就跳走，
  // 过期 token 会在应用壳里触发 401，把用户从注册页拽到登录并弹「已过期」。
  if (isAuthPage) {
    return true;
  }

  if (isAppPage && !token) {
    return "/auth/login";
  }

  if (to.meta.requiresAdmin && token) {
    const auth = useAuthStore();
    if (!auth.isAdmin) {
      return "/projects";
    }
  }

  return true;
});
