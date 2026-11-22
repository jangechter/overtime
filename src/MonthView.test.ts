import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent, h, KeepAlive, ref } from "vue";
import MonthView from "./MonthView.vue";
import { currentMonth, shiftMonth } from "./month";

const calls: unknown[][] = [];
/** Antwort von `month_summary` in Minuten. */
const monthAnswer = (actual = 0, average: number | null = null, days = 0, other = 0, missing = false) => ({
  actualMinutes: actual,
  averageMinutes: average,
  countedDays: days,
  otherMinutes: other,
  holidaysMissing: missing,
});
let backend: (...args: unknown[]) => Promise<unknown> = async () => monthAnswer();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => {
    calls.push(args);
    return backend(...args);
  },
}));

/** Die Monatsansicht wie in der App: in KeepAlive, damit sie beim Öffnen lädt; `shown` schaltet sie aus und ein. */
function mountInKeepAlive() {
  const shown = ref(true);
  const wrapper = mount(defineComponent({ render: () => h(KeepAlive, null, [shown.value ? h(MonthView) : null]) }));
  return { wrapper, shown };
}

const text = (wrapper: ReturnType<typeof mount>, test: string) => wrapper.get(`[data-test="${test}"]`).text();

describe("MonthView", () => {
  beforeEach(() => {
    calls.length = 0;
    backend = async () => monthAnswer();
  });

  it("lädt beim Öffnen den aktuellen Monat und nennt ihn", async () => {
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(calls).toEqual([["month_summary", { date: currentMonth() }]]);
    const [year, month] = currentMonth().split("-").map(Number);
    const expected = new Date(year, month - 1, 1).toLocaleDateString("de-DE", { month: "long", year: "numeric" });
    expect(text(wrapper, "month-label")).toBe(expected);
  });

  it("zeigt Monatssumme und Durchschnitt mit der Zahl der gezählten Arbeitstage", async () => {
    backend = async () => monthAnswer(9360, 492, 19);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-total")).toBe("Monatssumme 156:00 h (156,00 h)");
    expect(text(wrapper, "month-average")).toBe("Durchschnitt pro Arbeitstag 8:12 h (8,20 h) an 19 Arbeitstagen");
  });

  it("schreibt einen einzelnen Arbeitstag in der Einzahl", async () => {
    backend = async () => monthAnswer(480, 480, 1);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-average")).toBe("Durchschnitt pro Arbeitstag 8:00 h (8,00 h) an 1 Arbeitstag");
  });

  it("zeigt für einen Monat ohne Arbeitstag keinen Durchschnitt", async () => {
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-total")).toBe("Monatssumme 0:00 h (0,00 h)");
    expect(text(wrapper, "month-average")).toBe("Durchschnitt pro Arbeitstag – kein Arbeitstag mit Eintrag");
  });

  it("nennt Stunden am Wochenende oder an Feiertagen, die nicht im Durchschnitt stecken", async () => {
    backend = async () => monthAnswer(840, 480, 1, 360);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-other")).toBe("davon 6:00 h am Wochenende oder an Feiertagen (nicht im Durchschnitt)");
  });

  it("zeigt den Hinweis zu Wochenende und Feiertagen nicht, wenn dort nichts gearbeitet wurde", async () => {
    backend = async () => monthAnswer(480, 480, 1, 0);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(wrapper.find('[data-test="month-other"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="month-holidays-missing"]').exists()).toBe(false);
  });

  it("weist darauf hin, wenn für gezählte Tage Feiertage fehlen", async () => {
    backend = async () => monthAnswer(480, 480, 1, 0, true);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-holidays-missing")).toContain("Feiertage");
  });

  it("zeigt die Meldung des Backends, wenn der Monat nicht gelesen werden kann", async () => {
    backend = async () => {
      throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
    };
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    expect(text(wrapper, "month-error")).toBe("Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.");
    expect(wrapper.find('[data-test="month-total"]').exists()).toBe(false);
  });

  it("blättert zum vorherigen und nächsten Monat und mit „Heute“ zurück", async () => {
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    await wrapper.get('[data-test="prev-month"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["month_summary", { date: shiftMonth(currentMonth(), -1) }]);
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["month_summary", { date: shiftMonth(currentMonth(), 1) }]);
    await wrapper.get('[data-test="this-month"]').trigger("click");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["month_summary", { date: currentMonth() }]);
  });

  it("lädt mit „Heute“ im aktuellen Monat nicht erneut", async () => {
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    await wrapper.get('[data-test="this-month"]').trigger("click");
    await flushPromises();
    expect(calls).toHaveLength(1);
  });

  it("übernimmt beim schnellen Blättern nur die Antwort des zuletzt gewählten Monats", async () => {
    const pending: Array<(answer: unknown) => void> = [];
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    backend = () => new Promise((resolve) => pending.push(resolve));
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await wrapper.get('[data-test="next-month"]').trigger("click");
    pending[1](monthAnswer(240, 240, 1));
    await flushPromises();
    pending[0](monthAnswer(9999, 480, 20));
    await flushPromises();
    expect(text(wrapper, "month-total")).toBe("Monatssumme 4:00 h (4,00 h)");
  });

  it("zeigt keinen Fehler einer überholten Anfrage", async () => {
    const pending: Array<{ resolve: (answer: unknown) => void; reject: (reason: unknown) => void }> = [];
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    backend = () => new Promise((resolve, reject) => pending.push({ resolve, reject }));
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await wrapper.get('[data-test="next-month"]').trigger("click");
    pending[1].resolve(monthAnswer(240, 240, 1));
    await flushPromises();
    pending[0].reject("Veralteter Fehler");
    await flushPromises();
    expect(wrapper.find('[data-test="month-error"]').exists()).toBe(false);
    expect(text(wrapper, "month-total")).toBe("Monatssumme 4:00 h (4,00 h)");
  });

  it("blendet einen Ladefehler aus, sobald der Monat wieder gelesen werden kann", async () => {
    backend = async () => {
      throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
    };
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    backend = async () => monthAnswer(480, 480, 1);
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="month-error"]').exists()).toBe(false);
    expect(text(wrapper, "month-total")).toBe("Monatssumme 8:00 h (8,00 h)");
  });

  it("zeigt beim Blättern keine Zahlen des vorherigen Monats, solange der neue lädt", async () => {
    backend = async () => monthAnswer(9360, 492, 19);
    const { wrapper } = mountInKeepAlive();
    await flushPromises();
    backend = () => new Promise(() => {});
    await wrapper.get('[data-test="next-month"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="month-total"]').exists()).toBe(false);
  });

  it("liest den Monat beim erneuten Öffnen neu und behält den gewählten Monat", async () => {
    const { wrapper, shown } = mountInKeepAlive();
    await flushPromises();
    await wrapper.get('[data-test="prev-month"]').trigger("click");
    await flushPromises();
    shown.value = false;
    await flushPromises();
    backend = async () => monthAnswer(480, 480, 1);
    shown.value = true;
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["month_summary", { date: shiftMonth(currentMonth(), -1) }]);
    expect(text(wrapper, "month-total")).toBe("Monatssumme 8:00 h (8,00 h)");
  });
});
