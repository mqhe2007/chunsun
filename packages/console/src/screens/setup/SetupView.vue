<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import { useField, useForm } from "vee-validate";
import { toTypedSchema } from "@vee-validate/zod";
import { z } from "zod";
import { BrandMark } from "@chunsun/web-shared";
import { AppAlert, AppField, PasswordInput } from "@/ui";
import { api } from "@/utils/api";
import { useSetupStore } from "@/stores/setup";

const router = useRouter();
const setup = useSetupStore();
const year = new Date().getFullYear();

/** 向导预填站点地址：优先当前 origin，历史 Vite 5173 口纠正为 11111 */
function defaultPublicOrigin(): string {
  if (typeof window === "undefined") return "http://127.0.0.1:11111";
  const { protocol, hostname, port } = window.location;
  if (port === "5173") return `${protocol}//127.0.0.1:11111`;
  return window.location.origin;
}

const testing = ref(false);
const submitting = ref(false);
const submitError = ref("");
const testMessage = ref("");
const testOk = ref(false);

const schema = toTypedSchema(
  z
    .object({
      host: z.string().min(1, "请填写数据库主机"),
      port: z.coerce.number().int().min(1, "请填写端口").max(65535, "端口无效"),
      user: z.string().min(1, "请填写数据库用户"),
      password: z.string(),
      name: z.string().min(1, "请填写数据库名"),
      ssl: z.boolean(),
      publicOrigin: z
        .string()
        .url("请填写完整站点地址，例如 http://10.0.0.2:11111")
        .refine(v => v.startsWith("http://") || v.startsWith("https://"), "需以 http:// 或 https:// 开头"),
      adminEmail: z.string().email("请输入有效邮箱").max(100),
      adminPassword: z
        .string()
        .min(8, "密码至少 8 位")
        .refine(v => /\d/.test(v), "密码需要包含数字"),
      confirmPassword: z.string().min(1, "请确认密码"),
      nickname: z.string().max(50).optional(),
    })
    .refine(data => data.adminPassword === data.confirmPassword, {
      message: "两次输入的密码不一致",
      path: ["confirmPassword"],
    }),
);

const { handleSubmit } = useForm({
  validationSchema: schema,
  initialValues: {
    host: "127.0.0.1",
    port: 5432,
    user: "postgres",
    password: "",
    name: "chunsun",
    ssl: false,
    publicOrigin: defaultPublicOrigin(),
    adminEmail: "",
    adminPassword: "",
    confirmPassword: "",
    nickname: "管理员",
  },
});

const { value: host, errorMessage: hostError } = useField<string>("host");
const { value: port, errorMessage: portError } = useField<number>("port");
const { value: user, errorMessage: userError } = useField<string>("user");
const { value: password, errorMessage: passwordError } = useField<string>("password");
const { value: name, errorMessage: nameError } = useField<string>("name");
const { value: ssl } = useField<boolean>("ssl");
const { value: publicOrigin, errorMessage: originError } = useField<string>("publicOrigin");
const { value: adminEmail, errorMessage: adminEmailError } = useField<string>("adminEmail");
const { value: adminPassword, errorMessage: adminPasswordError } = useField<string>("adminPassword");
const { value: confirmPassword, errorMessage: confirmError } = useField<string>("confirmPassword");
const { value: nickname, errorMessage: nicknameError } = useField<string>("nickname");

const databasePayload = computed(() => ({
  host: host.value,
  port: Number(port.value),
  user: user.value,
  password: password.value,
  name: name.value,
  ssl: Boolean(ssl.value),
}));

async function testDatabase() {
  testing.value = true;
  testMessage.value = "";
  testOk.value = false;
  submitError.value = "";
  try {
    await api.post("/setup/test-database", { database: databasePayload.value });
    testOk.value = true;
    testMessage.value = "数据库连接成功";
  } catch (error) {
    testOk.value = false;
    testMessage.value = setupErrorMessage(error, "无法连接数据库");
  } finally {
    testing.value = false;
  }
}

const onSubmit = handleSubmit(async values => {
  submitting.value = true;
  submitError.value = "";
  try {
    await api.post("/setup/complete", {
      database: {
        host: values.host,
        port: Number(values.port),
        user: values.user,
        password: values.password,
        name: values.name,
        ssl: values.ssl,
      },
      publicOrigin: values.publicOrigin.replace(/\/$/, ""),
      admin: {
        email: values.adminEmail,
        password: values.adminPassword,
        nickname: values.nickname || undefined,
      },
    });
    setup.markComplete();
    await router.push("/auth/login");
  } catch (error) {
    submitError.value = setupErrorMessage(error, "安装失败");
  } finally {
    submitting.value = false;
  }
});

function setupErrorMessage(error: unknown, fallback: string): string {
  const data = (error as { response?: { data?: { message?: string; error?: string } } })?.response
    ?.data;
  return data?.message || data?.error || fallback;
}
</script>

