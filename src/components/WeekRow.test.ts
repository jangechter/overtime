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

  it("markiert einen Feiertag mit seinem Namen", () => {
    const wrapper = mount(WeekRow, { props: { date: "2026-12-25", holiday: "1. Weihnachtsfeiertag" } });
    expect(wrapper.get('[data-test="holiday"]').text()).toBe("1. Weihnachtsfeiertag");
    expect(wrapper.classes()).toContain("is-holiday");
  });

  it("markiert einen normalen Tag nicht als Feiertag", () => {
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    expect(wrapper.find('[data-test="holiday"]').exists()).toBe(false);
    expect(wrapper.classes()).not.toContain("is-holiday");
  });

  it("erlaubt an einem Feiertag weiterhin Eingaben", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-12-25", holiday: "1. Weihnachtsfeiertag" } });
    await fill(wrapper, "08:00", "16:30", "30");
    expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("zeigt die vom Backend berechnete Tagesstunde", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "16:30", "30");
    expect(calls).toContainEqual(["save_entry", { date: "2026-11-16", start: "08:00", end: "16:30", breakMinutes: 30 }]);
    expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
  });

  it("zeigt die Tagesstunden zusätzlich als Dezimalstunden", async () => {
    backend = async () => 510;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "17:00", "30");
    expect(wrapper.get('[data-test="hours"]').text()).toBe("8:30 h");
    expect(wrapper.get('[data-test="hours-decimal"]').text()).toBe("(8,50 h)");
  });

  it("meldet nach dem Speichern, dass sich die Tagesstunden geändert haben", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "16:30", "30");
    expect(wrapper.emitted("saved")).toBeDefined();
  });

  it("meldet auch, wenn ein gespeicherter Eintrag gelöscht wurde", async () => {
    backend = async () => 480;
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "08:00", "16:30", "30");
    const before = wrapper.emitted("saved")?.length ?? 0;
    await wrapper.get('[data-test="end"]').setValue("");
    await flushPromises();
    expect(calls).toContainEqual(["delete_entry", { date: "2026-11-16" }]);
    expect(wrapper.emitted("saved")?.length).toBe(before + 1);
  });

  it("meldet nichts, wenn das Speichern fehlschlägt", async () => {
    backend = async () => {
      throw "Das Ende muss nach dem Start liegen.";
    };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "16:00", "08:00", "0");
    expect(wrapper.emitted("saved")).toBeUndefined();
  });

  it("hält einen überholten, aber erfolgreichen Speichervorgang fest, auch wenn die neuere Eingabe fehlschlägt", async () => {
    const saves: Array<{ resolve: (minutes: number) => void; reject: (reason: string) => void }> = [];
    backend = (command) =>
      command === "save_entry"
        ? new Promise((resolve, reject) => saves.push({ resolve: resolve as (minutes: number) => void, reject }))
        : Promise.resolve(undefined);
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await wrapper.get('[data-test="start"]').setValue("08:00");
    await wrapper.get('[data-test="end"]').setValue("16:00");
    await wrapper.get('[data-test="end"]').setValue("07:00");
    expect(saves).toHaveLength(2);

    saves[0].resolve(480);
    await flushPromises();
    saves[1].reject("Das Ende muss nach dem Start liegen.");
    await flushPromises();
    expect(wrapper.emitted("saved")).toBeDefined();
    expect(wrapper.get('[data-test="error"]').text()).toContain("Der zuletzt gespeicherte Eintrag bleibt erhalten.");

    await wrapper.get('[data-test="end"]').setValue("");
    await flushPromises();
    expect(calls).toContainEqual(["delete_entry", { date: "2026-11-16" }]);
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
    expect(calls[calls.length - 1]).toEqual(["save_entry", { date: "2026-11-16", start: "08:00", end: "16:30", breakMinutes: 0 }]);
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

  it("zeigt einen gespeicherten Eintrag mit Eingaben und Stunden", () => {
    const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
    expect((wrapper.get('[data-test="start"]').element as HTMLInputElement).value).toBe("08:00");
    expect((wrapper.get('[data-test="end"]').element as HTMLInputElement).value).toBe("16:30");
    expect((wrapper.get('[data-test="break"]').element as HTMLInputElement).value).toBe("30");
    expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
    expect(calls).toEqual([]);
  });

  it("löscht den gespeicherten Eintrag, wenn das Ende geleert wird", async () => {
    const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
    await wrapper.get('[data-test="end"]').setValue("");
    await flushPromises();
    expect(calls).toEqual([["delete_entry", { date: "2026-11-16" }]]);
  });

  it("speichert einen ungültigen Eintrag nicht und zeigt die Meldung", async () => {
    backend = async (command) => {
      if (command === "save_entry") throw "Das Ende muss nach dem Start liegen.";
    };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
    await fill(wrapper, "16:00", "08:00", "0");
    expect(calls.some(([command]) => command === "delete_entry")).toBe(false);
    expect(wrapper.get('[data-test="error"]').text()).toBe("Das Ende muss nach dem Start liegen.");
  });

  it("weist bei einem gespeicherten Tag darauf hin, dass der alte Eintrag erhalten bleibt", async () => {
    backend = async () => {
      throw "Das Ende muss nach dem Start liegen.";
    };
    const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
    const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
    await wrapper.get('[data-test="end"]').setValue("07:00");
    await flushPromises();
    expect(wrapper.get('[data-test="error"]').text()).toBe(
      "Das Ende muss nach dem Start liegen. Der zuletzt gespeicherte Eintrag bleibt erhalten.",
    );
  });

  describe("Krank/Urlaub", () => {
    const select = (wrapper: ReturnType<typeof mount>) => wrapper.get('[data-test="kind"]');
    const kindOf = (wrapper: ReturnType<typeof mount>) => (select(wrapper).element as HTMLSelectElement).value;

    it("bietet Arbeit, Krank und Urlaub zur Auswahl, voreingestellt ist Arbeit", () => {
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      expect(select(wrapper).findAll("option").map((option) => option.text())).toEqual(["Arbeit", "Krank", "Urlaub"]);
      expect(kindOf(wrapper)).toBe("work");
      expect(wrapper.find('[data-test="start"]').exists()).toBe(true);
    });

    it("markiert einen Tag als krank, blendet die Zeiten aus und zeigt die angerechneten Stunden", async () => {
      backend = async () => 480;
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      expect(calls).toEqual([["mark_absence", { date: "2026-11-16", kind: "sick" }]]);
      expect(wrapper.find('[data-test="start"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="end"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="break"]').exists()).toBe(false);
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
      expect(wrapper.emitted("saved")).toHaveLength(1);
    });

    it("zeigt die Stunden, die das Backend anrechnet, z. B. 0:00 h an einem Feiertag", async () => {
      backend = async () => 0;
      const wrapper = mount(WeekRow, { props: { date: "2026-12-25", holiday: "1. Weihnachtsfeiertag" } });
      await select(wrapper).setValue("vacation");
      await flushPromises();
      expect(wrapper.get('[data-test="hours"]').text()).toBe("0:00 h");
    });

    it("speichert beim Zurückwechseln auf Arbeit die zuvor eingetragenen Zeiten wieder", async () => {
      backend = async () => 480;
      const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      await select(wrapper).setValue("work");
      await flushPromises();
      expect(calls).toEqual([
        ["mark_absence", { date: "2026-11-16", kind: "sick" }],
        ["save_entry", { date: "2026-11-16", start: "08:00", end: "16:30", breakMinutes: 30 }],
      ]);
      expect((wrapper.get('[data-test="start"]').element as HTMLInputElement).value).toBe("08:00");
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
    });

    it("behält nach einem fehlgeschlagenen Markieren die Zeiten und löscht beim Zurückwechseln nichts", async () => {
      backend = async (command) => {
        if (command === "mark_absence") throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
        return 480;
      };
      const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      expect(wrapper.get('[data-test="error"]').text()).toContain("Der zuletzt gespeicherte Eintrag bleibt erhalten.");
      await select(wrapper).setValue("work");
      await flushPromises();
      expect(calls.some(([command]) => command === "delete_entry")).toBe(false);
      expect(calls[calls.length - 1]).toEqual(["save_entry", { date: "2026-11-16", start: "08:00", end: "16:30", breakMinutes: 30 }]);
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
      expect(wrapper.find('[data-test="error"]').exists()).toBe(false);
    });

    it("weist darauf hin, dass die Markierung erhalten bleibt, wenn das Entfernen fehlschlägt", async () => {
      backend = async () => {
        throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
      };
      const entry = { kind: "sick" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("work");
      await flushPromises();
      expect(wrapper.get('[data-test="error"]').text()).toBe(
        "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten. Der zuletzt gespeicherte Eintrag bleibt erhalten.",
      );
      expect(wrapper.emitted("saved")).toBeUndefined();
    });

    it("löscht die Markierung auch, wenn schon vor der Antwort auf das Markieren wieder Arbeit gewählt wird", async () => {
      const marks: Array<(minutes: number) => void> = [];
      backend = (command) =>
        command === "mark_absence"
          ? new Promise((resolve) => marks.push(resolve as (minutes: number) => void))
          : Promise.resolve(undefined);
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      await select(wrapper).setValue("work");
      await flushPromises();
      expect(calls).toEqual([
        ["mark_absence", { date: "2026-11-16", kind: "sick" }],
        ["delete_entry", { date: "2026-11-16" }],
      ]);
      marks[0](480);
      await flushPromises();
      expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
    });

    it("löscht auch einen Arbeitseintrag, dessen Speichern noch läuft, wenn das Ende geleert wird", async () => {
      const saves: Array<(minutes: number) => void> = [];
      backend = (command) =>
        command === "save_entry"
          ? new Promise((resolve) => saves.push(resolve as (minutes: number) => void))
          : Promise.resolve(undefined);
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      await fill(wrapper, "08:00", "16:30", "30");
      await wrapper.get('[data-test="end"]').setValue("");
      await flushPromises();
      expect(calls[calls.length - 1]).toEqual(["delete_entry", { date: "2026-11-16" }]);
      saves.forEach((resolve) => resolve(480));
      await flushPromises();
    });

    it("ersetzt eingetragene Zeiten durch die Markierung Urlaub, ohne sie zu löschen oder neu zu speichern", async () => {
      backend = async () => 480;
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      await fill(wrapper, "08:00", "16:30", "30");
      calls.length = 0;
      await select(wrapper).setValue("vacation");
      await flushPromises();
      expect(calls).toEqual([["mark_absence", { date: "2026-11-16", kind: "vacation" }]]);
    });

    it("zeigt einen gespeicherten Urlaubstag mit Auswahl und Stunden, ohne das Backend zu fragen", () => {
      const entry = { kind: "vacation" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      expect(kindOf(wrapper)).toBe("vacation");
      expect(wrapper.find('[data-test="start"]').exists()).toBe(false);
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
      expect(calls).toEqual([]);
    });

    it("entfernt die Markierung, wenn wieder Arbeit gewählt wird, und zeigt leere Felder", async () => {
      const entry = { kind: "sick" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("work");
      await flushPromises();
      expect(calls).toEqual([["delete_entry", { date: "2026-11-16" }]]);
      expect(wrapper.emitted("saved")).toHaveLength(1);
      expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
      expect((wrapper.get('[data-test="start"]').element as HTMLInputElement).value).toBe("");
      expect((wrapper.get('[data-test="end"]').element as HTMLInputElement).value).toBe("");
    });

    it("wechselt direkt von Krank zu Urlaub", async () => {
      backend = async () => 480;
      const entry = { kind: "sick" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("vacation");
      await flushPromises();
      expect(calls).toEqual([["mark_absence", { date: "2026-11-16", kind: "vacation" }]]);
    });

    it("zeigt die Meldung des Backends, wenn das Markieren fehlschlägt", async () => {
      backend = async () => {
        throw "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.";
      };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16" } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      expect(wrapper.get('[data-test="error"]').text()).toBe(
        "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.",
      );
      expect(wrapper.find('[data-test="hours"]').exists()).toBe(false);
      expect(wrapper.emitted("saved")).toBeUndefined();
    });

    it("übernimmt neu angerechnete Stunden, wenn der gespeicherte Eintrag neu geladen wird", async () => {
      const entry = { kind: "vacation" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2027-03-29", entry } });
      await wrapper.setProps({ entry: { ...entry, minutes: 0 } });
      expect(wrapper.get('[data-test="hours"]').text()).toBe("0:00 h");
      expect(calls).toEqual([]);
    });

    it("lässt einen Arbeitstag unverändert, wenn sein Eintrag neu geladen wird", async () => {
      const entry = { kind: "work" as const, start: "08:00", end: "16:30", breakMinutes: 30, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await wrapper.setProps({ entry: { ...entry, end: "12:00", minutes: 240 } });
      await flushPromises();
      expect((wrapper.get('[data-test="end"]').element as HTMLInputElement).value).toBe("16:30");
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
      expect(calls).toEqual([]);
    });

    it("übernimmt keine Stunden eines neu geladenen Eintrags, dessen Art inzwischen geändert wurde", async () => {
      backend = async () => 480;
      const entry = { kind: "vacation" as const, start: null, end: null, breakMinutes: null, minutes: 480 };
      const wrapper = mount(WeekRow, { props: { date: "2026-11-16", entry } });
      await select(wrapper).setValue("sick");
      await flushPromises();
      await wrapper.setProps({ entry: { ...entry, minutes: 0 } });
      expect(wrapper.get('[data-test="hours"]').text()).toBe("8:00 h");
    });
  });
});
