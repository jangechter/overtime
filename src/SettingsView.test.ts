import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import SettingsView from "./SettingsView.vue";

const calls: unknown[][] = [];
const settings = (daily: number, weekly: number) => ({ dailyMinutes: daily, weeklyMinutes: weekly, subdivision: "DE-NW" });
/** Antwort von `load_settings`/`save_settings`: Werte der laufenden Woche und eine geplante Änderung. */
const overview = (current = settings(480, 2400), pending: { validFrom: string; settings: unknown } | null = null) => ({
  current,
  pending,
});
let backend: (...args: unknown[]) => Promise<unknown> = async () => overview();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => {
    calls.push(args);
    return backend(...args);
  },
}));

const text = (wrapper: ReturnType<typeof mount>, test: string) => wrapper.get(`[data-test="${test}"]`).text();
const value = (wrapper: ReturnType<typeof mount>, test: string) =>
  (wrapper.get(`[data-test="${test}"]`).element as HTMLInputElement).value;

describe("SettingsView", () => {
  beforeEach(() => {
    calls.length = 0;
    backend = async () => overview();
  });

  it("zeigt die Werte dieser Woche und trägt sie in die Felder ein", async () => {
    const wrapper = mount(SettingsView);
    await flushPromises();
    expect(calls).toEqual([["load_settings"]]);
    expect(text(wrapper, "settings-current")).toBe("Diese Woche: Tagessoll 8:00 h, Wochen-Soll 40:00 h");
    expect(wrapper.find('[data-test="settings-pending"]').exists()).toBe(false);
    expect(value(wrapper, "daily")).toBe("8:00");
    expect(value(wrapper, "weekly")).toBe("40:00");
  });

  it("zeigt eine geplante Änderung mit ihrem Montag und trägt ihre Werte in die Felder ein", async () => {
    backend = async () => overview(settings(480, 2400), { validFrom: "2026-11-30", settings: settings(450, 1920) });
    const wrapper = mount(SettingsView);
    await flushPromises();
    expect(text(wrapper, "settings-pending")).toBe("Ab Montag, 30.11.2026: Tagessoll 7:30 h, Wochen-Soll 32:00 h");
    expect(value(wrapper, "daily")).toBe("7:30");
    expect(value(wrapper, "weekly")).toBe("32:00");
  });

  it("speichert die Eingaben, zeigt die Antwort des Backends und meldet das Speichern", async () => {
    const wrapper = mount(SettingsView);
    await flushPromises();
    backend = async () => overview(settings(480, 2400), { validFrom: "2026-11-30", settings: settings(480, 1920) });
    await wrapper.get('[data-test="weekly"]').setValue("32:00");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(calls[calls.length - 1]).toEqual(["save_settings", { daily: "8:00", weekly: "32:00" }]);
    expect(text(wrapper, "settings-pending")).toBe("Ab Montag, 30.11.2026: Tagessoll 8:00 h, Wochen-Soll 32:00 h");
    expect(text(wrapper, "settings-current")).toBe("Diese Woche: Tagessoll 8:00 h, Wochen-Soll 40:00 h");
    expect(wrapper.emitted("saved")).toHaveLength(1);
  });

  it("zeigt die Meldung des Backends bei einer ungültigen Eingabe und meldet nichts", async () => {
    const wrapper = mount(SettingsView);
    await flushPromises();
    backend = async () => {
      throw "Bitte das Tagessoll als Stunden und Minuten eingeben, z. B. 8:00.";
    };
    await wrapper.get('[data-test="daily"]').setValue("acht");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(text(wrapper, "settings-save-error")).toBe("Bitte das Tagessoll als Stunden und Minuten eingeben, z. B. 8:00.");
    expect(value(wrapper, "daily")).toBe("acht");
    expect(wrapper.emitted("saved")).toBeUndefined();
  });

  it("blendet die Fehlermeldung nach erfolgreichem Speichern aus", async () => {
    const wrapper = mount(SettingsView);
    await flushPromises();
    backend = async () => {
      throw "Das Tagessoll muss zwischen 0:01 und 24:00 h liegen.";
    };
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    backend = async () => overview();
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.find('[data-test="settings-save-error"]').exists()).toBe(false);
  });

  it("sperrt den Knopf während des Speicherns und speichert nur einmal", async () => {
    const wrapper = mount(SettingsView);
    await flushPromises();
    let finish: (answer: unknown) => void = () => {};
    backend = () => new Promise((resolve) => (finish = resolve));
    await wrapper.get("form").trigger("submit");
    await wrapper.get("form").trigger("submit");
    expect(wrapper.get('[data-test="save-settings"]').attributes("disabled")).toBeDefined();
    expect(calls.filter(([command]) => command === "save_settings")).toHaveLength(1);
    finish(overview());
    await flushPromises();
    expect(wrapper.get('[data-test="save-settings"]').attributes("disabled")).toBeUndefined();
  });

  it("zeigt eine Meldung, wenn die Einstellungen nicht gelesen werden können", async () => {
    backend = async () => {
      throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
    };
    const wrapper = mount(SettingsView);
    await flushPromises();
    expect(text(wrapper, "settings-load-error")).toBe(
      "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.",
    );
    expect(wrapper.find("form").exists()).toBe(false);
  });
});
