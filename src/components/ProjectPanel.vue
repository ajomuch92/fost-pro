<script setup lang="ts">
// The open project: the programs, forms, classes and tables of its folder.
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { Play, Plus, RefreshCw, Star, X } from "@lucide/vue";
import { assets, type MenuItem, pending, project, report } from "../engine";
import MenuItems from "./MenuItems.vue";

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
const emit = defineEmits<{
  open: [file: string, line?: number];
  run: [program: string];
  /** A file of the project changed its name, or is gone (`to` null). */
  moved: [from: string, to: string | null];
}>();
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

// The context menu: where it is, and the file it was opened on, if any.
const menu = ref<{ x: number; y: number; file: string | null } | null>(null);
const menuItems = () => {
  const item = (key: string, prompt: string): MenuItem => ({ key, prompt, disabled: false, popup: null, items: null });
  const forFile = menu.value?.file ? [item("-", "\\-"), item("rename", "Renombrar..."), item("delete", "Eliminar")] : [];
  return [item("program", "Nuevo programa..."), item("form", "Nuevo formulario..."), ...forFile];
};
function showMenu(e: MouseEvent, file: string | null = null) {
  menu.value = { x: e.clientX, y: e.clientY, file };
}

// The name being asked for: of a new file, or the new one of `file`.
const naming = ref<{ kind: "program" | "form" | "rename"; file: string | null; value: string } | null>(null);
const extensions = { program: ".fost", form: ".fostform" };
const taken = (name: string) =>
  [...info.value!.programs, ...info.value!.forms, ...info.value!.assets].some((f) => f.toLowerCase() === name.toLowerCase());

async function choose(action: string) {
  const file = menu.value!.file;
  menu.value = null;
  if (action !== "delete") {
    naming.value = { kind: action as "program" | "form" | "rename", file, value: action === "rename" ? file!.replace(/^.*\//, "") : "" };
    return;
  }
  if (!(await confirm(`¿Eliminar ${file}? No se puede deshacer.`, { kind: "warning" }))) return;
  try {
    await invoke("delete_file", { path: `${project.value}/${file}` });
    if (info.value!.main === file) await save(info.value!.file!, null);
    emit("moved", file!, null);
  } catch (e) {
    report(String(e));
  }
  await refresh();
}

/** Creates the file, or renames it, with the name that was typed. */
async function named() {
  const { kind, file } = naming.value!;
  let name = naming.value!.value.trim();
  if (!name || /[\\/]/.test(name)) return;
  naming.value = null;
  try {
    if (kind === "rename") {
      // the kind of a file is its extension, so a new name keeps it
      const extension = file!.match(/\.[^./]*$/)?.[0] ?? "";
      if (!name.toLowerCase().endsWith(extension.toLowerCase())) name += extension;
      const to = file!.replace(/[^/]*$/, name);
      if (to === file) return;
      await invoke("rename_file", { from: `${project.value}/${file}`, to: `${project.value}/${to}` });
      if (info.value!.main === file) await save(info.value!.file!, to);
      emit("moved", file!, to);
    } else {
      if (!name.toLowerCase().endsWith(extensions[kind])) name += extensions[kind];
      if (taken(name)) throw `Ya existe ${name}`;
      const stem = name.replace(/\.[^.]*$/, "").replace(/\W/g, "_");
      const text = kind === "form" ? JSON.stringify({ props: { Name: stem } }, null, 2) + "\n" : "";
      await invoke("write_file", { path: `${project.value}/${name}`, text });
      emit("open", name);
    }
  } catch (e) {
    report(String(e));
  }
  await refresh();
}
const vFocus = { mounted: (el: HTMLInputElement) => el.select() };

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
    @contextmenu.prevent="showMenu($event)"
    @keydown.esc="(menu = null), (naming = null)"
  >
    <!-- the menu of the right button: new files anywhere, and what can be done to the file under the pointer -->
    <div v-if="menu" class="fixed inset-0 z-40" @pointerdown.self="menu = null" @contextmenu.prevent.stop="menu = null">
      <div class="fixed" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }">
        <MenuItems :items="menuItems()" popup="proyecto" @choose="(_, action) => choose(action)" />
      </div>
    </div>
    <form v-if="naming" class="border-b border-neutral-300 p-2 dark:border-neutral-700" @submit.prevent="named">
      <label class="block">
        {{ naming.kind === "rename" ? `Nuevo nombre de ${naming.file}` : naming.kind === "form" ? "Nombre del formulario" : "Nombre del programa" }}
        <input
          v-model="naming.value"
          v-focus
          class="mt-1 w-full rounded border border-neutral-400 bg-transparent px-1"
          spellcheck="false"
          @blur="naming = null"
        />
      </label>
    </form>
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
          <button type="button" :class="item" @click="emit('open', f)" @contextmenu.prevent.stop="showMenu($event, f)">{{ f }}</button>
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
        <button v-for="f in info.forms" :key="f" type="button" :class="item" @click="emit('open', f)" @contextmenu.prevent.stop="showMenu($event, f)">
          {{ f }}
        </button>
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
          <li v-for="a in info.assets" :key="a" class="truncate px-2 py-0.5 select-text" :title="a" @contextmenu.prevent.stop="showMenu($event, a)">
            {{ a }}
          </li>
        </ul>
        <button type="button" :class="item" class="flex items-center gap-1 text-neutral-500" @click="addAssets">
          <Plus :size="14" aria-hidden="true" />Agregar...
        </button>
      </details>
    </div>
  </aside>
</template>
