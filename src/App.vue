<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { confirm, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { AppWindow, CircleQuestionMark, FilePlus, FolderKanban, FolderOpen, Play, Save } from "@lucide/vue";
import { dir, helpOpen, openRequest, project, report, run, runForm } from "./engine";
import CommandWindow from "./components/CommandWindow.vue";
import FormDesigner from "./components/FormDesigner.vue";
import FormLayer from "./components/FormLayer.vue";
import MenuBar from "./components/MenuBar.vue";
import HelpScreen from "./components/HelpScreen.vue";
import ProjectPanel from "./components/ProjectPanel.vue";
import PromptDialog from "./components/PromptDialog.vue";
import ProgramEditor from "./components/ProgramEditor.vue";

/** One document is open at a time: a program in the editor, or a form in the designer. */
type Kind = "program" | "form";
const kinds = {
  program: { extension: "fost", label: "Programa Fost", untitled: "programa" },
  form: { extension: "fostform", label: "Formulario Fost", untitled: "formulario" },
};
const kindOf = (file: string): Kind => (/\.fostform$/i.test(file) ? "form" : "program");

const editor = ref<InstanceType<typeof ProgramEditor>>();
const designer = ref<InstanceType<typeof FormDesigner>>();
const panel = ref<InstanceType<typeof ProjectPanel>>();
const kind = ref<Kind>("program");
const path = ref<string | null>(null);
const dirty = ref(false);

const view = (k: Kind) => (k === "form" ? designer.value! : editor.value!);
const name = computed(() => path.value?.replace(/^.*[\\/]/, "") ?? "sin título");
/** The file name without its extension: what DO and DO FORM call it. */
const stem = (file: string) => file.replace(/^.*[\\/]/, "").replace(/\.[^.]*$/, "");

// DO and DO FORM look for files next to the open document
watch(path, (p) => {
  if (p) dir.value = p.replace(/[\\/][^\\/]*$/, "");
});

const canDiscard = async () =>
  !dirty.value || (await confirm("Hay cambios sin guardar. ¿Descartarlos?", { kind: "warning" }));

/** Runs a file action; a failure is shown in the command window instead of being lost. */
async function attempt(action: () => Promise<unknown>) {
  try {
    await action();
  } catch (e) {
    report(String(e));
  }
}

/** Makes `text` the open document. A form that cannot be read leaves the current document alone. */
function show(k: Kind, text: string, file: string | null) {
  view(k).setText(text);
  kind.value = k;
  path.value = file;
  dirty.value = false;
}

/** Opens a file. With `orNew`, a file that does not exist yet becomes an empty document with that name. */
async function load(file: string, orNew = false) {
  const k = kindOf(file);
  let text = "";
  try {
    text = await invoke<string>("read_file", { path: file });
  } catch (e) {
    if (!orNew) throw e;
    if (k === "form") text = JSON.stringify({ props: { Name: stem(file).replace(/\W/g, "_") } });
  }
  show(k, text, file);
}

const newDoc = (k: Kind) =>
  attempt(async () => {
    if (await canDiscard()) show(k, "", null);
  });

const openFile = () =>
  attempt(async () => {
    if (!(await canDiscard())) return;
    const chosen = await open({ filters: [{ name: "Programas y formularios Fost", extensions: ["fost", "fostform"] }] });
    if (chosen) await load(chosen);
  });

const saveFile = () =>
  attempt(async () => {
    const { extension, label, untitled } = kinds[kind.value];
    const filters = [{ name: label, extensions: [extension] }];
    // a new file is offered next to the others of the project
    const folder = dir.value ? `${dir.value}/` : "";
    let target = path.value ?? (await saveDialog({ filters, defaultPath: `${folder}${untitled}.${extension}` }));
    if (!target) return;
    // the extension decides how the file is opened and run, so it is not optional
    if (kindOf(target) !== kind.value || !/\.fost(form)?$/i.test(target)) target += `.${extension}`;
    await invoke("write_file", { path: target, text: view(kind.value).getText() });
    path.value = target;
    dirty.value = false;
    panel.value?.refresh();
  });

/** A project is a folder: the one chosen here, which the dialog can also create. */
const openProject = () =>
  attempt(async () => {
    const chosen = await open({ directory: true, title: "Carpeta del proyecto" });
    if (chosen) project.value = chosen;
  });

/** Opens a file of the project from its panel; `line` is where a class is defined. */
const openFromProject = (file: string, line?: number) =>
  attempt(async () => {
    if (!(await canDiscard())) return;
    await load(`${project.value}/${file}`);
    if (line) editor.value!.gotoLine(line);
  });

async function runDoc() {
  const text = view(kind.value).getText();
  if (kind.value === "form") {
    await runForm(text, path.value ? stem(path.value) : kinds.form.untitled);
    return;
  }
  const result = await run(text, `DO ${name.value}`);
  if (result?.line) editor.value!.gotoLine(result.line);
}

// MODIFY COMMAND, MODIFY FORM and CREATE FORM typed in the command window
watch(openRequest, (request) => {
  if (!request) return;
  openRequest.value = null;
  attempt(async () => {
    if (!(await canDiscard())) return;
    if (request.path) await load(request.path, true);
    else show(request.form ? "form" : "program", "", null);
  });
});

function shortcut(e: KeyboardEvent) {
  const action =
    e.key === "F5"
      ? runDoc
      : e.key === "F1"
        ? () => (helpOpen.value = !helpOpen.value)
        : (e.ctrlKey || e.metaKey) && e.key === "s"
        ? saveFile
        : (e.ctrlKey || e.metaKey) && e.key === "o"
          ? openFile
          : null;
  if (!action) return;
  e.preventDefault();
  action();
}

let stopCloseGuard = () => {};
onMounted(async () => {
  window.addEventListener("keydown", shortcut);
  // closing the window must not silently drop unsaved work
  stopCloseGuard = await getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await canDiscard())) event.preventDefault();
  });
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", shortcut);
  stopCloseGuard();
});

