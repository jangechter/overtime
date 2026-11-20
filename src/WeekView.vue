<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekRow, { type StoredEntry } from "./components/WeekRow.vue";
import { currentWeek, formatHours, shiftWeek } from "./week";

const days = ref(currentWeek());
const entries = ref<Record<string, StoredEntry>>({});
const holidays = ref<Record<string, string>>({});
/** Ist-Summe der angezeigten Woche in Minuten, berechnet im Backend. */
const weekTotal = ref(0);
const loaded = ref(false);
const loadError = ref("");
const saveError = ref("");
const holidayWarning = ref("");
/** Für die angezeigte Woche wurden nie Feiertage geladen (Backend: Zeitraum nicht in der Tabelle der geladenen). */
const holidaysMissing = ref(false);
const weekFetchError = ref("");
/** Wochen (Montag), deren Feiertage gerade per Knopf nachgeladen werden. */
const fetching = ref(new Set<string>());
let latestRequest = 0;
let latestTotalRequest = 0;

const formatDate = (date: string) => new Date(`${date}T00:00:00`).toLocaleDateString("de-DE");
const weekLabel = computed(() => `${formatDate(days.value[0])} – ${formatDate(days.value[6])}`);

interface WeekHolidays {
  holidays: Array<{ date: string; name: string }>;
  missing: boolean;
}

/** Gespeicherte Feiertage der angezeigten Woche und ob für sie nie geladen wurde. */
function fetchHolidays(): Promise<WeekHolidays> {
  return invoke<WeekHolidays>("load_holidays", { from: days.value[0], to: days.value[6] });
}

function showHolidays(week: WeekHolidays) {
  holidays.value = Object.fromEntries(week.holidays.map((holiday) => [holiday.date, holiday.name]));
  holidaysMissing.value = week.missing;
}

/** Liest die Feiertage der angezeigten Woche neu, ohne die Zeilen anzufassen. */
async function reloadHolidays() {
  const weekStart = days.value[0];
  try {
    const week = await fetchHolidays();
    if (days.value[0] === weekStart) showHolidays(week);
  } catch (e) {
    loadError.value = String(e);
  }
}

/** Liest die Wochensumme neu (nach Speichern oder Löschen einer Zeile), ohne die Zeilen anzufassen. */
async function reloadTotal() {
  const request = ++latestTotalRequest;
  const weekStart = days.value[0];
  try {
    const minutes = await fetchTotal();
    if (request === latestTotalRequest && days.value[0] === weekStart) weekTotal.value = minutes;
  } catch (e) {
    loadError.value = String(e);
  }
}

function fetchTotal(): Promise<number> {
  return invoke<number>("week_total", { from: days.value[0], to: days.value[6] });
}

async function loadWeek() {
  const request = ++latestRequest;
  loaded.value = false;
  loadError.value = "";
  saveError.value = "";
  weekFetchError.value = "";
  holidaysMissing.value = false;
  try {
    const [rows, week, minutes] = await Promise.all([
      invoke<Array<StoredEntry & { date: string }>>("load_entries", {
        from: days.value[0],
        to: days.value[6],
      }),
      fetchHolidays(),
      fetchTotal(),
    ]);
    if (request !== latestRequest) return;
    entries.value = Object.fromEntries(rows.map((row) => [row.date, row]));
    showHolidays(week);
    latestTotalRequest++;
    weekTotal.value = minutes;
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
  await reloadHolidays();
}

/** Knopf „Feiertage laden“: holt genau die angezeigte Woche. Ohne Netz bleibt die Warnung, es erscheint eine Meldung. */
async function loadMissingHolidays() {
  const weekStart = days.value[0];
  if (fetching.value.has(weekStart)) return;
  fetching.value.add(weekStart);
  weekFetchError.value = "";
  try {
    await invoke("fetch_week_holidays", { date: weekStart });
  } catch (e) {
    if (days.value[0] === weekStart) weekFetchError.value = String(e);
    return;
  } finally {
    fetching.value.delete(weekStart);
  }
  await reloadHolidays();
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
    <p v-if="holidayWarning && holidayWarning !== loadError" class="warning" data-test="holiday-warning">{{ holidayWarning }}</p>
    <p v-if="holidaysMissing" class="warning" data-test="holidays-missing">
      Feiertage für diese Woche fehlen.
      <button type="button" data-test="load-holidays" :disabled="fetching.has(days[0])" @click="loadMissingHolidays">
        Feiertage laden
      </button>
    </p>
    <p v-if="weekFetchError" class="error" data-test="holidays-load-error">{{ weekFetchError }}</p>
    <template v-if="loaded">
      <WeekRow
        v-for="day in days"
        :key="day"
        :date="day"
        :entry="entries[day]"
        :holiday="holidays[day]"
        :on-save-failed="reportSaveFailure"
        @saved="reloadTotal"
      />
      <p class="total" data-test="week-total">Wochensumme: {{ formatHours(weekTotal) }}</p>
    </template>
    <footer class="source" data-test="holiday-source">Feiertage: OpenHolidays API (ODbL)</footer>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
.week-nav { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
.error { color: #b00020; }
.warning { color: #8a5a00; }
.total { margin-top: 1rem; font-weight: 600; }
.source { margin-top: 1rem; font-size: 0.8rem; color: #666; }
</style>
