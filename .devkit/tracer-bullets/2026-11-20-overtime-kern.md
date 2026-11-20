# Tracer bullets: Overtime – Kern (Erfassen, Speichern, Feiertage, Wochen-Soll)

- Date: 2026-11-20
- Source: .devkit/grill-me/2026-11-19-overtime-zeiterfassung.md
- Entry point: Tauri-Desktopfenster mit Vue-Wochenübersicht (UI; noch nicht im Projekt vorhanden)

## How to work with this file

1. Take the first slice with `Status: todo` whose `Depends on` slices are all `done`. Set it to `in progress`.
2. Implement it including its tests. Stay within the slice.
3. Set `Status: done` only when the slice's tests pass (shown in tool output), the `Demo` steps were executed,
   and nothing was skipped. If the demo could not be executed, keep `in progress` and say why under `Notes`.
4. If a slice turns out too big, split it: `S2` becomes `S2a`, `S2b`, each with `Added: split from S2 – <reason>`;
   mark `S2` as `Status: split`. If a slice is missing, add it with the next free number and
   `Added: during S<k> – <reason>`. Never delete or renumber existing slices.
5. Every added or split slice must meet the same rules: end to end through the entry point, tested, demonstrable.

## Assumptions
- Die Woche läuft Montag bis Sonntag; Soll (8 h/Tag, 40 h/Woche) gilt nur Montag bis Freitag.
- Testwerkzeuge sind noch nicht gewählt; vermutlich `cargo test` für Rust und ein Vue-Komponententest. Es gibt keine `CLAUDE.md` mit `## Devkit`-Sektion (`/devkit:devkit-setup` nach dem ersten Code).
- Rust (`cargo`, `rustc`) und die Tauri-Voraussetzungen sind auf dem Rechner nicht installiert und müssen vor S1 eingerichtet werden. Node ist vorhanden.
- Dieser Plan ist Plan A von zwei. Plan B (Krank/Urlaub, Einstellungen für Soll/Bundesland, Monatssumme und Durchschnitt, CSV-Export) bekommt eine eigene Datei, sobald Plan A steht.

## Decided (nach Erstellung des Plans, 2026-11-20)
- Feiertags-API: OpenHolidays (`https://openholidaysapi.org/PublicHolidays?countryIsoCode=DE&subdivisionCode=DE-NW&languageIsoCode=DE&validFrom=…&validTo=…`). Gründe: ein Aufruf deckt den Zeitraum ab (auch über den Jahreswechsel), kein Schlüssel, laut FAQ kostenlos und kommerziell erlaubt, Lizenz ODbL (Quelle nennen), dokumentierte Datenquellen. Verworfen: Nager.Date (Nutzung kommerziell nur mit Sponsoring, Nutzungsbedingungen jederzeit änderbar, Doku wechselt auf v4/neue Domain) und feiertage-api.de (nur pro Jahr abfragbar, keine Lizenz genannt, Privatdienst auf Wikipedia-Basis). Alle drei lieferten für NRW 2026 dieselben 11 Feiertage.
- Lücken über 2 Wochen: Die App lädt beim Start nur das feste Fenster (−2 Wochen bis +2 Monate). Für eine angezeigte Woche ohne geladene Feiertage erscheint eine Warnung „Feiertage für diese Woche fehlen“ mit dem Knopf „Feiertage laden“, der genau diese Woche von der API holt und speichert. Ohne Netz bleibt die Warnung, es erscheint eine Meldung, die App rechnet weiter. Das Soll wird für solche Wochen nicht automatisch gesenkt, solange die Feiertage fehlen (betrifft S5).
- Die App merkt sich, welche Zeiträume geladen wurden (eigene Tabelle in SQLite); sonst ist „keine Feiertage in dieser Woche“ nicht von „nie geladen“ zu unterscheiden.

## Open
- Reihenfolge der Teile nach der Speicherung war nicht festgelegt; die Reihenfolge S3 bis S5 ist nur nach Risiko und Abhängigkeit sortiert (betrifft S3, S4, S5).
- Grund für die Berechnung im Rust-Backend und Gründe gegen SwiftUI/Electron – nicht genannt (kein Slice betroffen).

