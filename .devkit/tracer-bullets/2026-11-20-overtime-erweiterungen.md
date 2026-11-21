# Tracer bullets: Overtime – Erweiterungen (Krank/Urlaub, Monat, Export, Einstellungen)

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
- Dies ist Plan B von zwei. Er setzt Plan A (`.devkit/tracer-bullets/2026-11-20-overtime-kern.md`) voraus. Die Angabe „Plan A S5" in `Depends on` meint den Slice S5 aus Plan A. Plan A war beim Schreiben noch nicht umgesetzt; wie er Datenmodell und Soll genau speichert, ist hier nicht bekannt und muss vor Beginn gegen den dann vorhandenen Code geprüft werden. Stand 2026-11-20: Plan A S5 ist in S5a (Tagesminuten, Wochensumme) und S5b (Soll, Saldo, eingefrorenes Soll in `week_targets`) aufgeteilt; „Plan A S5“ in `Depends on` meint das Soll aus S5b.
- Krank/Urlaub ist eine Markierung pro Tag. Ein markierter Tag hat keine Start/Ende-Zeiten, zählt mit 8 h als Ist und hat 8 h Soll, der Saldo des Tages ist 0.
- Fällt ein Krank/Urlaub-Tag auf einen Feiertag, gilt der Feiertag (Soll 0) und die Markierung ändert nichts. Auf einem Wochenende hat die Markierung keine Wirkung.
- „Durchschnitt pro Tag" ist der Durchschnitt der Ist-Stunden über alle Arbeitstage des Monats (Montag bis Freitag, ohne Feiertage), die einen Eintrag oder eine Krank/Urlaub-Markierung haben. Tage ohne Eintrag zählen nicht mit.
- Die Monatssumme ist die Summe der Ist-Stunden aller Tage des Kalendermonats; Wochen, die über die Monatsgrenze reichen, werden tageweise zugeordnet. Ein Monatssaldo (Über-/Minusstunden pro Monat) wurde nicht verlangt und ist nicht Teil dieses Plans.
- Der CSV-Export enthält alle gespeicherten Tage mit den Spalten Datum, Start, Ende, Pause (Min.), Stunden, Art (Arbeit, Krank, Urlaub). Trennzeichen ist das Semikolon (deutsches Excel), Kodierung UTF-8. Der Speicherort wird über einen Speichern-Dialog gewählt.
- Eine Änderung von Soll oder Bundesland gilt ab dem Montag der nächsten Woche; bereits vergangene Wochen und bereits gespeicherte Feiertage des alten Bundeslands bleiben unverändert.
- Sowohl Tages-Soll (8 h) als auch Wochen-Soll (40 h) sind einstellbar.

## Decided (2026-11-21, vor S1)
- Abgleich mit dem Code nach Plan A (alle Slices `done`): `entries(date, start, end, break_minutes, minutes)` mit Schema-Version 2 (`PRAGMA user_version`), Soll abgeschlossener Wochen eingefroren in `week_targets`, Soll aktueller und künftiger Wochen live über `target::target_for(Tagessoll, Feiertage)`, Tagessoll `DAILY_TARGET_MINUTES` als Parameter. Feiertags-API ist OpenHolidays, die Lückenregel (Warnung und Knopf „Feiertage laden“) steht in Plan A unter „Decided“.
- Krank/Urlaub wird live angerechnet: Gespeichert wird nur die Art des Tages (Spalte `kind`, Start/Ende/Pause leer). Die angerechneten Minuten (Tagessoll Montag bis Freitag, 0 an Feiertagen und am Wochenende) entstehen bei jedem Lesen aus den dann gespeicherten Feiertagen. Grund: Urlaub wird oft außerhalb des geladenen Feiertagsfensters eingetragen; fest gespeicherte 8 h zählten dann auch an einem später geladenen Feiertag (Saldo +8 h), und S4 kann das Tagessoll der jeweiligen Woche einrechnen. Bekannte Einschränkung: Korrigiert der Abruf beim Start einen Feiertag in einer der letzten zwei, schon eingefrorenen Wochen genau auf einem markierten Tag, ändert sich das Ist dieser Woche (das Soll bleibt eingefroren). Verworfen: Minuten beim Markieren speichern (wie S5a) und die Mischform mit Nachrechnen beim Laden von Feiertagen (zu aufwendig für den Nutzen).

