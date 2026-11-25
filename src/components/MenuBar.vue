<script setup lang="ts">
// The menus a program has put on screen, as bars under the toolbar.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { menuEvent, menus } from "../engine";
import MenuItems, { Prompt } from "./MenuItems.vue";

const root = ref<HTMLElement>();
/** The pad whose popup is showing, as `id` names it. */
const open = ref<string | null>(null);
const id = (menu: string, pad: string) => `${menu}/${pad}`;

function choose(menu: string, pad: string, popup: string | null = null, bar: string | null = null) {
  open.value = null;
  menuEvent(menu, pad, popup, bar);
}

/** Escape, or a click anywhere else, closes the open popup. */
function dismiss(e: Event) {
  if (e instanceof KeyboardEvent ? e.key === "Escape" : !root.value?.contains(e.target as Node)) open.value = null;
}
onMounted(() => {
  addEventListener("pointerdown", dismiss);
  addEventListener("keydown", dismiss);
});
onBeforeUnmount(() => {
  removeEventListener("pointerdown", dismiss);
  removeEventListener("keydown", dismiss);
});
</script>

<template>
  <div ref="root">
    <nav
      v-for="menu in menus"
      :key="menu.name"
      role="menubar"
      :aria-label="`Menú ${menu.name}`"
      class="flex border-b border-neutral-300 px-1 text-sm dark:border-neutral-700"
    >
      <div v-for="pad in menu.items" :key="pad.key" class="relative">
        <button
          type="button"
          role="menuitem"
          class="rounded px-2 py-0.5 hover:bg-neutral-200 disabled:pointer-events-none disabled:opacity-40 dark:hover:bg-neutral-700"
          :class="{ 'bg-neutral-200 dark:bg-neutral-700': open === id(menu.name, pad.key) }"
          :disabled="pad.disabled"
          :aria-haspopup="pad.items ? 'menu' : undefined"
          :aria-expanded="pad.items ? open === id(menu.name, pad.key) : undefined"
          @click="pad.items ? (open = open === id(menu.name, pad.key) ? null : id(menu.name, pad.key)) : choose(menu.name, pad.key)"
          @pointerenter="open && pad.items && (open = id(menu.name, pad.key))"
        >
          <Prompt :text="pad.prompt" />
        </button>
        <MenuItems
          v-if="pad.items && open === id(menu.name, pad.key)"
          :items="pad.items"
          :popup="pad.popup!"
          class="top-full left-0"
          @choose="(popup, bar) => choose(menu.name, pad.key, popup, bar)"
        />
      </div>
    </nav>
  </div>
</template>
