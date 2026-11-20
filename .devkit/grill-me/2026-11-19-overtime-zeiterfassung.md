# Grill-me: Overtime – Arbeitszeiterfassung

- Mode: both
- Date: 2026-11-19
- Source: interview only

## Clarified
- Ziel und Erfolgskriterium: Eingabe von Start, Pause und Ende pro Tag; Liste pro Tag und Woche; Wochensumme, Monatssumme, Durchschnitt pro Tag und Über-/Minusstunden gegen Soll.
- Soll: 8 h pro Tag, 40 h pro Woche (aktuell). Änderungen an Soll oder Bundesland wirken erst ab der nächsten Woche, alte Wochen bleiben unverändert.
- Feiertage: reduzieren das Wochen-Soll automatisch; Bundesland NRW, einstellbar; Abruf über öffentliche API; lokal gespeichert; geladen für die letzten 2 Wochen und die nächsten 2 Monate; bei fehlendem Netz nur Warnung, die App rechnet weiter.
- Krank/Urlaub: pro Tag markierbar, zählt automatisch mit 8 h.
- Eingabemodell: ein Block pro Tag (Start, Ende), Pause als Dauer in Minuten. Kein Mehrfachblock, keine Mehrfachpausen, keine Arbeit über Mitternacht.
- Randbedingungen: visuelle Eingabemaske, nur ein Gerät (kein Sync), wenige Abende pro Woche, kein fester Termin, voller Umfang bleibt; Ziel ist, eine neue Technologie zu lernen.
- Datensicherheit: Export als CSV, kein Import.
- Oberfläche: Wochenübersicht mit allen Tagen einer Woche; Eintragen direkt dort.
- Reihenfolge: zuerst Wochenübersicht mit Eingabe, danach Speicherung.
- Nicht im Umfang: Mehrbenutzer, Projekte/Kunden pro Eintrag, Stoppuhr, Handy-Version, Diagramme (erstmal), Erinnerungen, Sync über Geräte, Import.

## Decisions
- Technik: Tauri mit Vue (TypeScript), lokale Speicherung (SQLite) – Wunsch, eine neue Technologie zu lernen und Vue-Kenntnisse zu nutzen
- Berechnung von Stunden, Summen und Überstunden liegt im Rust-Backend – kein Grund genannt
- Feiertage über öffentliche API statt selbst pflegen – will sie nicht selbst pflegen, hat meist Internet
- Voller Umfang ohne Terminvorgabe – "fertig wenn fertig"
- Feiertage zusätzlich für die letzten 2 Wochen zurück laden – Zeiten werden manchmal erst am Wochenende nachgetragen

## Challenged
- "Erste Version in zwei Wochen" vs. "kein Zeitdruck" → Widerspruch benannt → changed: kein fester Termin, voller Umfang
- Änderung von Soll/Bundesland würde alte Wochen rückwirkend neu berechnen → Frage nach Stichtag → changed: Wirkung erst ab nächster Woche
- Feiertage nur bei App-Start laden → fehlende Feiertage bei längerer Nichtnutzung → changed: 2 Wochen rückwirkend; längere Lücken bleiben offen

## Open
- Reihenfolge der Teile nach der Speicherung (Auswertungen, Feiertage, Krank/Urlaub, Export) – nicht festgelegt
- Feiertage bei Lücken von mehr als 2 Wochen – keine Regel
- Gründe gegen abgelehnte Alternativen (SwiftUI, Electron) – nur vom Assistenten genannt, nicht bewertet
- Grund für Berechnung im Rust-Backend – nicht genannt
- Konkrete öffentliche Feiertags-API – nicht ausgewählt, Konditionen ungeprüft

## Not checked
- none
