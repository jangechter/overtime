<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface StoredEntry {
  start: string;
  end: string;
  breakMinutes: number;
  minutes: number;
}

const props = defineProps<{
  date: string;
  entry?: StoredEntry;
  /** Meldet einen Fehler, der erst eintrifft, nachdem die Zeile (z. B. durch Blättern) entfernt wurde. */
  onSaveFailed?: (date: string, message: string) => void;
}>();

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
const hoursLabel = computed(() =>
  minutes.value === null
    ? ""
    : `${Math.floor(minutes.value / 60)}:${String(minutes.value % 60).padStart(2, "0")} h`,
);

let latestRequest = 0;

watch([start, end, breakMinutes], async () => {
  const request = ++latestRequest;
  try {
    if (!start.value || !end.value) {
      minutes.value = null;
      error.value = "";
      if (stored) {
        await invoke("delete_entry", { date: props.date });
        stored = false;
      }
      return;
    }
    const result = await invoke<number>("save_entry", {
      date: props.date,
      start: start.value,
      end: end.value,
      breakMinutes: Number(breakMinutes.value) || 0,
    });
    if (request !== latestRequest) return;
    stored = true;
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
  <div class="week-row">
    <div class="day">
      <strong>{{ weekday }}</strong>
      <span>{{ dateLabel }}</span>
    </div>
    <label>Start <input v-model="start" type="text" inputmode="numeric" placeholder="HH:MM" maxlength="5" data-test="start" /></label>
    <label>Ende <input v-model="end" type="text" inputmode="numeric" placeholder="HH:MM" maxlength="5" data-test="end" /></label>
    <label>Pause (Min) <input v-model="breakMinutes" type="number" min="0" data-test="break" /></label>
    <span v-if="minutes !== null" class="hours" data-test="hours">{{ hoursLabel }}</span>
    <span v-if="error" class="error" data-test="error">{{ error }}</span>
  </div>
</template>

<style scoped>
.week-row { display: flex; align-items: center; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid #ddd; }
.day { display: flex; flex-direction: column; width: 8rem; }
.hours { font-weight: 600; }
.error { color: #b00020; }
input[type="number"] { width: 4rem; }
input[type="text"] { width: 4.5rem; }
</style>
