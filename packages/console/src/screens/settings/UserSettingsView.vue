<script setup lang="ts">
import { Lock, RefreshCw, TriangleAlert } from "@lucide/vue";
import { onMounted, ref, computed } from "vue";
import { AppField, AppPage, PasswordInput, useToast } from "@/ui";
import { api } from "@/utils/api";
import UserAvatar from "@/components/common/UserAvatar.vue";
import SettingsMegaNav from "@/components/settings/SettingsMegaNav.vue";

type User = {
  id: string;
  email: string;
  qq?: string | null;
  nickname?: string | null;
  role: string;
  status: string;
  createdAt: string;
  updatedAt: string;
};

const props = withDefaults(
  defineProps<{ section?: "profile" | "password" }>(),
  { section: "profile" },
);

const toast = useToast();
const loading = ref(false);
const loadError = ref(false);
const saving = ref(false);
const changingPassword = ref(false);
const user = ref<User | null>(null);
const profileForm = ref({ nickname: "", qq: "" });
const passwordForm = ref({ currentPassword: "", newPassword: "", confirmPassword: "" });

const displayName = computed(() => user.value?.nickname || user.value?.email || "");
const previewQq = computed(() => profileForm.value.qq.trim() || user.value?.qq || "");

async function fetchUser() {
  loading.value = true;
  loadError.value = false;
  try {
    const { data } = await api.get<{ success: boolean; data: User }>("/users/me");
    if (data.success) {
      user.value = data.data;
      profileForm.value = {
        nickname: data.data.nickname || "",
        qq: data.data.qq || "",
      };
    } else {
      loadError.value = true;
      toast.add({ severity: "error", summary: "获取失败", detail: "获取用户信息失败", life: 3000 });
    }
  } catch (err) {
    loadError.value = true;
    console.error("[UserSettings] fetchUser failed:", err);
    toast.add({ severity: "error", summary: "获取失败", detail: "获取用户信息失败，请重试", life: 3000 });
  } finally {
    loading.value = false;
  }
}

async function updateProfile() {
  saving.value = true;
  try {
    const { data } = await api.patch<{ success: boolean; data: User }>("/users/me/profile", {
      nickname: profileForm.value.nickname || undefined,
      qq: profileForm.value.qq || undefined,
    });
    if (data.success) {
      user.value = data.data;
      toast.add({ severity: "success", summary: "保存成功", detail: "个人信息已更新", life: 3000 });
    } else {
      toast.add({ severity: "error", summary: "保存失败", detail: "保存失败", life: 3000 });
    }
  } catch {
    toast.add({ severity: "error", summary: "保存失败", detail: "保存失败", life: 3000 });
  } finally {
    saving.value = false;
  }
}

async function changePassword() {
  if (!passwordForm.value.currentPassword) {
    toast.add({ severity: "warn", summary: "验证失败", detail: "请输入当前密码", life: 3000 });
    return;
  }
  if (passwordForm.value.newPassword.length < 6) {
    toast.add({ severity: "warn", summary: "验证失败", detail: "新密码至少 6 位", life: 3000 });
    return;
  }
  if (passwordForm.value.newPassword !== passwordForm.value.confirmPassword) {
    toast.add({ severity: "warn", summary: "验证失败", detail: "两次密码输入不一致", life: 3000 });
    return;
  }
  changingPassword.value = true;
  try {
    const { data } = await api.post<{ success: boolean }>("/users/me/change-password", {
      currentPassword: passwordForm.value.currentPassword,
      newPassword: passwordForm.value.newPassword,
    });
    if (data.success) {
      toast.add({ severity: "success", summary: "修改成功", detail: "密码已修改，请妥善保管", life: 3000 });
      passwordForm.value = { currentPassword: "", newPassword: "", confirmPassword: "" };
    } else {
      toast.add({ severity: "error", summary: "修改失败", detail: "当前密码错误或请求失败", life: 3000 });
    }
  } catch {
    toast.add({ severity: "error", summary: "修改失败", detail: "修改密码失败", life: 3000 });
  } finally {
    changingPassword.value = false;
  }
}

onMounted(fetchUser);
</script>

