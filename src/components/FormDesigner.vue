<script setup lang="ts">
// Visual editor for a form file: a tree of { class, props, methods, controls }, saved as JSON.
// Only the properties that differ from the class defaults are kept, so files stay small.
import { nextTick, ref, watch } from "vue";
import { classes } from "../engine";
import FormControl, { colorCss } from "./FormControl.vue";
import ProgramEditor from "./ProgramEditor.vue";

type DesignNode = {
  class: string;
  props: Record<string, unknown>;
  /** Source code by event or method name. */
  methods: Record<string, string>;
  controls: DesignNode[];
};

const emit = defineEmits<{ change: [] }>();

const blank = (): DesignNode => ({ class: "Form", props: { Name: "Form1" }, methods: {}, controls: [] });
const form = ref<DesignNode>(blank());
const selected = ref<DesignNode>(form.value);
const event = ref("Init");
const code = ref<InstanceType<typeof ProgramEditor>>();
const nameError = ref("");

const info = (node: DesignNode) => classes.value.find((c) => c.name === node.class);
const defaults = (node: DesignNode): Record<string, unknown> => Object.fromEntries(info(node)?.props ?? []);
/** A property as it will be at run time: the designed value, or the default of the class. */
const value = (node: DesignNode, key: string) => node.props[key] ?? defaults(node)[key];
const num = (node: DesignNode, key: string) => Number(value(node, key));
const px = (node: DesignNode, key: string) => `${num(node, key)}px`;
const box = (node: DesignNode) => ({
  left: px(node, "Left"),
  top: px(node, "Top"),
  width: px(node, "Width"),
  height: px(node, "Height"),
});

/** The node as FormControl draws it: every property, upper-cased like the interpreter's. */
function view(node: DesignNode) {
  const all = { ...defaults(node), ...node.props };
  const props = Object.fromEntries(Object.entries(all).map(([k, v]) => [k.toUpperCase(), v]));
  const list = all.RowSourceType === 1 ? String(all.RowSource).split(",") : [];
  return { base: node.class.toUpperCase(), props, items: list.map((s) => s.trim()).filter(Boolean) };
}

function set(node: DesignNode, key: string, v: unknown) {
  if (v === defaults(node)[key]) delete node.props[key];
  else node.props[key] = v;
  emit("change");
}

function rename(node: DesignNode, name: string) {
  const taken = [form.value, ...form.value.controls].some(
    (n) => n !== node && String(n.props.Name).toUpperCase() === name.toUpperCase(),
  );
  nameError.value = !/^[A-Za-z_]\w*$/.test(name)
    ? "Usa letras, números y guion bajo, sin empezar por un número."
    : taken
      ? "Ya hay un objeto con ese nombre."
      : "";
  if (nameError.value) return;
  node.props.Name = name;
  emit("change");
}

// FoxPro's own names for new controls: Text1, Command1...
const prefixes: Record<string, string> = {
  TextBox: "Text",
  EditBox: "Edit",
  CommandButton: "Command",
  CheckBox: "Check",
  ComboBox: "Combo",
  ListBox: "List",
};

function add(cls: string) {
  const prefix = prefixes[cls] ?? cls;
  const names = form.value.controls.map((c) => c.props.Name);
  let n = 1;
  while (names.includes(prefix + n)) n++;
  // each new control lands under the lowest one, so they do not pile up
  const top = Math.max(0, ...form.value.controls.map((c) => num(c, "Top") + num(c, "Height"))) + 10;
  const node: DesignNode = { class: cls, props: { Name: prefix + n, Left: 10, Top: top }, methods: {}, controls: [] };
  if ("Caption" in defaults(node)) node.props.Caption = prefix + n;
  form.value.controls.push(node);
  selected.value = form.value.controls[form.value.controls.length - 1];
  emit("change");
}

function remove() {
  if (selected.value === form.value) return;
  form.value.controls = form.value.controls.filter((c) => c !== selected.value);
  selected.value = form.value;
  emit("change");
}

