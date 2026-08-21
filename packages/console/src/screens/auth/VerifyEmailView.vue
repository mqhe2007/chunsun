<script setup lang="ts">
import { CircleCheck } from "@lucide/vue";
import { onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api } from "@/utils/api";

const route = useRoute();
const router = useRouter();

const loading = ref(true);
const success = ref(false);
const error = ref("");

onMounted(async () => {
  const token = route.query.token as string;
  if (!token) {
    loading.value = false;
    error.value = "链接无效，缺少验证令牌";
    return;
  }

  try {
    const { data } = await api.post("/auth/verify-email", { token });
    success.value = data.success;
    if (!data.success) {
      error.value = "链接无效或已过期";
    }
  } catch (err: unknown) {
    const message =
      (err as { response?: { data?: { error?: string } } })?.response?.data?.error ||
      "验证失败";
    error.value = message === "INVALID_OR_EXPIRED_TOKEN" ? "链接无效或已过期" : "验证失败";
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="verify-result">
    <div v-if="loading" class="state">
      <span class="loading loading-spinner loading-lg" />
      <span>正在验证邮箱...</span>
    </div>

    <div v-else-if="success" class="state success">
      <div class="icon success-icon" aria-hidden="true">
        <CircleCheck :size="28" />
      </div>
      <h2>邮箱验证成功</h2>
      <p>你的邮箱已通过验证，现在可以登录使用春笋。</p>
      <button type="button" class="btn btn-primary" @click="router.push('/auth/login')">
        去登录
      </button>
    </div>

    <div v-else class="state error">
      <div class="icon error-icon">✗</div>
      <h2>验证失败</h2>
      <p>{{ error }}</p>
      <button type="button" class="btn btn-ghost" @click="router.push('/auth/login')">
        返回登录
      </button>
    </div>
  </div>
</template>

<style scoped>
.verify-result {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 1rem 0;
}

.state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
  text-align: center;
}

.state h2 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.state p {
  margin: 0;
  font-size: 0.9rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  max-width: 300px;
  line-height: 1.5;
}

.icon {
  width: 4rem;
  height: 4rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.75rem;
  font-weight: 700;
}

.success-icon {
  background: color-mix(in oklch, var(--color-primary) 15%, transparent);
  color: var(--color-primary);
}

.error-icon {
  background: color-mix(in oklch, var(--color-error) 15%, transparent);
  color: var(--color-error);
}
</style>