<template>
  <div class="min-h-screen bg-base-200 flex items-center justify-center p-4 sm:p-6">
    <main class="card bg-base-100 w-full max-w-2xl shadow-xl">
      <div class="card-body gap-6">
        <header class="flex items-start gap-4">
          <BrandMark size="2rem" class="text-[var(--chunsun-shoot)] shrink-0" />
          <div>
            <p class="text-sm text-base-content/60">春笋 · 首次安装</p>
            <h1 class="text-2xl font-bold">配置你的平台实例</h1>
            <p class="text-sm text-base-content/60 mt-2">
              填写 PostgreSQL 连接与管理员账号。完成后配置会写入程序同级的实例文件，无需再维护 .env。
            </p>
          </div>
        </header>

      <form class="flex flex-col gap-6" @submit.prevent="onSubmit">
        <section class="flex flex-col gap-3">
          <h2 class="text-lg font-semibold">数据库</h2>
          <p class="text-sm text-base-content/60">请事先创建空库；春笋会自动执行迁移。</p>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <AppField label="主机" html-for="db-host" :error="hostError">
              <input
                id="db-host"
                v-model="host"
                type="text"
                class="input w-full"
                :class="{ 'input-error': !!hostError }"
              />
            </AppField>

            <AppField label="端口" html-for="db-port" :error="portError">
              <input
                id="db-port"
                v-model.number="port"
                type="number"
                class="input w-full"
                :class="{ 'input-error': !!portError }"
              />
            </AppField>

            <AppField label="用户" html-for="db-user" :error="userError">
              <input
                id="db-user"
                v-model="user"
                type="text"
                class="input w-full"
                :class="{ 'input-error': !!userError }"
                autocomplete="username"
              />
            </AppField>

            <AppField label="密码" html-for="db-password" :error="passwordError">
              <PasswordInput
                id="db-password"
                v-model="password"
                :invalid="!!passwordError"
                autocomplete="new-password"
              />
            </AppField>

            <AppField label="数据库名" html-for="db-name" :error="nameError">
              <input
                id="db-name"
                v-model="name"
                type="text"
                class="input w-full"
                :class="{ 'input-error': !!nameError }"
              />
            </AppField>

            <AppField label="SSL" html-for="db-ssl">
              <label class="fieldset-label cursor-pointer justify-start gap-3 py-2">
                <input id="db-ssl" v-model="ssl" type="checkbox" class="toggle" />
                {{ ssl ? "要求 SSL" : "不使用 SSL" }}
              </label>
            </AppField>
          </div>
          <div class="flex flex-wrap items-center gap-3 mt-2">
            <button
              type="button"
              class="btn btn-ghost"
              :disabled="testing || submitting"
              @click="testDatabase"
            >
              <span v-if="testing" class="loading loading-spinner loading-sm" />
              测试连接
            </button>
            <small v-if="testMessage" :class="testOk ? 'text-success' : 'text-error'">
              {{ testMessage }}
            </small>
          </div>
        </section>

        <section class="flex flex-col gap-3">
          <h2 class="text-lg font-semibold">站点</h2>
          <AppField
            label="对外访问地址"
            html-for="public-origin"
            :error="originError"
            hint="用于邮件链接与 CLI 安装地址，需浏览器能访问到本实例。"
          >
            <input
              id="public-origin"
              v-model="publicOrigin"
              type="url"
              class="input w-full"
              :class="{ 'input-error': !!originError }"
              placeholder="http://10.0.0.2:11111"
            />
          </AppField>
        </section>

        <section class="flex flex-col gap-3">
          <h2 class="text-lg font-semibold">管理员</h2>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <AppField label="邮箱" html-for="admin-email" :error="adminEmailError">
              <input
                id="admin-email"
                v-model="adminEmail"
                type="email"
                class="input w-full"
                :class="{ 'input-error': !!adminEmailError }"
                autocomplete="email"
              />
            </AppField>

            <AppField label="昵称" html-for="admin-nickname" :error="nicknameError">
              <input
                id="admin-nickname"
                v-model="nickname"
                type="text"
                class="input w-full"
                :class="{ 'input-error': !!nicknameError }"
              />
            </AppField>

            <AppField label="密码" html-for="admin-password" :error="adminPasswordError">
              <PasswordInput
                id="admin-password"
                v-model="adminPassword"
                :invalid="!!adminPasswordError"
                autocomplete="new-password"
              />
            </AppField>

            <AppField label="确认密码" html-for="admin-confirm" :error="confirmError">
              <PasswordInput
                id="admin-confirm"
                v-model="confirmPassword"
                :invalid="!!confirmError"
                autocomplete="new-password"
              />
            </AppField>
          </div>
        </section>

        <AppAlert v-if="submitError" severity="error">{{ submitError }}</AppAlert>

        <button type="submit" class="btn btn-primary w-full" :disabled="submitting">
          <span v-if="submitting" class="loading loading-spinner loading-sm" />
          {{ submitting ? "正在安装..." : "完成安装" }}
        </button>
      </form>

        <p class="text-center text-xs text-base-content/50">© {{ year }} 春笋</p>
      </div>
    </main>
  </div>
</template>
