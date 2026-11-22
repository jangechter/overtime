<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekRow, { type StoredEntry } from "./components/WeekRow.vue";
import { currentWeek, formatBalance, formatDecimalBalance, formatDecimalHours, formatHours, shiftWeek } from "./week";

const days = ref(currentWeek());
const entries = ref<Record<string, StoredEntry>>({});
const holidays = ref<Record<string, string>>({});
interface WeekSummary {
  actualMinutes: number;
  targetMinutes: number;
  balanceMinutes: number;
  /** Für die Woche fehlen Feiertage, das Soll ist deshalb nicht um Feiertage gesenkt. */
  targetIncomplete: boolean;
}

/** Ist, Soll und Saldo der angezeigten Woche in Minuten, berechnet im Backend. */
const summary = ref<WeekSummary>({ actualMinutes: 0, targetMinutes: 0, balanceMinutes: 0, targetIncomplete: false });
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
let latestSummaryRequest = 0;

const props = withDefaults(
  defineProps<{
    /** Ändert sich, wenn Einstellungen gespeichert wurden; Soll und Krank/Urlaub der angezeigten Woche können sich ändern. */
    settingsVersion?: number;
  }>(),
  { settingsVersion: 0 },
);

const emit = defineEmits<{
  /** Gespeicherte Feiertage haben sich geändert (Abruf beim Start oder Knopf „Feiertage laden“). */
  holidaysChanged: [];
}>();

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

type DayEntry = StoredEntry & { date: string };

function fetchEntries(): Promise<DayEntry[]> {
  return invoke<DayEntry[]>("load_entries", { from: days.value[0], to: days.value[6] });
}

function showEntries(rows: DayEntry[]) {
  entries.value = Object.fromEntries(rows.map((row) => [row.date, row]));
}

/** Liest Feiertage und Einträge der angezeigten Woche neu, ohne die Zeilen neu aufzubauen. Die Einträge werden
 *  mitgelesen, weil das Backend Krank/Urlaub an einem Feiertag mit 0 h anrechnet. */
async function reloadHolidays() {
  const weekStart = days.value[0];
  try {
    const [week, rows] = await Promise.all([fetchHolidays(), fetchEntries()]);
    if (days.value[0] === weekStart) {
      showHolidays(week);
      showEntries(rows);
    }
  } catch (e) {
    loadError.value = String(e);
    return;
  }
  await reloadSummary();
}

/** Liest Ist, Soll und Saldo neu (nach Speichern, Löschen oder neuen Feiertagen), ohne die Zeilen anzufassen. */
async function reloadSummary() {
  const request = ++latestSummaryRequest;
  const weekStart = days.value[0];
  try {
    const week = await fetchSummary();
    if (request === latestSummaryRequest && days.value[0] === weekStart) summary.value = week;
  } catch (e) {
    loadError.value = String(e);
  }
}

function fetchSummary(): Promise<WeekSummary> {
  return invoke<WeekSummary>("week_summary", { date: days.value[0] });
}

async function loadWeek() {
  const request = ++latestRequest;
  loaded.value = false;
  loadError.value = "";
  saveError.value = "";
  weekFetchError.value = "";
  holidaysMissing.value = false;
  try {
    const [rows, week, weekSummary] = await Promise.all([fetchEntries(), fetchHolidays(), fetchSummary()]);
    if (request !== latestRequest) return;
    showEntries(rows);
    showHolidays(week);
    latestSummaryRequest++;
    summary.value = weekSummary;
  } catch (e) {
    if (request !== latestRequest) return;
    loadError.value = String(e);
    return;
  }
  loaded.value = true;
}

/** Ruft das Feiertagsfenster vom Netz ab; `true`, wenn neue Feiertage gespeichert wurden. */
async function fetchHolidayWindow(): Promise<boolean> {
  try {
    await invoke("refresh_holidays");
  } catch (e) {
    holidayWarning.value = String(e);
    return false;
  }
  holidayWarning.value = "";
  emit("holidaysChanged");
  return true;
}