## Not checked
- Verfügbarkeit und Ausfallhistorie von OpenHolidays (nicht gemessen); ob die ODbL-Quellenangabe in der App an einer bestimmten Stelle stehen muss (nur FAQ gelesen, Lizenztext nicht).
- Ob die Tauri-Voraussetzungen auf dem Mac vollständig sind (`cargo` und `rustc` fehlen laut Prüfung).

## S1: Wochenübersicht mit Eingabe und Tagesstunden
- Goal: In einer Wochenübersicht trägt man pro Tag Start, Ende und Pause (Minuten) ein und sieht die Tagesstunden, berechnet im Rust-Backend.
- Risk: 3 (Tauri, Rust und Vue sind im leeren Projekt neu), 4 (Schnittstelle zwischen Vue und Rust-Befehlen, spätere Slices hängen daran)
- Demo: App starten; die aktuelle Woche Montag bis Sonntag erscheint. Bei einem Tag Start 08:00, Ende 16:30, Pause 30 eingeben; die Tagesstunde zeigt 8:00 h. Ungültige Eingabe (Ende vor Start, Pause länger als die Arbeitszeit) zeigt eine Meldung.
- Tests: Rust-Unit-Tests für die Stundenberechnung (normaler Tag, Pause 0, Ende vor Start, Pause zu lang); Vue-Komponententest für die Wochenzeile.
- Touches: none yet (Projekt ist leer)
- Depends on: none
- Open: Reihenfolge nach der Speicherung
- Status: done
- Notes: Tests grün (Rust 9/9, Vue 13/13), Demo vom Nutzer im Fenster bestätigt. Ende == Start gilt als ungültig (Annahme). Uhrzeiten sind Textfelder im Format HH:MM (24 h), weil `type="time"` der Systemeinstellung folgt. CSP ist noch `null` (bewusst zurückgestellt).

## S2: Einträge in SQLite speichern
- Goal: Eingetragene Tage bleiben nach dem Neustart der App erhalten.
- Risk: 3 (SQLite im Projekt neu), 4 (Datenmodell, auf das alle weiteren Slices aufbauen)
- Demo: In der Wochenübersicht einen Tag eintragen, App beenden und neu starten; der Eintrag ist noch da, mit denselben Tagesstunden.
- Tests: Rust-Tests für Schreiben und Lesen eines Eintrags gegen eine temporäre Datenbank; Test, dass ein überschriebener Eintrag den alten ersetzt.
- Touches: none yet (Projekt ist leer)
- Depends on: S1
- Open: none
- Status: done
- Notes: Tests grün (Rust 20/20, Vue 19/19), Demo vom Nutzer im Fenster bestätigt. Gespeichert werden nur gültige Einträge; ein ungültig gewordener Eintrag überschreibt den gespeicherten nicht. `save_entry` ersetzt `calculate_day`. `load_entries(from, to)` lädt schon beliebige Zeiträume, S3 braucht dafür nur noch die Navigation und den Jahreswechsel-Test.

## S3: Zwischen Wochen blättern
- Goal: Man wechselt in der Wochenübersicht zwischen Wochen und sieht die jeweils gespeicherten Tage.
- Risk: none (Kriterien 1 bis 4 geprüft: keine offene Frage, keine externe API, keine neue Technik über S1/S2 hinaus, keine Entscheidung, auf der weitere Slices aufbauen)
- Demo: In der aktuellen Woche einen Tag eintragen, auf „vorherige Woche" und zurück wechseln; die Woche davor ist leer bzw. zeigt ihre Einträge, die aktuelle Woche zeigt den Eintrag wieder.
- Tests: Rust-Test für das Laden einer Woche aus der Datenbank (auch über Jahreswechsel); Vue-Test für die Navigation.
- Touches: none yet (Projekt ist leer)
- Depends on: S2
- Open: Reihenfolge nach der Speicherung
- Status: done
- Notes: Tests grün (Rust 21/21, Vue 32/32), Demo vom Nutzer im Fenster bestätigt. Über den ursprünglichen Plan hinaus gibt es einen „Heute“-Knopf (auf Wunsch des Nutzers), der aus jeder Woche in die aktuelle springt; in der aktuellen Woche lädt er nicht neu.

