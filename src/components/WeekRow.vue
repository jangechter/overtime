<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{ date: string }>();

const start = ref("");
const end = ref("");
const breakMinutes = ref(0);
const minutes = ref<number | null>(null);
const error = ref("");

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
  if (!start.value || !end.value) {
    minutes.value = null;
    error.value = "";
    return;
  }
  try {
    const result = await invoke<number>("calculate_day", {
      start: start.value,
      end: end.value,
      breakMinutes: Number(breakMinutes.value) || 0,
    });
    if (request !== latestRequest) return;
    minutes.value = result;
    error.value = "";
  } catch (e) {
    if (request !== latestRequest) return;
    minutes.value = null;
    error.value = String(e);
  }
});
</script>

<template>
  <div class="week-row">
    <div class="day">
      <strong>{{ weekday }}</strong>
      <span>{{ dateLabel }}</span>
    </div>
    <label>Start <input v-model="start" type="time" data-test="start" /></label>
    <label>Ende <input v-model="end" type="time" data-test="end" /></label>
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
</style>
