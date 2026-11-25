<script lang="ts">
import { h } from "vue";

/** A prompt as FoxPro writes it: `\<` marks the letter to underline. */
export function Prompt(props: { text: string }) {
  const at = props.text.indexOf("\\<");
  if (at < 0 || at + 2 >= props.text.length) return props.text;
  return [props.text.slice(0, at), h("u", props.text[at + 2]), props.text.slice(at + 3)];
}
Prompt.props = ["text"];

/** The prompt that draws a line instead of an option. */
const SEPARATOR = "\\-";
</script>

<script setup lang="ts">
// The bars of a popup; a bar that opens another popup shows it beside itself.
import { ChevronRight } from "@lucide/vue";
import type { MenuItem } from "../engine";

defineProps<{ items: MenuItem[]; popup: string }>();
const emit = defineEmits<{ choose: [popup: string, bar: string] }>();
</script>

<template>
  <ul
    role="menu"
    class="absolute z-50 min-w-40 rounded border border-neutral-400 bg-white py-1 text-neutral-900 shadow-lg dark:border-neutral-600 dark:bg-neutral-800 dark:text-neutral-100"
  >
    <template v-for="item in items" :key="item.key">
      <li v-if="item.prompt === SEPARATOR" role="separator" class="my-1 border-t border-neutral-300 dark:border-neutral-600" />
      <!-- only its own popup opens, not the ones further down -->
      <li v-else role="none" class="relative [&:focus-within>ul]:block [&:hover>ul]:block">
        <button
          type="button"
          role="menuitem"
          class="flex w-full items-center justify-between gap-4 px-3 py-1 text-left whitespace-nowrap hover:bg-blue-600 hover:text-white focus-visible:bg-blue-600 focus-visible:text-white disabled:pointer-events-none disabled:opacity-40"
          :disabled="item.disabled"
          :aria-haspopup="item.items ? 'menu' : undefined"
          @click="item.items || emit('choose', popup, item.key)"
        >
          <span><Prompt :text="item.prompt" /></span>
          <ChevronRight v-if="item.items" :size="14" aria-hidden="true" />
        </button>
        <MenuItems
          v-if="item.items && !item.disabled"
          :items="item.items"
          :popup="item.popup!"
          class="top-0 left-full hidden"
          @choose="(p, b) => emit('choose', p, b)"
        />
      </li>
    </template>
  </ul>
</template>