<template>
  <AppPage title="账户设置" class="profile-page">
    <SettingsMegaNav />

    <template v-if="loading">
      <div class="skeleton h-56 w-full rounded-box" />
    </template>

    <template v-else-if="loadError">
      <div class="console-empty-state">
        <TriangleAlert class="empty-icon text-warning" :size="40" aria-hidden="true" />
        <p class="text-base-content/60">获取用户信息失败，请重试</p>
        <button type="button" class="btn btn-ghost" @click="fetchUser">
          <RefreshCw :size="14" aria-hidden="true" />
          重试
        </button>
      </div>
    </template>

    <template v-else-if="user">
      <section v-if="props.section === 'profile'" class="card bg-base-100 p-6 flex flex-col gap-5">
        <div class="info-row">
          <div class="info-item">
            <div class="info-label text-xs font-semibold text-base-content/65">昵称</div>
            <div class="info-value text-[0.9rem] font-medium">{{ displayName || "—" }}</div>
          </div>
          <div class="info-item">
            <div class="info-label text-xs font-semibold text-base-content/65">注册邮箱</div>
            <div class="info-value text-[0.9rem] font-medium">{{ user.email }}</div>
          </div>
        </div>

        <div class="edit-fields">
          <AppField label="昵称" html-for="nickname">
            <input
              id="nickname"
              v-model="profileForm.nickname"
              type="text"
              class="input w-full"
              placeholder="设置一个展示昵称"
            />
          </AppField>

          <AppField
            label="QQ 号"
            html-for="qq"
            hint="未填写时将显示默认灰白头像。"
          >
            <div class="qq-input-row">
              <input
                id="qq"
                v-model="profileForm.qq"
                type="text"
                class="input qq-input"
                placeholder="选填；用于自动展示 QQ 头像"
                autocomplete="off"
                inputmode="numeric"
              />
              <UserAvatar :qq="previewQq" :size="40" />
            </div>
          </AppField>
        </div>

        <div class="panel-actions">
          <button
            type="button"
            class="btn btn-primary"
            :disabled="saving"
            @click="updateProfile"
          >
            <span v-if="saving" class="loading loading-spinner loading-xs" />
            保存资料
          </button>
        </div>
      </section>

      <section v-else class="card bg-base-100 p-6 flex flex-col gap-5">
        <p class="m-0 text-sm leading-relaxed text-base-content/65">
          定期更换密码有助于保护账户安全。修改成功后请妥善保管新密码。
        </p>

        <div class="edit-fields">
          <AppField label="当前密码" html-for="currentPwd">
            <PasswordInput
              id="currentPwd"
              v-model="passwordForm.currentPassword"
              placeholder="请输入当前密码"
              autocomplete="current-password"
            />
          </AppField>

          <div class="pwd-new-row">
            <AppField label="新密码" html-for="newPwd">
              <PasswordInput
                id="newPwd"
                v-model="passwordForm.newPassword"
                placeholder="至少 6 位"
                autocomplete="new-password"
              />
            </AppField>

            <AppField label="确认新密码" html-for="confirmPwd">
              <PasswordInput
                id="confirmPwd"
                v-model="passwordForm.confirmPassword"
                placeholder="再次输入新密码"
                autocomplete="new-password"
              />
            </AppField>
          </div>
        </div>

        <div class="panel-actions">
          <button
            type="button"
            class="btn btn-primary"
            :disabled="changingPassword"
            @click="changePassword"
          >
            <span v-if="changingPassword" class="loading loading-spinner loading-xs" />
            <Lock :size="14" aria-hidden="true" />
            修改密码
          </button>
        </div>
      </section>
    </template>
  </AppPage>
</template>

<style scoped>
.profile-page {
  max-width: 720px;
}

.info-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.edit-fields {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.qq-input-row {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.qq-input {
  flex: 1;
}

.pwd-new-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.panel-actions {
  display: flex;
  justify-content: flex-end;
}

.empty-icon {
  font-size: 2rem;
  line-height: 1;
}

@media (max-width: 560px) {
  .info-row,
  .pwd-new-row {
    grid-template-columns: 1fr;
  }
}
</style>
