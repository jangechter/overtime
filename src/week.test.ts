import { describe, it, expect } from "vitest";
import { currentWeek, formatHours, shiftWeek } from "./week";

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

describe("shiftWeek", () => {
  const week = currentWeek(new Date(2026, 10, 18));

  it("springt eine Woche zurück und vor", () => {
    expect(shiftWeek(week, -1)[0]).toBe("2026-11-09");
    expect(shiftWeek(week, -1)[6]).toBe("2026-11-15");
    expect(shiftWeek(week, 1)[0]).toBe("2026-11-23");
  });

  it("läuft über den Jahreswechsel in beide Richtungen", () => {
    const lastOfYear = currentWeek(new Date(2026, 11, 28));
    expect(shiftWeek(lastOfYear, 1)).toEqual(currentWeek(new Date(2027, 0, 4)));
    expect(shiftWeek(currentWeek(new Date(2027, 0, 4)), -1)).toEqual(lastOfYear);
  });

  it("läuft über die Zeitumstellung, ohne Tage zu verschieben", () => {
    const beforeDst = currentWeek(new Date(2026, 9, 21));
    expect(shiftWeek(beforeDst, 1)[0]).toBe("2026-10-26");
    expect(shiftWeek(beforeDst, 1)[6]).toBe("2026-11-01");
  });

  it("bleibt bei 0 Wochen unverändert", () => {
    expect(shiftWeek(week, 0)).toEqual(week);
  });
});

describe("formatHours", () => {
  it("zeigt Stunden und zweistellige Minuten", () => {
    expect(formatHours(480)).toBe("8:00 h");
    expect(formatHours(59)).toBe("0:59 h");
    expect(formatHours(1445)).toBe("24:05 h");
  });

  it("zeigt 0 Minuten als 0:00 h", () => {
    expect(formatHours(0)).toBe("0:00 h");
  });
});
