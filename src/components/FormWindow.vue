<script setup lang="ts">
// The window of a top-level form (ShowWindow = 2): only the form, filling a window of its own.
import { computed, onBeforeUnmount, onMounted, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { forms, ownForm, report, uiEvent } from "../engine";
import FormLayer from "./FormLayer.vue";

const id = ownForm!;
const caption = computed(() => forms.value.find((f) => f.id === id)?.props.CAPTION);
watch(caption, (text) => text != null && getCurrentWindow().setTitle(text).catch((e) => report(String(e))));

let stopCloseGuard = () => {};
onMounted(async () => {
  // an event that does nothing: its answer brings the form to draw
  uiEvent(id, {}, null);
  // the window's own X closes the form the way its Release() would; the IDE then closes the window
  stopCloseGuard = await getCurrentWindow().onCloseRequested((event) => {
    event.preventDefault();
    uiEvent(id, {}, "Close");
  });
});
onBeforeUnmount(() => stopCloseGuard());
</script>

<template>
  <main class="relative h-screen bg-white">
    <FormLayer :only="id" />
  </main>
</template>
