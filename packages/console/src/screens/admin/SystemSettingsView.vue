<script setup lang="ts">
import { onMounted, ref, computed } from "vue";
import { Gauge, Globe, Mail, Shield, UserPlus } from "@lucide/vue";
import { AppField, AppPage, PasswordInput, useToast } from "@/ui";
import { api } from "@/utils/api";
import ProjectMegaNav, { type MegaNavItem } from "@/components/projects/ProjectMegaNav.vue";
import {
  pickSectionSettings,
  type SettingsMap,
  type SettingsSection,
} from "./systemSettingsSections";

const toast = useToast();
const loading = ref(false);
const saving = ref(false);
const section = ref<SettingsSection>("site");
const settings = ref<SettingsMap>({});
const publicOrigin = ref("");

const sectionOptions: MegaNavItem[] = [
  { key: "site", label: "站点", icon: Globe },
  { key: "registration", label: "注册设置", icon: UserPlus },
  { key: "security", label: "安全设置", icon: Shield },
  { key: "rateLimit", label: "限流设置", icon: Gauge },
  { key: "email", label: "邮件设置", icon: Mail },
];

function onSectionSelect(key: string) {
  section.value = key as SettingsSection;
}

const inviteOnly = computed({
  get: () => settings.value["registration.inviteOnly"] === "true",
  set: v => (settings.value["registration.inviteOnly"] = String(v)),
});

const passwordMinLength = computed({
  get: () => Number(settings.value["security.passwordMinLength"] || "8"),
  set: v => (settings.value["security.passwordMinLength"] = String(v)),
});

const passwordRequireNumber = computed({
  get: () => settings.value["security.passwordRequireNumber"] === "true",
  set: v => (settings.value["security.passwordRequireNumber"] = String(v)),
});

const passwordRequireUppercase = computed({
  get: () => settings.value["security.passwordRequireUppercase"] === "true",
  set: v => (settings.value["security.passwordRequireUppercase"] = String(v)),
});

const passwordRequireSpecialChar = computed({
  get: () => settings.value["security.passwordRequireSpecialChar"] === "true",
  set: v => (settings.value["security.passwordRequireSpecialChar"] = String(v)),
});

const loginMaxAttempts = computed({
  get: () => Number(settings.value["security.loginMaxAttempts"] || "5"),
  set: v => (settings.value["security.loginMaxAttempts"] = String(v)),
});

const loginLockoutMinutes = computed({
  get: () => Number(settings.value["security.loginLockoutMinutes"] || "30"),
  set: v => (settings.value["security.loginLockoutMinutes"] = String(v)),
});

const rateLimitGeneralMax = computed({
  get: () => Number(settings.value["rateLimit.generalMax"] || "1000"),
  set: v => (settings.value["rateLimit.generalMax"] = String(v)),
});

const rateLimitGeneralWindowMs = computed({
  get: () => Number(settings.value["rateLimit.generalWindowMs"] || "60000"),
  set: v => (settings.value["rateLimit.generalWindowMs"] = String(v)),
});

const rateLimitAuthMax = computed({
  get: () => Number(settings.value["rateLimit.authMax"] || "20"),
  set: v => (settings.value["rateLimit.authMax"] = String(v)),
});

const rateLimitAuthWindowMs = computed({
  get: () => Number(settings.value["rateLimit.authWindowMs"] || "60000"),
  set: v => (settings.value["rateLimit.authWindowMs"] = String(v)),
});

const smtpSecure = computed({
  get: () => settings.value["email.smtpSecure"] === "true",
  set: v => (settings.value["email.smtpSecure"] = String(v)),
});

const smtpPort = computed({
  get: () => Number(settings.value["email.smtpPort"] || "587"),
  set: v => (settings.value["email.smtpPort"] = String(v)),
});

const testTo = ref("");
const testing = ref(false);

async function fetchSettings() {
  loading.value = true;
  try {
    const [settingsRes, instanceRes] = await Promise.all([
      api.get<{ success: boolean; data: SettingsMap }>("/admin/settings"),
      api.get<{ success: boolean; data: { publicOrigin: string } }>("/admin/instance"),
    ]);
    if (settingsRes.data.success) {
      settings.value = settingsRes.data.data;
    }
    if (instanceRes.data.success) {
      publicOrigin.value = instanceRes.data.data.publicOrigin;
    }
  } catch {
    toast.error("获取失败", "无法加载平台设置");
  } finally {
    loading.value = false;
  }
}

