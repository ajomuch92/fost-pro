<script lang="ts">
/** FoxPro stores a colour as red + green * 256 + blue * 65536. */
export const colorCss = (c: number) => `rgb(${c & 255}, ${(c >> 8) & 255}, ${(c >> 16) & 255})`;
/** Height in pixels of a page frame's row of tabs. The designer needs it to place what is on a page. */
export const TAB_HEIGHT = 24;
const TITLE_HEIGHT = 20;
/** How far below a page frame's top its pages start: the tabs, plus the title when it has one. */
export const pageTop = (caption: unknown) => TAB_HEIGHT + (caption ? TITLE_HEIGHT : 0);
</script>

<script setup lang="ts">
// One control of a form, drawn from its properties, with the controls it contains.
// Used by running forms and, with `design`, by the designer.
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Image as ImageIcon, Timer as TimerIcon } from "@lucide/vue";
import { dir, type DrawNode, pending, report } from "../engine";

const props = defineProps<{
  node: DrawNode;
  /** Being laid out, not used: only the tabs react, and what is invisible at run time gets a stand-in. */
  design?: boolean;
  /** The control that contains this one, and this one's place in it, counting from 0. */
  owner?: DrawNode;
  index?: number;
}>();
/** The user did something to `node`: the values that changed, if any, and the events that follow. */
const emit = defineEmits<{ act: [node: DrawNode, props: Record<string, unknown> | null, events: string[]] }>();

const p = computed(() => props.node.props);
const tip = computed(() => p.value.TOOLTIPTEXT || undefined);
const act = (changed: Record<string, unknown> | null, events: string[]) => emit("act", props.node, changed, events);
/** Whether this control has its own handler for an event. */
const handles = (event: string) => !!props.node.events?.includes(event);

const box = computed(() => ({
  left: `${p.value.LEFT}px`,
  top: `${p.value.TOP}px`,
  width: `${p.value.WIDTH}px`,
  height: `${p.value.HEIGHT}px`,
}));
const fill = computed(() =>
  p.value.BACKCOLOR == null || p.value.BACKSTYLE === 0 ? undefined : colorCss(p.value.BACKCOLOR),
);
// drawn inside the box, so the border does not shift what the control contains
const border = computed(() =>
  p.value.BORDERWIDTH ? `inset 0 0 0 ${p.value.BORDERWIDTH}px ${colorCss(p.value.BORDERCOLOR)}` : undefined,
);
const style = computed(() => ({
  ...box.value,
  fontSize: p.value.FONTSIZE == null ? undefined : `${p.value.FONTSIZE}pt`,
  fontWeight: p.value.FONTBOLD ? "bold" : undefined,
  color: p.value.FORECOLOR == null ? undefined : colorCss(p.value.FORECOLOR),
  backgroundColor: fill.value,
}));

/** The contained controls to draw; hidden ones are still shown while designing. */
const shown = (nodes: DrawNode[] = []) => nodes.filter((c) => props.design || c.props.VISIBLE !== false);
const page = computed(() => props.node.children?.[(p.value.ACTIVEPAGE || 1) - 1]);

// The text being typed lives here. The value the interpreter reports replaces it only once
// nothing is on its way: an answer that left before the last keystroke must not undo it.
const text = ref("");
watch(
  [() => p.value.VALUE, pending],
  ([value, waiting]) => {
    const same = typeof value === "number" ? text.value !== "" && Number(text.value) === value : String(value ?? "") === text.value;
    if (!waiting && !same) text.value = String(value ?? "");
  },
  { immediate: true },
);

function typed(e: Event) {
  text.value = (e.target as HTMLInputElement).value;
  // a control holding a number keeps holding a number
  const value = typeof p.value.VALUE === "number" ? Number(text.value) || 0 : text.value;
  act({ Value: value }, ["InteractiveChange"]);
}

function checked(e: Event) {
  const on = (e.target as HTMLInputElement).checked;
  act({ Value: typeof p.value.VALUE === "boolean" ? on : on ? 1 : 0 }, ["Click", "InteractiveChange"]);
}

function picked(e: Event) {
  const el = e.target as HTMLSelectElement;
  // ListIndex counts from 1; the combo's blank first option already shifts it
  const shift = props.node.base === "COMBOBOX" ? 0 : 1;
  act({ Value: el.value, ListIndex: el.selectedIndex + shift }, ["InteractiveChange"]);
}

