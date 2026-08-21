<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { AppField, useToast } from "@/ui";
import { api } from "../utils/api";
import type { Project } from "../types/project";
import {
  buildCreateProjectPayload,
  createProjectFormState,
} from "./projectFormState";

const props = defineProps<{
  mode: "create" | "edit";
  project?: Project | null;
}>();

const emit = defineEmits<{
  success: [project: Project];
  cancel: [];
}>();

const toast = useToast();
const loading = ref(false);
const formData = ref(createProjectFormState());
const errors = ref<Record<string, string>>({});

const dialogDescription = computed(() =>
  props.mode === "create" ? "填写项目基础信息" : "更新项目设置",
);
const submitLabel = computed(() => {
  if (loading.value) {
    return props.mode === "create" ? "创建中..." : "保存中...";
  }
  return props.mode === "create" ? "创建项目" : "保存修改";
});

watch(
  () => props.project,
  () => {
    errors.value = {};
    formData.value = createProjectFormState(
      props.mode === "edit" ? props.project : null,
    );
  },
  { immediate: true },
);

function validate(): boolean {
  errors.value = {};
  if (!formData.value.name.trim()) {
    errors.value.name = "请填写项目名称";
  }
  return Object.keys(errors.value).length === 0;
}

async function onSubmit() {
  if (!validate()) return;

  loading.value = true;
  try {
    if (props.mode === "create") {
      const { data } = await api.post<{ success: boolean; data: Project }>(
        "/projects",
        buildCreateProjectPayload(formData.value),
      );

      if (data.success) {
        toast.success("创建成功", "项目创建成功");
        emit("success", data.data);
      } else {
        toast.error("创建失败", "项目创建失败");
      }
    } else {
      if (!props.project) return;
      const { data } = await api.patch<{ success: boolean; data: Project }>(
        `/projects/${props.project.id}`,
        {
          name: formData.value.name,
          description: formData.value.description || undefined,
        },
      );

      if (data.success) {
        toast.success("保存成功", "项目信息已更新");
        emit("success", data.data);
      } else {
        toast.error("保存失败", "项目信息更新失败");
      }
    }
  } catch {
    toast.error(
      props.mode === "create" ? "创建失败" : "保存失败",
      "请稍后重试",
    );
  } finally {
    loading.value = false;
  }
}

function closeDialog() {
  emit("cancel");
}
</script>

<template>
  <div class="project-form">
    <p class="form-desc">{{ dialogDescription }}</p>
    <form class="form-grid" @submit.prevent="onSubmit">
      <AppField label="项目名称 *" html-for="project-name" :error="errors.name">
        <input
          id="project-name"
          v-model="formData.name"
          type="text"
          class="input w-full"
          :class="{ 'input-error': errors.name }"
          placeholder="输入项目名称"
        />
      </AppField>
      <AppField label="项目描述" html-for="project-desc">
        <textarea
          id="project-desc"
          v-model="formData.description"
          rows="3"
          class="textarea w-full"
          placeholder="可选"
        />
      </AppField>
      <div class="form-actions">
        <button type="button" class="btn btn-ghost" @click="closeDialog">取消</button>
        <button type="submit" class="btn btn-primary" :disabled="loading">
          <span v-if="loading" class="loading loading-spinner loading-sm" />
          {{ submitLabel }}
        </button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.project-form {
  padding: 0.25rem 0;
}

.form-desc {
  margin: 0 0 1rem;
  color: var(--color-base-content);
  opacity: 0.65;
  font-size: 0.9rem;
}

.form-grid {
  display: grid;
  gap: 1rem;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 0.5rem;
}
</style>
