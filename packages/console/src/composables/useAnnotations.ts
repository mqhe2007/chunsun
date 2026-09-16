import { computed, ref } from "vue";
import { api } from "@/utils/api";

/** 与后端 routes/project_knowledge_annotation.rs 的 annotation_dto 对齐。 */
export type KnowledgeAnnotation = {
  id: string;
  docRef: string;
  docKind: "constitution" | "memory" | "document";
  documentId: string | null;
  anchorText: string | null;
  anchorPrefix: string | null;
  anchorSuffix: string | null;
  body: string;
  status: "open" | "resolved" | "stale";
  outcome: "addressed" | "dismissed" | null;
  resolvedNote: string | null;
  resolvedBy: string | null;
  resolvedAt: string | null;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
};

export type CreateAnnotationPayload = {
  body: string;
  anchorText?: string;
  anchorPrefix?: string;
  anchorSuffix?: string;
};

/** 批注数据面：拉取 / 增删改 + 按 status 分组（阅读页两段式列表用）。 */
export function useAnnotations(projectId: () => string, docRef: () => string) {
  const annotations = ref<KnowledgeAnnotation[]>([]);
  const loading = ref(false);

  const openList = computed(() =>
    annotations.value
      .filter(a => a.status === "open" || a.status === "stale")
      // stale 排到未处理段末尾但带标记；同级保持创建顺序
      .sort((a, b) => (a.status === "stale" ? 1 : 0) - (b.status === "stale" ? 1 : 0)),
  );
  const resolvedList = computed(() => annotations.value.filter(a => a.status === "resolved"));
  const openCount = computed(() => openList.value.length);

  function base() {
    return `/projects/${projectId()}/knowledge/documents/${docRef()}/annotations`;
  }

  async function load() {
    loading.value = true;
    try {
      const { data } = await api.get<{ success: boolean; data: { annotations: KnowledgeAnnotation[] } }>(
        base(),
      );
      if (!data.success) throw new Error("load failed");
      annotations.value = data.data.annotations ?? [];
    } finally {
      loading.value = false;
    }
  }

  async function create(payload: CreateAnnotationPayload): Promise<KnowledgeAnnotation | null> {
    const { data } = await api.post<{ success: boolean; data: KnowledgeAnnotation }>(base(), payload);
    if (!data.success) return null;
    await load();
    return data.data;
  }

  async function updateBody(id: string, body: string): Promise<boolean> {
    const { data } = await api.patch<{ success: boolean }>(
      `/projects/${projectId()}/knowledge/annotations/${id}`,
      { body },
    );
    if (!data.success) return false;
    await load();
    return true;
  }

  /** 结案 / 重新打开。结案必须带 outcome（后端缺省 addressed）与依据 note。 */
  async function setStatus(
    id: string,
    status: "open" | "resolved",
    opts?: { outcome?: "addressed" | "dismissed"; resolvedNote?: string },
  ): Promise<boolean> {
    const { data } = await api.patch<{ success: boolean }>(
      `/projects/${projectId()}/knowledge/annotations/${id}`,
      { status, ...opts },
    );
    if (!data.success) return false;
    await load();
    return true;
  }

  async function remove(id: string): Promise<boolean> {
    const { data } = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/knowledge/annotations/${id}`,
    );
    if (!data.success) return false;
    await load();
    return true;
  }

  return { annotations, loading, openList, resolvedList, openCount, load, create, updateBody, setStatus, remove };
}
