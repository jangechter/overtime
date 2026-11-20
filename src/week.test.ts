import { describe, it, expect } from "vitest";
import { currentWeek } from "./week";

describe("currentWeek", () => {
  it("liefert Montag bis Sonntag mitten in der Woche", () => {
    expect(currentWeek(new Date(2026, 10, 18))).toEqual([
      "2026-11-16", "2026-11-17", "2026-11-18", "2026-11-19", "2026-11-20", "2026-11-21", "2026-11-22",
    ]);
  });

  it("zählt den Sonntag zur Woche davor", () => {
    expect(currentWeek(new Date(2026, 10, 22))[0]).toBe("2026-11-16");
  });

  it("beginnt am Montag selbst mit diesem Tag", () => {
    expect(currentWeek(new Date(2026, 10, 16))[0]).toBe("2026-11-16");
  });

  it("läuft über Monats- und Jahreswechsel", () => {
    const week = currentWeek(new Date(2026, 11, 31));
    expect(week[0]).toBe("2026-12-28");
    expect(week[6]).toBe("2027-01-03");
  });
});
