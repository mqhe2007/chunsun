<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup } from "codemirror";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { markdown } from "@codemirror/lang-markdown";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";

const model = defineModel<string>({ default: "" });

const emit = defineEmits<{
  save: [];
}>();

const host = ref<HTMLElement | null>(null);
let view: EditorView | null = null;
let applyingExternal = false;

function createState(doc: string) {
  return EditorState.create({
    doc,
    extensions: [
      basicSetup,
      markdown(),
      history(),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        indentWithTab,
        {
          key: "Mod-s",
          run: () => {
            emit("save");
            return true;
          },
        },
      ]),
      EditorView.lineWrapping,
      EditorView.updateListener.of(update => {
        if (!update.docChanged || applyingExternal) return;
        model.value = update.state.doc.toString();
      }),
      EditorView.theme({
        "&": { height: "100%", fontSize: "0.85rem" },
        ".cm-scroller": { overflow: "auto", fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace" },
        ".cm-content": { minHeight: "100%" },
      }),
    ],
  });
}

onMounted(() => {
  if (!host.value) return;
  view = new EditorView({
    state: createState(model.value ?? ""),
    parent: host.value,
  });
});

onBeforeUnmount(() => {
  view?.destroy();
  view = null;
});

watch(model, value => {
  if (!view) return;
  const current = view.state.doc.toString();
  if (value === current) return;
  applyingExternal = true;
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: value ?? "" },
  });
  applyingExternal = false;
});
</script>

<template>
  <div ref="host" class="cm-host h-full min-h-72 w-full overflow-hidden rounded-box border border-base-300 bg-base-100" />
</template>