async function saveSiteSection() {
  saving.value = true;
  try {
    const { data } = await api.patch<{ success: boolean }>("/admin/instance", {
      publicOrigin: publicOrigin.value,
    });
    if (data.success) {
      toast.success("保存成功", "站点设置已更新");
    }
  } catch {
    toast.error("保存失败", "请检查输入后重试");
  } finally {
    saving.value = false;
  }
}

async function saveSettingsSection(target: Exclude<SettingsSection, "site">) {
  saving.value = true;
  try {
    const patch = pickSectionSettings(target, settings.value);
    const { data } = await api.patch<{ success: boolean }>("/admin/settings", patch);
    if (data.success) {
      toast.success("保存成功", "本页设置已更新");
    }
  } catch {
    toast.error("保存失败", "请检查输入后重试");
  } finally {
    saving.value = false;
  }
}

async function sendTestEmail() {
  const to = testTo.value.trim();
  if (!to) {
    toast.error("请填写收件人", "测试邮件需要收件人邮箱");
    return;
  }
  testing.value = true;
  try {
    const { data } = await api.post<{ success: boolean; message?: string }>(
      "/admin/email/test",
      { to },
    );
    if (data.success) {
      toast.success("已发送", `测试邮件已发往 ${to}`);
    }
  } catch (error) {
    const message =
      (error as { response?: { data?: { message?: string } } })?.response?.data?.message ||
      "请确认已保存 SMTP 配置后重试";
    toast.error("发送失败", message);
  } finally {
    testing.value = false;
  }
}

onMounted(fetchSettings);
</script>

