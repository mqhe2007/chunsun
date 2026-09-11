<script setup lang="ts">
import { Folder } from "@lucide/vue";
import { computed, ref, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { AppModal, AppPage, useToast } from "@/ui";
import { api } from "@/utils/api";
import { useAuthStore } from "@/stores/auth";
import ProjectFormModal from "@/components/ProjectFormModal.vue";
import type { Project } from "@/types/project";
import { partitionProjectsByOwnership } from "./projectListPartition";

type ProjectTab = "mine" | "participating";

const router = useRouter();
const toast = useToast();
const authStore = useAuthStore();
const currentUserId = authStore.userId;

const loading = ref(false);
const projects = ref<Project[]>([]);
const activeTab = ref<ProjectTab>("mine");
const page = ref(1);
const pageSize = ref(20);

const modalOpen = ref(false);

const partitioned = computed(() =>
  partitionProjectsByOwnership(projects.value, currentUserId),
);
const mineProjects = computed(() => partitioned.value.mine);
const participatingProjects = computed(() => partitioned.value.participating);
const tabProjects = computed(() =>
  activeTab.value === "mine" ? mineProjects.value : participatingProjects.value,
);
const total = computed(() => tabProjects.value.length);
const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)));
const showPaginator = computed(() => total.value > pageSize.value);
const pagedProjects = computed(() => {
  const start = (page.value - 1) * pageSize.value;
  return tabProjects.value.slice(start, start + pageSize.value);
});
const emptyCopy = computed(() =>
  activeTab.value === "mine"
    ? { title: "暂无项目", action: true as const }
    : { title: "暂无参与的项目", action: false as const },
);

function formatDate(value: string) {
  return new Date(value).toLocaleDateString("zh-CN");
}

async function fetchProjects() {
  loading.value = true;
  try {
    const collected: Project[] = [];
    let currentPage = 1;
    let totalPagesFromApi = 1;
    const fetchPageSize = 100;

    do {
      const { data } = await api.get<{
        success: boolean;
        data: Project[];
        meta: { total: number; page: number; pageSize: number; totalPages: number };
      }>("/projects", {
        params: {
          page: currentPage,
          pageSize: fetchPageSize,
        },
      });
      if (!data.success) {
        toast.error("获取失败", "获取项目列表失败");
        return;
      }
      collected.push(...data.data);
      totalPagesFromApi = Math.max(1, data.meta.totalPages);
      currentPage += 1;
    } while (currentPage <= totalPagesFromApi);

    projects.value = collected;
  } catch {
    toast.error("获取失败", "获取项目列表失败");
  } finally {
    loading.value = false;
  }
}

function openCreateModal() {
  modalOpen.value = true;
}

function handleModalSuccess(project: Project) {
  projects.value.unshift(project);
  activeTab.value = "mine";
  page.value = 1;
}

function enterProject(project: Project) {
  router.push(`/projects/${project.id}`);
}

function onCardKeydown(event: KeyboardEvent, project: Project) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    enterProject(project);
  }
}

function goPrev() {
  page.value = Math.max(1, page.value - 1);
}

function goNext() {
  page.value = Math.min(totalPages.value, page.value + 1);
}

function setTab(tab: ProjectTab) {
  if (activeTab.value === tab) return;
  activeTab.value = tab;
  page.value = 1;
}

watch(pageSize, () => {
  page.value = 1;
});

watch(totalPages, (pages) => {
  if (page.value > pages) page.value = pages;
});

onMounted(async () => {
  await fetchProjects();
});
</script>

