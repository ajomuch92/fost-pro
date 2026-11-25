<script setup lang="ts">
// The forms a program has shown, floating over the IDE.
import { nextTick, ref, watch } from "vue";
import { Copy, Minus, Square, X } from "@lucide/vue";
import { focus, forms, uiEvent, type FormNode } from "../engine";
import FormControl, { colorCss } from "./FormControl.vue";

const layer = ref<HTMLElement>();

function act(node: FormNode, props: Record<string, unknown> | null, events: string[]) {
  const handled = events.filter((e) => node.events.includes(e.toUpperCase()));
  if (!props && !handled.length) return;
  // the changed values travel with the first event, so its handler already sees them
  uiEvent(node.id, props ?? {}, handled[0] ?? null);
  for (const event of handled.slice(1)) uiEvent(node.id, {}, event);
}

// WindowState, as in FoxPro: 0 normal, 1 minimized, 2 maximized
const MINIMIZED = 1;
const MAXIMIZED = 2;

/** Minimizes or maximizes; asking for the state the form is already in restores it. */
function setState(form: FormNode, state: number) {
  form.props.WINDOWSTATE = form.props.WINDOWSTATE === state ? 0 : state;
  uiEvent(form.id, { WindowState: form.props.WINDOWSTATE }, null);
}

function drag(form: FormNode, down: PointerEvent) {
  if (form.props.WINDOWSTATE === MAXIMIZED) return;
  const from = { x: down.clientX - form.props.LEFT, y: down.clientY - form.props.TOP };
  const move = (e: PointerEvent) => {
    form.props.LEFT = Math.max(0, e.clientX - from.x);
    form.props.TOP = Math.max(0, e.clientY - from.y);
  };
  const up = () => {
    removeEventListener("pointermove", move);
    uiEvent(form.id, { Left: form.props.LEFT, Top: form.props.TOP }, null);
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
      v-for="form in forms"
      :key="form.id"
      class="pointer-events-auto absolute border border-neutral-600 shadow-xl"
      :class="{ 'inset-0 flex flex-col': form.props.WINDOWSTATE === MAXIMIZED }"
      :style="form.props.WINDOWSTATE === MAXIMIZED ? {} : { left: `${form.props.LEFT}px`, top: `${form.props.TOP}px` }"
      role="dialog"
      :aria-label="form.props.CAPTION"
    >
      <header
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
        v-show="form.props.WINDOWSTATE !== MINIMIZED"
        class="relative overflow-hidden text-black"
        :class="{ 'flex-1': form.props.WINDOWSTATE === MAXIMIZED }"
        :style="{
          width: form.props.WINDOWSTATE === MAXIMIZED ? undefined : `${form.props.WIDTH}px`,
          height: form.props.WINDOWSTATE === MAXIMIZED ? undefined : `${form.props.HEIGHT}px`,
          backgroundColor: colorCss(form.props.BACKCOLOR),
        }"
        @click.self="act(form, null, ['Click'])"
      >
        <FormControl
          v-for="c in form.children.filter((c) => c.props.VISIBLE !== false)"
          :key="c.id"
          :node="c"
          :data-fost-id="c.id"
          @act="(props, events) => act(c, props, events)"
        />
      </div>
    </section>
  </div>
</template>
