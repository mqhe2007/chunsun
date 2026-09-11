<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { extractToc, renderMarkdown, type TocItem } from "@/utils/markdown";
import "@/assets/markdown.css";

const props = defineProps<{
  content: string;
  showToc?: boolean;
}>();

const rootEl = ref<HTMLElement | null>(null);
const activeId = ref("");

const html = computed(() => renderMarkdown(props.content ?? ""));
const toc = computed<TocItem[]>(() =>
  props.showToc === false ? [] : extractToc(props.content ?? ""),
);

function scrollTo(id: string) {
  const el = rootEl.value?.querySelector(`#${CSS.escape(id)}`);
  el?.scrollIntoView({ behavior: "smooth", block: "start" });
  activeId.value = id;
}

function onScroll() {
  if (!rootEl.value || toc.value.length === 0) return;
  const body = rootEl.value.querySelector(".markdown-body");
  if (!body) return;
  let current = toc.value[0]?.id ?? "";
  for (const item of toc.value) {
    const el = body.querySelector(`#${CSS.escape(item.id)}`) as HTMLElement | null;
    if (!el) continue;
    if (el.getBoundingClientRect().top <= 120) current = item.id;
  }
  activeId.value = current;
}

onMounted(() => {
  void nextTick(onScroll);
  window.addEventListener("scroll", onScroll, { passive: true });
});
onUnmounted(() => window.removeEventListener("scroll", onScroll));
watch(html, () => void nextTick(onScroll));
</script>

<template>
  <div ref="rootEl" class="md-reader flex flex-col gap-4 lg:flex-row lg:items-start">
    <aside v-if="toc.length" class="toc-panel w-full shrink-0 lg:sticky lg:top-4 lg:w-52">
      <details class="collapse collapse-arrow bg-base-200 lg:hidden" open>
        <summary class="collapse-title text-sm font-medium min-h-0 py-2">目录</summary>
        <div class="collapse-content">
          <nav class="toc-nav flex flex-col gap-1">
            <button
              v-for="item in toc"
              :key="item.id"
              type="button"
              class="toc-link btn btn-ghost btn-xs justify-start font-normal h-auto min-h-0 py-1"
              :class="{ 'text-primary': activeId === item.id }"
              :style="{ paddingLeft: `${(item.level - 1) * 0.65 + 0.35}rem` }"
              @click="scrollTo(item.id)"
            >
              {{ item.text }}
            </button>
          </nav>
        </div>
      </details>
      <nav class="toc-nav hidden flex-col gap-1 lg:flex">
        <p class="mb-1 text-xs font-medium text-base-content/50">目录</p>
        <button
          v-for="item in toc"
          :key="`d-${item.id}`"
          type="button"
          class="toc-link btn btn-ghost btn-xs justify-start font-normal h-auto min-h-0 py-1"
          :class="{ 'text-primary': activeId === item.id }"
          :style="{ paddingLeft: `${(item.level - 1) * 0.65 + 0.35}rem` }"
          @click="scrollTo(item.id)"
        >
          {{ item.text }}
        </button>
      </nav>
    </aside>
    <div class="markdown-body min-w-0 flex-1" v-html="html" />
  </div>
</template>