## S4: Feiertage NRW laden, speichern und anzeigen
- Goal: Feiertage für NRW werden von OpenHolidays für die letzten 2 Wochen und die nächsten 2 Monate geholt, lokal gespeichert und in der Wochenübersicht markiert. Für eine Woche ohne geladene Feiertage erscheint eine Warnung mit dem Knopf „Feiertage laden“, der genau diese Woche nachlädt. Ohne Netz erscheint nur eine Warnung bzw. Meldung, die App rechnet weiter.
- Risk: 2 (externe API, im Projekt noch nie angebunden), 3 (HTTP-Aufruf aus Rust neu), 4 (Tabelle der geladenen Zeiträume und die gespeicherten Feiertage; S5 baut darauf auf)
- Demo: App mit Netz starten und in eine Woche mit einem NRW-Feiertag blättern; der Tag ist als Feiertag markiert. Netz trennen und App neu starten; gespeicherte Feiertage bleiben sichtbar. In eine Woche außerhalb des Fensters blättern: Warnung „Feiertage für diese Woche fehlen“ mit Knopf. Ohne Netz auf „Feiertage laden“: Meldung, Warnung bleibt, die App rechnet weiter. Mit Netz auf „Feiertage laden“: Feiertage erscheinen markiert, Warnung verschwindet.
- Tests: Rust-Tests mit gemockter API-Antwort (Parsen inkl. `nationwide`/`regionalScope`, Speichern, Zeitfenster −2 Wochen bis +2 Monate, auch über den Jahreswechsel); Test für den Ausfall der API (Warnung statt Fehler); Rust-Test für die Tabelle der geladenen Zeiträume (geladen vs. nie geladen, Woche ohne Feiertage gilt als geladen); Vue-Tests für die Markierung, die Warnung und den Knopf „Feiertage laden“ (Erfolg, Ausfall).
- Touches: src-tauri/src/store.rs (Schema, Feiertage und geladene Zeiträume), src-tauri/src/lib.rs (neue Commands), src/WeekView.vue, src/components/WeekRow.vue; neue Rust-Abhängigkeit für HTTP
- Depends on: S2, S3
- Open: Reihenfolge nach der Speicherung
- Status: split
- Notes: API und Lückenregel sind entschieden (siehe „Decided“ oben). Die Quelle „OpenHolidays API (ODbL)“ in der App nennen. Beim Parsen beachten: Einträge haben `regionalScope: "Regional"` neben `nationwide: true`; Zeiträume über 3 Jahre lehnt die API mit HTTP 400 ab (für unser Fenster irrelevant). Der Knopf „Feiertage laden“ geht über den ursprünglichen Plan hinaus (Wunsch des Nutzers). Aufgeteilt in S4a (Fenster laden, speichern, markieren) und S4b (Warnung und Nachladen per Knopf), siehe dort.
- Added: none (Originalslice); aufgeteilt am 2026-11-20 in S4a und S4b – zu groß für einen Slice (HTTP, Tabelle der geladenen Zeiträume, Fenster laden, Markierung, Warnung und Knopf), eine Demo pro Hälfte ist klarer.

