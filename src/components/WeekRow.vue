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
  <div class="week-row" :class="{ 'is-holiday': holiday }">
    <div class="day">
      <strong>{{ weekday }}</strong>
      <span>{{ dateLabel }}</span>
    </div>
    <span v-if="holiday" class="holiday" data-test="holiday">{{ holiday }}</span>
    <select v-model="kind" aria-label="Art des Tages" data-test="kind">
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
.week-row { display: flex; align-items: center; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid #ddd; }
.day { display: flex; flex-direction: column; width: 8rem; }
.hours { font-weight: 600; }
.is-holiday { background: #fff4d6; }
.holiday { font-weight: 600; color: #8a5a00; width: 10rem; }
.error { color: #b00020; }
input[type="number"] { width: 4rem; }
input[type="text"] { width: 4.5rem; }
</style>
