import { createRouter, createWebHistory } from "vue-router";
import { routes } from "vue-router/auto-routes";

function isConsolePath(path: string) {
  return path === "/console" || path.startsWith("/console/");
}

export const router = createRouter({
  history: createWebHistory("/"),
  routes: [
    ...routes,
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
});

router.beforeEach(to => {
  if (isConsolePath(to.path)) {
    window.location.assign(to.fullPath);
    return false;
  }
  return true;
});
