import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import WeekView from "./WeekView.vue";
import { currentWeek } from "./week";

const calls: unknown[][] = [];
let backend: (...args: unknown[]) => Promise<unknown> = async () => [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => {
    calls.push(args);
    return backend(...args);
  },
}));

describe("WeekView", () => {
  beforeEach(() => {
    calls.length = 0;
    backend = async () => [];
  });

  it("lädt die Einträge der aktuellen Woche und trägt sie in die Zeilen ein", async () => {
    const week = currentWeek();
    backend = async () => [{ date: week[2], start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 }];
    const wrapper = mount(WeekView);
    await flushPromises();
    expect(calls[0]).toEqual(["load_entries", { from: week[0], to: week[6] }]);
    const rows = wrapper.findAll(".week-row");
    expect(rows).toHaveLength(7);
    expect(rows[2].get('[data-test="hours"]').text()).toBe("8:00 h");
    expect(rows[0].find('[data-test="hours"]').exists()).toBe(false);
  });

  it("zeigt eine Meldung, wenn das Laden fehlschlägt", async () => {
    backend = async () => {
      throw "Datenbank nicht lesbar.";
    };
    const wrapper = mount(WeekView);
    await flushPromises();
    expect(wrapper.get('[data-test="load-error"]').text()).toBe("Datenbank nicht lesbar.");
    expect(wrapper.findAll(".week-row")).toHaveLength(0);
  });
});
