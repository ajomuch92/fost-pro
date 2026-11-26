<script setup lang="ts">
// Everything the language supports, searchable. The lists come from the interpreter itself.
import { computed, nextTick, ref, watch } from "vue";
import { X } from "@lucide/vue";
import { classes, helpOpen, reference } from "../engine";
import { t } from "../i18n";

type Entry = (typeof reference.value.commands)[number];

const query = ref("");
const search = ref<HTMLInputElement>();
const has = (text: string) => text.toLowerCase().includes(query.value.trim().toLowerCase());
const matching = (entries: Entry[]) => entries.filter((e) => has(`${e.syntax} ${e.text}`));

/** Sections of entries: the commands, the functions by group, and what objects answer to. */
const sections = computed(() => {
  const functions = matching(reference.value.functions);
  const groups = [...new Set(functions.map((f) => f.group!))];
  return [
    { title: t("Comandos"), entries: matching(reference.value.commands) },
    ...groups.map((g) => ({ title: t("Funciones: {0}", g), entries: functions.filter((f) => f.group === g) })),
    { title: t("Objetos"), entries: matching(reference.value.members) },
  ].filter((s) => s.entries.length);
});
const shownClasses = computed(() =>
  classes.value.filter((c) => has([c.name, ...c.props.map((p) => p[0]), ...c.events].join(" "))),
);

watch(helpOpen, async (open) => {
  if (!open) return;
  await nextTick();
  search.value?.focus();
});
</script>

<template>
  <section
    v-if="helpOpen"
    class="absolute inset-0 z-10 flex flex-col bg-white text-sm dark:bg-neutral-900"
    role="dialog"
    :aria-label="t('Ayuda')"
    @keydown.esc="helpOpen = false"
  >
    <header class="flex items-center gap-3 border-b border-neutral-300 p-3 dark:border-neutral-700">
      <h1 class="text-base font-semibold">{{ t("Ayuda de Fost Pro") }}</h1>
      <input
        ref="search"
        v-model="query"
        type="search"
        class="min-w-0 flex-1 rounded border border-neutral-400 bg-transparent px-2 py-1"
        :placeholder="t('Buscar un comando, función o propiedad')"
        :aria-label="t('Buscar en la ayuda')"
      />
      <button type="button" class="rounded p-1 hover:bg-neutral-200 dark:hover:bg-neutral-700" :aria-label="t('Cerrar la ayuda')" @click="helpOpen = false">
        <X :size="18" />
      </button>
    </header>
    <div class="overflow-y-auto p-4">
      <p v-if="!sections.length && !shownClasses.length" class="text-neutral-500">{{ t('Nada coincide con "{0}".', query) }}</p>
      <template v-for="s in sections" :key="s.title">
        <h2 class="mt-4 mb-1 font-semibold first:mt-0">{{ s.title }}</h2>
        <dl class="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-x-4 gap-y-1">
          <template v-for="e in s.entries" :key="e.syntax">
            <dt class="font-mono break-words">{{ e.syntax }}</dt>
            <dd>{{ e.text }}</dd>
          </template>
        </dl>
      </template>
      <template v-if="shownClasses.length">
        <h2 class="mt-4 mb-1 font-semibold">{{ t("Clases") }}</h2>
        <dl class="grid grid-cols-[minmax(0,1fr)_minmax(0,4fr)] gap-x-4 gap-y-2">
          <template v-for="c in shownClasses" :key="c.name">
            <dt class="font-mono">{{ c.name }}</dt>
            <dd>
              <template v-if="c.props.length">{{ t("Propiedades: {0}.", c.props.map((p) => p[0]).join(", ")) }}<br /></template>
              {{ t("Eventos: {0}.", c.events.join(", ")) }}
            </dd>
          </template>
        </dl>
      </template>
    </div>
  </section>
</template>