<template>
  <AppPage title="项目">
    <template #actions>
      <button type="button" class="btn btn-primary" @click="openCreateModal">新建项目</button>
    </template>

    <div role="tablist" class="tabs tabs-border project-tabs">
      <button
        type="button"
        role="tab"
        class="tab"
        :class="{ 'tab-active': activeTab === 'mine' }"
        :aria-selected="activeTab === 'mine'"
        @click="setTab('mine')"
      >
        我的项目
        <span class="badge badge-ghost badge-sm">{{ mineProjects.length }}</span>
      </button>
      <button
        type="button"
        role="tab"
        class="tab"
        :class="{ 'tab-active': activeTab === 'participating' }"
        :aria-selected="activeTab === 'participating'"
        @click="setTab('participating')"
      >
        参与的项目
        <span class="badge badge-ghost badge-sm">{{ participatingProjects.length }}</span>
      </button>
    </div>

    <div v-if="loading" class="empty-state">
      <span class="loading loading-spinner loading-lg text-primary" />
      <p class="text-base-content/60">加载中...</p>
    </div>

    <div v-else-if="pagedProjects.length === 0" class="empty-state">
      <Folder class="empty-icon text-primary" :size="40" aria-hidden="true" />
      <p class="text-base-content/60">{{ emptyCopy.title }}</p>
      <button
        v-if="emptyCopy.action"
        type="button"
        class="btn btn-primary"
        @click="openCreateModal"
      >
        创建第一个项目
      </button>
    </div>

    <div v-else class="card-grid">
      <article
        v-for="project in pagedProjects"
        :key="project.id"
        class="card bg-base-100 project-card"
        role="button"
        tabindex="0"
        :aria-label="`进入项目 ${project.name}`"
        @click="enterProject(project)"
        @keydown="onCardKeydown($event, project)"
      >
        <div class="card-body gap-3">
          <div class="project-header">
            <div class="project-icon" aria-hidden="true">
              <Folder :size="20" />
            </div>
            <div class="project-title-area">
              <div class="project-title-row">
                <span class="project-name">{{ project.name }}</span>
              </div>
            </div>
          </div>

          <p class="project-desc" :class="{ 'is-empty': !project.description }">
            {{ project.description || "暂无描述" }}
          </p>

          <div class="project-footer">
            <span>更新于 {{ formatDate(project.updatedAt) }}</span>
          </div>
        </div>
      </article>
    </div>

    <div v-if="showPaginator" class="pagination">
      <label class="page-size">
        每页
        <select v-model.number="pageSize" class="select">
          <option :value="10">10</option>
          <option :value="20">20</option>
          <option :value="50">50</option>
        </select>
        条
      </label>
      <div class="join">
        <button type="button" class="join-item btn" :disabled="page <= 1" @click="goPrev">
          上一页
        </button>
        <button type="button" class="join-item btn pointer-events-none">
          {{ page }} / {{ totalPages }}
        </button>
        <button
          type="button"
          class="join-item btn"
          :disabled="page >= totalPages"
          @click="goNext"
        >
          下一页
        </button>
      </div>
    </div>

    <AppModal v-model="modalOpen" title="新建项目">
      <ProjectFormModal
        v-if="modalOpen"
        mode="create"
        @success="(project: Project) => { handleModalSuccess(project); modalOpen = false; }"
        @cancel="modalOpen = false"
      />
    </AppModal>
  </AppPage>
</template>

<style scoped>
.project-tabs {
  width: fit-content;
  max-width: 100%;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 3rem 1rem;
}

.empty-icon {
  font-size: 3rem;
  opacity: 0.5;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 1rem;
}

.project-card {
  cursor: pointer;
  transition: background-color 0.15s, box-shadow 0.15s;
}

.project-card:hover,
.project-card:focus-visible {
  outline: none;
  box-shadow: 0 8px 24px -12px color-mix(in oklch, var(--color-primary) 35%, transparent);
}

.project-header {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 0;
}

.project-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 42px;
  height: 42px;
  border-radius: 10px;
  flex-shrink: 0;
  background: color-mix(in oklch, var(--color-primary) 15%, transparent);
  color: var(--color-primary);
}

.project-title-area {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.project-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  min-width: 0;
}

.project-name {
  min-width: 0;
  flex: 1;
  font-weight: 600;
  font-size: 1.05rem;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-desc {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.45;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 2.5em;
}

.project-desc.is-empty {
  opacity: 0.7;
}

.project-footer {
  margin-top: auto;
  padding-top: 0.75rem;
  display: flex;
  align-items: center;
  gap: 1rem;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.pagination {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-top: 1.25rem;
}

.page-size {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

@media (max-width: 640px) {
  .project-icon {
    width: 38px;
    height: 38px;
    font-size: 1rem;
  }

  .project-name {
    font-size: 1rem;
  }
}
</style>
