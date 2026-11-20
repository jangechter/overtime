const pad = (n: number) => String(n).padStart(2, "0");

/** Die sieben Tage (Montag bis Sonntag) der Woche, in der `today` liegt, als `YYYY-MM-DD`. */
export function currentWeek(today: Date = new Date()): string[] {
  const monday = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7));
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + i);
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  });
}
