<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import WeekView from "./WeekView.vue";
import MonthView from "./MonthView.vue";
import SettingsView from "./SettingsView.vue";

const view = ref<"week" | "month" | "settings">("week");
/** Zählt gespeicherte Einstellungen, damit die Wochenübersicht Soll und Krank/Urlaub neu liest. */
const settingsVersion = ref(0);
/** Zählt Änderungen an den gespeicherten Feiertagen, damit die Monatsansicht danach neu liest. */
const holidaysVersion = ref(0);

const exporting = ref(false);
const exportMessage = ref("");
const exportError = ref("");

/** Export: Das Backend öffnet den Speichern-Dialog und schreibt die Datei; abgebrochen liefert es `null`. */
async function exportCsv() {
  if (exporting.value) return;
  exporting.value = true;
  exportMessage.value = "";
  exportError.value = "";
  try {
    const path = await invoke<string | null>("export_csv");
    if (path) exportMessage.value = `Export gespeichert: ${path}`;
  } catch (e) {
    exportError.value = String(e);
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <main>
    <header class="app-header">
      <h1>Overtime</h1>
      <nav class="views" aria-label="Ansicht">
        <button type="button" data-test="view-week" :aria-pressed="view === 'week'" @click="view = 'week'">Woche</button>
        <button type="button" data-test="view-month" :aria-pressed="view === 'month'" @click="view = 'month'">Monat</button>
        <button type="button" data-test="view-settings" :aria-pressed="view === 'settings'" @click="view = 'settings'">
          Einstellungen
        </button>
      </nav>
      <button type="button" data-test="export" :disabled="exporting" @click="exportCsv">Export (CSV)</button>
    </header>
    <p v-if="exportMessage" class="notice info" role="status" data-test="export-message">{{ exportMessage }}</p>
    <p v-if="exportError" class="notice error" role="alert" data-test="export-error">{{ exportError }}</p>
    <!-- Beide Ansichten bleiben beim Umschalten erhalten, damit Woche und Monat ihre Position behalten. -->
    <WeekView v-show="view === 'week'" :settings-version="settingsVersion" @holidays-changed="holidaysVersion++" />
    <KeepAlive>
      <MonthView v-if="view === 'month'" :holidays-version="holidaysVersion" />
    </KeepAlive>
    <SettingsView v-if="view === 'settings'" @saved="settingsVersion++" />
    <footer class="source" data-test="holiday-source">Feiertage: OpenHolidays API (ODbL)</footer>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: var(--space-4) var(--space-4) var(--space-6); }
.app-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); margin-bottom: var(--space-4); }
/* Segmentierte Umschaltung: die aktive Ansicht ist hervorgehoben. */
.views { display: flex; padding: 3px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-muted); }
.views button { border-color: transparent; background: transparent; color: var(--color-text-muted); }
.views button[aria-pressed="true"] { background: var(--color-surface); color: var(--color-text); font-weight: 600; box-shadow: var(--shadow-sm); }
.source { margin-top: var(--space-6); font-size: 0.8rem; color: var(--color-text-muted); }
</style>
