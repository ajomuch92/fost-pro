<script setup lang="ts">
import { t } from "../i18n";
// The bar between two areas. Dragging it resizes the area it belongs to, and dragging far enough
// closes it; a double click closes the area or brings it back.
const CLOSE_BELOW = 60;
/** The area's width or height in pixels; 0 is closed. */
const size = defineModel<number>({ required: true });
const props = defineProps<{
  label: string;
  /** The bar is horizontal and changes a height. */
  vertical?: boolean;
  /** The area is after the bar (to its right, or below it), so it grows as the pointer goes back. */
  after?: boolean;
}>();

// what a closed area opens to again
let open = size.value || 220;
function set(to: number) {
  const room = (props.vertical ? innerHeight : innerWidth) - 120;
  // kept in a variable: the model only shows its new value once the parent has taken it
  const next = to < CLOSE_BELOW ? 0 : Math.round(Math.min(to, room));
  if (next) open = next;
  size.value = next;
  return next;
}
const toggle = () => (size.value = size.value ? 0 : open);

function drag(down: PointerEvent) {
  const at = (e: PointerEvent) => (props.vertical ? e.clientY : e.clientX);
  const [from, start] = [at(down), size.value];
  const move = (e: PointerEvent) => {
    // dragged shut, it opens again as it was before the drag, not as it was on the way
    if (!set(start + (at(e) - from) * (props.after ? -1 : 1)) && start) open = start;
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", () => removeEventListener("pointermove", move), { once: true });
}

/** The arrows that point along the bar's travel resize; Enter closes or opens. */
function keyed(e: KeyboardEvent) {
  const keys = props.vertical ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
  const way = keys.indexOf(e.key);
  if (way < 0 && e.key !== "Enter") return;
  e.preventDefault();
  if (e.key === "Enter") toggle();
  else set((size.value || 0) + (way ? 16 : -16) * (props.after ? -1 : 1));
}
</script>

<template>
  <div
    role="separator"
    tabindex="0"
    class="shrink-0 bg-neutral-300 hover:bg-blue-500 focus-visible:bg-blue-500 focus-visible:outline-none dark:bg-neutral-700"
    :class="vertical ? 'h-1 cursor-row-resize' : 'w-1 cursor-col-resize'"
    :aria-label="label"
    :aria-orientation="vertical ? 'horizontal' : 'vertical'"
    :aria-valuenow="size"
    :title="t(size ? '{0}: arrastra para ajustar; doble clic para cerrar' : '{0}: arrastra para ajustar; doble clic para abrir', label)"
    @pointerdown.prevent="drag"
    @dblclick="toggle"
    @keydown="keyed"
  />
</template>
