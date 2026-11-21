const pad = (n: number) => String(n).padStart(2, "0");

/** Der erste Tag des Monats, in dem `today` liegt, als `YYYY-MM-01`. */
export function currentMonth(today: Date = new Date()): string {
  return `${today.getFullYear()}-${pad(today.getMonth() + 1)}-01`;
}

/** Derselbe Monat um `months` Monate verschoben (negativ = zurück), als `YYYY-MM-01`. */
export function shiftMonth(month: string, months: number): string {
  const [year, monthNumber] = month.split("-").map(Number);
  return currentMonth(new Date(year, monthNumber - 1 + months, 1));
}

/** Monat und Jahr, z. B. „November 2026“. */
export function monthLabel(month: string): string {
  return new Date(`${month}T00:00:00`).toLocaleDateString("de-DE", { month: "long", year: "numeric" });
}