const button = "flex items-center gap-1.5 rounded px-2 py-1 hover:bg-neutral-200 dark:hover:bg-neutral-700";
</script>

<template>
  <main class="flex h-screen flex-col bg-white text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
    <header class="flex items-center gap-1 border-b border-neutral-300 px-2 py-1 text-sm dark:border-neutral-700">
      <button type="button" :class="button" @click="newDoc('program')"><FilePlus :size="16" aria-hidden="true" />Nuevo programa</button>
      <button type="button" :class="button" @click="newDoc('form')"><AppWindow :size="16" aria-hidden="true" />Nuevo formulario</button>
      <button type="button" :class="button" title="Abrir o crear la carpeta de un proyecto" @click="openProject"><FolderKanban :size="16" aria-hidden="true" />Proyecto</button>
      <button type="button" :class="button" title="Ctrl+O" @click="openFile"><FolderOpen :size="16" aria-hidden="true" />Abrir</button>
      <button type="button" :class="button" title="Ctrl+S" @click="saveFile"><Save :size="16" aria-hidden="true" />Guardar</button>
      <button type="button" :class="button" title="F5" @click="runDoc"><Play :size="16" aria-hidden="true" />Ejecutar (F5)</button>
      <button type="button" :class="button" title="F1" @click="helpOpen = !helpOpen"><CircleQuestionMark :size="16" aria-hidden="true" />Ayuda (F1)</button>
      <span class="ml-auto text-neutral-500">{{ name }}{{ dirty ? " (sin guardar)" : "" }}</span>
    </header>
    <MenuBar />
    <div class="flex min-h-0 flex-1">
    <ProjectPanel ref="panel" @open="openFromProject" />
    <div class="relative flex min-h-0 min-w-0 flex-1 flex-col">
      <ProgramEditor v-show="kind === 'program'" ref="editor" class="min-h-0 flex-1" @change="dirty = true" />
      <FormDesigner v-show="kind === 'form'" ref="designer" class="min-h-0 flex-1" @change="dirty = true" />
      <CommandWindow class="h-1/3 border-t border-neutral-300 dark:border-neutral-700" />
      <FormLayer />
      <HelpScreen />
      <PromptDialog />
    </div>
    </div>
  </main>
</template>
