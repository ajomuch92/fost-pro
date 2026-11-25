<script setup lang="ts">
// Visual editor for a form file: a tree of { class, props, methods, controls }, saved as JSON.
// Only the properties that differ from the class defaults are kept, so files stay small.
import { type Component, computed, nextTick, reactive, ref, watch } from "vue";
import {
  ChevronsUpDown,
  Circle,
  CircleDot,
  Image as ImageIcon,
  Link,
  List,
  Minus,
  NotebookText,
  PanelTop,
  PanelsTopLeft,
  Rows2,
  Shapes,
  Slash,
  Square,
  SquareCheck,
  SquareChevronDown,
  SquareDashed,
  SquareMousePointer,
  TextCursorInput,
  Timer,
  Type,
  X,
} from "@lucide/vue";
import { classes, type DrawNode } from "../engine";
import FormControl, { colorCss, pageTop } from "./FormControl.vue";
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
const num = (node: DesignNode, key: string) => Number(value(node, key) ?? 0);
const px = (node: DesignNode, key: string) => `${num(node, key)}px`;

// The page of each page frame being worked on. It is the designer's own business: the form still
// opens on the page its ActivePage property says, not on whichever was edited last.
const front = reactive(new WeakMap<DesignNode, number>());
const frontPage = (frame: DesignNode) => front.get(frame) ?? num(frame, "ActivePage");

/** The classes that hold only one kind of control, and which. */
const groups: Record<string, string> = { OptionGroup: "OptionButton", CommandGroup: "CommandButton", PageFrame: "Page" };

/** Whether a control of class `cls` can be put directly inside `parent`. */
function accepts(parent: DesignNode, cls: string) {
  if (parent.class in groups) return groups[parent.class] === cls;
  return ["Form", "Container", "Page"].includes(parent.class) && cls !== "OptionButton" && cls !== "Page";
}

type Placed = { node: DesignNode; parent: DesignNode | null; depth: number; x: number; y: number; shown: boolean };

/** Every node of the form, with its parent, its depth, and where its corner is on the form's surface. */
const tree = computed(() => {
  const out: Placed[] = [];
  // (ox, oy) is where the parent's contents start; `shown` is false inside a page that is not the active one
  const walk = (node: DesignNode, parent: DesignNode | null, depth: number, ox: number, oy: number, shown: boolean) => {
    const fills = node.class === "Form" || node.class === "Page"; // these have no position of their own
    const x = fills ? ox : ox + num(node, "Left");
    const y = fills ? oy : oy + num(node, "Top");
    out.push({ node, parent, depth, x, y, shown });
    node.controls.forEach((c, i) => {
      if (node.class === "PageFrame") walk(c, node, depth + 1, x, y + pageTop(value(node, "Caption")), shown && frontPage(node) === i + 1);
      else walk(c, node, depth + 1, x, y, shown);
    });
  };
  walk(form.value, null, 0, 0, 0, true);
  return out;
});
const parentOf = (node: DesignNode) => tree.value.find((p) => p.node === node)?.parent ?? null;

/** The part of the surface where a container's controls go. */
function area(p: Placed) {
  // a page is as big as its frame, minus the tabs and title above it
  const frame = p.node.class === "Page" ? p.parent! : p.node;
  const above = p.node.class === "Page" ? pageTop(value(frame, "Caption")) : 0;
  return { x: p.x, y: p.y, w: num(frame, "Width"), h: num(frame, "Height") - above };
}

/** The container under the middle of `node` that could take it: the innermost one, and never one inside `node`. */
function containerUnder(node: DesignNode) {
  const me = tree.value.find((p) => p.node === node)!;
  const [cx, cy] = [me.x + num(node, "Width") / 2, me.y + num(node, "Height") / 2];
  const within = (p: Placed) => {
    for (let n: DesignNode | null = p.node; n; n = parentOf(n)) if (n === node) return false;
    const a = area(p);
    return cx >= a.x && cx <= a.x + a.w && cy >= a.y && cy <= a.y + a.h;
  };
  // later in the tree means deeper, or drawn on top
  return tree.value.filter((p) => p.shown && accepts(p.node, node.class) && within(p)).pop();
}

