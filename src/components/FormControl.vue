<script lang="ts">
/** FoxPro stores a colour as red + green * 256 + blue * 65536. */
export const colorCss = (c: number) => `rgb(${c & 255}, ${(c >> 8) & 255}, ${(c >> 16) & 255})`;
</script>

<script setup lang="ts">
// One control of a form, drawn from its properties. Used by running forms and by the designer.
import { computed, ref, watch } from "vue";
import { pending, type FormNode } from "../engine";

const props = defineProps<{ node: Pick<FormNode, "base" | "props" | "items"> }>();
/** The user did something: the values that changed, if any, and the events that follow. */
const emit = defineEmits<{ act: [props: Record<string, unknown> | null, events: string[]] }>();

const p = computed(() => props.node.props);
const tip = computed(() => p.value.TOOLTIPTEXT || undefined);
const style = computed(() => ({
  left: `${p.value.LEFT}px`,
  top: `${p.value.TOP}px`,
  width: `${p.value.WIDTH}px`,
  height: `${p.value.HEIGHT}px`,
  fontSize: `${p.value.FONTSIZE}pt`,
  fontWeight: p.value.FONTBOLD ? "bold" : "normal",
  color: colorCss(p.value.FORECOLOR),
  backgroundColor: p.value.BACKCOLOR == null ? undefined : colorCss(p.value.BACKCOLOR),
}));

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
  // a text box holding a number keeps holding a number
  const value = typeof p.value.VALUE === "number" ? Number(text.value) || 0 : text.value;
  emit("act", { Value: value }, ["InteractiveChange"]);
}

function checked(e: Event) {
  const on = (e.target as HTMLInputElement).checked;
  emit("act", { Value: typeof p.value.VALUE === "boolean" ? on : on ? 1 : 0 }, ["Click", "InteractiveChange"]);
}

function picked(e: Event) {
  const el = e.target as HTMLSelectElement;
  // ListIndex counts from 1; the combo's blank first option already shifts it
  const shift = props.node.base === "COMBOBOX" ? 0 : 1;
  emit("act", { Value: el.value, ListIndex: el.selectedIndex + shift }, ["InteractiveChange"]);
}
</script>

<template>
  <span
    v-if="node.base === 'LABEL'"
    class="absolute overflow-hidden whitespace-nowrap select-none"
    :style="style"
    :title="tip"
    @click="emit('act', null, ['Click'])"
    >{{ p.CAPTION }}</span
  >
  <input
    v-else-if="node.base === 'TEXTBOX'"
    class="absolute border border-neutral-500 px-1 disabled:opacity-60"
    :type="p.PASSWORDCHAR ? 'password' : 'text'"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="text"
    :readonly="p.READONLY"
    :disabled="!p.ENABLED"
    @input="typed"
    @focus="emit('act', null, ['GotFocus'])"
    @blur="emit('act', null, ['LostFocus'])"
  />
  <textarea
    v-else-if="node.base === 'EDITBOX'"
    class="absolute resize-none border border-neutral-500 px-1 disabled:opacity-60"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="text"
    :readonly="p.READONLY"
    :disabled="!p.ENABLED"
    @input="typed"
    @focus="emit('act', null, ['GotFocus'])"
    @blur="emit('act', null, ['LostFocus'])"
  />
  <button
    v-else-if="node.base === 'COMMANDBUTTON'"
    type="button"
    class="absolute overflow-hidden border border-neutral-500 bg-neutral-200 whitespace-nowrap active:bg-neutral-300 disabled:opacity-60"
    :style="style"
    :title="tip"
    :disabled="!p.ENABLED"
    @click="emit('act', null, ['Click'])"
  >
    {{ p.CAPTION }}
  </button>
  <label
    v-else-if="node.base === 'CHECKBOX'"
    class="absolute flex items-center gap-1 overflow-hidden whitespace-nowrap"
    :style="style"
    :title="tip"
  >
    <input type="checkbox" :checked="p.VALUE === true || p.VALUE === 1" :disabled="!p.ENABLED" @change="checked" />
    {{ p.CAPTION }}
  </label>
  <select
    v-else-if="node.base === 'COMBOBOX' || node.base === 'LISTBOX'"
    class="absolute border border-neutral-500 disabled:opacity-60"
    :size="node.base === 'LISTBOX' ? 2 : undefined"
    :style="style"
    :title="tip"
    :aria-label="p.NAME"
    :value="p.VALUE"
    :disabled="!p.ENABLED"
    @change="picked"
  >
    <option v-if="node.base === 'COMBOBOX'" value="" />
    <option v-for="item in node.items" :key="item">{{ item }}</option>
  </select>
</template>
