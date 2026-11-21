<script setup lang="ts">
import { computed, onActivated, onDeactivated, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { currentMonth, monthLabel, shiftMonth } from "./month";
import { formatHours } from "./week";

/** Monatssumme und Durchschnitt in Minuten, berechnet im Backend. */
interface MonthSummary {
  actualMinutes: number;
  /** `null`, wenn kein Arbeitstag gezählt wurde. */
  averageMinutes: number | null;
  countedDays: number;
  /** Stunden am Wochenende und an Feiertagen: in der Summe, nicht im Durchschnitt. */
  otherMinutes: number;
  holidaysMissing: boolean;
}

const props = withDefaults(
  defineProps<{
    /** Ändert sich, wenn neue Feiertage gespeichert wurden (z. B. Abruf beim Start nach dem Umschalten). */
    holidaysVersion?: number;
  }>(),
  { holidaysVersion: 0 },
);

/** Der angezeigte Monat als `YYYY-MM-01`. */
const month = ref(currentMonth());
const summary = ref<MonthSummary | null>(null);
const loadError = ref("");
let latestRequest = 0;

const averageLabel = computed(() => {
  const days = summary.value?.countedDays ?? 0;
  if (summary.value?.averageMinutes == null || days === 0) return "– (kein Arbeitstag mit Eintrag)";
  return `${formatHours(summary.value.averageMinutes)} an ${days} ${days === 1 ? "Arbeitstag" : "Arbeitstagen"}`;
});

async function loadMonth() {
  const request = ++latestRequest;
  loadError.value = "";
  try {
    const result = await invoke<MonthSummary>("month_summary", { date: month.value });
    if (request === latestRequest) summary.value = result;
  } catch (e) {
    if (request !== latestRequest) return;
    summary.value = null;
    loadError.value = String(e);
  }
}

function goToMonth(offset: number) {
  month.value = shiftMonth(month.value, offset);
  summary.value = null;
  loadMonth();
}

function goToThisMonth() {
  const thisMonth = currentMonth();
  if (thisMonth === month.value) return;
  month.value = thisMonth;
  summary.value = null;
  loadMonth();
}

// Die Ansicht bleibt beim Umschalten erhalten (KeepAlive); beim Öffnen neu lesen, weil sich Einträge geändert haben können.
let active = false;
onActivated(() => {
  active = true;
  loadMonth();
});
onDeactivated(() => {
  active = false;
});
// Neue Feiertage ändern Durchschnitt und Hinweis; ausgeblendet genügt das Neulesen beim nächsten Öffnen.
watch(
  () => props.holidaysVersion,
  () => {
    if (active) loadMonth();
  },
);
</script>

<template>
  <section>
    <nav class="month-nav" aria-label="Monat wählen">
      <button type="button" data-test="prev-month" @click="goToMonth(-1)">← Vorheriger Monat</button>
      <span data-test="month-label">{{ monthLabel(month) }}</span>
      <button type="button" data-test="this-month" @click="goToThisMonth">Heute</button>
      <button type="button" data-test="next-month" @click="goToMonth(1)">Nächster Monat →</button>
    </nav>
    <p v-if="loadError" class="error" role="alert" data-test="month-error">{{ loadError }}</p>
    <section v-if="summary" class="summary" aria-live="polite">
      <p data-test="month-total">Monatssumme: {{ formatHours(summary.actualMinutes) }}</p>
      <p v-if="summary.otherMinutes > 0" class="detail" data-test="month-other">
        davon {{ formatHours(summary.otherMinutes) }} am Wochenende oder an Feiertagen (nicht im Durchschnitt)
      </p>
      <p data-test="month-average">Durchschnitt pro Arbeitstag: {{ averageLabel }}</p>
      <p v-if="summary.holidaysMissing" class="warning" data-test="month-holidays-missing">
        Für einzelne Tage dieses Monats fehlen Feiertage; ein Feiertag könnte als Arbeitstag mitgezählt sein.
      </p>
    </section>
  </section>
</template>

<style scoped>
.month-nav { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
.error { color: #b00020; }
.warning { color: #8a5a00; }
.summary { font-weight: 600; }
.summary p { margin: 0.25rem 0; }
.detail { font-weight: normal; color: #444; }
</style>
