<script setup lang="ts">
import { ref } from "vue";
import { useField, useForm } from "vee-validate";
import { toTypedSchema } from "@vee-validate/zod";
import { z } from "zod";
import { useAuthStore } from "@/stores/auth";
import { AppField, PasswordInput } from "@/ui";

const auth = useAuthStore();
const loading = ref(false);
const submitError = ref("");

const schema = toTypedSchema(
  z.object({
    email: z.string().email("请输入有效邮箱"),
    password: z.string().min(1, "请输入密码"),
  }),
);

const { handleSubmit } = useForm({
  validationSchema: schema,
  initialValues: { email: "", password: "" },
});

const { value: email, errorMessage: emailError } = useField<string>("email");
const { value: password, errorMessage: passwordError } = useField<string>("password");

const onSubmit = handleSubmit(async values => {
  loading.value = true;
  submitError.value = "";
  try {
    await auth.login(values.email, values.password);
  } catch (error) {
    submitError.value = error instanceof Error ? error.message : "登录失败";
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <form class="auth-form" @submit.prevent="onSubmit">
    <AppField label="邮箱" html-for="login-email" :error="emailError">
      <input
        id="login-email"
        v-model="email"
        type="email"
        class="input w-full"
        :class="{ 'input-error': !!emailError }"
        placeholder="you@example.com"
        autocomplete="email"
      />
    </AppField>

    <AppField label="密码" html-for="login-password" :error="passwordError">
      <PasswordInput
        id="login-password"
        v-model="password"
        :invalid="!!passwordError"
        placeholder="请输入密码"
        autocomplete="current-password"
      />
    </AppField>

    <div class="auth-options">
      <RouterLink to="/auth/forgot-password" class="link link-primary text-sm">
        忘记密码？
      </RouterLink>
    </div>

    <div v-if="submitError" class="alert alert-error text-sm">
      <span>{{ submitError }}</span>
    </div>

    <button type="submit" class="btn btn-primary w-full" :disabled="loading">
      <span v-if="loading" class="loading loading-spinner loading-sm" />
      {{ loading ? "登录中..." : "登录" }}
    </button>

    <div class="auth-switch">
      还没有账号？
      <RouterLink to="/auth/register" class="link link-primary">立即注册</RouterLink>
    </div>
  </form>
</template>

<style scoped>
.auth-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.auth-options {
  display: flex;
  justify-content: flex-end;
}

.auth-switch {
  text-align: center;
  font-size: 0.875rem;
  color: color-mix(in oklab, var(--color-base-content) 60%, transparent);
}
</style>