function startDrag(node: DesignNode, down: PointerEvent, mode: "move" | "size") {
  selected.value = node;
  const [a, b] = mode === "move" ? ["Left", "Top"] : ["Width", "Height"];
  const min = mode === "move" ? 0 : 8;
  const from = { x: down.clientX, y: down.clientY, a: num(node, a), b: num(node, b) };
  const move = (e: PointerEvent) => {
    set(node, a, Math.max(min, Math.round(from.a + e.clientX - from.x)));
    set(node, b, Math.max(min, Math.round(from.b + e.clientY - from.y)));
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", () => removeEventListener("pointermove", move), { once: true });
}

/** Arrows move the control, with Shift they resize it; Delete removes it. */
function keyed(node: DesignNode, e: KeyboardEvent) {
  const steps: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
  };
  if (e.key === "Delete") return remove();
  const step = steps[e.key];
  if (!step) return;
  e.preventDefault();
  const [a, b] = e.shiftKey ? ["Width", "Height"] : ["Left", "Top"];
  const min = e.shiftKey ? 8 : 0;
  set(node, a, Math.max(min, num(node, a) + step[0]));
  set(node, b, Math.max(min, num(node, b) + step[1]));
}

/** The events of the selected object's class, plus any other method the file already has. */
const methodNames = (node: DesignNode) => [...new Set([...(info(node)?.events ?? []), ...Object.keys(node.methods)])];

// The code editor shows the chosen event of the selected object.
let loading = false;
watch([selected, event], async () => {
  if (!methodNames(selected.value).includes(event.value)) event.value = "Init";
  await nextTick();
  loading = true;
  code.value?.setText(selected.value.methods[event.value] ?? "");
  loading = false;
});

function edited() {
  if (loading) return;
  const text = code.value!.getText();
  if (text.trim()) selected.value.methods[event.value] = text;
  else delete selected.value.methods[event.value];
  emit("change");
}

const hex = (c: number) =>
  "#" + [c & 255, (c >> 8) & 255, (c >> 16) & 255].map((x) => x.toString(16).padStart(2, "0")).join("");
const fromHex = (h: string) =>
  parseInt(h.slice(1, 3), 16) + parseInt(h.slice(3, 5), 16) * 256 + parseInt(h.slice(5, 7), 16) * 65536;
const typed = (e: Event) => (e.target as HTMLInputElement).value;
const ticked = (e: Event) => (e.target as HTMLInputElement).checked;

/** Fills in what a hand-written file may leave out, and rejects classes the designer cannot draw. */
function normalize(node: Partial<DesignNode>, fallback: string): DesignNode {
  const cls = classes.value.find((c) => c.name.toUpperCase() === (node.class ?? fallback).toUpperCase());
  if (!cls || cls.name === "Custom") throw new Error(`El diseñador no conoce la clase ${node.class}`);
  const controls = (node.controls ?? []).map((c) => normalize(c, "Label"));
  return { class: cls.name, props: { Name: cls.name, ...node.props }, methods: { ...node.methods }, controls };
}

defineExpose({
  getText: () =>
    // empty `methods` and `controls` are left out of the file
    JSON.stringify(form.value, (k, v) => ((k === "methods" || k === "controls") && !Object.keys(v).length ? undefined : v), 2) +
    "\n",
  setText(text: string) {
    const next = text.trim() ? normalize(JSON.parse(text), "Form") : blank();
    if (next.class !== "Form") throw new Error("El archivo no describe un formulario");
    form.value = next;
    selected.value = form.value;
    nameError.value = "";
  },
});

const field = "min-w-0 rounded border border-neutral-400 bg-transparent px-1";
const tool = "rounded border border-neutral-400 px-2 py-0.5 text-left hover:bg-neutral-200 dark:hover:bg-neutral-700";
const handle = "absolute -right-1 -bottom-1 size-2 cursor-nwse-resize bg-blue-600";
</script>