/** Moves a control into another container, keeping it where it is on screen. */
function moveInto(node: DesignNode, target: Placed) {
  const me = tree.value.find((p) => p.node === node)!;
  const [left, top] = [me.x - target.x, me.y - target.y];
  const parent = me.parent!;
  parent.controls = parent.controls.filter((c) => c !== node);
  // names must be unique among the controls of one container
  const taken = target.node.controls.map((c) => String(c.props.Name).toUpperCase());
  let name = String(node.props.Name);
  for (let n = 2; taken.includes(name.toUpperCase()); n++) name = `${node.props.Name}_${n}`;
  node.props.Name = name;
  target.node.controls.push(node);
  put(node, "Left", left);
  put(node, "Top", top);
}

/** Where a new control of class `cls` goes: the selected object if it can hold it, else the nearest one around it that can. */
function hostFor(cls: string) {
  for (let n: DesignNode | null = selected.value; n; n = parentOf(n)) if (accepts(n, cls)) return n;
  return null;
}

/** The node as FormControl draws it: every property, upper-cased like the interpreter's. */
function view(node: DesignNode): DrawNode {
  const all = { ...defaults(node), ...node.props };
  const props = Object.fromEntries(Object.entries(all).map(([k, v]) => [k.toUpperCase(), v]));
  if (node.class === "PageFrame") props.ACTIVEPAGE = frontPage(node);
  const list = all.RowSourceType === 1 ? String(all.RowSource).split(",") : [];
  return {
    base: node.class.toUpperCase(),
    props,
    items: list.map((s) => s.trim()).filter(Boolean),
    children: node.controls.map(view),
    source: node,
  };
}

/** Stores a property, or drops it when it is back at the class default. */
function put(node: DesignNode, key: string, v: unknown) {
  if (v === defaults(node)[key]) delete node.props[key];
  else node.props[key] = v;
}

function set(node: DesignNode, key: string, v: unknown) {
  put(node, key, v);
  // changing the property itself is also a wish to see that page
  if (key === "ActivePage") front.delete(node);
  emit("change");
}

function rename(node: DesignNode, name: string) {
  // names only need to be unique among the controls of the same container
  const siblings = parentOf(node)?.controls ?? [];
  const taken = siblings.some((n) => n !== node && String(n.props.Name).toUpperCase() === name.toUpperCase());
  nameError.value = !/^[A-Za-z_]\w*$/.test(name)
    ? "Usa letras, números y guion bajo, sin empezar por un número."
    : taken
      ? "Ya hay un objeto con ese nombre."
      : "";
  if (nameError.value) return;
  node.props.Name = name;
  emit("change");
}

const icons: Record<string, Component> = {
  Label: Type,
  TextBox: TextCursorInput,
  EditBox: NotebookText,
  Spinner: ChevronsUpDown,
  CommandButton: SquareMousePointer,
  CheckBox: SquareCheck,
  ComboBox: SquareChevronDown,
  ListBox: List,
  OptionGroup: CircleDot,
  OptionButton: Circle,
  CommandGroup: Rows2,
  HyperLink: Link,
  Image: ImageIcon,
  Line: Slash,
  Shape: Shapes,
  Timer,
  Container: SquareDashed,
  PageFrame: PanelsTopLeft,
  Page: PanelTop,
};

// FoxPro's own names for new controls: Text1, Command1...
const prefixes: Record<string, string> = {
  TextBox: "Text",
  EditBox: "Edit",
  CommandButton: "Command",
  CheckBox: "Check",
  ComboBox: "Combo",
  ListBox: "List",
  OptionButton: "Option",
};

function add(cls: string, into = hostFor(cls)) {
  if (!into) return;
  const prefix = prefixes[cls] ?? cls;
  const names = into.controls.map((c) => c.props.Name);
  let n = 1;
  while (names.includes(prefix + n)) n++;
  const node: DesignNode = { class: cls, props: { Name: prefix + n }, methods: {}, controls: [] };
  // a page frame's caption is an optional title, so it starts without one
  if ("Caption" in defaults(node) && cls !== "PageFrame") node.props.Caption = prefix + n;
  if ("Top" in defaults(node)) {
    // each new control lands under the lowest one, so they do not pile up
    const margin = into.class in groups ? 5 : 10;
    node.props.Left = margin;
    node.props.Top = Math.max(0, ...into.controls.map((c) => num(c, "Top") + num(c, "Height"))) + margin;
  }
  into.controls.push(node);
  const made = into.controls[into.controls.length - 1];
  // as in FoxPro, a group or page frame starts with two members
  if (cls in groups) for (let i = 0; i < 2; i++) add(groups[cls], made);
  selected.value = made;
  emit("change");
}