/** A button of an OptionGroup or CommandGroup: the group learns which one, then the button gets its Click. */
function pressed() {
  const group = props.owner?.base;
  if (group === "OPTIONGROUP" || group === "COMMANDGROUP") {
    const events = group === "OPTIONGROUP" ? ["InteractiveChange"] : [];
    // as in FoxPro, the group's Click runs only when the button has none of its own
    if (!handles("CLICK")) events.push("Click");
    emit("act", props.owner!, { Value: props.index! + 1 }, events);
  }
  act(null, ["Click"]);
}

function follow() {
  act(null, ["Click"]);
  if (!props.design && p.value.URL) openUrl(p.value.URL).catch((e) => report(String(e)));
}

// Image: the picture's bytes come from the interpreter's side, since a web page cannot read local files
const picture = ref("");
watch(
  () => (props.node.base === "IMAGE" ? String(p.value.PICTURE ?? "") : ""),
  async (path) => {
    URL.revokeObjectURL(picture.value);
    picture.value = "";
    if (!path) return;
    try {
      const bytes = await invoke<ArrayBuffer>("read_binary", { path, dir: dir.value });
      // SVG is the one format the browser will not recognise from its bytes alone
      const type = /\.svg$/i.test(path) ? "image/svg+xml" : "";
      picture.value = URL.createObjectURL(new Blob([bytes], { type }));
    } catch (e) {
      if (!props.design) report(String(e));
    }
  },
  { immediate: true },
);

// Timer: ticks from here; a tick is skipped while the interpreter is busy so they cannot pile up
let ticking: ReturnType<typeof setInterval> | undefined;
watch(
  () => (props.node.base === "TIMER" && !props.design && p.value.ENABLED ? Number(p.value.INTERVAL) : 0),
  (ms) => {
    clearInterval(ticking);
    if (ms > 0) ticking = setInterval(() => pending.value || act(null, ["Timer"]), ms);
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  clearInterval(ticking);
  URL.revokeObjectURL(picture.value);
});

const base = computed(() => props.node.base);
</script>