<template>
  <div class="grid h-full min-h-0 grid-cols-[9.5rem_1fr_17rem] text-sm">
    <nav class="flex flex-col gap-1 overflow-y-auto border-r border-neutral-300 p-2 dark:border-neutral-700" aria-label="Agregar control">
      <button
        v-for="c in classes.filter((c) => c.name !== 'Custom' && c.name !== 'Form')"
        :key="c.name"
        type="button"
        :class="tool"
        @click="add(c.name)"
      >
        {{ c.name }}
      </button>
    </nav>

    <div class="flex min-h-0 min-w-0 flex-col">
      <div class="min-h-0 flex-1 overflow-auto bg-neutral-300 p-4 select-none dark:bg-neutral-800" @pointerdown.self="selected = form">
        <div class="inline-block border border-neutral-600 shadow-lg" :class="{ 'outline-2 outline-blue-500': selected === form }">
          <div class="bg-blue-700 px-2 py-1 text-white" @pointerdown="selected = form">{{ value(form, "Caption") }}</div>
          <div
            class="relative text-black"
            :style="{ width: px(form, 'Width'), height: px(form, 'Height'), backgroundColor: colorCss(num(form, 'BackColor')) }"
            @pointerdown.self="selected = form"
          >
            <FormControl v-for="(c, i) in form.controls" :key="`view${i}`" :node="view(c)" class="pointer-events-none" />
            <!-- a transparent grip over each control takes the pointer and the keyboard -->
            <div
              v-for="(c, i) in form.controls"
              :key="`grip${i}`"
              class="absolute cursor-move"
              :class="{ 'outline-2 outline-blue-500': selected === c }"
              :style="box(c)"
              tabindex="0"
              role="button"
              :aria-label="`${c.props.Name}: flechas para mover, Mayús+flechas para cambiar el tamaño, Supr para eliminar`"
              @pointerdown.stop="startDrag(c, $event, 'move')"
              @focus="selected = c"
              @keydown="keyed(c, $event)"
            >
              <span v-if="selected === c" :class="handle" @pointerdown.stop="startDrag(c, $event, 'size')" />
            </div>
            <span :class="handle" title="Cambiar el tamaño del formulario" @pointerdown.stop="startDrag(form, $event, 'size')" />
          </div>
        </div>
      </div>

      <div class="flex h-2/5 min-h-0 flex-col border-t border-neutral-300 dark:border-neutral-700">
        <label class="flex items-center gap-2 px-2 py-1">
          Código de {{ selected.props.Name }}
          <select v-model="event" :class="field">
            <option v-for="m in methodNames(selected)" :key="m" :value="m">{{ m }}{{ selected.methods[m] ? " •" : "" }}</option>
          </select>
        </label>
        <ProgramEditor ref="code" class="min-h-0 flex-1" @change="edited" />
      </div>
    </div>

    <aside class="flex min-h-0 flex-col border-l border-neutral-300 dark:border-neutral-700" aria-label="Propiedades">
      <div class="flex items-center gap-1 border-b border-neutral-300 p-2 dark:border-neutral-700">
        <select v-model="selected" class="flex-1" :class="field" aria-label="Objeto seleccionado">
          <option :value="form">{{ form.props.Name }}</option>
          <option v-for="(c, i) in form.controls" :key="i" :value="c">{{ c.props.Name }}</option>
        </select>
        <button type="button" :class="tool" class="disabled:opacity-40" :disabled="selected === form" @click="remove">Eliminar</button>
      </div>
      <div class="overflow-y-auto p-2">
        <label class="grid grid-cols-2 items-center gap-1 py-0.5">
          Name
          <input :class="field" :value="selected.props.Name" @change="rename(selected, typed($event))" />
        </label>
        <p v-if="nameError" class="text-red-600 dark:text-red-400" role="alert">{{ nameError }}</p>
        <label v-for="[key, initial] in info(selected)?.props" :key="key" class="grid grid-cols-2 items-center gap-1 py-0.5">
          {{ key }}
          <input
            v-if="typeof initial === 'boolean'"
            type="checkbox"
            class="justify-self-start"
            :checked="!!value(selected, key)"
            @change="set(selected, key, ticked($event))"
          />
          <input
            v-else-if="key.endsWith('Color')"
            type="color"
            :value="hex(num(selected, key))"
            @input="set(selected, key, fromHex(typed($event)))"
          />
          <input
            v-else-if="typeof initial === 'number'"
            type="number"
            :class="field"
            :value="num(selected, key)"
            @change="set(selected, key, Number(typed($event)))"
          />
          <input v-else :class="field" :value="value(selected, key)" @change="set(selected, key, typed($event))" />
        </label>
      </div>
    </aside>
  </div>
</template>
