<script setup lang="ts">
import WeekRow from "./components/WeekRow.vue";
import { currentWeek } from "./week";

const days = currentWeek();
</script>

<template>
  <main>
    <h1>Overtime – aktuelle Woche</h1>
    <WeekRow v-for="day in days" :key="day" :date="day" />
  </main>
</template>

<style scoped>
main { max-width: 56rem; margin: 0 auto; padding: 1rem; font-family: system-ui, sans-serif; }
</style>