## S4a: Feiertage NRW für das Fenster laden, speichern und markieren
- Goal: Beim Start holt die App von OpenHolidays die NRW-Feiertage für die letzten 2 Wochen und die nächsten 2 Monate, speichert sie lokal samt Tabelle der geladenen Zeiträume und markiert sie in der Wochenübersicht. Schlägt der Abruf fehl (kein Netz), erscheint nur eine Warnung „Feiertage konnten nicht aktualisiert werden“, die App rechnet weiter und zeigt die gespeicherten Feiertage.
- Risk: 2 (externe API, im Projekt noch nie angebunden), 3 (HTTP-Aufruf aus Rust neu), 4 (Schema für Feiertage und geladene Zeiträume; S4b und S5 bauen darauf auf)
- Demo: App mit Netz starten und in eine Woche mit einem NRW-Feiertag im Fenster blättern (z. B. 25.12.2026); der Tag ist als Feiertag markiert. Netz trennen und App neu starten; die gespeicherten Feiertage bleiben markiert, oben erscheint die Warnung, Eingabe und Tagesstunden funktionieren weiter.
- Tests: Rust-Tests mit gemockter API-Antwort (Parsen inkl. `nationwide`/`regionalScope`, Speichern, Zeitfenster −2 Wochen bis +2 Monate, auch über den Jahreswechsel); Rust-Test für die Tabelle der geladenen Zeiträume (nach dem Laden gilt das Fenster als geladen, auch eine Woche ohne Feiertage); Test für den Ausfall der API (Warnung statt Fehler, gespeicherte Feiertage bleiben); Vue-Tests für die Markierung und die Warnung.
- Touches: src-tauri/src/store.rs (Schema), neues Modul für den API-Abruf, src-tauri/src/lib.rs (Commands, Aufruf beim Start), src/WeekView.vue, src/components/WeekRow.vue; neue Rust-Abhängigkeit für HTTP
- Depends on: S2, S3
- Open: Reihenfolge nach der Speicherung
- Status: done
- Added: split from S4 – zu groß für einen Slice; S4a ist der durchgehende Weg API → Speicher → Anzeige.
- Notes: API und Lückenregel sind entschieden (siehe „Decided“ oben). Die Quelle „OpenHolidays API (ODbL)“ in der App nennen. Beim Parsen beachten: Einträge haben `regionalScope: "Regional"` neben `nationwide: true`; Zeiträume über 3 Jahre lehnt die API mit HTTP 400 ab (für das Fenster irrelevant). Der HTTP-Abruf muss in Tests ersetzbar sein (gemockte Antwort).
  Tests grün (Rust 45/45 plus 1 ignorierter Live-Test, Vue 46/46, `npm run build` und `cargo build` ohne Fehler); echter Abruf einmal mit `cargo test live_refresh -- --ignored` gegen OpenHolidays geprüft (3 Feiertage für 2026-12-06 bis 2027-02-20). Demo vom Nutzer im Fenster bestätigt. HTTP über `ureq` (10 s Timeout), Daten über `chrono`. Der Abruf läuft in einem eigenen Thread (`spawn_blocking`) und sperrt den Datenspeicher erst zum Speichern. Es zählt nur `startDate` (öffentliche Feiertage sind eintägig). `save_holidays` fasst überlappende geladene Zeiträume zu einem zusammen (aus dem Review: die Tabelle wuchs sonst mit jedem Starttag, und eine nur von mehreren Fenstern gemeinsam abgedeckte Woche hätte als „nie geladen“ gegolten); `Store::is_loaded` prüft, ob ein einzelner Zeitraum die Woche abdeckt. Nur aneinandergrenzende, nicht überlappende Zeiträume (z. B. Sonntag/Montag) werden nicht zusammengefasst; S4b muss das bei nachgeladenen einzelnen Wochen beachten.

