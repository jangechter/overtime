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

## Open
- Reihenfolge der Teile nach der Speicherung war nicht festgelegt; die Reihenfolge S3 bis S5 ist nur nach Risiko und Abhängigkeit sortiert (betrifft S3, S4, S5).
- Feiertage bei Lücken von mehr als 2 Wochen – keine Regel (betrifft S4, S5).
- Konkrete öffentliche Feiertags-API und deren Konditionen – nicht ausgewählt, ungeprüft (betrifft S4).
- Grund für die Berechnung im Rust-Backend und Gründe gegen SwiftUI/Electron – nicht genannt (kein Slice betroffen).

## Not checked
- Konditionen der Feiertags-APIs (keine Recherche durchgeführt).
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
- Status: todo
- Notes: none

## S4: Feiertage NRW laden, speichern und anzeigen
- Goal: Feiertage für NRW werden über eine öffentliche API für die letzten 2 Wochen und die nächsten 2 Monate geholt, lokal gespeichert und in der Wochenübersicht markiert; ohne Netz erscheint nur eine Warnung.
- Risk: 1 (Open: API nicht ausgewählt, Lücken über 2 Wochen ohne Regel), 2 (externe API, im leeren Projekt noch nie angebunden), 3 (HTTP-Aufruf aus Rust neu)
- Demo: App mit Netz starten und in eine Woche mit einem NRW-Feiertag blättern; der Tag ist als Feiertag markiert. Netz trennen und App neu starten; gespeicherte Feiertage bleiben sichtbar, für fehlende Zeiträume erscheint eine Warnung, die App rechnet weiter.
- Tests: Rust-Tests mit gemockter API-Antwort (Parsen, Speichern, Zeitfenster −2 Wochen bis +2 Monate); Test für den Ausfall der API (Warnung statt Fehler); Vue-Test für die Markierung.
- Touches: none yet (Projekt ist leer)
- Depends on: S2, S3
- Open: Konkrete API und Konditionen; Lücken über 2 Wochen; Reihenfolge nach der Speicherung
- Status: todo
- Notes: Vor Beginn die API auswählen und ihre Konditionen prüfen; die Wahl dem Nutzer vorlegen.

## S5: Wochensumme, Soll und Über-/Minusstunden
- Goal: Pro Woche werden Ist-Summe, Soll (40 h, Feiertage senken es um 8 h pro Feiertag Montag bis Freitag) und der Saldo als Über- oder Minusstunden angezeigt, berechnet im Rust-Backend.
- Risk: 4 (Soll wird so gespeichert, dass spätere Änderungen erst ab der Folgewoche wirken; Plan B baut darauf auf), 1 (Open: Feiertage bei Lücken über 2 Wochen)
- Demo: Woche mit 4 Tagen à 8 h und einem Feiertag eintragen; Soll zeigt 32 h, Saldo 0. Woche ohne Feiertag mit 45 h zeigt +5 h. Woche mit 35 h zeigt −5 h.
- Tests: Rust-Tests für Ist-Summe, Soll mit 0, 1 und 2 Feiertagen, Saldo positiv, negativ und null; Test, dass ein Feiertag am Wochenende das Soll nicht senkt; Vue-Test für die Anzeige.
- Touches: none yet (Projekt ist leer)
- Depends on: S1, S2, S4
- Open: Feiertage bei Lücken über 2 Wochen
- Status: todo
- Notes: none