## Open
- Grund für die Berechnung im Rust-Backend und Gründe gegen SwiftUI/Electron – nicht genannt (kein Slice betroffen).
- Wochen-Soll und Tagessoll getrennt einstellbar (S4): Heute ist das Wochen-Soll `Tagessoll × (5 − Feiertage Mo–Fr)`. Bei getrennten Werten (z. B. 8 h und 32 h) ist offen, um wie viel ein Feiertag das Wochen-Soll senkt (betrifft S4).
- Bundeslandwechsel (S4): Der Abruf beim Start ersetzt die gespeicherten Feiertage ab heute −14 Tage (Plan A S4a), die Tabelle `holidays` kennt kein Bundesland, und `DE-NW` steht fest in `holidays::request_url`. Nach einem Wechsel zeigten die letzten zwei abgeschlossenen Wochen die Feiertage des neuen Bundeslands (ihr Soll bleibt eingefroren). Widerspricht der Annahme „gespeicherte Feiertage des alten Bundeslands bleiben unverändert“ (betrifft S4).
- Monatsansicht (S2): Es gibt noch keine Monatsansicht und keinen Router; offen, ob Stunden am Wochenende in die Monatssumme zählen, obwohl der Durchschnitt nur Montag bis Freitag zählt (betrifft S2).

## Resolved (2026-11-21)
- „Wie Plan A S2 und S5 Datenmodell und Soll speichern“ – siehe „Decided“.
- „Konkrete öffentliche Feiertags-API“ und „Feiertage bei Lücken von mehr als 2 Wochen“ – in Plan A entschieden (OpenHolidays; Warnung mit Knopf „Feiertage laden“, Soll wird bei fehlenden Feiertagen nicht gesenkt).
- „Reihenfolge der Teile nach der Speicherung“ – Plan A ist vollständig, Plan B folgt der Reihenfolge S1 bis S4.

## Not checked
- Ob die für den Speichern-Dialog nötige Tauri-Funktion im Projekt verfügbar ist: Stand 2026-11-21 ist weder ein Dialog- noch ein FS-Plugin eingebunden (`Cargo.toml`, `package.json`), die Capabilities enthalten nur `core:default` (betrifft S3).
- Konditionen der Feiertags-APIs (keine Recherche durchgeführt).

