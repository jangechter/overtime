import { describe, it, expect } from "vitest";
import { currentMonth, monthLabel, shiftMonth } from "./month";

describe("currentMonth", () => {
  it("liefert den ersten Tag des Monats", () => {
    expect(currentMonth(new Date(2026, 10, 21))).toBe("2026-11-01");
    expect(currentMonth(new Date(2026, 10, 30))).toBe("2026-11-01");
    expect(currentMonth(new Date(2027, 0, 1))).toBe("2027-01-01");
  });
});

describe("shiftMonth", () => {
  it("blättert einen Monat vor und zurück", () => {
    expect(shiftMonth("2026-11-01", 1)).toBe("2026-12-01");
    expect(shiftMonth("2026-11-01", -1)).toBe("2026-10-01");
  });

  it("läuft über den Jahreswechsel", () => {
    expect(shiftMonth("2026-12-01", 1)).toBe("2027-01-01");
    expect(shiftMonth("2027-01-01", -1)).toBe("2026-12-01");
  });
});

describe("monthLabel", () => {
  it("nennt Monat und Jahr auf Deutsch", () => {
    expect(monthLabel("2026-11-01")).toBe("November 2026");
    expect(monthLabel("2028-02-01")).toBe("Februar 2028");
  });
});