function remove() {
  const parent = parentOf(selected.value);
  if (!parent) return;
  parent.controls = parent.controls.filter((c) => c !== selected.value);
  selected.value = parent;
  emit("change");
}

/** While a control is dragged over a container other than its own: where it would land. */
const drop = ref<ReturnType<typeof area> | null>(null);

function startDrag(node: DesignNode, down: PointerEvent, mode: "move" | "size") {
  selected.value = node;
  const moving = mode === "move" && node !== form.value;
  const [a, b] = mode === "move" ? ["Left", "Top"] : ["Width", "Height"];
  // a control being moved may leave its container by any side; a size stays big enough to grab, except a Line's
  const min = moving ? -Infinity : node.class === "Line" ? 0 : 8;
  const from = { x: down.clientX, y: down.clientY, a: num(node, a), b: num(node, b) };
  /** The container the control is over, when it is not the one it is already in. */
  const destination = () => {
    const target = moving ? containerUnder(node) : undefined;
    return target && target.node !== parentOf(node) ? target : undefined;
  };
  const move = (e: PointerEvent) => {
    set(node, a, Math.max(min, Math.round(from.a + e.clientX - from.x)));
    set(node, b, Math.max(min, Math.round(from.b + e.clientY - from.y)));
    const target = destination();
    drop.value = target ? area(target) : null;
  };
  const up = () => {
    removeEventListener("pointermove", move);
    drop.value = null;
    if (!moving) return;
    const target = destination();
    if (target) moveInto(node, target);
    // wherever it ended up, it must not be left out of reach above or left of its container
    set(node, "Left", Math.max(0, num(node, "Left")));
    set(node, "Top", Math.max(0, num(node, "Top")));
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", up, { once: true });
}

/** The title and tabs of a page frame are left uncovered, so its tabs can be clicked. */
const gripTop = (node: DesignNode) => (node.class === "PageFrame" ? pageTop(value(node, "Caption")) : 0);

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
  set(node, a, Math.max(0, num(node, a) + step[0]));
  set(node, b, Math.max(0, num(node, b) + step[1]));
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

// Selecting something on a page brings that page to the front, so it can be seen and edited.
watch(selected, (node) => {
  for (let child = node, parent = parentOf(child); parent; child = parent, parent = parentOf(parent)) {
    if (parent.class === "PageFrame") front.set(parent, parent.controls.indexOf(child) + 1);
  }
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

/** Fills in what a hand-written file may leave out, and rejects what the designer cannot draw. */
function normalize(node: Partial<DesignNode>, fallback: string): DesignNode {
  const cls = classes.value.find((c) => c.name.toUpperCase() === (node.class ?? fallback).toUpperCase());
  if (!cls || cls.name === "Custom") throw new Error(`El diseñador no conoce la clase ${node.class}`);
  const made: DesignNode = { class: cls.name, props: { Name: cls.name, ...node.props }, methods: { ...node.methods }, controls: [] };
  for (const c of node.controls ?? []) {
    const child = normalize(c, "Label");
    if (!accepts(made, child.class)) throw new Error(`Un ${made.class} no puede contener un ${child.class}`);
    made.controls.push(child);
  }
  return made;
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
const tool =
  "flex items-center gap-2 rounded border border-neutral-400 px-2 py-1 text-left hover:bg-neutral-200 disabled:opacity-40 dark:hover:bg-neutral-700";
const handle = "absolute -right-1 -bottom-1 size-2 cursor-nwse-resize bg-blue-600";
</script>

<template>
  <div class="grid h-full min-h-0 grid-cols-[12rem_1fr_17rem] text-sm">
    <nav class="flex flex-col gap-1 overflow-y-auto border-r border-neutral-300 p-2 dark:border-neutral-700" aria-label="Agregar control">
      <button
        v-for="c in classes.filter((c) => c.name !== 'Custom' && c.name !== 'Form')"
        :key="c.name"
        type="button"
        :class="tool"
        :disabled="!hostFor(c.name)"
        :title="hostFor(c.name) ? `Agregar a ${hostFor(c.name)!.props.Name}` : 'Selecciona antes el grupo o el marco de páginas que lo contendrá'"
        @click="add(c.name)"
      >
        <component :is="icons[c.name]" :size="16" class="shrink-0" aria-hidden="true" />
        {{ c.name }}
      </button>
    </nav>

    <div class="flex min-h-0 min-w-0 flex-col">
      <div class="min-h-0 flex-1 overflow-auto bg-neutral-300 p-4 select-none dark:bg-neutral-800" @pointerdown.self="selected = form">
        <!-- the corners are rounded piece by piece: clipping the whole window would cut off the resize handle -->
        <div
          class="inline-block border border-neutral-600 shadow-lg"
          :class="{ 'outline-2 outline-blue-500': selected === form }"
          :style="{ borderRadius: px(form, 'BorderRadius') }"
        >
          <div
            class="flex items-center gap-2 bg-blue-700 px-2 py-1 text-white"
            :style="{ borderRadius: `${px(form, 'BorderRadius')} ${px(form, 'BorderRadius')} 0 0` }"
            @pointerdown="selected = form"
          >
            <span class="flex-1 truncate">{{ value(form, "Caption") }}</span>
            <Minus v-if="value(form, 'MinButton')" :size="14" aria-hidden="true" />
            <Square v-if="value(form, 'MaxButton')" :size="12" aria-hidden="true" />
            <X :size="14" aria-hidden="true" />
          </div>
          <div
            class="relative text-black"
            :style="{
              width: px(form, 'Width'),
              height: px(form, 'Height'),
              backgroundColor: colorCss(num(form, 'BackColor')),
              // the design grid: a dot every 10 pixels
              backgroundImage: 'radial-gradient(circle, rgb(0 0 0 / 0.4) 1px, transparent 1px)',
              backgroundSize: '10px 10px',
              borderRadius: `0 0 ${px(form, 'BorderRadius')} ${px(form, 'BorderRadius')}`,
            }"
            @pointerdown.self="selected = form"
          >
            <!-- the only thing a drawn control reports while designing is a click on a tab: select that page -->
            <FormControl
              v-for="(c, i) in form.controls"
              :key="`view${i}`"
              :node="view(c)"
              design
              class="pointer-events-none"
              @act="(n) => n.base === 'PAGE' && (selected = n.source as DesignNode)"
            />
            <!-- a transparent grip over each control takes the pointer and the keyboard; inner controls come later, so on top -->
            <div
              v-for="(p, i) in tree.filter((p) => p.shown && p.parent && p.node.class !== 'Page')"
              :key="`grip${i}`"
              class="absolute min-h-1.5 min-w-1.5 cursor-move"
              :class="{ 'outline-2 outline-blue-500': selected === p.node }"
              :style="{
                left: `${p.x}px`,
                top: `${p.y + gripTop(p.node)}px`,
                width: px(p.node, 'Width'),
                height: `${num(p.node, 'Height') - gripTop(p.node)}px`,
              }"
              tabindex="0"
              role="button"
              :aria-label="`${p.node.props.Name}: flechas para mover, Mayús+flechas para cambiar el tamaño, Supr para eliminar`"
              @pointerdown.stop="startDrag(p.node, $event, 'move')"
              @focus="selected = p.node"
              @keydown="keyed(p.node, $event)"
            >
              <span v-if="selected === p.node" :class="handle" @pointerdown.stop="startDrag(p.node, $event, 'size')" />
            </div>
            <div
              v-if="drop"
              class="pointer-events-none absolute outline-2 outline-green-600 outline-dashed"
              :style="{ left: `${drop.x}px`, top: `${drop.y}px`, width: `${drop.w}px`, height: `${drop.h}px` }"
            />
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
        <!-- every object of the form, indented by depth: the way to reach pages and what a container hides -->
        <select v-model="selected" class="flex-1" :class="field" aria-label="Objeto seleccionado">
          <option v-for="(p, i) in tree" :key="i" :value="p.node">{{ "  ".repeat(p.depth) }}{{ p.node.props.Name }}</option>
        </select>
        <button type="button" :class="tool" :disabled="selected === form" @click="remove">Eliminar</button>
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
