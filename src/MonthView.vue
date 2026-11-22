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

/** Durchschnitt für die Kachel: der Wert groß, darunter die Zahl der gezählten Arbeitstage. */
const average = computed(() => {
  const days = summary.value?.countedDays ?? 0;
  if (summary.value?.averageMinutes == null || days === 0) return { value: "–", detail: "kein Arbeitstag mit Eintrag" };
  return {
    value: formatHours(summary.value.averageMinutes),
    detail: `an ${days} ${days === 1 ? "Arbeitstag" : "Arbeitstagen"}`,
  };
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
    <nav class="period-nav" aria-label="Monat wählen">
      <span class="period-label" data-test="month-label">{{ monthLabel(month) }}</span>
      <div class="period-buttons">
        <button type="button" data-test="prev-month" @click="goToMonth(-1)">← Vorheriger Monat</button>
        <button type="button" data-test="this-month" @click="goToThisMonth">Heute</button>
        <button type="button" data-test="next-month" @click="goToMonth(1)">Nächster Monat →</button>
      </div>
    </nav>
    <p v-if="loadError" class="notice error" role="alert" data-test="month-error">{{ loadError }}</p>
    <section v-if="summary" class="summary" aria-live="polite">
      <div class="tile card">
        <p class="tile-text" data-test="month-total">
          <span class="tile-label">Monatssumme</span> <span class="tile-value">{{ formatHours(summary.actualMinutes) }}</span>
        </p>
        <p v-if="summary.otherMinutes > 0" class="tile-detail" data-test="month-other">
          davon {{ formatHours(summary.otherMinutes) }} am Wochenende oder an Feiertagen (nicht im Durchschnitt)
        </p>
      </div>
      <p class="tile card" data-test="month-average">
        <span class="tile-label">Durchschnitt pro Arbeitstag</span> <span class="tile-value">{{ average.value }}</span> <span class="tile-detail">{{ average.detail }}</span>
      </p>
      <p v-if="summary.holidaysMissing" class="notice warning" data-test="month-holidays-missing">
        Für einzelne Tage dieses Monats fehlen Feiertage; ein Feiertag könnte als Arbeitstag mitgezählt sein.
      </p>
    </section>
  </section>
</template>

<style scoped>
.summary { display: grid; grid-template-columns: repeat(2, 1fr); gap: var(--space-3); }
/* Die Monatssumme hat eine eigene Zusatzzeile (eigenes data-test); Beschriftung und Wert stehen darüber untereinander. */
.tile-text { display: flex; flex-direction: column; gap: var(--space-1); margin: 0; }
.summary .notice { grid-column: 1 / -1; margin: 0; }
</style>
