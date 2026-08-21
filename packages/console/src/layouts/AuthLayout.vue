<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { BrandMark } from "@chunsun/web-shared";
import ConsoleBackButton from "@/components/common/ConsoleBackButton.vue";

const router = useRouter();
const route = useRoute();
const year = new Date().getFullYear();

const pageTitles: Record<string, string> = {
  "/auth/login": "登录",
  "/auth/register": "注册",
  "/auth/verify-email": "验证邮箱",
  "/auth/forgot-password": "忘记密码",
  "/auth/reset-password": "重置密码",
};

const pageTitle = computed(() => pageTitles[route.path] ?? "认证");
const backTarget = computed(() => {
  if (route.path === "/auth/login") return "/";
  return "/auth/login";
});
const backLabel = computed(() =>
  route.path === "/auth/login" ? "返回首页" : "返回登录",
);

function goBack() {
  if (backTarget.value === "/") {
    location.assign("/");
    return;
  }
  router.push(backTarget.value);
}
</script>

<template>
  <div class="min-h-screen bg-base-200 flex items-center justify-center p-4 sm:p-6">
    <main class="card bg-base-100 w-full max-w-md shadow-xl">
      <div class="card-body gap-5">
        <div class="flex items-center justify-between gap-3">
          <ConsoleBackButton :label="backLabel" @click="goBack" />
          <div class="flex items-center gap-2 text-[var(--chunsun-shoot)]">
            <BrandMark size="1.5rem" />
            <span class="font-bold">春笋</span>
          </div>
        </div>

        <div>
          <h1 class="text-2xl font-bold">{{ pageTitle }}</h1>
        </div>

        <router-view />

        <p class="text-center text-xs text-base-content/50 pt-2">© {{ year }} 春笋</p>
      </div>
    </main>
  </div>
</template>
