import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import App from "./App.vue";
import { currentMonth } from "./month";

const calls: unknown[][] = [];
/** Antwort auf `refresh_holidays`; in einem Test zurückgehalten, um den späten Abruf beim Start nachzustellen. */
let refresh: () => Promise<unknown> = () => Promise.resolve(undefined);
/** Antwort auf `export_csv`: gespeicherter Pfad, `null` bei abgebrochenem Dialog, oder ein Fehler. */
let exportCsv: () => Promise<unknown> = () => Promise.resolve(null);
/** Standardantworten: leere Woche ohne fehlende Feiertage, leerer Monat. */
const answer = (command: unknown) => {
  if (command === "load_holidays") return { holidays: [], missing: false };
  if (command === "week_summary") return { actualMinutes: 0, targetMinutes: 2400, balanceMinutes: -2400, targetIncomplete: false };
  if (command === "month_summary") return { actualMinutes: 0, averageMinutes: null, countedDays: 0, otherMinutes: 0, holidaysMissing: false };
  return [];
};
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => {
    calls.push(args);
    if (args[0] === "refresh_holidays") return refresh();
    if (args[0] === "export_csv") return exportCsv();
    return Promise.resolve(answer(args[0]));
  },
}));

const count = (command: string) => calls.filter((call) => call[0] === command).length;
const isVisible = (wrapper: ReturnType<typeof mount>, test: string) =>
  wrapper.find(`[data-test="${test}"]`).exists() && wrapper.get(`[data-test="${test}"]`).isVisible();

