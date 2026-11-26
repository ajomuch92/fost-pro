<script setup lang="ts">
// The open project: the programs, forms, classes and tables of its folder.
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Play, Plus, RefreshCw, Star, X } from "@lucide/vue";
import { assets, pending, project, report } from "../engine";

type Field = { name: string; kind: string; width: number; decimals: number };
type Info = {
  /** The project file, `name.fostproj`, and the main program it names. */
  file: string | null;
  main: string | null;
  assets: string[];
  programs: string[];
  forms: string[];
  classes: { name: string; parent: string | null; file: string; line: number }[];
  tables: { name: string; fields: Field[] }[];
};

/** A file of the project to open, and the line to go to when it matters; or the main program to run. */
const emit = defineEmits<{ open: [file: string, line?: number]; run: [program: string] }>();
const info = ref<Info | null>(null);

const folderName = () => project.value!.replace(/^.*[\\/]/, "");

/** Writes the project file. What it holds today is the main program. */
const save = (file: string, main: string | null) =>
  invoke("write_file", { path: `${project.value}/${file}`, text: JSON.stringify({ main }, null, 2) + "\n" });

async function refresh() {
  assets.value = [];
  if (!project.value) return (info.value = null);
  try {
    let found = await invoke<Info>("project_info", { dir: project.value });
    // a folder becomes a project by getting its project file
    if (!found.file) {
      await save(`${folderName()}.fostproj`, null);
      found = await invoke<Info>("project_info", { dir: project.value });
    }
    info.value = found;
    assets.value = found.assets;
  } catch (e) {
    report(String(e));
  }
}

/** Makes `program` the main one; choosing the main one again leaves the project without any. */
async function setMain(program: string) {
  try {
    await save(info.value!.file!, info.value!.main === program ? null : program);
  } catch (e) {
    report(String(e));
  }
  await refresh();
}

/** Copies files the user picks into the project's `assets` folder. */
async function addAssets() {
  const chosen = await open({ multiple: true, title: "Agregar recursos al proyecto" });
  for (const from of chosen ?? []) {
    const to = `${project.value}/assets/${from.replace(/^.*[\\/]/, "")}`;
    await invoke("copy_file", { from, to }).catch((e) => report(String(e)));
  }
  await refresh();
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
      <span class="flex-1 truncate font-semibold" :title="project">{{ folderName() }}</span>
      <button
        type="button"
        class="rounded p-1 hover:bg-neutral-200 disabled:opacity-40 dark:hover:bg-neutral-700"
        :disabled="!info.main"
        :aria-label="info.main ? `Ejecutar ${info.main}` : 'Ejecutar el proyecto: marca antes un programa principal'"
        :title="info.main ? `Ejecutar ${info.main}` : 'Marca un programa principal con la estrella'"
        @click="emit('run', info.main!)"
      >
        <Play :size="14" />
      </button>
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
        <div v-for="f in info.programs" :key="f" class="flex items-center">
          <button type="button" :class="item" @click="emit('open', f)">{{ f }}</button>
          <button
            type="button"
            class="shrink-0 rounded p-1 hover:bg-neutral-200 dark:hover:bg-neutral-700"
            :class="info.main === f ? 'text-amber-500' : 'text-neutral-400'"
            :aria-label="`${f}: programa principal`"
            :aria-pressed="info.main === f"
            title="Programa principal"
            @click="setMain(f)"
          >
            <Star :size="14" :fill="info.main === f ? 'currentColor' : 'none'" />
          </button>
        </div>
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
      <details open>
        <summary :class="title">Recursos ({{ info.assets.length }})</summary>
        <!-- not buttons: a resource is used by writing its path in a property, such as an Image's Picture -->
        <ul>
          <li v-for="a in info.assets" :key="a" class="truncate px-2 py-0.5 select-text" :title="a">{{ a }}</li>
        </ul>
        <button type="button" :class="item" class="flex items-center gap-1 text-neutral-500" @click="addAssets">
          <Plus :size="14" aria-hidden="true" />Agregar...
        </button>
      </details>
    </div>
  </aside>
</template>
