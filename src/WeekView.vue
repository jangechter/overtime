<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekRow, { type StoredEntry } from "./components/WeekRow.vue";
import { currentWeek, shiftWeek } from "./week";

const days = ref(currentWeek());
const entries = ref<Record<string, StoredEntry>>({});
const holidays = ref<Record<string, string>>({});
const loaded = ref(false);
const loadError = ref("");
const saveError = ref("");
const holidayWarning = ref("");
let latestRequest = 0;

const formatDate = (date: string) => new Date(`${date}T00:00:00`).toLocaleDateString("de-DE");
const weekLabel = computed(() => `${formatDate(days.value[0])} – ${formatDate(days.value[6])}`);

/** Gespeicherte Feiertage der angezeigten Woche, nach Datum. */
async function fetchHolidays(): Promise<Record<string, string>> {
  const rows = await invoke<Array<{ date: string; name: string }>>("load_holidays", {
    from: days.value[0],
    to: days.value[6],
  });
  return Object.fromEntries(rows.map((row) => [row.date, row.name]));
}

async function loadWeek() {
  const request = ++latestRequest;
  loaded.value = false;
  loadError.value = "";
  saveError.value = "";
  try {
    const [rows, weekHolidays] = await Promise.all([
      invoke<Array<StoredEntry & { date: string }>>("load_entries", {
        from: days.value[0],
        to: days.value[6],
      }),
      fetchHolidays(),
    ]);
    if (request !== latestRequest) return;
    entries.value = Object.fromEntries(rows.map((row) => [row.date, row]));
    holidays.value = weekHolidays;
  } catch (e) {
    if (request !== latestRequest) return;
    loadError.value = String(e);
    return;
  }
  loaded.value = true;
}

/** Holt beim Start die Feiertage vom Netz. Die Zeilen bleiben unberührt, nur die Markierung wird aufgefrischt. */
async function refreshHolidays() {
  try {
    await invoke("refresh_holidays");
  } catch (e) {
    holidayWarning.value = String(e);
    return;
  }
  const weekStart = days.value[0];
  try {
    const weekHolidays = await fetchHolidays();
    if (days.value[0] === weekStart) holidays.value = weekHolidays;
  } catch (e) {
    loadError.value = String(e);
  }
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

onMounted(() => {
  loadWeek();
  refreshHolidays();
});
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
    <p v-if="holidayWarning" class="warning" data-test="holiday-warning">{{ holidayWarning }}</p>
    <template v-if="loaded">
      <WeekRow
        v-for="day in days"
        :key="day"
        :date="day"
        :entry="entries[day]"
        :holiday="holidays[day]"
        :on-save-failed="reportSaveFailure"
      />
    </template>
    <footer class="source" data-test="holiday-source">Feiertage: OpenHolidays API (ODbL)</footer>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
.week-nav { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
.error { color: #b00020; }
.warning { color: #8a5a00; }
.source { margin-top: 1rem; font-size: 0.8rem; color: #666; }
</style>
