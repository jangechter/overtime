import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import WeekView from "./WeekView.vue";
import { currentWeek, shiftWeek } from "./week";

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

  it("zeigt den Zeitraum der Woche", async () => {
    const week = currentWeek();
    const wrapper = mount(WeekView);
    await flushPromises();
    const label = wrapper.get('[data-test="week-label"]').text();
    expect(label).toContain(new Date(`${week[0]}T00:00:00`).toLocaleDateString("de-DE"));
    expect(label).toContain(new Date(`${week[6]}T00:00:00`).toLocaleDateString("de-DE"));
  });

  it("blättert zur vorherigen Woche und wieder zurück, jeweils mit den gespeicherten Tagen", async () => {
    const week = currentWeek();
    const previous = shiftWeek(week, -1);
    backend = async (_command, args) => {
      const { from } = args as { from: string };
      if (from === week[0]) return [{ date: week[1], start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 }];
      return [];
    };
    const wrapper = mount(WeekView);
    await flushPromises();
    expect(wrapper.findAll(".week-row")[1].find('[data-test="hours"]').exists()).toBe(true);

    await wrapper.get('[data-test="prev"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["load_entries", { from: previous[0], to: previous[6] }]);
    expect(wrapper.findAll(".week-row")).toHaveLength(7);
    expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);

    await wrapper.get('[data-test="next"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["load_entries", { from: week[0], to: week[6] }]);
    expect(wrapper.findAll(".week-row")[1].get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("übernimmt beim schnellen Blättern nur die Antwort der zuletzt gewählten Woche", async () => {
    const week = currentWeek();
    const pending: Array<(rows: unknown[]) => void> = [];
    backend = (_command, args) => {
      if ((args as { from: string }).from === week[0] && pending.length === 0) return Promise.resolve([]);
      return new Promise((resolve) => pending.push(resolve as (rows: unknown[]) => void));
    };
    const wrapper = mount(WeekView);
    await flushPromises();
    await wrapper.get('[data-test="prev"]').trigger("click");
    await wrapper.get('[data-test="prev"]').trigger("click");
    const twoBack = shiftWeek(week, -2);
    pending[1]([{ date: twoBack[3], start: "09:00", end: "17:00", breakMinutes: 0, minutes: 480 }]);
    await flushPromises();
    pending[0]([]);
    await flushPromises();
    expect(wrapper.findAll(".week-row")[3].get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("zeigt nach einem Ladefehler beim Blättern keine Zeilen", async () => {
    const week = currentWeek();
    backend = async (_command, args) => {
      if ((args as { from: string }).from !== week[0]) throw "Datenbank nicht lesbar.";
      return [];
    };
    const wrapper = mount(WeekView);
    await flushPromises();
    await wrapper.get('[data-test="prev"]').trigger("click");
    await flushPromises();
    expect(wrapper.get('[data-test="load-error"]').text()).toBe("Datenbank nicht lesbar.");
    expect(wrapper.findAll(".week-row")).toHaveLength(0);
  });

  it("springt mit „Heute“ aus einer anderen Woche zurück in die aktuelle Woche", async () => {
    const week = currentWeek();
    backend = async (_command, args) => {
      const { from } = args as { from: string };
      if (from === week[0]) return [{ date: week[1], start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 }];
      return [];
    };
    const wrapper = mount(WeekView);
    await flushPromises();
    await wrapper.get('[data-test="prev"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-test="prev"]').trigger("click");
    await flushPromises();

    await wrapper.get('[data-test="today"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["load_entries", { from: week[0], to: week[6] }]);
    expect(wrapper.get('[data-test="week-label"]').text()).toContain(
      new Date(`${week[0]}T00:00:00`).toLocaleDateString("de-DE"),
    );
    expect(wrapper.findAll(".week-row")[1].get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("lädt die Woche nicht erneut, wenn „Heute“ in der aktuellen Woche geklickt wird", async () => {
    const wrapper = mount(WeekView);
    await flushPromises();
    const before = calls.length;
    await wrapper.get('[data-test="today"]').trigger("click");
    await flushPromises();
    expect(calls).toHaveLength(before);
  });

  it("schreibt eine laufende Speicherung beim Blättern in die ursprüngliche Woche", async () => {
    const week = currentWeek();
    let finishSave: (minutes: number) => void = () => {};
    backend = (command) =>
      command === "save_entry"
        ? new Promise((resolve) => {
            finishSave = resolve as (minutes: number) => void;
          })
        : Promise.resolve([]);
    const wrapper = mount(WeekView);
    await flushPromises();
    const row = wrapper.findAll(".week-row")[1];
    await row.get('[data-test="start"]').setValue("08:00");
    await row.get('[data-test="end"]').setValue("16:00");
    await wrapper.get('[data-test="prev"]').trigger("click");
    finishSave(480);
    await flushPromises();
    const saves = calls.filter((call) => call[0] === "save_entry");
    expect(saves.length).toBeGreaterThan(0);
    expect(saves.every((call) => (call[1] as { date: string }).date === week[1])).toBe(true);
    expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
  });

  it("meldet ein fehlgeschlagenes Speichern, auch wenn die Zeile schon weggeblättert wurde", async () => {
    const week = currentWeek();
    let failSave: (reason: string) => void = () => {};
    backend = (command) =>
      command === "save_entry"
        ? new Promise((_resolve, reject) => {
            failSave = reject;
          })
        : Promise.resolve([]);
    const wrapper = mount(WeekView);
    await flushPromises();
    const row = wrapper.findAll(".week-row")[1];
    await row.get('[data-test="start"]').setValue("08:00");
    await row.get('[data-test="end"]').setValue("07:00");
    await wrapper.get('[data-test="prev"]').trigger("click");
    failSave("Das Ende muss nach dem Start liegen.");
    await flushPromises();
    const message = wrapper.get('[data-test="save-error"]').text();
    expect(message).toContain(new Date(`${week[1]}T00:00:00`).toLocaleDateString("de-DE"));
    expect(message).toContain("Das Ende muss nach dem Start liegen.");
  });

  it("blendet die Speichermeldung aus, sobald die nächste Woche geladen wird", async () => {
    let failSave: (reason: string) => void = () => {};
    backend = (command) =>
      command === "save_entry"
        ? new Promise((_resolve, reject) => {
            failSave = reject;
          })
        : Promise.resolve([]);
    const wrapper = mount(WeekView);
    await flushPromises();
    const row = wrapper.findAll(".week-row")[1];
    await row.get('[data-test="start"]').setValue("08:00");
    await row.get('[data-test="end"]').setValue("07:00");
    await wrapper.get('[data-test="prev"]').trigger("click");
    failSave("Fehler");
    await flushPromises();
    expect(wrapper.find('[data-test="save-error"]').exists()).toBe(true);
    await wrapper.get('[data-test="next"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="save-error"]').exists()).toBe(false);
  });
});
