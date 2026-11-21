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

/** Dieselbe Woche um `weeks` Wochen verschoben (negativ = zurück). */
export function shiftWeek(week: string[], weeks: number): string[] {
  const [year, month, day] = week[0].split("-").map(Number);
  return currentWeek(new Date(year, month - 1, day + weeks * 7));
}

/** Minuten als `H:MM`, z. B. 480 → „8:00“ (für Eingabefelder). */
export function formatDuration(minutes: number): string {
  return `${Math.floor(minutes / 60)}:${pad(minutes % 60)}`;
}

/** Minuten als `H:MM h`, z. B. 480 → „8:00 h“. */
export function formatHours(minutes: number): string {
  return `${formatDuration(minutes)} h`;
}

/** Saldo mit Vorzeichen: „+5:00 h“, „−5:00 h“ (Minuszeichen), ausgeglichen „0:00 h“. */
export function formatBalance(minutes: number): string {
  if (minutes === 0) return formatHours(0);
  return `${minutes > 0 ? "+" : "−"}${formatHours(Math.abs(minutes))}`;
}
