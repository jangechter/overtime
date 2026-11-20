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
- Dies ist Plan B von zwei. Er setzt Plan A (`.devkit/tracer-bullets/2026-11-20-overtime-kern.md`) voraus. Die Angabe „Plan A S5" in `Depends on` meint den Slice S5 aus Plan A. Plan A war beim Schreiben noch nicht umgesetzt; wie er Datenmodell und Soll genau speichert, ist hier nicht bekannt und muss vor Beginn gegen den dann vorhandenen Code geprüft werden.
- Krank/Urlaub ist eine Markierung pro Tag. Ein markierter Tag hat keine Start/Ende-Zeiten, zählt mit 8 h als Ist und hat 8 h Soll, der Saldo des Tages ist 0.
- Fällt ein Krank/Urlaub-Tag auf einen Feiertag, gilt der Feiertag (Soll 0) und die Markierung ändert nichts. Auf einem Wochenende hat die Markierung keine Wirkung.
- „Durchschnitt pro Tag" ist der Durchschnitt der Ist-Stunden über alle Arbeitstage des Monats (Montag bis Freitag, ohne Feiertage), die einen Eintrag oder eine Krank/Urlaub-Markierung haben. Tage ohne Eintrag zählen nicht mit.
- Die Monatssumme ist die Summe der Ist-Stunden aller Tage des Kalendermonats; Wochen, die über die Monatsgrenze reichen, werden tageweise zugeordnet. Ein Monatssaldo (Über-/Minusstunden pro Monat) wurde nicht verlangt und ist nicht Teil dieses Plans.
- Der CSV-Export enthält alle gespeicherten Tage mit den Spalten Datum, Start, Ende, Pause (Min.), Stunden, Art (Arbeit, Krank, Urlaub). Trennzeichen ist das Semikolon (deutsches Excel), Kodierung UTF-8. Der Speicherort wird über einen Speichern-Dialog gewählt.
- Eine Änderung von Soll oder Bundesland gilt ab dem Montag der nächsten Woche; bereits vergangene Wochen und bereits gespeicherte Feiertage des alten Bundeslands bleiben unverändert.
- Sowohl Tages-Soll (8 h) als auch Wochen-Soll (40 h) sind einstellbar.

## Open
- Reihenfolge der Teile nach der Speicherung war nicht festgelegt; die Reihenfolge hier ist nach Risiko und Abhängigkeit sortiert (betrifft alle Slices).
- Feiertage bei Lücken von mehr als 2 Wochen – keine Regel (betrifft S4).
- Konkrete öffentliche Feiertags-API und deren Konditionen – wird in Plan A S4 entschieden; ein Bundeslandwechsel in S4 hängt davon ab.
- Grund für die Berechnung im Rust-Backend und Gründe gegen SwiftUI/Electron – nicht genannt (kein Slice betroffen).

## Not checked
- Wie Plan A S2 und S5 Datenmodell und Soll speichern (Plan A war nicht umgesetzt).
- Ob die für den Speichern-Dialog nötige Tauri-Funktion im Projekt verfügbar ist.
- Konditionen der Feiertags-APIs (keine Recherche durchgeführt).

## S1: Krank/Urlaub markieren
- Goal: Ein Tag lässt sich in der Wochenübersicht als Krank oder Urlaub markieren und zählt mit 8 h, ohne Zeiten einzugeben.
- Risk: 1 (Annahmen zu Wechselwirkung mit Feiertag und Wochenende), 4 (verändert die Ist-Berechnung, auf der S2 und S3 aufbauen)
- Demo: In einer Woche Montag als Urlaub und Dienstag als Krank markieren und Mittwoch bis Freitag je 8 h eintragen; die Wochensumme zeigt 40 h, der Saldo 0. Markierung wieder entfernen; Ist und Saldo ändern sich entsprechend. Markierung auf einem Feiertag ändert das Soll nicht.
- Tests: Rust-Tests für Ist und Soll mit Markierung, Markierung auf Feiertag und auf Wochenende, Entfernen der Markierung, Speichern und Laden; Vue-Test für die Auswahl.
- Touches: none yet (Plan A nicht umgesetzt; Dateien nicht geprüft)
- Depends on: Plan A S5
- Open: Reihenfolge nach der Speicherung
- Status: todo
- Notes: none

