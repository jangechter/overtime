<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { formatHours } from "../week";

export type Kind = "work" | "sick" | "vacation";

/** Gespeicherter Tag. Bei Krank/Urlaub sind die Zeiten leer und `minutes` sind die vom Backend angerechneten Minuten. */
export interface StoredEntry {
  kind: Kind;
  start: string | null;
  end: string | null;
  breakMinutes: number | null;
  minutes: number;
}

const props = defineProps<{
  date: string;
  entry?: StoredEntry;
  /** Name des Feiertags, falls der Tag einer ist. */
  holiday?: string;
  /** Meldet einen Fehler, der erst eintrifft, nachdem die Zeile (z. B. durch Blättern) entfernt wurde. */
  onSaveFailed?: (date: string, message: string) => void;
}>();

const emit = defineEmits<{
  /** Der gespeicherte Eintrag dieses Tages hat sich geändert (gespeichert oder gelöscht). */
  saved: [];
}>();

const kind = ref<Kind>(props.entry?.kind ?? "work");
const start = ref(props.entry?.start ?? "");
const end = ref(props.entry?.end ?? "");
const breakMinutes = ref(props.entry?.breakMinutes ?? 0);
const minutes = ref<number | null>(props.entry?.minutes ?? null);
const error = ref("");
let stored = props.entry !== undefined;
let unmounted = false;
onUnmounted(() => {
  unmounted = true;
});

const parsed = computed(() => new Date(`${props.date}T00:00:00`));
const weekday = computed(() => parsed.value.toLocaleDateString("de-DE", { weekday: "long" }));
const dateLabel = computed(() =>
  parsed.value.toLocaleDateString("de-DE", { day: "2-digit", month: "2-digit", year: "numeric" }),
);
const hoursLabel = computed(() => (minutes.value === null ? "" : formatHours(minutes.value)));
const isWeekend = computed(() => [0, 6].includes(parsed.value.getDay()));

let latestRequest = 0;
/** Laufende schreibende Befehle. Solange einer läuft, kann der Tag schon gespeichert sein, obwohl `stored` noch false ist. */
let writesInFlight = 0;

async function write<T>(command: string, args: Record<string, unknown>): Promise<T> {
  writesInFlight++;
  try {
    return await invoke<T>(command, args);
  } finally {
    writesInFlight--;
  }
}

// Die angerechneten Minuten von Krank/Urlaub hängen von den Feiertagen ab; nach neu geladenen Feiertagen kommen sie über `entry`.
watch(
  () => props.entry,
  (entry) => {
    if (entry && entry.kind !== "work" && entry.kind === kind.value) minutes.value = entry.minutes;
  },
);

// Bei Krank/Urlaub bleiben die eingetragenen Zeiten in der Zeile (ausgeblendet) erhalten: Das Backend ersetzt den Eintrag,
// zurück auf „Arbeit“ werden sie wieder gespeichert. Schlägt das Markieren fehl, gehen sie so nicht verloren.
watch([kind, start, end, breakMinutes], async () => {
  const request = ++latestRequest;
  try {
    if (kind.value !== "work") {
      const result = await write<number>("mark_absence", { date: props.date, kind: kind.value });
      stored = true;
      emit("saved");
      if (request !== latestRequest) return;
      minutes.value = result;
      error.value = "";
      return;
    }
    if (!start.value || !end.value) {
      minutes.value = null;
      error.value = "";
      if (stored || writesInFlight > 0) {
        await write("delete_entry", { date: props.date });
        stored = false;
        emit("saved");
      }
      return;
    }
    const result = await write<number>("save_entry", {
      date: props.date,
      start: start.value,
      end: end.value,
      breakMinutes: Number(breakMinutes.value) || 0,
    });
    // Das Backend hat geschrieben, auch wenn inzwischen eine neuere Eingabe läuft.
    stored = true;
    emit("saved");
    if (request !== latestRequest) return;
    minutes.value = result;
    error.value = "";
  } catch (e) {
    if (request !== latestRequest) return;
    const message = stored ? `${String(e)} Der zuletzt gespeicherte Eintrag bleibt erhalten.` : String(e);
    if (unmounted) {
      props.onSaveFailed?.(props.date, message);
      return;
    }
    minutes.value = null;
    error.value = message;
  }
});
</script>

<template>
  <div class="week-row" :class="[`kind-${kind}`, { 'is-holiday': holiday, 'is-weekend': isWeekend }]">
    <div class="day">
      <strong>{{ weekday }}</strong>
      <span class="date">{{ dateLabel }}</span>
      <span v-if="holiday" class="holiday" data-test="holiday">{{ holiday }}</span>
    </div>
    <select v-model="kind" class="kind" aria-label="Art des Tages" data-test="kind">
      <option value="work">Arbeit</option>
      <option value="sick">Krank</option>
      <option value="vacation">Urlaub</option>
    </select>
    <template v-if="kind === 'work'">
      <label>Start <input v-model="start" type="text" inputmode="numeric" placeholder="HH:MM" maxlength="5" data-test="start" /></label>
      <label>Ende <input v-model="end" type="text" inputmode="numeric" placeholder="HH:MM" maxlength="5" data-test="end" /></label>
      <label>Pause (Min) <input v-model="breakMinutes" type="number" min="0" data-test="break" /></label>
    </template>
    <span v-if="minutes !== null" class="hours" data-test="hours">{{ hoursLabel }}</span>
    <span v-if="error" class="error" data-test="error">{{ error }}</span>
  </div>
</template>

<style scoped>
/* Feste Spalten, damit Zeiten und Stunden über alle Tage bündig stehen: Tag | Art | Start | Ende | Pause | Stunden. */
.week-row {
  display: grid;
  grid-template-columns: 11rem 7rem 5.5rem 5.5rem 6rem 1fr;
  align-items: center;
  column-gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--color-border);
  border-left: 3px solid transparent;
}
.week-row:last-child { border-bottom: none; }
.is-weekend { background: var(--color-surface-muted); }
.is-holiday { background: var(--color-warning-soft); }
.kind-sick { border-left-color: var(--color-sick); }
.kind-vacation { border-left-color: var(--color-vacation); }
.day { display: flex; flex-direction: column; line-height: 1.3; }
.date { font-size: 0.85rem; color: var(--color-text-muted); }
.holiday { margin-top: 2px; font-size: 0.8rem; font-weight: 600; color: var(--color-warning); }
label { display: flex; flex-direction: column; gap: 2px; font-size: 0.75rem; color: var(--color-text-muted); }
label input { width: 100%; font-size: 0.95rem; color: var(--color-text); font-variant-numeric: tabular-nums; }
.hours { grid-column: -2; justify-self: end; font-weight: 700; font-variant-numeric: tabular-nums; }
.error { grid-column: 1 / -1; margin-top: var(--space-2); font-size: 0.85rem; color: var(--color-danger); }
</style>
