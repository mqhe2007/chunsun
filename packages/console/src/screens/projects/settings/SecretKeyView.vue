<script setup lang="ts">
import { Copy, Eye, EyeOff } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { AppPage, confirm, useToast } from "@/ui";
import { api } from "@/utils/api";
import { useProjectPermissions } from "@/utils/permissions";
import type { ProjectMember } from "@/types/project";

const route = useRoute();
const toast = useToast();
const projectId = () => (route.params as Record<string, string>).id;

const secretKey = ref<string | null>(null);
const keyVisible = ref(false);
const skLoading = ref(false);
const skGenerating = ref(false);
const skRevoking = ref(false);
const ownerId = ref<string | null>(null);
const members = ref<ProjectMember[]>([]);

const { can } = useProjectPermissions(ownerId, members);
const canManageSecretKey = computed(() => can("secretKey.write"));

const maskedSecretKey = computed(() => {
  if (!secretKey.value) return "";
  if (keyVisible.value) return secretKey.value;
  return `${secretKey.value.slice(0, 7)}${"•".repeat(30)}${secretKey.value.slice(-4)}`;
});

async function fetchProjectOwner() {
  try {
    const [projectRes, membersRes] = await Promise.all([
      api.get<{ success: boolean; data: { userId: string } }>(
        `/projects/${projectId()}`,
      ),
      api.get<{ success: boolean; data: ProjectMember[] }>(
        `/projects/${projectId()}/members`,
      ),
    ]);
    if (projectRes.data.success) ownerId.value = projectRes.data.data.userId;
    if (membersRes.data.success) members.value = membersRes.data.data;
  } catch {
    // ignore
  }
}

async function fetchSecretKey() {
  skLoading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: { secretKey: string | null; hasSecretKey: boolean };
    }>(`/projects/${projectId()}/secret-key`);
    if (data.success) secretKey.value = data.data.secretKey;
  } catch {
    toast.error("获取失败", "无法加载项目密钥");
  } finally {
    skLoading.value = false;
  }
}

async function doGenerateSecretKey() {
  skGenerating.value = true;
  try {
    const { data } = await api.post<{ success: boolean; data: { secretKey: string } }>(
      `/projects/${projectId()}/secret-key/generate`,
    );
    if (data.success) {
      secretKey.value = data.data.secretKey;
      keyVisible.value = true;
      toast.success("生成成功", "项目密钥已生成，请妥善保管");
      window.dispatchEvent(new CustomEvent("chunsun:secret-key-changed"));
    }
  } catch {
    toast.error("生成失败", "请稍后重试");
  } finally {
    skGenerating.value = false;
  }
}

async function handleGenerateSecretKey() {
  if (secretKey.value) {
    const ok = await confirm({
      title: "重新生成密钥",
      message:
        "重新生成会使旧密钥立即失效，所有已配置的 CLI 和 Skill 工作流需同步更新，确认继续？",
      confirmLabel: "重新生成",
    });
    if (!ok) return;
    await doGenerateSecretKey();
    return;
  }
  await doGenerateSecretKey();
}

async function handleRevokeSecretKey() {
  const ok = await confirm({
    title: "撤销密钥",
    message: "撤销后 CLI / Skill 将无法认证，确认继续？",
    confirmLabel: "撤销",
    danger: true,
  });
  if (!ok) return;
  skRevoking.value = true;
  try {
    const { data } = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/secret-key`,
    );
    if (data.success) {
      secretKey.value = null;
      keyVisible.value = false;
      toast.success("已撤销", "项目密钥已撤销");
      window.dispatchEvent(new CustomEvent("chunsun:secret-key-changed"));
    }
  } catch {
    toast.error("撤销失败", "请稍后重试");
  } finally {
    skRevoking.value = false;
  }
}

async function copySecretKey() {
  if (!secretKey.value) return;
  try {
    await navigator.clipboard.writeText(secretKey.value);
    toast.success("已复制");
  } catch {
    toast.error("复制失败");
  }
}

onMounted(async () => {
  await fetchProjectOwner();
  await fetchSecretKey();
});
</script>

<template>
  <AppPage title="项目密钥">
    <template v-if="canManageSecretKey" #actions>
      <button
        v-if="!secretKey"
        type="button"
        class="btn btn-primary"
        :disabled="skGenerating"
        @click="handleGenerateSecretKey"
      >
        <span v-if="skGenerating" class="loading loading-spinner loading-xs" />
        生成密钥
      </button>
      <template v-else>
        <button
          type="button"
          class="btn btn-ghost btn-warning"
          :disabled="skGenerating"
          @click="handleGenerateSecretKey"
        >
          <span v-if="skGenerating" class="loading loading-spinner loading-xs" />
          重新生成
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-error"
          :disabled="skRevoking"
          @click="handleRevokeSecretKey"
        >
          <span v-if="skRevoking" class="loading loading-spinner loading-xs" />
          撤销
        </button>
      </template>
    </template>

    <div v-if="skLoading" class="empty-state">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>

    <div v-else class="sk-panel">
      <div v-if="secretKey" class="sk-key-row">
        <code class="sk-key-value">{{ maskedSecretKey }}</code>
        <div class="sk-key-btns">
          <button
            type="button"
            class="btn btn-ghost btn-sm btn-square"
            :title="keyVisible ? '隐藏' : '显示'"
            @click="keyVisible = !keyVisible"
          >
            <EyeOff v-if="keyVisible" :size="14" />
            <Eye v-else :size="14" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-sm btn-square"
            title="复制"
            @click="copySecretKey"
          >
            <Copy :size="14" />
          </button>
        </div>
      </div>
      <div v-else class="sk-empty text-base-content/60">
        暂未生成密钥
        <span v-if="!canManageSecretKey">，请联系项目管理员生成</span>
      </div>

      <div class="sk-hint text-base-content/60">
        <span>
          Secret Key 等同凭证，请勿泄露。写入本地仓库 <code>.env</code> 的
          <code>CHUNSUN_SECRET_KEY</code>，再执行 <code>chunsun init</code>。
        </span>
      </div>
    </div>
  </AppPage>
</template>

<style scoped>

.empty-state {
  display: grid;
  place-items: center;
  min-height: 10rem;
}

.sk-panel {
  display: grid;
  gap: 1rem;
  max-width: 40rem;
}

.sk-key-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
  background: color-mix(in oklab, var(--color-base-200) 50%, transparent);
  border-radius: 8px;
  padding: 0.75rem 0.85rem;
}

.sk-key-value {
  flex: 1;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.88rem;
  word-break: break-all;
}

.sk-key-btns {
  display: flex;
  gap: 0.25rem;
  flex-shrink: 0;
}

.sk-empty {
  font-size: 0.9rem;
}

.sk-hint {
  font-size: 0.85rem;
  line-height: 1.55;
}

.sk-hint code {
  font-size: 0.8rem;
  background: var(--color-base-200);
  padding: 0.05rem 0.35rem;
  border-radius: 4px;
}
</style>
