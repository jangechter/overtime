<script setup lang="ts">
import { ref } from "vue";
import WeekView from "./WeekView.vue";
import MonthView from "./MonthView.vue";

const view = ref<"week" | "month">("week");
/** Zählt Änderungen an den gespeicherten Feiertagen, damit die Monatsansicht danach neu liest. */
const holidaysVersion = ref(0);
</script>

<template>
  <main>
    <header class="app-header">
      <h1>Overtime</h1>
      <nav class="views" aria-label="Ansicht">
        <button type="button" data-test="view-week" :aria-pressed="view === 'week'" @click="view = 'week'">Woche</button>
        <button type="button" data-test="view-month" :aria-pressed="view === 'month'" @click="view = 'month'">Monat</button>
      </nav>
    </header>
    <!-- Beide Ansichten bleiben beim Umschalten erhalten, damit Woche und Monat ihre Position behalten. -->
    <WeekView v-show="view === 'week'" @holidays-changed="holidaysVersion++" />
    <KeepAlive>
      <MonthView v-if="view === 'month'" :holidays-version="holidaysVersion" />
    </KeepAlive>
    <footer class="source" data-test="holiday-source">Feiertage: OpenHolidays API (ODbL)</footer>
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
.app-header { display: flex; align-items: center; justify-content: space-between; }
.views button[aria-pressed="true"] { font-weight: 700; }
.source { margin-top: 1rem; font-size: 0.8rem; color: #666; }
</style>