<template>
  <span
    v-if="base === 'LABEL'"
    class="absolute overflow-hidden whitespace-nowrap select-none"
    :style="style"
    :title="tip"
    @click="act(null, ['Click'])"
    >{{ p.CAPTION }}</span
  >
  <a
    v-else-if="base === 'HYPERLINK'"
    class="absolute cursor-pointer overflow-hidden whitespace-nowrap underline"
    :style="style"
    :title="tip"
    :href="p.URL || '#'"
    @click.prevent="follow"
    >{{ p.CAPTION }}</a
  >
  <input
    v-else-if="base === 'TEXTBOX' || base === 'SPINNER'"
    class="absolute border border-neutral-500 px-1 disabled:opacity-60"
    :type="base === 'SPINNER' ? 'number' : p.PASSWORDCHAR ? 'password' : 'text'"
    :min="p.SPINNERLOWVALUE"
    :max="p.SPINNERHIGHVALUE"
    :step="p.INCREMENT"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="text"
    :readonly="p.READONLY"
    :disabled="!p.ENABLED"
    @input="typed"
    @focus="act(null, ['GotFocus'])"
    @blur="act(null, ['LostFocus'])"
  />
  <textarea
    v-else-if="base === 'EDITBOX'"
    class="absolute resize-none border border-neutral-500 px-1 disabled:opacity-60"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="text"
    :readonly="p.READONLY"
    :disabled="!p.ENABLED"
    @input="typed"
    @focus="act(null, ['GotFocus'])"
    @blur="act(null, ['LostFocus'])"
  />
  <button
    v-else-if="base === 'COMMANDBUTTON'"
    type="button"
    class="absolute overflow-hidden border border-neutral-500 bg-neutral-200 whitespace-nowrap active:bg-neutral-300 disabled:opacity-60"
    :style="style"
    :title="tip"
    :disabled="!p.ENABLED"
    @click="pressed"
  >
    {{ p.CAPTION }}
  </button>
  <label
    v-else-if="base === 'CHECKBOX' || base === 'OPTIONBUTTON'"
    class="absolute flex items-center gap-1 overflow-hidden whitespace-nowrap"
    :style="style"
    :title="tip"
  >
    <input v-if="base === 'CHECKBOX'" type="checkbox" :checked="p.VALUE === true || p.VALUE === 1" :disabled="!p.ENABLED" @change="checked" />
    <input v-else type="radio" :checked="owner?.props.VALUE === index! + 1" :disabled="!p.ENABLED" @change="pressed" />
    {{ p.CAPTION }}
  </label>
  <select
    v-else-if="base === 'COMBOBOX' || base === 'LISTBOX'"
    class="absolute border border-neutral-500 disabled:opacity-60"
    :size="base === 'LISTBOX' ? 2 : undefined"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="p.VALUE"
    :disabled="!p.ENABLED"
    @change="picked"
  >
    <option v-if="base === 'COMBOBOX'" value="" />
    <option v-for="item in node.items" :key="item">{{ item }}</option>
  </select>
  <img
    v-else-if="base === 'IMAGE' && picture"
    class="absolute object-left-top"
    :class="['object-none', 'object-contain', 'object-fill'][p.STRETCH] ?? 'object-none'"
    :style="box"
    :src="picture"
    :alt="p.NAME"
    @click="act(null, ['Click'])"
  />
  <span
    v-else-if="base === 'IMAGE'"
    class="absolute grid place-items-center"
    :class="{ 'border border-dashed border-neutral-500 text-neutral-500': design }"
    :style="box"
    @click="act(null, ['Click'])"
  >
    <ImageIcon v-if="design" :size="20" aria-hidden="true" />
  </span>
  <!-- never 0 pixels wide or tall: a browser does not draw an empty svg, and a flat line is the usual one -->
  <svg
    v-else-if="base === 'LINE'"
    class="absolute overflow-visible"
    :style="{ ...box, width: `${Math.max(1, p.WIDTH)}px`, height: `${Math.max(1, p.HEIGHT)}px` }"
    aria-hidden="true"
  >
    <line
      x1="0"
      :y1="p.LINESLANT === '/' ? p.HEIGHT : 0"
      :x2="p.WIDTH"
      :y2="p.LINESLANT === '/' ? 0 : p.HEIGHT"
      :stroke="colorCss(p.BORDERCOLOR)"
      :stroke-width="p.BORDERWIDTH"
    />
  </svg>
  <div
    v-else-if="base === 'SHAPE'"
    class="absolute"
    :style="{ ...box, backgroundColor: fill, boxShadow: border, borderRadius: `${p.CURVATURE / 2}%` }"
    @click="act(null, ['Click'])"
  />
  <!-- a Timer is not seen at run time; the designer shows where it was put -->
  <span
    v-else-if="base === 'TIMER'"
    v-show="design"
    class="absolute grid place-items-center border border-neutral-500 bg-white text-neutral-600"
    :style="box"
  >
    <TimerIcon :size="16" aria-hidden="true" />
  </span>
  <div
    v-else-if="base === 'CONTAINER' || base === 'OPTIONGROUP' || base === 'COMMANDGROUP'"
    class="absolute"
    :style="{ ...box, backgroundColor: fill, boxShadow: border }"
    @click.self="act(null, ['Click'])"
  >
    <FormControl
      v-for="(c, i) in shown(node.children)"
      :key="c.id ?? i"
      :node="c"
      :owner="node"
      :index="i"
      :design="design"
      :data-fost-id="c.id"
      @act="(...args) => emit('act', ...args)"
    />
  </div>
  <div v-else-if="base === 'PAGEFRAME'" class="absolute flex flex-col" :style="box">
    <div v-if="p.CAPTION" class="shrink-0 truncate text-xs font-semibold" :style="{ height: `${TITLE_HEIGHT}px` }">
      {{ p.CAPTION }}
    </div>
    <div class="flex shrink-0" :style="{ height: `${TAB_HEIGHT}px` }" role="tablist">
      <button
        v-for="(tab, i) in node.children"
        :key="tab.id ?? i"
        type="button"
        role="tab"
        class="overflow-hidden border border-b-0 border-neutral-500 px-2 text-xs whitespace-nowrap disabled:opacity-60"
        :class="[tab === page ? 'bg-white font-semibold' : 'bg-neutral-300', { 'pointer-events-auto': design }]"
        :aria-selected="tab === page"
        :disabled="!tab.props.ENABLED"
        @click="(act({ ActivePage: i + 1 }, []), emit('act', tab, null, ['Activate']))"
      >
        {{ tab.props.CAPTION }}
      </button>
    </div>
    <!-- not clipped while designing: a control being dragged off the page must stay in sight -->
    <div
      class="relative min-h-0 flex-1 shadow-[inset_0_0_0_1px_#737373]"
      :class="{ 'overflow-hidden': !design }"
      :style="{ backgroundColor: page ? colorCss(page.props.BACKCOLOR) : undefined }"
      role="tabpanel"
    >
      <FormControl
        v-for="(c, i) in shown(page?.children)"
        :key="c.id ?? i"
        :node="c"
        :owner="page"
        :index="i"
        :design="design"
        :data-fost-id="c.id"
        @act="(...args) => emit('act', ...args)"
      />
    </div>
  </div>
</template>