## S1: Krank/Urlaub markieren
- Goal: Ein Tag lässt sich in der Wochenübersicht als Krank oder Urlaub markieren und zählt mit 8 h, ohne Zeiten einzugeben.
- Risk: 1 (Annahmen zu Wechselwirkung mit Feiertag und Wochenende), 4 (verändert die Ist-Berechnung, auf der S2 und S3 aufbauen)
- Demo: In einer Woche Montag als Urlaub und Dienstag als Krank markieren und Mittwoch bis Freitag je 8 h eintragen; die Wochensumme zeigt 40 h, der Saldo 0. Markierung wieder entfernen; Ist und Saldo ändern sich entsprechend. Markierung auf einem Feiertag ändert das Soll nicht.
- Tests: Rust-Tests für Ist und Soll mit Markierung, Markierung auf Feiertag und auf Wochenende, Entfernen der Markierung, Speichern und Laden; Vue-Test für die Auswahl.
- Touches: src-tauri/src/store.rs (Spalte `kind`, Schema-Version 3 mit Umbau der Tabelle `entries`), src-tauri/src/target.rs (Anrechnung Krank/Urlaub im Ist), src-tauri/src/lib.rs (Befehl zum Markieren), src/components/WeekRow.vue (Auswahl), src/WeekView.vue (Zeilen nach neu geladenen Feiertagen auffrischen)
- Depends on: Plan A S5
- Open: none (Anrechnung entschieden, siehe „Decided“)
- Status: done
- Notes: Tests grün (Rust 131/131 plus 2 ignorierte Live-Tests, Vue 92/92, `npm run build` und `cargo build` ohne Fehler). Die Migration von Version 2 auf 3 lief auf einer Kopie der echten Datenbank (12 Arbeitstage mit unveränderten Minuten, `week_targets` erhalten, danach `user_version` 3). Demo vom Nutzer im Fenster bestätigt (einschließlich Krank → Arbeit mit wiederhergestellten Zeiten). Schema-Version 3: `entries` wird neu angelegt (SQLite kann `NOT NULL` nicht entfernen), mit Spalte `kind` (`work`, `sick`, `vacation`) und einer CHECK-Regel: Arbeitstage haben Start, Ende und Pause, markierte Tage keine. Für markierte Tage speichert die Datenbank 0 Minuten; `target::day_entries` rechnet sie beim Lesen an (`absence_minutes`: Tagessoll Mo–Fr, 0 an Feiertagen und am Wochenende). `week_summary` und `load_entries` gehen beide über `day_entries`, `Store::week_total` ist entfallen (seine Tests sind als `week_summary`-Tests nach `target.rs` umgezogen). S2 soll die Monatssumme ebenfalls über `day_entries` bilden. Neuer Befehl `mark_absence(date, kind)` gibt die angerechneten Minuten zurück; die Markierung entfernen heißt in der Oberfläche „Arbeit“ wählen, das löscht den Eintrag über `delete_entry`. In der Datenbank ersetzt die Markierung Start, Ende und Pause; in der Zeile bleiben die eingetragenen Zeiten ausgeblendet erhalten und werden beim Zurückwechseln auf „Arbeit“ wieder gespeichert (nach Neustart sind sie leer). Am Wochenende und an Feiertagen lässt sich markieren, es zählt 0:00 h. Nach dem Laden von Feiertagen (Start oder Knopf) liest `WeekView` auch die Einträge neu, damit ein markierter Tag, der zum Feiertag wird, in der Zeile 0:00 h zeigt. Aus dem Review: (1) Schlug das Markieren fehl, waren die Zeiten in der Zeile schon geleert, und „Arbeit“ löschte danach den echten Eintrag – deshalb bleiben die Zeiten jetzt in der Zeile erhalten. (2) Wurde vor der Antwort auf ein Markieren oder Speichern wieder geleert bzw. „Arbeit“ gewählt, unterblieb das Löschen, weil `stored` erst nach der Antwort gesetzt wird; `WeekRow` zählt jetzt laufende Schreibbefehle und löscht auch dann. (3) Der CHECK-Test prüft auf „CHECK constraint failed“ statt auf irgendeinen Fehler; die `load_entries`-Fixtures in `WeekView.test.ts` enthalten `kind`. (4) Hinweis zum Tagessoll abgeschlossener Wochen in S4 eingetragen. Nicht übernommen: ein eigener Test für einen Wochenwechsel während `load_entries` in `reloadHolidays` – `showEntries` steht im selben Wochen-Guard wie `showHolidays`, den der bestehende Test „übernimmt nach dem Abruf keine Feiertage einer Woche, die inzwischen weggeblättert wurde“ abdeckt, und veraltete Einträge einer anderen Woche träfen keine Zeile.

## S2: Monatssumme und Durchschnitt pro Tag
- Goal: Zu jedem Monat werden die Summe der Ist-Stunden und der Durchschnitt pro Arbeitstag angezeigt, berechnet im Rust-Backend.
- Risk: 1 (Definition von Monat und Durchschnitt ist angenommen)
- Demo: Für einen Monat mit mehreren eingetragenen Wochen (darunter ein Krank/Urlaub-Tag und ein Feiertag) die Monatsansicht öffnen; Summe und Durchschnitt entsprechen der von Hand gerechneten Erwartung. Eine Woche über die Monatsgrenze wird tageweise zugeordnet.
- Tests: Rust-Tests für Monatssumme und Durchschnitt (leerer Monat, Monat mit Feiertag, Krank/Urlaub, Woche über Monatsgrenze, Schaltjahr-Februar); Vue-Test für die Anzeige.
- Touches: src-tauri/src/target.rs (Ist über `day_entries` aus S1), src-tauri/src/lib.rs, src/WeekView.vue oder neue Monatsansicht
- Depends on: S1, Plan A S5
- Open: none
- Status: todo
- Notes: none