describe("App", () => {
  beforeEach(() => {
    calls.length = 0;
    refresh = () => Promise.resolve(undefined);
    exportCsv = () => Promise.resolve(null);
  });

  it("startet mit der Wochenübersicht und lädt den Monat noch nicht", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    expect(isVisible(wrapper, "week-label")).toBe(true);
    expect(wrapper.find('[data-test="month-label"]').exists()).toBe(false);
    expect(count("month_summary")).toBe(0);
    wrapper.unmount();
  });

  it("schaltet zur Monatsansicht um und lädt den aktuellen Monat", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    expect(isVisible(wrapper, "month-label")).toBe(true);
    expect(isVisible(wrapper, "week-label")).toBe(false);
    expect(calls).toContainEqual(["month_summary", { date: currentMonth() }]);
    expect(wrapper.get('[data-test="view-month"]').attributes("aria-pressed")).toBe("true");
    wrapper.unmount();
  });

  it("behält beim Zurückschalten die gewählte Woche, ohne sie neu zu laden", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-test="prev"]').trigger("click");
    await flushPromises();
    const label = wrapper.get('[data-test="week-label"]').text();
    const loads = count("load_entries");
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-test="view-week"]').trigger("click");
    await flushPromises();
    expect(wrapper.get('[data-test="week-label"]').text()).toBe(label);
    expect(count("load_entries")).toBe(loads);
    expect(count("refresh_holidays")).toBe(1);
    wrapper.unmount();
  });

  it("liest den Monat bei jedem Öffnen neu", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    for (let i = 0; i < 2; i++) {
      await wrapper.get('[data-test="view-month"]').trigger("click");
      await flushPromises();
      await wrapper.get('[data-test="view-week"]').trigger("click");
      await flushPromises();
    }
    expect(count("month_summary")).toBe(2);
    wrapper.unmount();
  });

  it("liest den geöffneten Monat neu, wenn der Abruf der Feiertage beim Start erst danach fertig wird", async () => {
    let finishRefresh: () => void = () => {};
    refresh = () => new Promise((resolve) => (finishRefresh = () => resolve(undefined)));
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    expect(count("month_summary")).toBe(1);
    finishRefresh();
    await flushPromises();
    expect(count("month_summary")).toBe(2);
    wrapper.unmount();
  });

  it("liest einen ausgeblendeten Monat nach neuen Feiertagen erst beim nächsten Öffnen", async () => {
    let finishRefresh: () => void = () => {};
    refresh = () => new Promise((resolve) => (finishRefresh = () => resolve(undefined)));
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-test="view-week"]').trigger("click");
    await flushPromises();
    finishRefresh();
    await flushPromises();
    expect(count("month_summary")).toBe(1);
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    expect(count("month_summary")).toBe(2);
    wrapper.unmount();
  });

  it("benennt Umschalter und Navigation für Screenreader", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    const labels = wrapper.findAll("nav").map((nav) => nav.attributes("aria-label"));
    expect(labels).toEqual(["Ansicht", "Woche wählen", "Monat wählen"]);
    wrapper.unmount();
  });

  describe("Export", () => {
    it("exportiert per Knopf und nennt den gespeicherten Pfad", async () => {
      exportCsv = () => Promise.resolve("/Users/jan/Documents/overtime-2026-11-21.csv");
      const wrapper = mount(App);
      await flushPromises();
      await wrapper.get('[data-test="export"]').trigger("click");
      await flushPromises();
      expect(calls).toContainEqual(["export_csv"]);
      expect(wrapper.get('[data-test="export-message"]').text()).toBe(
        "Export gespeichert: /Users/jan/Documents/overtime-2026-11-21.csv",
      );
    });

    it("zeigt nichts an, wenn der Speichern-Dialog abgebrochen wurde", async () => {
      const wrapper = mount(App);
      await flushPromises();
      await wrapper.get('[data-test="export"]').trigger("click");
      await flushPromises();
      expect(wrapper.find('[data-test="export-message"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="export-error"]').exists()).toBe(false);
    });

    it("zeigt die Meldung des Backends, wenn die Datei nicht gespeichert werden kann", async () => {
      exportCsv = () => Promise.reject("Die Datei konnte nicht gespeichert werden. Bitte einen anderen Speicherort wählen.");
      const wrapper = mount(App);
      await flushPromises();
      await wrapper.get('[data-test="export"]').trigger("click");
      await flushPromises();
      expect(wrapper.get('[data-test="export-error"]').text()).toBe(
        "Die Datei konnte nicht gespeichert werden. Bitte einen anderen Speicherort wählen.",
      );
    });

    it("sperrt den Knopf, solange der Dialog offen ist, und exportiert nur einmal", async () => {
      let finish: (path: string | null) => void = () => {};
      exportCsv = () => new Promise((resolve) => (finish = resolve));
      const wrapper = mount(App);
      await flushPromises();
      await wrapper.get('[data-test="export"]').trigger("click");
      await wrapper.get('[data-test="export"]').trigger("click");
      expect(wrapper.get('[data-test="export"]').attributes("disabled")).toBeDefined();
      expect(count("export_csv")).toBe(1);
      finish(null);
      await flushPromises();
      expect(wrapper.get('[data-test="export"]').attributes("disabled")).toBeUndefined();
    });

    it("blendet die Meldung des letzten Exports beim nächsten aus", async () => {
      exportCsv = () => Promise.reject("Die Datei konnte nicht gespeichert werden. Bitte einen anderen Speicherort wählen.");
      const wrapper = mount(App);
      await flushPromises();
      await wrapper.get('[data-test="export"]').trigger("click");
      await flushPromises();
      exportCsv = () => Promise.resolve("/tmp/overtime.csv");
      await wrapper.get('[data-test="export"]').trigger("click");
      await flushPromises();
      expect(wrapper.find('[data-test="export-error"]').exists()).toBe(false);
      expect(wrapper.get('[data-test="export-message"]').text()).toBe("Export gespeichert: /tmp/overtime.csv");
    });
  });

  it("nennt die Quelle der Feiertage in beiden Ansichten", async () => {
    const wrapper = mount(App, { attachTo: document.body });
    await flushPromises();
    expect(wrapper.get('[data-test="holiday-source"]').text()).toContain("OpenHolidays API (ODbL)");
    await wrapper.get('[data-test="view-month"]').trigger("click");
    await flushPromises();
    expect(isVisible(wrapper, "holiday-source")).toBe(true);
    wrapper.unmount();
  });
});
