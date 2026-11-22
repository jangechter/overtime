<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { formatDuration, formatHours } from "./week";

interface Settings {
  dailyMinutes: number;
  weeklyMinutes: number;
  subdivision: string;
}

/** Vom Backend: die Werte der laufenden Woche und eine noch nicht wirksame Änderung. */
interface Overview {
  current: Settings;
  pending: { validFrom: string; settings: Settings } | null;
}

const emit = defineEmits<{
  /** Eine Änderung wurde gespeichert (oder zurückgenommen). */
  saved: [];
}>();

interface Subdivision {
  code: string;
  name: string;
}

const overview = ref<Overview | null>(null);
/** Die wählbaren Bundesländer, vom Backend. */
const subdivisions = ref<Subdivision[]>([]);
const daily = ref("");
const weekly = ref("");
const subdivision = ref("");
const loadError = ref("");
const saveError = ref("");
const saving = ref(false);

/** Zeigt die Übersicht; die Felder enthalten die Werte, die ab nächster Woche gelten. */
function show(result: Overview) {
  overview.value = result;
  const upcoming = result.pending?.settings ?? result.current;
  daily.value = formatDuration(upcoming.dailyMinutes);
  weekly.value = formatDuration(upcoming.weeklyMinutes);
  subdivision.value = upcoming.subdivision;
}

async function load() {
  try {
    const [result, list] = await Promise.all([
      invoke<Overview>("load_settings"),
      invoke<Subdivision[]>("list_subdivisions"),
    ]);
    subdivisions.value = list;
    show(result);
  } catch (e) {
    loadError.value = String(e);
  }
}

/** Das Backend prüft die Eingaben und bestimmt den Montag, ab dem sie gelten. */
async function save() {
  if (saving.value) return;
  saving.value = true;
  saveError.value = "";
  try {
    show(
      await invoke<Overview>("save_settings", {
        daily: daily.value,
        weekly: weekly.value,
        subdivision: subdivision.value,
      }),
    );
    emit("saved");
  } catch (e) {
    saveError.value = String(e);
  } finally {
    saving.value = false;
  }
}

const formatDate = (date: string) =>
  new Date(`${date}T00:00:00`).toLocaleDateString("de-DE", {
    weekday: "long",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  });
const subdivisionName = (code: string) => subdivisions.value.find((entry) => entry.code === code)?.name ?? code;
const describe = (settings: Settings) =>
  `Tagessoll ${formatHours(settings.dailyMinutes)}, Wochen-Soll ${formatHours(settings.weeklyMinutes)}, ` +
  subdivisionName(settings.subdivision);

onMounted(load);
</script>

<template>
  <section>
    <p v-if="loadError" class="notice error" role="alert" data-test="settings-load-error">{{ loadError }}</p>
    <div v-if="overview" class="settings card">
      <p class="current" data-test="settings-current">Diese Woche: {{ describe(overview.current) }}</p>
      <p v-if="overview.pending" class="pending" data-test="settings-pending">
        Ab {{ formatDate(overview.pending.validFrom) }}: {{ describe(overview.pending.settings) }}
      </p>
      <form class="settings-form" @submit.prevent="save">
        <label>
          Tagessoll
          <input v-model="daily" type="text" inputmode="numeric" placeholder="H:MM" data-test="daily" />
        </label>
        <label>
          Wochen-Soll
          <input v-model="weekly" type="text" inputmode="numeric" placeholder="H:MM" data-test="weekly" />
        </label>
        <label>
          Bundesland
          <select v-model="subdivision" data-test="subdivision">
            <option v-for="entry in subdivisions" :key="entry.code" :value="entry.code">{{ entry.name }}</option>
          </select>
        </label>
        <button type="submit" class="primary" :disabled="saving" data-test="save-settings">Ab nächster Woche übernehmen</button>
      </form>
      <p v-if="saveError" class="notice error" role="alert" data-test="settings-save-error">{{ saveError }}</p>
    </div>
    <template v-if="overview">
      <p class="hint">
        Änderungen gelten ab Montag der nächsten Woche; die laufende und alle vergangenen Wochen behalten ihre Werte.
        Ein Feiertag senkt das Wochen-Soll um ein Tagessoll, ein Krank- oder Urlaubstag zählt mit dem Tagessoll.
        Nach einem Wechsel des Bundeslands gelten ab diesem Montag dessen Feiertage (ohne Feiertage, die nur in einem
        Teil des Landes gelten); die laufende und vergangene Wochen behalten ihre.
        Um eine geplante Änderung zurückzunehmen, die Werte dieser Woche eintragen.
      </p>
    </template>
  </section>
</template>

<style scoped>
.settings { padding: var(--space-4); }
.current { margin: 0 0 var(--space-2); font-weight: 600; }
.pending {
  margin: 0 0 var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--color-accent-soft);
  color: var(--color-accent);
  font-weight: 600;
}
.settings-form { display: flex; align-items: end; gap: var(--space-4); margin: var(--space-4) 0 0; }
.settings-form label { display: flex; flex-direction: column; gap: 2px; font-size: 0.75rem; color: var(--color-text-muted); }
.settings-form input,
.settings-form select { font-size: 0.95rem; color: var(--color-text); }
.settings-form input { width: 5.5rem; font-variant-numeric: tabular-nums; }
.settings .notice { margin: var(--space-3) 0 0; }
.hint { margin-top: var(--space-4); font-size: 0.9rem; color: var(--color-text-muted); }
</style>
