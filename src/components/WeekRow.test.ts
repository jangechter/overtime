import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import WeekRow from "./WeekRow.vue";

const calls: unknown[][] = [];
let backend: (...args: unknown[]) => Promise<unknown> = async () => undefined;
const invoke = (...args: unknown[]) => {
  calls.push(args);
  return backend(...args);
};
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

async function fill(wrapper: ReturnType<typeof mount>, start: string, end: string, pause: string) {
  await wrapper.get('[data-test="start"]').setValue(start);
  await wrapper.get('[data-test="end"]').setValue(end);
  await wrapper.get('[data-test="break"]').setValue(pause);
  await flushPromises();
}

describe("WeekRow", () => {
  beforeEach(() => {
    calls.length = 0;
    backend = async () => undefined;
  });

  it("zeigt Wochentag und Datum", () => {
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    expect(wrapper.text()).toContain("Montag");
    expect(wrapper.text()).toContain("16.11.2026");
  });

  it("zeigt die vom Backend berechnete Tagesstunde", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "16:30", "30");
    expect(calls).toContainEqual(["calculate_day", { start: "08:00", end: "16:30", breakMinutes: 30 }]);
    expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("zeigt die Meldung des Backends bei ungültiger Eingabe", async () => {
    backend = async () => {
      throw "Das Ende muss nach dem Start liegen.";
    };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "16:00", "08:00", "0");
    expect(wrapper.get('[data-test="error"]').text()).toBe("Das Ende muss nach dem Start liegen.");
    expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
  });

  it("rechnet erst, wenn Start und Ende eingetragen sind", async () => {
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await wrapper.get('[data-test="start"]').setValue("08:00");
    await flushPromises();
    expect(calls).toEqual([]);
  });

  it("entfernt die Stunden, wenn das Ende wieder geleert wird", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "16:30", "30");
    await wrapper.get('[data-test="end"]').setValue("");
    await flushPromises();
    expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
  });

  it("schickt 0, wenn die Pause leer ist", async () => {
    backend = async () => 510;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await wrapper.get('[data-test="start"]').setValue("08:00");
    await wrapper.get('[data-test="end"]').setValue("16:30");
    await wrapper.get('[data-test="break"]').setValue("");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["calculate_day", { start: "08:00", end: "16:30", breakMinutes: 0 }]);
  });

  it("übernimmt nur die Antwort auf die letzte Eingabe", async () => {
    const pending: Array<(minutes: number) => void> = [];
    backend = () => new Promise((resolve) => pending.push(resolve as (minutes: number) => void));
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await wrapper.get('[data-test="start"]').setValue("08:00");
    await wrapper.get('[data-test="end"]').setValue("16:00");
    await wrapper.get('[data-test="break"]').setValue("30");
    expect(pending).toHaveLength(2);
    pending[1](450);
    await flushPromises();
    pending[0](480);
    await flushPromises();
    expect(wrapper.get('[data-test="hours"]').text()).toBe("7:30 h");
  });

  it("nimmt Uhrzeiten als 24-Stunden-Text HH:MM entgegen", () => {
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    for (const field of ["start", "end"]) {
      const input = wrapper.get(`[data-test="${field}"]`);
      expect(input.attributes("type")).toBe("text");
      expect(input.attributes("placeholder")).toBe("HH:MM");
      expect(input.attributes("maxlength")).toBe("5");
    }
  });

  it("zeigt die Backend-Meldung bei einer ungültigen Uhrzeit wie 25:00", async () => {
    backend = async () => {
      throw "Bitte Uhrzeiten im Format HH:MM eingeben.";
    };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "25:00", "0");
    expect(wrapper.get('[data-test="error"]').text()).toBe("Bitte Uhrzeiten im Format HH:MM eingeben.");
  });
});
