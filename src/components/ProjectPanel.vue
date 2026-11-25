<script setup lang="ts">
// The open project: the programs, forms, classes and tables of its folder.
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, X } from "@lucide/vue";
import { pending, project, report } from "../engine";

type Field = { name: string; kind: string; width: number; decimals: number };
type Info = {
  programs: string[];
  forms: string[];
  classes: { name: string; parent: string | null; file: string; line: number }[];
  tables: { name: string; fields: Field[] }[];
};

/** A file of the project to open, and the line to go to when it matters. */
const emit = defineEmits<{ open: [file: string, line?: number] }>();
const info = ref<Info | null>(null);

async function refresh() {
  if (!project.value) return (info.value = null);
  try {
    info.value = await invoke<Info>("project_info", { dir: project.value });
  } catch (e) {
    report(String(e));
  }
}
watch(project, refresh, { immediate: true });
// a program that just ran may have created tables
watch(pending, (waiting) => waiting || refresh());
// saving a file may add it, or change its classes
defineExpose({ refresh });

/** A field as CREATE TABLE writes it: C(10), N(8, 2), L. */
const type = (f: Field) => ("CN".includes(f.kind) ? `${f.kind}(${f.width}${f.decimals ? `, ${f.decimals}` : ""})` : f.kind);
const item = "block w-full truncate rounded px-2 py-0.5 text-left hover:bg-neutral-200 dark:hover:bg-neutral-700";
const title = "cursor-pointer px-2 py-1 font-semibold select-none";
</script>

<template>
  <aside
    v-if="project && info"
    class="flex w-56 shrink-0 flex-col border-r border-neutral-300 text-sm dark:border-neutral-700"
    aria-label="Proyecto"
  >
    <header class="flex items-center gap-1 border-b border-neutral-300 px-2 py-1 dark:border-neutral-700">
      <span class="flex-1 truncate font-semibold" :title="project">{{ project.replace(/^.*[\\/]/, "") }}</span>
      <button type="button" class="rounded p-1 hover:bg-neutral-200 dark:hover:bg-neutral-700" aria-label="Actualizar" @click="refresh">
        <RefreshCw :size="14" />
      </button>
      <button type="button" class="rounded p-1 hover:bg-neutral-200 dark:hover:bg-neutral-700" aria-label="Cerrar proyecto" @click="project = null">
        <X :size="14" />
      </button>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto">
      <details open>
        <summary :class="title">Programas ({{ info.programs.length }})</summary>
        <button v-for="f in info.programs" :key="f" type="button" :class="item" @click="emit('open', f)">{{ f }}</button>
      </details>
      <details open>
        <summary :class="title">Formularios ({{ info.forms.length }})</summary>
        <button v-for="f in info.forms" :key="f" type="button" :class="item" @click="emit('open', f)">{{ f }}</button>
      </details>
      <details open>
        <summary :class="title">Clases ({{ info.classes.length }})</summary>
        <button
          v-for="c in info.classes"
          :key="c.file + c.line"
          type="button"
          :class="item"
          :title="`${c.file}, línea ${c.line}`"
          @click="emit('open', c.file, c.line)"
        >
          {{ c.name }} <span v-if="c.parent" class="text-neutral-500">: {{ c.parent }}</span>
        </button>
      </details>
      <details open>
        <summary :class="title">Datos ({{ info.tables.length }})</summary>
        <details v-for="t in info.tables" :key="t.name" class="pl-2">
          <summary class="cursor-pointer truncate px-2 py-0.5 select-none">{{ t.name }}</summary>
          <ul class="pl-6 font-mono text-xs">
            <li v-for="f in t.fields" :key="f.name" class="truncate">{{ f.name }} <span class="text-neutral-500">{{ type(f) }}</span></li>
          </ul>
        </details>
      </details>
    </div>
  </aside>
</template>
