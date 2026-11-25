<script setup lang="ts">
import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

type Entry = { kind: "cmd" | "out" | "err"; text: string };
type RunResult = { output: string; error: string | null; clear: boolean };

const entries = ref<Entry[]>([]);
const input = ref("");
const running = ref(false);
const log = ref<HTMLElement>();
const history: string[] = [];
let cursor = 0;

async function run() {
  const code = input.value;
  if (!code.trim() || running.value) return;
  history.push(code);
  cursor = history.length;
  input.value = "";
  entries.value.push({ kind: "cmd", text: code });
  running.value = true;
  try {
    const r = await invoke<RunResult>("run_code", { code });
    if (r.clear) entries.value = [];
    // `?` prints its newline first, so the output starts with one
    if (r.output) entries.value.push({ kind: "out", text: r.output.replace(/^\n/, "") });
    if (r.error) entries.value.push({ kind: "err", text: r.error });
  } catch (e) {
    entries.value.push({ kind: "err", text: String(e) });
  }
  running.value = false;
  await nextTick();
  log.value?.scrollTo(0, log.value.scrollHeight);
}

function recall(step: number, event: KeyboardEvent) {
  // in a multi-line command the arrows move the caret instead
  if (input.value.includes("\n")) return;
  event.preventDefault();
  cursor = Math.min(Math.max(cursor + step, 0), history.length);
  input.value = history[cursor] ?? "";
}

function escape() {
  if (running.value) invoke("stop_code");
  else input.value = "";
}
</script>

<template>
  <section class="flex h-full flex-col font-mono text-sm" aria-label="Ventana de comandos">
    <div ref="log" class="flex-1 overflow-y-auto p-3" role="log" aria-live="polite">
      <p v-if="!entries.length" class="text-neutral-500">
        Escribe un comando y pulsa Enter. Por ejemplo: ? "Hola", 2 + 2
      </p>
      <pre
        v-for="(e, i) in entries"
        :key="i"
        class="whitespace-pre-wrap break-words"
        :class="{
          'text-neutral-500': e.kind === 'cmd',
          'text-red-600 dark:text-red-400': e.kind === 'err',
        }"
        >{{ e.kind === "cmd" ? "> " + e.text : e.text }}</pre
      >
    </div>
    <div class="flex items-start gap-2 border-t border-neutral-300 p-3 dark:border-neutral-700">
      <span class="text-neutral-500" aria-hidden="true">&gt;</span>
      <textarea
        v-model="input"
        class="flex-1 resize-none bg-transparent outline-none"
        :rows="input.split('\n').length"
        aria-label="Comando"
        placeholder="Enter ejecuta · Shift+Enter nueva línea · Esc detiene"
        spellcheck="false"
        autocapitalize="off"
        autofocus
        @keydown.enter.exact.prevent="run"
        @keydown.up="recall(-1, $event)"
        @keydown.down="recall(1, $event)"
        @keydown.esc="escape"
      />
      <button
        v-if="running"
        type="button"
        class="rounded border border-neutral-400 px-2 hover:bg-neutral-200 dark:hover:bg-neutral-700"
        @click="escape"
      >
        Detener
      </button>
    </div>
  </section>
</template>
