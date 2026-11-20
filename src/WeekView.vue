<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekRow, { type StoredEntry } from "./components/WeekRow.vue";
import { currentWeek } from "./week";

const days = currentWeek();
const entries = ref<Record<string, StoredEntry>>({});
const loaded = ref(false);
const loadError = ref("");

onMounted(async () => {
  try {
    const rows = await invoke<Array<StoredEntry & { date: string }>>("load_entries", {
      from: days[0],
      to: days[6],
    });
    entries.value = Object.fromEntries(rows.map((row) => [row.date, row]));
  } catch (e) {
    loadError.value = String(e);
    return;
  }
  loaded.value = true;
});
</script>

<template>
  <main>
    <h1>Overtime – aktuelle Woche</h1>
    <p v-if="loadError" class="error" data-test="load-error">{{ loadError }}</p>
    <template v-if="loaded">
      <WeekRow v-for="day in days" :key="day" :date="day" :entry="entries[day]" />
    </template>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
.error { color: #b00020; }
</style>
