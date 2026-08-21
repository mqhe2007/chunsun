<script setup lang="ts">
import { CircleCheck } from "@lucide/vue";
import { ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useField, useForm } from "vee-validate";
import { toTypedSchema } from "@vee-validate/zod";
import { z } from "zod";
import { AppAlert, AppField, PasswordInput } from "@/ui";
import { api } from "@/utils/api";

const route = useRoute();
const router = useRouter();

const loading = ref(false);
const success = ref(false);
const submitError = ref("");

const schema = toTypedSchema(
  z
    .object({
      newPassword: z.string().min(6, "密码至少 6 位").max(100, "密码过长"),
      confirmPassword: z.string().min(1, "请确认密码"),
    })
    .refine(data => data.newPassword === data.confirmPassword, {
      message: "两次输入的密码不一致",
      path: ["confirmPassword"],
    }),
);

const { handleSubmit } = useForm({
  validationSchema: schema,
  initialValues: { newPassword: "", confirmPassword: "" },
});

const { value: newPassword, errorMessage: newPasswordError } = useField<string>("newPassword");
const { value: confirmPassword, errorMessage: confirmPasswordError } =
  useField<string>("confirmPassword");

const onSubmit = handleSubmit(async values => {
  const token = route.query.token as string;
  if (!token) {
    submitError.value = "链接无效，缺少重置令牌";
    return;
  }

  loading.value = true;
  submitError.value = "";
  try {
    const { data } = await api.post("/auth/reset-password", {
      token,
      newPassword: values.newPassword,
    });
    if (data.success) {
      success.value = true;
    } else {
      submitError.value = "重置失败，请检查链接是否有效";
    }
  } catch (err: unknown) {
    const code = (err as { response?: { data?: { error?: string; message?: string } } })?.response
      ?.data?.error;
    const message = (err as { response?: { data?: { message?: string } } })?.response?.data
      ?.message;
    if (code === "WEAK_PASSWORD") {
      submitError.value = message || "密码强度不足";
    } else if (code === "INVALID_OR_EXPIRED_TOKEN") {
      submitError.value = "链接无效或已过期";
    } else {
      submitError.value = "重置失败，请稍后重试";
    }
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div v-if="success" class="reset-success">
    <div class="success-icon" aria-hidden="true">
      <CircleCheck :size="28" />
    </div>
    <h2 class="success-title">密码重置成功</h2>
    <p class="success-desc">你的密码已更新，请使用新密码登录。</p>
    <button type="button" class="btn btn-primary" @click="router.push('/auth/login')">
      去登录
    </button>
  </div>

  <form v-else class="auth-form" @submit.prevent="onSubmit">
    <AppField label="新密码" html-for="reset-password" :error="newPasswordError">
      <PasswordInput
        id="reset-password"
        v-model="newPassword"
        placeholder="请输入新密码"
        :invalid="!!newPasswordError"
        autocomplete="new-password"
      />
    </AppField>

    <AppField label="确认新密码" html-for="reset-confirm-password" :error="confirmPasswordError">
      <PasswordInput
        id="reset-confirm-password"
        v-model="confirmPassword"
        placeholder="请再次输入新密码"
        :invalid="!!confirmPasswordError"
        autocomplete="new-password"
      />
    </AppField>

    <AppAlert v-if="submitError" severity="error">{{ submitError }}</AppAlert>

    <button type="submit" class="btn btn-primary w-full" :disabled="loading">
      <span v-if="loading" class="loading loading-spinner loading-sm" />
      {{ loading ? "重置中..." : "重置密码" }}
    </button>
  </form>
</template>

<style scoped>
.auth-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.reset-success {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 1rem;
  padding: 1rem 0;
}

.success-icon {
  width: 4rem;
  height: 4rem;
  border-radius: 50%;
  background: color-mix(in oklch, var(--color-primary) 15%, transparent);
  color: var(--color-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.75rem;
  font-weight: 700;
}

.success-title {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.success-desc {
  margin: 0;
  font-size: 0.9rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  max-width: 320px;
  line-height: 1.5;
}
</style>
