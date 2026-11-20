<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekRow, { type StoredEntry } from "./components/WeekRow.vue";
import { currentWeek, shiftWeek } from "./week";

const days = ref(currentWeek());
const entries = ref<Record<string, StoredEntry>>({});
const loaded = ref(false);
const loadError = ref("");
const saveError = ref("");
let latestRequest = 0;

const formatDate = (date: string) => new Date(`${date}T00:00:00`).toLocaleDateString("de-DE");
const weekLabel = computed(() => `${formatDate(days.value[0])} – ${formatDate(days.value[6])}`);

async function loadWeek() {
  const request = ++latestRequest;
  loaded.value = false;
  loadError.value = "";
  saveError.value = "";
  try {
    const rows = await invoke<Array<StoredEntry & { date: string }>>("load_entries", {
      from: days.value[0],
      to: days.value[6],
    });
    if (request !== latestRequest) return;
    entries.value = Object.fromEntries(rows.map((row) => [row.date, row]));
  } catch (e) {
    if (request !== latestRequest) return;
    loadError.value = String(e);
    return;
  }
  loaded.value = true;
}

function reportSaveFailure(date: string, message: string) {
  saveError.value = `Speichern für ${formatDate(date)} fehlgeschlagen: ${message}`;
}

function goToWeek(offset: number) {
  days.value = shiftWeek(days.value, offset);
  loadWeek();
}

function goToToday() {
  const thisWeek = currentWeek();
  if (thisWeek[0] === days.value[0]) return;
  days.value = thisWeek;
  loadWeek();
}

onMounted(loadWeek);
</script>

<template>
  <main>
    <h1>Overtime</h1>
    <nav class="week-nav">
      <button type="button" data-test="prev" @click="goToWeek(-1)">← Vorherige Woche</button>
      <span data-test="week-label">{{ weekLabel }}</span>
      <button type="button" data-test="today" @click="goToToday">Heute</button>
      <button type="button" data-test="next" @click="goToWeek(1)">Nächste Woche →</button>
    </nav>
    <p v-if="loadError" class="error" data-test="load-error">{{ loadError }}</p>
    <p v-if="saveError" class="error" data-test="save-error">{{ saveError }}</p>
    <template v-if="loaded">
      <WeekRow v-for="day in days" :key="day" :date="day" :entry="entries[day]" :on-save-failed="reportSaveFailure" />
    </template>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
.week-nav { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
.error { color: #b00020; }
</style>
