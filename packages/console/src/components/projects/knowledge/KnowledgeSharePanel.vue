<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { api } from "@/utils/api";
import { useToast } from "@/ui";

const props = defineProps<{
  projectId: string;
  docId: string;
  /** 系统文档不可分享 */
  shareable: boolean;
}>();

const toast = useToast();
const open = defineModel<boolean>("open", { default: false });

const loading = ref(false);
const saving = ref(false);
const hasToken = ref(false);
const enabled = ref(false);
const active = ref(false);
const expiresAt = ref<string | null>(null);
const plaintextUrl = ref("");
const expirePreset = ref<"none" | "7d" | "30d" | "custom">("none");
const customExpires = ref("");

const shareApi = computed(
  () => `/projects/${props.projectId}/knowledge/documents/${props.docId}/share`,
);

function expiresIsoFromPreset(): string | null {
  if (expirePreset.value === "none") return null;
  if (expirePreset.value === "custom") {
    if (!customExpires.value) return null;
    return new Date(customExpires.value).toISOString();
  }
  const days = expirePreset.value === "7d" ? 7 : 30;
  return new Date(Date.now() + days * 24 * 60 * 60 * 1000).toISOString();
}

async function loadStatus() {
  if (!props.shareable) return;
  loading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: {
        hasToken: boolean;
        enabled: boolean;
        active?: boolean;
        expiresAt: string | null;
      };
    }>(shareApi.value);
    if (!data.success) throw new Error("load failed");
    hasToken.value = data.data.hasToken;
    enabled.value = data.data.enabled;
    active.value = Boolean(data.data.active);
    expiresAt.value = data.data.expiresAt;
  } catch {
    toast.error("加载分享状态失败");
  } finally {
    loading.value = false;
  }
}

async function enableOrRotate() {
  saving.value = true;
  try {
    const expires = expiresIsoFromPreset();
    const { data } = await api.post<{
      success: boolean;
      data: { token: string; url: string; enabled: boolean; expiresAt: string | null; active: boolean };
    }>(shareApi.value, { expiresAt: expires });
    if (!data.success) throw new Error("create failed");
    plaintextUrl.value = data.data.url;
    hasToken.value = true;
    enabled.value = data.data.enabled;
    active.value = data.data.active;
    expiresAt.value = data.data.expiresAt;
    toast.success("已生成分享链接");
  } catch {
    toast.error("生成失败");
  } finally {
    saving.value = false;
  }
}

async function setEnabled(next: boolean) {
  saving.value = true;
  try {
    if (!next) {
      const { data } = await api.delete<{ success: boolean }>(shareApi.value);
      if (!data.success) throw new Error("disable failed");
      enabled.value = false;
      active.value = false;
      plaintextUrl.value = "";
      toast.success("已停用分享");
    } else {
      const { data } = await api.patch<{
        success: boolean;
        data: { enabled: boolean; active: boolean; expiresAt: string | null };
      }>(shareApi.value, { enabled: true });
      if (!data.success) throw new Error("enable failed");
      enabled.value = data.data.enabled;
      active.value = data.data.active;
      expiresAt.value = data.data.expiresAt;
      toast.success("已重新启用");
    }
  } catch {
    toast.error("更新失败", "若尚未生成链接请先生成");
  } finally {
    saving.value = false;
  }
}

async function copyUrl() {
  if (!plaintextUrl.value) {
    toast.warn("请先生成或重新生成链接（明文链接仅在生成时展示）");
    return;
  }
  try {
    await navigator.clipboard.writeText(plaintextUrl.value);
    toast.success("已复制");
  } catch {
    toast.error("复制失败");
  }
}

watch(open, v => {
  if (v) {
    plaintextUrl.value = "";
    void loadStatus();
  }
});

onMounted(() => {
  if (open.value) void loadStatus();
});
</script>

<template>
  <dialog class="modal" :class="{ 'modal-open': open }">
    <div class="modal-box max-w-lg">
      <h3 class="text-lg font-semibold">分享文档</h3>
      <p v-if="!shareable" class="mt-2 text-sm text-base-content/60">
        系统固定文档（宪法 / 项目记忆）不支持对外分享。
      </p>
      <template v-else>
        <p class="mt-2 text-sm text-base-content/60">
          生成只读公开链接。链接含随机 token，可随时停用或轮换；停用后旧链接立即失效。
        </p>
        <div v-if="loading" class="mt-4 flex justify-center py-6">
          <span class="loading loading-spinner" />
        </div>
        <div v-else class="mt-4 flex flex-col gap-3">
          <div class="flex items-center justify-between gap-3">
            <span class="text-sm">启用分享</span>
            <input
              type="checkbox"
              class="toggle"
              :checked="enabled"
              :disabled="saving || (!hasToken && !enabled)"
              @change="setEnabled(($event.target as HTMLInputElement).checked)"
            />
          </div>
          <p v-if="hasToken" class="text-xs text-base-content/50">
            状态：{{ active ? "有效" : "已停用或已过期" }}
            <template v-if="expiresAt"> · 过期 {{ new Date(expiresAt).toLocaleString() }}</template>
          </p>
          <label class="form-control w-full">
            <span class="label-text text-sm">过期（生成/轮换时生效）</span>
            <select v-model="expirePreset" class="select select-bordered w-full">
              <option value="none">永不过期</option>
              <option value="7d">7 天</option>
              <option value="30d">30 天</option>
              <option value="custom">自定义</option>
            </select>
          </label>
          <input
            v-if="expirePreset === 'custom'"
            v-model="customExpires"
            type="datetime-local"
            class="input input-bordered w-full"
          />
          <div v-if="plaintextUrl" class="rounded-box bg-base-200 p-3">
            <p class="mb-1 text-xs text-base-content/50">链接（仅此时可见，请立即复制）</p>
            <code class="block break-all text-xs">{{ plaintextUrl }}</code>
          </div>
        </div>
      </template>
      <div class="modal-action flex-wrap">
        <button type="button" class="btn btn-ghost" @click="open = false">关闭</button>
        <template v-if="shareable">
          <button type="button" class="btn btn-ghost" :disabled="!plaintextUrl" @click="copyUrl">
            复制链接
          </button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="saving"
            @click="enableOrRotate"
          >
            <span v-if="saving" class="loading loading-spinner loading-sm" />
            {{ hasToken ? "重新生成链接" : "生成链接" }}
          </button>
        </template>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop" @submit.prevent="open = false">
      <button type="submit">close</button>
    </form>
  </dialog>
</template>
