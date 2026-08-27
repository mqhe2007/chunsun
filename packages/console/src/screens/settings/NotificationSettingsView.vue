<script setup lang="ts">
import { Bell, RefreshCw, TriangleAlert } from "@lucide/vue";
import { onMounted, ref } from "vue";
import { AppPage, useToast } from "@/ui";
import { api } from "@/utils/api";
import SettingsMegaNav from "@/components/settings/SettingsMegaNav.vue";

type ChannelState = {
  enabled: boolean;
  locked: boolean;
};

type CategoryPref = {
  key: string;
  label: string;
  description: string;
  inApp: ChannelState;
  email: ChannelState;
};

type PrefsResponse = {
  categories: CategoryPref[];
  emailDeliveryAvailable: boolean;
};

const toast = useToast();
const loading = ref(false);
const loadError = ref(false);
const saving = ref(false);
const resetting = ref(false);
const prefs = ref<PrefsResponse | null>(null);

async function fetchPrefs() {
  loading.value = true;
  loadError.value = false;
  try {
    const { data } = await api.get<{ success: boolean; data: PrefsResponse }>(
      "/users/me/notification-preferences",
    );
    if (data.success) {
      prefs.value = data.data;
    } else {
      loadError.value = true;
    }
  } catch {
    loadError.value = true;
    toast.error("加载失败", "无法加载通知偏好");
  } finally {
    loading.value = false;
  }
}

async function patchCategory(
  key: string,
  channel: "inApp" | "email",
  enabled: boolean,
) {
  if (!prefs.value) return;
  const cat = prefs.value.categories.find(c => c.key === key);
  if (!cat) return;
  const state = channel === "inApp" ? cat.inApp : cat.email;
  if (state.locked || state.enabled === enabled) return;

  saving.value = true;
  try {
    const { data } = await api.patch<{ success: boolean; data: PrefsResponse }>(
      "/users/me/notification-preferences",
      {
        categories: {
          [key]: { [channel]: enabled },
        },
      },
    );
    if (data.success) {
      prefs.value = data.data;
    }
  } catch {
    toast.error("保存失败", "通知偏好未能更新");
    await fetchPrefs();
  } finally {
    saving.value = false;
  }
}

async function resetPrefs() {
  resetting.value = true;
  try {
    const { data } = await api.post<{ success: boolean; data: PrefsResponse }>(
      "/users/me/notification-preferences/reset",
    );
    if (data.success) {
      prefs.value = data.data;
      toast.success("已恢复", "通知偏好已恢复为默认");
    }
  } catch {
    toast.error("操作失败", "无法恢复默认偏好");
  } finally {
    resetting.value = false;
  }
}

onMounted(fetchPrefs);
</script>

<template>
  <AppPage title="账户设置" class="settings-page">
    <SettingsMegaNav />

    <template v-if="loading">
      <div class="skeleton h-56 w-full rounded-box" />
    </template>

    <template v-else-if="loadError">
      <div class="console-empty-state">
        <TriangleAlert class="empty-icon text-warning" :size="40" aria-hidden="true" />
        <p class="text-base-content/60">加载通知偏好失败</p>
        <button type="button" class="btn btn-ghost" @click="fetchPrefs">
          <RefreshCw :size="14" aria-hidden="true" />
          重试
        </button>
      </div>
    </template>

    <template v-else-if="prefs">
      <section class="card bg-base-100 p-6 flex flex-col gap-5">
        <div class="flex items-start justify-between gap-4 flex-wrap">
          <div>
            <h2 class="text-base font-semibold m-0 flex items-center gap-2">
              <Bell :size="16" aria-hidden="true" />
              消息通知
            </h2>
            <p class="m-0 mt-1 text-sm text-base-content/65">
              按分类控制站内信与邮件。安全类站内信不可关闭。
            </p>
          </div>
          <button
            type="button"
            class="btn btn-ghost btn-sm"
            :disabled="resetting || saving"
            @click="resetPrefs"
          >
            <span v-if="resetting" class="loading loading-spinner loading-xs" />
            恢复默认
          </button>
        </div>

        <div
          v-if="!prefs.emailDeliveryAvailable"
          class="alert alert-warning text-sm"
          role="status"
        >
          管理员尚未配置邮件服务，邮件渠道暂时不可用。
        </div>

        <div class="overflow-x-auto">
          <table class="table">
            <thead>
              <tr>
                <th>通知类型</th>
                <th class="w-28 text-center">站内信</th>
                <th class="w-28 text-center">邮件</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="cat in prefs.categories" :key="cat.key">
                <td>
                  <div class="font-medium">{{ cat.label }}</div>
                  <div class="text-xs text-base-content/55 mt-0.5">{{ cat.description }}</div>
                  <div
                    v-if="cat.inApp.locked"
                    class="text-xs text-base-content/45 mt-1"
                  >
                    安全通知不可关闭
                  </div>
                </td>
                <td class="text-center align-middle">
                  <input
                    type="checkbox"
                    class="toggle toggle-sm toggle-primary"
                    :checked="cat.inApp.enabled"
                    :disabled="cat.inApp.locked || saving"
                    :aria-label="`${cat.label} 站内信`"
                    @change="
                      patchCategory(
                        cat.key,
                        'inApp',
                        ($event.target as HTMLInputElement).checked,
                      )
                    "
                  />
                </td>
                <td class="text-center align-middle">
                  <input
                    type="checkbox"
                    class="toggle toggle-sm toggle-primary"
                    :checked="cat.email.enabled"
                    :disabled="cat.email.locked || saving"
                    :aria-label="`${cat.label} 邮件`"
                    @change="
                      patchCategory(
                        cat.key,
                        'email',
                        ($event.target as HTMLInputElement).checked,
                      )
                    "
                  />
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>
  </AppPage>
</template>

<style scoped>
.settings-page {
  max-width: 720px;
}

.empty-icon {
  font-size: 2rem;
  line-height: 1;
}
</style>
