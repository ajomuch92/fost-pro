<script setup lang="ts">
// The question of an INPUTBOX() a program is waiting on.
import { nextTick, ref, watch } from "vue";
import { answerPrompt, question } from "../engine";

const text = ref("");
const input = ref<HTMLInputElement>();

watch(question, async (q) => {
  if (!q) return;
  text.value = q.default;
  await nextTick();
  input.value?.select();
});

const button = "rounded border border-neutral-400 px-3 py-1 hover:bg-neutral-200 dark:hover:bg-neutral-700";
</script>

<template>
  <div v-if="question" class="absolute inset-0 z-20 grid place-items-center bg-black/40" @keydown.esc="answerPrompt(null)">
    <form
      class="w-96 max-w-[90%] rounded border border-neutral-500 bg-white p-4 text-sm shadow-xl dark:bg-neutral-800"
      role="dialog"
      aria-modal="true"
      :aria-label="question.title || 'Fost Pro'"
      @submit.prevent="answerPrompt(text)"
    >
      <h2 class="font-semibold">{{ question.title || "Fost Pro" }}</h2>
      <label class="mt-2 block">
        {{ question.prompt }}
        <input ref="input" v-model="text" class="mt-1 w-full rounded border border-neutral-400 bg-transparent px-2 py-1" />
      </label>
      <div class="mt-3 flex justify-end gap-2">
        <button type="button" :class="button" @click="answerPrompt(null)">Cancelar</button>
        <button type="submit" :class="button">Aceptar</button>
      </div>
    </form>
  </div>
</template>
