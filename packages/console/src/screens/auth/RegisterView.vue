<script setup lang="ts">
import { Mail } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useField, useForm } from "vee-validate";
import { toTypedSchema } from "@vee-validate/zod";
import { z } from "zod";
import { useAuthStore } from "@/stores/auth";
import { AppAlert, AppField, PasswordInput, useToast } from "@/ui";
import { api } from "@/utils/api";

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const toast = useToast();

const registeredEmail = ref("");
const registered = ref(false);
const loading = ref(false);
const submitError = ref("");
const configLoading = ref(true);
const inviteOnly = ref(false);

const schema = computed(() =>
  toTypedSchema(
    z
      .object({
        email: z.string().email("请输入有效邮箱").max(100, "邮箱过长"),
        password: z.string().min(6, "密码至少 6 位").max(100, "密码过长"),
        confirmPassword: z.string().min(1, "请确认密码"),
        inviteCode: inviteOnly.value
          ? z.string().min(1, "请输入邀请码").max(64, "邀请码过长")
          : z.string().max(64, "邀请码过长").optional(),
        nickname: z.string().max(50, "昵称过长").optional(),
      })
      .refine(data => data.password === data.confirmPassword, {
        message: "两次输入的密码不一致",
        path: ["confirmPassword"],
      }),
  ),
);

const { handleSubmit, resetForm } = useForm({
  validationSchema: schema,
  initialValues: {
    email: "",
    password: "",
    confirmPassword: "",
    inviteCode: (route.query.code as string) || "",
    nickname: "",
  },
});

const { value: email, errorMessage: emailError } = useField<string>("email");
const { value: password, errorMessage: passwordError } = useField<string>("password");
const { value: confirmPassword, errorMessage: confirmPasswordError } =
  useField<string>("confirmPassword");
const { value: inviteCode, errorMessage: inviteCodeError } = useField<string>("inviteCode");
const { value: nickname, errorMessage: nicknameError } = useField<string>("nickname");

onMounted(async () => {
  try {
    const { data } = await api.get<{ success: boolean; data: { inviteOnly: boolean } }>(
      "/auth/registration-config",
    );
    inviteOnly.value = data.data.inviteOnly;
    resetForm({
      values: {
        email: "",
        password: "",
        confirmPassword: "",
        inviteCode: inviteOnly.value ? (route.query.code as string) || "" : "",
        nickname: "",
      },
    });
  } catch {
    submitError.value = "无法加载注册配置，请刷新重试";
  } finally {
    configLoading.value = false;
  }
});

const onSubmit = handleSubmit(async values => {
  loading.value = true;
  submitError.value = "";
  try {
    const result = await auth.register({
      email: values.email,
      password: values.password,
      inviteCode: inviteOnly.value ? values.inviteCode || undefined : undefined,
      nickname: values.nickname || undefined,
    });
    registeredEmail.value = result.email;
    registered.value = true;
  } catch (error) {
    submitError.value = error instanceof Error ? error.message : "注册失败";
  } finally {
    loading.value = false;
  }
});

async function resendEmail() {
  if (!registeredEmail.value) return;
  try {
    await api.post("/auth/resend-verification", { email: registeredEmail.value });
    toast.success("已发送", "验证邮件已重新发送，请查收");
  } catch {
    toast.error("发送失败", "请稍后重试");
  }
}
</script>

<template>
  <div v-if="registered" class="register-success">
    <div class="success-icon" aria-hidden="true">
      <Mail :size="28" />
    </div>
    <h2 class="success-title">注册成功</h2>
    <p class="success-desc">
      我们已向 <strong>{{ registeredEmail }}</strong> 发送验证邮件，请点击邮件中的链接完成验证。
    </p>
    <div class="success-actions">
      <button type="button" class="btn btn-ghost w-full" @click="resendEmail">
        重新发送验证邮件
      </button>
      <button type="button" class="btn btn-primary w-full" @click="router.push('/auth/login')">
        去登录
      </button>
    </div>
  </div>

  <div v-else-if="configLoading" class="auth-form">
    <span class="loading loading-spinner loading-md mx-auto" />
  </div>

  <form v-else class="auth-form" @submit.prevent="onSubmit">
    <AppAlert v-if="inviteOnly" severity="warning">
      当前仅支持邀请注册，请填写有效邀请码。
    </AppAlert>

    <AppField label="邮箱" html-for="register-email" :error="emailError">
      <input
        id="register-email"
        v-model="email"
        type="email"
        class="input w-full"
        :class="{ 'input-error': !!emailError }"
        placeholder="you@example.com"
        autocomplete="email"
      />
    </AppField>

    <AppField label="昵称（可选）" html-for="register-nickname" :error="nicknameError">
      <input
        id="register-nickname"
        v-model="nickname"
        type="text"
        class="input w-full"
        :class="{ 'input-error': !!nicknameError }"
        placeholder="请输入昵称"
      />
    </AppField>

    <AppField
      v-if="inviteOnly"
      label="邀请码"
      html-for="register-invite-code"
      :error="inviteCodeError"
    >
      <input
        id="register-invite-code"
        v-model="inviteCode"
        type="text"
        class="input w-full"
        :class="{ 'input-error': !!inviteCodeError }"
        placeholder="请输入邀请码"
      />
    </AppField>

    <AppField label="密码" html-for="register-password" :error="passwordError">
      <PasswordInput
        id="register-password"
        v-model="password"
        placeholder="请输入密码"
        :invalid="!!passwordError"
        autocomplete="new-password"
      />
    </AppField>

    <AppField label="确认密码" html-for="register-confirm-password" :error="confirmPasswordError">
      <PasswordInput
        id="register-confirm-password"
        v-model="confirmPassword"
        placeholder="请再次输入密码"
        :invalid="!!confirmPasswordError"
        autocomplete="new-password"
      />
    </AppField>

    <AppAlert v-if="submitError" severity="error">{{ submitError }}</AppAlert>

    <button type="submit" class="btn btn-primary w-full" :disabled="loading">
      <span v-if="loading" class="loading loading-spinner loading-sm" />
      {{ loading ? "注册中..." : "注册" }}
    </button>

    <div class="auth-switch">
      已有账号？<RouterLink to="/auth/login" class="switch-link">去登录</RouterLink>
    </div>
  </form>
</template>

<style scoped>
.auth-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.auth-switch {
  text-align: center;
  font-size: 0.875rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.switch-link {
  color: var(--color-primary);
  text-decoration: none;
}

.switch-link:hover {
  text-decoration: underline;
}

.register-success {
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

.success-actions {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  width: 100%;
  max-width: 280px;
  margin-top: 0.5rem;
}
</style>