/** Holt beim Start die Feiertage vom Netz. Die Zeilen bleiben unberührt, nur die Markierung wird aufgefrischt. */
async function refreshHolidays() {
  if (await fetchHolidayWindow()) await reloadHolidays();
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
  emit("holidaysChanged");
  await reloadHolidays();
}

// Neue Einstellungen ändern Soll und die Anrechnung von Krank/Urlaub. Ein Wechsel des Bundeslands verwirft die
// Feiertage ab dem Montag, ab dem er gilt; deshalb gleich neu abrufen. Neu gelesen wird auch ohne Netz.
watch(
  () => props.settingsVersion,
  async () => {
    await fetchHolidayWindow();
    await reloadHolidays();
  },
);

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
  <section>
    <nav class="period-nav" aria-label="Woche wählen">
      <span class="period-label" data-test="week-label">{{ weekLabel }}</span>
      <div class="period-buttons">
        <button type="button" data-test="prev" @click="goToWeek(-1)">← Vorherige Woche</button>
        <button type="button" data-test="today" @click="goToToday">Heute</button>
        <button type="button" data-test="next" @click="goToWeek(1)">Nächste Woche →</button>
      </div>
    </nav>
    <p v-if="loadError" class="notice error" role="alert" data-test="load-error">{{ loadError }}</p>
    <p v-if="saveError" class="notice error" data-test="save-error">{{ saveError }}</p>
    <p v-if="holidayWarning && holidayWarning !== loadError" class="notice warning" data-test="holiday-warning">
      {{ holidayWarning }}
    </p>
    <p v-if="holidaysMissing" class="notice warning" data-test="holidays-missing">
      Feiertage für diese Woche fehlen.
      <button type="button" data-test="load-holidays" :disabled="fetching.has(days[0])" @click="loadMissingHolidays">
        Feiertage laden
      </button>
    </p>
    <p v-if="weekFetchError" class="notice error" data-test="holidays-load-error">{{ weekFetchError }}</p>
    <template v-if="loaded">
      <div class="days card">
        <WeekRow
          v-for="day in days"
          :key="day"
          :date="day"
          :entry="entries[day]"
          :holiday="holidays[day]"
          :on-save-failed="reportSaveFailure"
          @saved="reloadSummary"
        />
      </div>
      <section class="summary">
        <!-- Leerzeichen zwischen Beschriftung und Wert auf derselben Zeile, das vor den Dezimalstunden in ihrer Spanne:
             Zeilenumbrüche zwischen Elementen entfernt Vue, der Text bliebe sonst ohne Leerzeichen (Screenreader). -->
        <p class="tile card" data-test="week-total">
          <span class="tile-label">Wochensumme</span> <span class="tile-value">
            {{ formatHours(summary.actualMinutes) }}<span class="decimal"> ({{ formatDecimalHours(summary.actualMinutes) }})</span>
          </span>
        </p>
        <p class="tile card" data-test="week-target">
          <span class="tile-label">Soll</span> <span class="tile-value">
            {{ formatHours(summary.targetMinutes) }}<span class="decimal"> ({{ formatDecimalHours(summary.targetMinutes) }})</span>
          </span>
        </p>
        <p
          class="tile card"
          data-test="week-balance"
          :class="{ negative: summary.balanceMinutes < 0, positive: summary.balanceMinutes > 0 }"
        >
          <span class="tile-label">Saldo</span> <span class="tile-value">
            {{ formatBalance(summary.balanceMinutes) }}<span class="decimal"> ({{ formatDecimalBalance(summary.balanceMinutes) }})</span>
          </span>
        </p>
        <p v-if="summary.targetIncomplete" class="notice warning" data-test="target-incomplete">
          Das Soll ist ohne Feiertagsabzug berechnet, weil für diese Woche Feiertage fehlen.
        </p>
      </section>
    </template>
  </section>
</template>

<style scoped>
.days { overflow: hidden; }
.summary { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-3); margin-top: var(--space-4); }
.positive .tile-value { color: var(--color-success); }
.negative .tile-value { color: var(--color-danger); }
.summary .notice { grid-column: 1 / -1; margin: 0; }
</style>