<template>
  <AppPage
    title="平台设置"
    class="settings-page"
  >
    <ProjectMegaNav
      class="mb-4"
      :items="sectionOptions"
      :active-key="section"
      label="平台设置菜单"
      @select="onSectionSelect"
    />

    <template v-if="loading">
      <div class="skeleton h-9 w-56 mb-3" />
      <div class="skeleton h-56 w-full rounded-box" />
    </template>

    <section v-show="section === 'site'" class="card bg-base-100 p-6 flex flex-col gap-5">
      <AppField
        label="对外访问地址"
        html-for="publicOrigin"
        hint="影响邮件链接、CLI 安装与更新基址。修改后立即生效，无需重装。"
      >
        <input
          id="publicOrigin"
          v-model="publicOrigin"
          type="text"
          class="input w-full"
          placeholder="http://10.0.0.2:11111"
        />
      </AppField>
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving || loading"
          @click="saveSiteSection"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs" />
          保存
        </button>
      </div>
    </section>

    <section v-show="section === 'registration'" class="card bg-base-100 p-6 flex flex-col gap-5">
      <AppField
        label="仅允许邀请注册"
        hint="开启后，新用户必须输入有效邀请码才能注册。公开注册用户角色固定为普通用户；邀请注册角色由邀请码决定。"
      >
        <label class="fieldset-label cursor-pointer justify-start gap-3 py-0">
          <input v-model="inviteOnly" type="checkbox" class="toggle toggle-primary" />
          {{ inviteOnly ? "已开启" : "已关闭" }}
        </label>
      </AppField>
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving || loading"
          @click="saveSettingsSection('registration')"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs" />
          保存
        </button>
      </div>
    </section>

    <section v-show="section === 'security'" class="card bg-base-100 p-6 flex flex-col gap-5">
      <AppField label="密码最小长度">
        <input
          id="passwordMinLength"
          v-model.number="passwordMinLength"
          type="number"
          class="input w-full max-w-xs"
          min="6"
          max="128"
        />
      </AppField>

      <label class="fieldset-label">
        <input
          id="passwordRequireNumber"
          v-model="passwordRequireNumber"
          type="checkbox"
          class="toggle toggle-primary"
        />
        <span>密码必须包含数字</span>
      </label>

      <label class="fieldset-label">
        <input
          id="passwordRequireUppercase"
          v-model="passwordRequireUppercase"
          type="checkbox"
          class="toggle toggle-primary"
        />
        <span>密码必须包含大写字母</span>
      </label>

      <label class="fieldset-label">
        <input
          id="passwordRequireSpecialChar"
          v-model="passwordRequireSpecialChar"
          type="checkbox"
          class="toggle toggle-primary"
        />
        <span>密码必须包含特殊字符</span>
      </label>

      <AppField
        label="登录失败锁定阈值"
        hint="同一标识连续失败多少次后触发锁定。"
      >
        <input
          id="loginMaxAttempts"
          v-model.number="loginMaxAttempts"
          type="number"
          class="input w-full max-w-xs"
          min="1"
          max="20"
        />
      </AppField>

      <AppField label="锁定时长（分钟）">
        <input
          id="loginLockoutMinutes"
          v-model.number="loginLockoutMinutes"
          type="number"
          class="input w-full max-w-xs"
          min="1"
          max="1440"
        />
      </AppField>
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving || loading"
          @click="saveSettingsSection('security')"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs" />
          保存
        </button>
      </div>
    </section>

    <section v-show="section === 'rateLimit'" class="card bg-base-100 p-6 flex flex-col gap-5">
      <AppField
        label="全局限流次数"
        hint="每个客户端 IP 在窗口期内允许的最大 API 请求数；设为 0 关闭全局限流。"
      >
        <input
          id="rateLimitGeneralMax"
          v-model.number="rateLimitGeneralMax"
          type="number"
          class="input w-full max-w-xs"
          min="0"
          max="100000"
        />
      </AppField>

      <AppField
        label="全局限流窗口（毫秒）"
        hint="默认 60000（1 分钟）。保存后约 30 秒内生效。"
      >
        <input
          id="rateLimitGeneralWindowMs"
          v-model.number="rateLimitGeneralWindowMs"
          type="number"
          class="input w-full max-w-xs"
          min="1000"
          max="3600000"
          step="1000"
        />
      </AppField>

      <AppField
        label="认证接口限流次数"
        hint="作用于登录 / 注册 / 找回密码等敏感接口；设为 0 关闭。"
      >
        <input
          id="rateLimitAuthMax"
          v-model.number="rateLimitAuthMax"
          type="number"
          class="input w-full max-w-xs"
          min="0"
          max="10000"
        />
      </AppField>

      <AppField label="认证限流窗口（毫秒）">
        <input
          id="rateLimitAuthWindowMs"
          v-model.number="rateLimitAuthWindowMs"
          type="number"
          class="input w-full max-w-xs"
          min="1000"
          max="3600000"
          step="1000"
        />
      </AppField>
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving || loading"
          @click="saveSettingsSection('rateLimit')"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs" />
          保存
        </button>
      </div>
    </section>

    <section v-show="section === 'email'" class="card bg-base-100 p-6 flex flex-col gap-5">
      <AppField label="发件人邮箱">
        <input
          id="fromAddress"
          v-model="settings['email.fromAddress']"
          type="email"
          class="input w-full"
          placeholder="noreply@example.com"
        />
      </AppField>

      <AppField label="发件人名称">
        <input
          id="fromName"
          v-model="settings['email.fromName']"
          type="text"
          class="input w-full"
          placeholder="春笋"
        />
      </AppField>

      <AppField label="SMTP 服务器">
        <input
          id="smtpHost"
          v-model="settings['email.smtpHost']"
          type="text"
          class="input w-full"
          placeholder="smtp.example.com"
        />
      </AppField>

      <AppField label="SMTP 端口">
        <input
          id="smtpPort"
          v-model.number="smtpPort"
          type="number"
          class="input w-full max-w-xs"
          min="1"
          max="65535"
        />
      </AppField>

      <label class="fieldset-label">
        <input id="smtpSecure" v-model="smtpSecure" type="checkbox" class="toggle toggle-primary" />
        <span>使用 SSL/TLS（端口通常为 465）</span>
      </label>

      <AppField label="SMTP 用户名">
        <input id="smtpUser" v-model="settings['email.smtpUser']" type="text" class="input w-full" />
      </AppField>

      <AppField label="SMTP 密码">
        <PasswordInput
          id="smtpPassword"
          v-model="settings['email.smtpPassword']"
          placeholder="留空表示使用环境变量 SMTP_PASS"
        />
      </AppField>

      <div class="divider my-1" />

      <AppField
        label="测试收件人"
        html-for="testTo"
        hint="使用已保存的 SMTP 配置发送；修改设置后请先保存再测试。"
      >
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
          <input
            id="testTo"
            v-model="testTo"
            type="email"
            class="input w-full"
            placeholder="you@example.com"
          />
          <button
            type="button"
            class="btn btn-outline shrink-0"
            :disabled="testing || loading || saving"
            @click="sendTestEmail"
          >
            <span v-if="testing" class="loading loading-spinner loading-xs" />
            发送测试邮件
          </button>
        </div>
      </AppField>
      <div class="panel-actions">
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving || loading"
          @click="saveSettingsSection('email')"
        >
          <span v-if="saving" class="loading loading-spinner loading-xs" />
          保存
        </button>
      </div>
    </section>
  </AppPage>
</template>

<style scoped>
.settings-page {
  max-width: 720px;
}

.panel-actions {
  display: flex;
  justify-content: flex-end;
}
</style>