## S2: Monatssumme und Durchschnitt pro Tag
- Goal: Zu jedem Monat werden die Summe der Ist-Stunden und der Durchschnitt pro Arbeitstag angezeigt, berechnet im Rust-Backend.
- Risk: 1 (Definition von Monat und Durchschnitt ist angenommen)
- Demo: Für einen Monat mit mehreren eingetragenen Wochen (darunter ein Krank/Urlaub-Tag und ein Feiertag) die Monatsansicht öffnen; Summe und Durchschnitt entsprechen der von Hand gerechneten Erwartung. Eine Woche über die Monatsgrenze wird tageweise zugeordnet.
- Tests: Rust-Tests für Monatssumme und Durchschnitt (leerer Monat, Monat mit Feiertag, Krank/Urlaub, Woche über Monatsgrenze, Schaltjahr-Februar); Vue-Test für die Anzeige.
- Touches: none yet (Plan A nicht umgesetzt; Dateien nicht geprüft)
- Depends on: S1, Plan A S5
- Open: Reihenfolge nach der Speicherung
- Status: todo
- Notes: none

## S3: CSV-Export
- Goal: Alle gespeicherten Tage lassen sich als CSV-Datei exportieren.
- Risk: 3 (Speichern-Dialog und Dateizugriff aus Tauri im Projekt neu, zu prüfen)
- Demo: In der App „Export" wählen, einen Speicherort im Dialog bestimmen; die Datei lässt sich in Excel öffnen und enthält pro Tag Datum, Start, Ende, Pause, Stunden und Art, auch für Krank/Urlaub.
- Tests: Rust-Tests für die CSV-Erzeugung (Semikolon, UTF-8, Umlaute, leere Felder bei Krank/Urlaub, Reihenfolge nach Datum, leere Datenbank); Test, dass ein abgebrochener Dialog nichts schreibt.
- Touches: none yet (Plan A nicht umgesetzt; Dateien nicht geprüft)
- Depends on: S1, Plan A S2
- Open: Reihenfolge nach der Speicherung
- Status: todo
- Notes: none

## S4: Soll und Bundesland einstellen
- Goal: Tages-Soll, Wochen-Soll und Bundesland lassen sich einstellen; eine Änderung wirkt erst ab der nächsten Woche, vergangene Wochen bleiben unverändert.
- Risk: 4 (rückwirkende Neuberechnung muss ausgeschlossen sein; baut auf der Soll-Speicherung aus Plan A S5 auf), 1 (Open: Feiertage bei Lücken über 2 Wochen und API-Wahl betreffen den Bundeslandwechsel)
- Demo: Wochen-Soll von 40 auf 32 h ändern; die aktuelle Woche zeigt weiter 40 h, die nächste Woche 32 h, vergangene Wochen unverändert mit ihrem alten Saldo. Bundesland von NRW auf ein anderes ändern; ab der nächsten Woche gelten dessen Feiertage und werden geladen, vergangene Wochen behalten die NRW-Feiertage.
- Tests: Rust-Tests für Soll mit Gültigkeit ab Montag der Folgewoche (Wechsel mitten in der Woche, mehrere Änderungen, Jahreswechsel), für den Bundeslandwechsel und das Laden der Feiertage über gemockte API; Test, dass vergangene Wochen ihren Saldo behalten; Vue-Test für das Einstellungsformular.
- Touches: none yet (Plan A nicht umgesetzt; Dateien nicht geprüft)
- Depends on: Plan A S4, Plan A S5
- Open: Feiertage bei Lücken über 2 Wochen; API-Wahl aus Plan A S4
- Status: todo
- Notes: none
