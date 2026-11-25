<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { confirm, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import CommandWindow from "./components/CommandWindow.vue";
import ProgramEditor from "./components/ProgramEditor.vue";

const editor = ref<InstanceType<typeof ProgramEditor>>();
const commands = ref<InstanceType<typeof CommandWindow>>();
const path = ref<string | null>(null);
const dirty = ref(false);
const filters = [{ name: "Programa Fost", extensions: ["fost"] }];

const name = computed(() => path.value?.replace(/^.*[\\/]/, "") ?? "sin título");
const dir = computed(() => path.value?.replace(/[\\/][^\\/]*$/, "") ?? null);

const canDiscard = async () =>
  !dirty.value || (await confirm("Hay cambios sin guardar. ¿Descartarlos?", { kind: "warning" }));

/** Runs a file action; a failure is shown in the command window instead of being lost. */
async function attempt(action: () => Promise<unknown>) {
  try {
    await action();
  } catch (e) {
    commands.value?.report(String(e));
  }
}

const newFile = () =>
  attempt(async () => {
    if (!(await canDiscard())) return;
    editor.value!.setText("");
    path.value = null;
    dirty.value = false;
  });

const openFile = () =>
  attempt(async () => {
    if (!(await canDiscard())) return;
    const chosen = await open({ filters });
    if (!chosen) return;
    editor.value!.setText(await invoke<string>("read_file", { path: chosen }));
    path.value = chosen;
    dirty.value = false;
  });

const saveFile = () =>
  attempt(async () => {
    const target = path.value ?? (await saveDialog({ filters, defaultPath: "programa.fost" }));
    if (!target) return;
    await invoke("write_file", { path: target, text: editor.value!.getText() });
    path.value = target;
    dirty.value = false;
  });

async function runProgram() {
  const result = await commands.value!.execute(editor.value!.getText(), `DO ${name.value}`);
  if (result?.line) editor.value!.gotoLine(result.line);
}

function shortcut(e: KeyboardEvent) {
  const action =
    e.key === "F5"
      ? runProgram
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

const button = "rounded px-2 py-1 hover:bg-neutral-200 dark:hover:bg-neutral-700";
</script>

<template>
  <main class="flex h-screen flex-col bg-white text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
    <header class="flex items-center gap-1 border-b border-neutral-300 px-2 py-1 text-sm dark:border-neutral-700">
      <button type="button" :class="button" @click="newFile">Nuevo</button>
      <button type="button" :class="button" title="Ctrl+O" @click="openFile">Abrir</button>
      <button type="button" :class="button" title="Ctrl+S" @click="saveFile">Guardar</button>
      <button type="button" :class="button" title="F5" @click="runProgram">Ejecutar (F5)</button>
      <span class="ml-auto text-neutral-500">{{ name }}{{ dirty ? " (sin guardar)" : "" }}</span>
    </header>
    <ProgramEditor ref="editor" class="min-h-0 flex-1" @change="dirty = true" />
    <CommandWindow ref="commands" :dir="dir" class="h-2/5 border-t border-neutral-300 dark:border-neutral-700" />
  </main>
</template>