## S4b: Warnung für fehlende Feiertage und Nachladen per Knopf
- Goal: Zeigt die Wochenübersicht eine Woche, für die keine Feiertage geladen wurden (außerhalb des Fensters), erscheint die Warnung „Feiertage für diese Woche fehlen“ mit dem Knopf „Feiertage laden“. Der Klick holt genau diese Woche von OpenHolidays und speichert sie. Ohne Netz bleibt die Warnung, es erscheint eine Meldung, die App rechnet weiter.
- Risk: none (Kriterien 1 bis 4 geprüft: keine offene Frage, API und Schema stehen seit S4a, keine neue Technik, keine neue Entscheidung, auf der andere Slices aufbauen)
- Demo: Mit Netz in eine Woche außerhalb des Fensters blättern (z. B. weit zurück oder in 2027, falls außerhalb): Warnung mit Knopf erscheint. Netz trennen, „Feiertage laden“: Meldung, Warnung bleibt, Eingabe funktioniert weiter. Netz wieder an, „Feiertage laden“: Feiertage erscheinen markiert, Warnung verschwindet; nach Neustart bleibt das so. Eine bereits geladene Woche ohne Feiertage zeigt keine Warnung.
- Tests: Rust-Test, dass für eine Woche ohne Eintrag in der Tabelle der geladenen Zeiträume „fehlt“ gemeldet wird und für eine geladene Woche ohne Feiertage nicht; Rust-Test für das Nachladen einer einzelnen Woche mit gemockter API-Antwort (speichert Feiertage und Zeitraum); Test für den Ausfall der API beim Nachladen (Meldung, nichts gespeichert); Vue-Tests für Warnung, Knopf (Erfolg, Ausfall) und dass die Warnung bei geladenen Wochen fehlt.
- Touches: src-tauri/src/store.rs, API-Modul aus S4a, src-tauri/src/lib.rs (Command zum Nachladen), src/WeekView.vue
- Depends on: S4a
- Open: Reihenfolge nach der Speicherung
- Status: todo
- Added: split from S4 – Warnung und Nachladen sind eine eigene, einzeln demonstrierbare Funktion auf S4a.
- Notes: Der Knopf „Feiertage laden“ geht über den ursprünglichen Plan hinaus (Wunsch des Nutzers). Das Soll wird für Wochen mit fehlenden Feiertagen nicht automatisch gesenkt (betrifft S5).

## S5: Wochensumme, Soll und Über-/Minusstunden
- Goal: Pro Woche werden Ist-Summe, Soll (40 h, Feiertage senken es um 8 h pro Feiertag Montag bis Freitag) und der Saldo als Über- oder Minusstunden angezeigt, berechnet im Rust-Backend.
- Risk: 4 (Soll wird so gespeichert, dass spätere Änderungen erst ab der Folgewoche wirken; Plan B baut darauf auf), 1 (Open: Feiertage bei Lücken über 2 Wochen)
- Demo: Woche mit 4 Tagen à 8 h und einem Feiertag eintragen; Soll zeigt 32 h, Saldo 0. Woche ohne Feiertag mit 45 h zeigt +5 h. Woche mit 35 h zeigt −5 h.
- Tests: Rust-Tests für Ist-Summe, Soll mit 0, 1 und 2 Feiertagen, Saldo positiv, negativ und null; Test, dass ein Feiertag am Wochenende das Soll nicht senkt; Vue-Test für die Anzeige.
- Touches: none yet (Projekt ist leer)
- Depends on: S1, S2, S4a, S4b
- Open: Feiertage bei Lücken über 2 Wochen
- Status: todo
- Notes: Aus dem S2-Review: `Store::between` berechnet die Tagesminuten bei jedem Laden aus den Rohzeiten neu und speichert sie nicht. Ändern sich später die Regeln in `worked_minutes`, ändern sich damit auch alte Wochen. Vor Beginn entscheiden, ob die Minuten beim Speichern mitgespeichert werden (Schema-Änderung, Migration nötig, es gibt noch keine Schema-Version); passt zu "alte Wochen bleiben unverändert".
  Aus dem S4a-Review: Der Abruf beim Start ersetzt die gespeicherten Feiertage ab heute −14 Tage (`Store::save_holidays`). Das Soll abgeschlossener Wochen darf deshalb nicht live aus der Tabelle `holidays` berechnet werden, sonst ändert eine Korrektur der API es rückwirkend (widerspricht „alte Wochen werden nie rückwirkend neu berechnet“). Entweder das Soll beim Speichern der Woche mitspeichern (wie die Tagesminuten oben) oder Feiertage vor der aktuellen Woche als schreibgeschützt behandeln. Vor Beginn mit der Entscheidung zu den Tagesminuten zusammen klären.