## S3: CSV-Export
- Goal: Alle gespeicherten Tage lassen sich als CSV-Datei exportieren.
- Risk: 3 (Speichern-Dialog und Dateizugriff aus Tauri im Projekt neu, zu prüfen)
- Demo: In der App „Export" wählen, einen Speicherort im Dialog bestimmen; die Datei lässt sich in Excel öffnen und enthält pro Tag Datum, Start, Ende, Pause, Stunden und Art, auch für Krank/Urlaub.
- Tests: Rust-Tests für die CSV-Erzeugung (Semikolon, UTF-8, Umlaute, leere Felder bei Krank/Urlaub, Reihenfolge nach Datum, leere Datenbank); Test, dass ein abgebrochener Dialog nichts schreibt.
- Touches: src-tauri/src/store.rs, neues Modul für die CSV-Erzeugung, src-tauri/src/lib.rs, src-tauri/Cargo.toml und package.json (Dialog-Plugin), src-tauri/capabilities/default.json, src/WeekView.vue
- Depends on: S1, Plan A S2
- Open: none
- Status: todo
- Notes: none

## S4: Soll und Bundesland einstellen
- Goal: Tages-Soll, Wochen-Soll und Bundesland lassen sich einstellen; eine Änderung wirkt erst ab der nächsten Woche, vergangene Wochen bleiben unverändert.
- Risk: 4 (rückwirkende Neuberechnung muss ausgeschlossen sein; baut auf der Soll-Speicherung aus Plan A S5 auf), 1 (Open: Feiertage bei Lücken über 2 Wochen und API-Wahl betreffen den Bundeslandwechsel)
- Demo: Wochen-Soll von 40 auf 32 h ändern; die aktuelle Woche zeigt weiter 40 h, die nächste Woche 32 h, vergangene Wochen unverändert mit ihrem alten Saldo. Bundesland von NRW auf ein anderes ändern; ab der nächsten Woche gelten dessen Feiertage und werden geladen, vergangene Wochen behalten die NRW-Feiertage.
- Tests: Rust-Tests für Soll mit Gültigkeit ab Montag der Folgewoche (Wechsel mitten in der Woche, mehrere Änderungen, Jahreswechsel), für den Bundeslandwechsel und das Laden der Feiertage über gemockte API; Test, dass vergangene Wochen ihren Saldo behalten; Vue-Test für das Einstellungsformular.
- Touches: src-tauri/src/store.rs (Einstellungen mit Gültigkeit ab Montag), src-tauri/src/target.rs (`DAILY_TARGET_MINUTES` ersetzen), src-tauri/src/holidays.rs (`DE-NW` aus `request_url`), src-tauri/src/lib.rs, neues Einstellungsformular in src/
- Depends on: Plan A S4, Plan A S5
- Open: Feiertag und getrennt eingestelltes Wochen-Soll; Feiertage des alten Bundeslands im Abruffenster (siehe „Open“)
- Status: todo
- Notes: Aus dem S1-Review: Krank/Urlaub wird live mit dem Tagessoll angerechnet (`target::day_entries`). Wird das Tagessoll einstellbar, muss jede Woche ihr gültiges Tagessoll bekommen, abgeschlossene Wochen das beim Einfrieren gültige (z. B. eigene Spalte in `week_targets`, von `day_entries` und `week_summary` statt des Parameters verwendet). Sonst ändert eine Änderung von 8 h auf 6 h das Ist aller vergangenen Wochen mit markierten Tagen rückwirkend.
