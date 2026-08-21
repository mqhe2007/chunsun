<script setup lang="ts">
import { Mail } from "@lucide/vue";
import { ref } from "vue";
import { useField, useForm } from "vee-validate";
import { toTypedSchema } from "@vee-validate/zod";
import { z } from "zod";
import { useRouter } from "vue-router";
import { AppAlert, AppField } from "@/ui";
import { api } from "@/utils/api";

const router = useRouter();
const loading = ref(false);
const submitted = ref(false);
const submitError = ref("");

const schema = toTypedSchema(
  z.object({
    email: z.string().email("请输入有效邮箱"),
  }),
);

const { handleSubmit } = useForm({
  validationSchema: schema,
  initialValues: { email: "" },
});

const { value: email, errorMessage: emailError } = useField<string>("email");

const onSubmit = handleSubmit(async values => {
  loading.value = true;
  submitError.value = "";
  try {
    await api.post("/auth/forgot-password", { email: values.email });
    submitted.value = true;
  } catch {
    submitError.value = "提交失败，请稍后重试";
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div v-if="submitted" class="forgot-success">
    <div class="success-icon" aria-hidden="true">
      <Mail :size="28" />
    </div>
    <h2 class="success-title">邮件已发送</h2>
    <p class="success-desc">
      如果该邮箱存在账户，你将收到一封包含重置密码链接的邮件。
    </p>
    <button type="button" class="btn btn-primary" @click="router.push('/auth/login')">
      返回登录
    </button>
  </div>

  <form v-else class="auth-form" @submit.prevent="onSubmit">
    <p class="form-hint">请输入你的注册邮箱，我们将向你发送重置密码链接。</p>

    <AppField label="邮箱" html-for="forgot-email" :error="emailError">
      <input
        id="forgot-email"
        v-model="email"
        type="email"
        class="input w-full"
        :class="{ 'input-error': !!emailError }"
        placeholder="you@example.com"
        autocomplete="email"
      />
    </AppField>

    <AppAlert v-if="submitError" severity="error">{{ submitError }}</AppAlert>

    <button type="submit" class="btn btn-primary w-full" :disabled="loading">
      <span v-if="loading" class="loading loading-spinner loading-sm" />
      {{ loading ? "发送中..." : "发送重置邮件" }}
    </button>
  </form>
</template>

<style scoped>
.auth-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.form-hint {
  margin: 0;
  font-size: 0.9rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  line-height: 1.5;
}

.forgot-success {
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
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.75rem;
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
