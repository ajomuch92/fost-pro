<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { EditorView, basicSetup } from "codemirror";
import { keymap } from "@codemirror/view";
import { indentWithTab, redo, selectAll, undo } from "@codemirror/commands";
import { StreamLanguage } from "@codemirror/language";
import { Compartment } from "@codemirror/state";
import type { CompletionContext } from "@codemirror/autocomplete";
import { openSearchPanel } from "@codemirror/search";
import { oneDark } from "@codemirror/theme-one-dark";
import { dark, type EditAction, reference } from "../engine";
import { t } from "../i18n";
import { fost, keywords } from "../fostLanguage";

/** `members` gives the names to offer after a dot, such as the controls of the form being designed. */
const props = defineProps<{ members?: () => string[] }>();
const emit = defineEmits<{ change: [] }>();
const host = ref<HTMLElement>();
let view: EditorView;

const NAME = /[\p{L}_][\p{L}\d_]*/u;
const REST = /^[\p{L}\d_]*$/u;

/** What to offer while typing: after a dot, members; otherwise keywords, functions and the program's own names. */
function complete(context: CompletionContext) {
  const member = context.matchBefore(/\.[\p{L}\d_]*/u);
  if (member) {
    const names = props.members?.() ?? [];
    if (!names.length) return null;
    return { from: member.from + 1, options: names.map((label) => ({ label, type: "property" })), validFor: REST };
  }
  const word = context.matchBefore(NAME);
  if (!word && !context.explicit) return null;
  const functions = reference.value.functions.map((f) => f.syntax.split("(")[0]);
  const known = new Set([...keywords, ...functions]);
  // its variables, procedures and fields: every name of three letters or more already in the program
  const own = new Set(context.state.doc.toString().match(/[\p{L}_][\p{L}\d_]{2,}/gu));
  if (word) own.delete(word.text);
  return {
    from: word?.from ?? context.pos,
    options: [
      ...keywords.map((label) => ({ label, type: "keyword" })),
      ...functions.map((label) => ({ label, type: "function", apply: `${label}(` })),
      ...[...own].filter((w) => !known.has(w.toUpperCase())).map((label) => ({ label, type: "variable" })),
    ],
    validFor: REST,
  };
}

const theme = new Compartment();
watch(dark, (on) => view.dispatch({ effects: theme.reconfigure(on ? oneDark : []) }));

onMounted(() => {
  const language = StreamLanguage.define(fost);
  view = new EditorView({
    parent: host.value!,
    extensions: [
      basicSetup,
      keymap.of([indentWithTab]),
      language,
      language.data.of({ autocomplete: complete }),
      EditorView.updateListener.of((u) => u.docChanged && emit("change")),
      EditorView.theme({ "&": { height: "100%" }, "&.cm-focused": { outline: "none" } }),
      EditorView.contentAttributes.of({ "aria-label": t("Editor de programa") }),
      theme.of(dark.value ? oneDark : []),
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
  /** Puts `text` where the cursor is, replacing what is selected. */
  insert(text: string) {
    view.dispatch(view.state.replaceSelection(text));
    view.focus();
  },
  /** An action of the Edit menu. The clipboard ones can fail where the system refuses the access. */
  async edit(action: EditAction) {
    view.focus();
    const { from, to } = view.state.selection.main;
    if (action === "undo") undo(view);
    else if (action === "redo") redo(view);
    else if (action === "selectAll") selectAll(view);
    else if (action === "find") openSearchPanel(view);
    else if (action === "paste") view.dispatch(view.state.replaceSelection(await navigator.clipboard.readText()));
    else {
      await navigator.clipboard.writeText(view.state.sliceDoc(from, to));
      if (action === "cut") view.dispatch(view.state.replaceSelection(""));
    }
  },
});
</script>

<template>
  <div ref="host" class="h-full overflow-hidden text-sm" />
</template>
