<script setup lang="ts">
// The forms a program has shown, floating over the IDE. With `only`, just that form, filling a window of its own.
import { computed, nextTick, ref, watch } from "vue";
import { Copy, Minus, Square, X } from "@lucide/vue";
import { type DrawNode, focus, forms, TOP_LEVEL, uiEvent, type FormNode } from "../engine";
import FormControl, { colorCss } from "./FormControl.vue";

const props = defineProps<{ only?: number }>();
const layer = ref<HTMLElement>();
/** A top-level form is drawn by its own window, not over the IDE. */
const shown = computed(() =>
  forms.value.filter((f) => (props.only ? f.id === props.only : f.props.SHOWWINDOW !== TOP_LEVEL)),
);
/** Whether the form takes all the room there is. */
const full = (form: FormNode) => !!props.only || form.props.WINDOWSTATE === MAXIMIZED;

/** A control, at any depth, reports what the user did to it. */
function act(node: DrawNode, props: Record<string, unknown> | null, events: string[]) {
  const handled = events.filter((e) => node.events!.includes(e.toUpperCase()));
  if (!props && !handled.length) return;
  // the changed values travel with the first event, so its handler already sees them
  uiEvent(node.id!, props ?? {}, handled[0] ?? null);
  for (const event of handled.slice(1)) uiEvent(node.id!, {}, event);
}

// WindowState, as in FoxPro: 0 normal, 1 minimized, 2 maximized
const MINIMIZED = 1;
const MAXIMIZED = 2;

/** Minimizes or maximizes; asking for the state the form is already in restores it. */
function setState(form: FormNode, state: number) {
  form.props.WINDOWSTATE = form.props.WINDOWSTATE === state ? 0 : state;
  uiEvent(form.id, { WindowState: form.props.WINDOWSTATE }, null);
}

/** The form being dragged and where. It is kept apart from the form's own properties because every
 * answer from the interpreter, such as a Timer's tick, replaces those with what it knew before the drag. */
const moving = ref<{ id: number; left: number; top: number } | null>(null);
const place = (form: FormNode) =>
  moving.value?.id === form.id ? moving.value : { left: form.props.LEFT, top: form.props.TOP };

function drag(form: FormNode, down: PointerEvent) {
  if (form.props.WINDOWSTATE === MAXIMIZED) return;
  const { id } = form;
  const from = { x: down.clientX - form.props.LEFT, y: down.clientY - form.props.TOP };
  moving.value = { id, left: form.props.LEFT, top: form.props.TOP };
  const move = (e: PointerEvent) => {
    moving.value = { id, left: Math.max(0, e.clientX - from.x), top: Math.max(0, e.clientY - from.y) };
  };
  const up = async () => {
    removeEventListener("pointermove", move);
    const { left, top } = moving.value!;
    // released only once the interpreter knows the new place, so the form does not jump back meanwhile
    await uiEvent(id, { Left: left, Top: top }, null);
    if (moving.value?.id === id) moving.value = null;
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", up, { once: true });
}

// SetFocus(): move the focus once the control is on screen
watch(focus, async (id) => {
  if (id == null) return;
  focus.value = null;
  await nextTick();
  const el = layer.value?.querySelector<HTMLElement>(`[data-fost-id="${id}"]`);
  (el?.matches("label") ? el.querySelector("input") : el)?.focus();
});
</script>

<template>
  <div ref="layer" class="pointer-events-none absolute inset-0 overflow-hidden">
    <section
      v-for="form in shown"
      :key="form.id"
      class="pointer-events-auto absolute overflow-hidden"
      :class="[full(form) ? 'inset-0 flex flex-col' : '', only ? '' : 'border border-neutral-600 shadow-xl']"
      :style="
        full(form)
          ? {}
          : { left: `${place(form).left}px`, top: `${place(form).top}px`, borderRadius: `${form.props.BORDERRADIUS}px` }
      "
      role="dialog"
      :aria-label="form.props.CAPTION"
    >
      <header
        v-if="!only"
        class="flex cursor-move items-center bg-blue-700 px-2 py-1 text-sm text-white select-none"
        @pointerdown="drag(form, $event)"
      >
        <span class="flex-1 truncate">{{ form.props.CAPTION }}</span>
        <button
          v-if="form.props.MINBUTTON"
          type="button"
          class="rounded p-1 hover:bg-blue-500"
          :aria-label="form.props.WINDOWSTATE === MINIMIZED ? 'Restaurar' : 'Minimizar'"
          @pointerdown.stop
          @click="setState(form, MINIMIZED)"
        >
          <Minus :size="14" />
        </button>
        <button
          v-if="form.props.MAXBUTTON"
          type="button"
          class="rounded p-1 hover:bg-blue-500"
          :aria-label="form.props.WINDOWSTATE === MAXIMIZED ? 'Restaurar' : 'Maximizar'"
          @pointerdown.stop
          @click="setState(form, MAXIMIZED)"
        >
          <component :is="form.props.WINDOWSTATE === MAXIMIZED ? Copy : Square" :size="12" />
        </button>
        <button
          type="button"
          class="rounded p-1 hover:bg-red-600"
          aria-label="Cerrar"
          @pointerdown.stop
          @click="uiEvent(form.id, {}, 'Close')"
        >
          <X :size="14" />
        </button>
      </header>
      <div
        v-show="only || form.props.WINDOWSTATE !== MINIMIZED"
        class="relative overflow-hidden text-black"
        :class="{ 'flex-1': full(form) }"
        :style="{
          width: full(form) ? undefined : `${form.props.WIDTH}px`,
          height: full(form) ? undefined : `${form.props.HEIGHT}px`,
          backgroundColor: colorCss(form.props.BACKCOLOR),
        }"
        @click.self="act(form, null, ['Click'])"
      >
        <FormControl
          v-for="c in form.children.filter((c) => c.props.VISIBLE !== false)"
          :key="c.id"
          :node="c"
          :data-fost-id="c.id"
          @act="act"
        />
      </div>
    </section>
  </div>
</template>
