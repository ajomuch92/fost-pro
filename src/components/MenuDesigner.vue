<script lang="ts">
export type Row = { level: number; prompt: string; command: string };

/** The FoxPro code of a menu: each row is an option, and its level says under which one it hangs. */
export function menuCode(rows: Row[]): string {
  const text = (s: string) => (!s.includes('"') ? `"${s}"` : !s.includes("'") ? `'${s}'` : `[${s}]`);
  const out = ["SET SYSMENU TO"];
  let popups = 0;
  /** Writes the options of rows[from..] at `level`, as pads of the menu or bars of `popup`; returns where it stopped. */
  function group(from: number, level: number, popup: string | null): number {
    const later: (() => void)[] = [];
    let i = from;
    for (let n = 1; i < rows.length && rows[i].level >= level; n++) {
      const { prompt, command } = rows[i];
      const target = popup ? `BAR ${n} OF ${popup}` : `PAD pad${n} OF _MSYSMENU`;
      out.push(`DEFINE ${target} PROMPT ${text(prompt === "-" ? "\\-" : prompt)}`);
      const children = i + 1 < rows.length && rows[i + 1].level > level;
      if (children) {
        const sub = `pop${++popups}`;
        out.push(`ON ${target} ACTIVATE POPUP ${sub}`);
        // a popup is written after the group that opens it, as FoxPro's own generator does
        const start = i + 1;
        later.push(() => {
          out.push("", `DEFINE POPUP ${sub}`);
          group(start, level + 1, sub);
        });
        i = start;
        while (i < rows.length && rows[i].level > level) i++;
      } else {
        if (command.trim()) out.push(`ON SELECTION ${target} ${command.trim()}`);
        i++;
      }
    }
    later.forEach((write) => write());
    return i;
  }
  group(0, 0, null);
  return out.join("\n") + "\n";
}
</script>

<script setup lang="ts">
// Draws a menu as a list of options, indented under the one that opens them, and writes its code.
import { ref } from "vue";
import { ArrowDown, ArrowUp, IndentDecrease, IndentIncrease, Plus, Trash2, X } from "@lucide/vue";
import { t } from "../i18n";

const emit = defineEmits<{ insert: [code: string]; close: [] }>();
const rows = ref<Row[]>([
  { level: 0, prompt: t("\\<Archivo"), command: "" },
  { level: 1, prompt: t("Nuevo"), command: "DO nuevo" },
  { level: 1, prompt: "-", command: "" },
  { level: 1, prompt: t("Salir"), command: "SET SYSMENU TO" },
]);

/** An option can hang at most one level under the one above it; the first one hangs from nothing. */
const deepest = (i: number) => (i === 0 ? 0 : rows.value[i - 1].level + 1);
function indent(i: number, by: number) {
  rows.value[i].level = Math.max(0, Math.min(deepest(i), rows.value[i].level + by));
  tidy();
}
function move(i: number, by: number) {
  const j = i + by;
  if (j < 0 || j >= rows.value.length) return;
  [rows.value[i], rows.value[j]] = [rows.value[j], rows.value[i]];
  tidy();
}
/** After a change, no option is left hanging deeper than it can. */
function tidy() {
  rows.value.forEach((r, i) => (r.level = Math.min(r.level, deepest(i))));
}
function remove(i: number) {
  rows.value.splice(i, 1);
  tidy();
}
const add = () => rows.value.push({ level: rows.value[rows.value.length - 1]?.level ?? 0, prompt: "", command: "" });

const icon = "rounded p-1 hover:bg-neutral-200 disabled:opacity-30 dark:hover:bg-neutral-700";
const field = "min-w-0 rounded border border-neutral-400 bg-transparent px-1";
const button = "rounded border border-neutral-400 px-3 py-1 hover:bg-neutral-200 dark:hover:bg-neutral-700";
</script>

<template>
  <div class="absolute inset-0 z-20 grid place-items-center bg-black/40" @keydown.esc="emit('close')">
    <section
      class="flex max-h-[90%] w-[44rem] max-w-[95%] flex-col rounded border border-neutral-500 bg-white text-sm shadow-xl dark:bg-neutral-800"
      role="dialog"
      aria-modal="true"
      :aria-label="t('Diseñador de menús')"
    >
      <header class="flex items-center border-b border-neutral-300 px-3 py-2 dark:border-neutral-700">
        <h2 class="flex-1 font-semibold">{{ t("Diseñador de menús") }}</h2>
        <button type="button" :class="icon" :aria-label="t('Cerrar')" @click="emit('close')"><X :size="16" /></button>
      </header>
      <p class="px-3 pt-2 text-neutral-500">
        {{ t("Cada fila es una opción. Las opciones con sangría cuelgan de la que tienen encima. Un guion (-) como texto dibuja una línea separadora y \\< subraya la letra que sigue.") }}
      </p>
      <ol class="min-h-0 flex-1 overflow-y-auto p-3">
        <li v-for="(row, i) in rows" :key="i" class="flex items-center gap-1 py-0.5" :style="{ paddingLeft: `${row.level * 24}px` }">
          <input v-model="row.prompt" :class="field" class="w-40" :placeholder="t('Texto')" :aria-label="t('Texto de la opción {0}', i + 1)" />
          <input v-model="row.command" :class="field" class="flex-1 font-mono" :placeholder="t('Comando, p. ej. DO guardar')" :aria-label="t('Comando de la opción {0}', i + 1)" />
          <button type="button" :class="icon" :disabled="row.level === 0" :aria-label="t('Quitar sangría')" @click="indent(i, -1)"><IndentDecrease :size="14" /></button>
          <button type="button" :class="icon" :disabled="row.level >= deepest(i)" :aria-label="t('Poner sangría')" @click="indent(i, 1)"><IndentIncrease :size="14" /></button>
          <button type="button" :class="icon" :disabled="i === 0" :aria-label="t('Subir')" @click="move(i, -1)"><ArrowUp :size="14" /></button>
          <button type="button" :class="icon" :disabled="i === rows.length - 1" :aria-label="t('Bajar')" @click="move(i, 1)"><ArrowDown :size="14" /></button>
          <button type="button" :class="icon" :aria-label="t('Eliminar')" @click="remove(i)"><Trash2 :size="14" /></button>
        </li>
      </ol>
      <footer class="flex items-center gap-2 border-t border-neutral-300 px-3 py-2 dark:border-neutral-700">
        <button type="button" :class="button" class="flex items-center gap-1" @click="add"><Plus :size="14" aria-hidden="true" />{{ t("Agregar opción") }}</button>
        <span class="flex-1" />
        <button type="button" :class="button" @click="emit('close')">{{ t("Cancelar") }}</button>
        <button type="button" :class="button" :disabled="!rows.length" @click="emit('insert', menuCode(rows))">{{ t("Insertar en el programa") }}</button>
      </footer>
    </section>
  </div>
</template>
