<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { EditorView, basicSetup } from "codemirror";
import { keymap } from "@codemirror/view";
import { indentWithTab } from "@codemirror/commands";
import { StreamLanguage } from "@codemirror/language";
import { oneDark } from "@codemirror/theme-one-dark";
import { fost } from "../fostLanguage";

const emit = defineEmits<{ change: [] }>();
const host = ref<HTMLElement>();
let view: EditorView;

onMounted(() => {
  view = new EditorView({
    parent: host.value!,
    extensions: [
      basicSetup,
      keymap.of([indentWithTab]),
      StreamLanguage.define(fost),
      EditorView.updateListener.of((u) => u.docChanged && emit("change")),
      EditorView.theme({ "&": { height: "100%" }, "&.cm-focused": { outline: "none" } }),
      EditorView.contentAttributes.of({ "aria-label": "Editor de programa" }),
      // shortcut: theme picked once at start-up, restart to follow a system theme change
      matchMedia("(prefers-color-scheme: dark)").matches ? oneDark : [],
    ],
  });
});
onBeforeUnmount(() => view.destroy());

defineExpose({
  getText: () => view.state.doc.toString(),
  setText(text: string) {
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
  },
  gotoLine(n: number) {
    const line = view.state.doc.line(Math.min(Math.max(n, 1), view.state.doc.lines));
    view.dispatch({ selection: { anchor: line.from, head: line.to }, scrollIntoView: true });
    view.focus();
  },
});
</script>

<template>
  <div ref="host" class="h-full overflow-hidden text-sm" />
</template>
