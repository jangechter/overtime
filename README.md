# Overtime

Eine kleine Desktop-App zur Arbeitszeiterfassung mit Überstundenkonto. Pro Tag trägst du Start, Ende und Pause ein, die App rechnet Wochen- und Monatssummen sowie Über- und Minusstunden gegen dein Soll aus – Feiertage deines Bundeslands eingerechnet.

Gebaut mit [Tauri 2](https://tauri.app), [Vue 3](https://vuejs.org) und SQLite. Läuft auf macOS, Windows und Linux, alle Daten bleiben lokal auf dem Rechner.

## Funktionen

- **Wochenübersicht:** alle Tage einer Woche auf einen Blick, direkt dort eintragen; Start, Ende und Pause in Minuten, ein Arbeitsblock pro Tag
- **Wochensumme und Saldo:** Ist-Stunden, Soll und Über-/Minusstunden der Woche, zusätzlich als Dezimalstunden
- **Monatsansicht:** Monatssumme und Durchschnitt pro Arbeitstag
- **Krank und Urlaub:** pro Tag markierbar, zählt automatisch mit dem Tagessoll
- **Feiertage:** werden für das eingestellte Bundesland automatisch geladen und senken das Wochen-Soll; ohne Netz gibt es nur eine Warnung, die App rechnet weiter
- **Einstellbares Soll und Bundesland:** Änderungen gelten erst ab der nächsten Woche, vergangene Wochen werden nie rückwirkend neu berechnet
- **CSV-Export:** alle Tage als CSV-Datei, direkt in Excel lesbar (Semikolon, Dezimalkomma)

Standardmäßig ist das Soll 8 Stunden pro Tag und 40 Stunden pro Woche in Nordrhein-Westfalen.

## Installation

Fertige Installer für macOS (Apple Silicon und Intel), Windows und Linux gibt es unter [Releases](https://github.com/jangechter/overtime/releases).

Die Installer sind nicht signiert. Beim ersten Start warnen macOS und Windows deshalb:

- **macOS:** *Systemeinstellungen → Datenschutz & Sicherheit → Dennoch öffnen*, oder im Terminal `xattr -cr /Applications/Overtime.app`
- **Windows:** *Weitere Informationen → Trotzdem ausführen*
- **Linux:** ein AppImage vorher ausführbar machen: `chmod +x Overtime_*.AppImage`

### Wo die Daten liegen

Alle Einträge stehen in einer SQLite-Datenbank `overtime.db`:

| System  | Pfad |
|---------|------|
| macOS   | `~/Library/Application Support/dev.jan.overtime/` |
| Windows | `%APPDATA%\dev.jan.overtime\` |
| Linux   | `~/.local/share/dev.jan.overtime/` |

Für ein Backup oder einen Rechnerwechsel die Datei bei geschlossener App kopieren.

## Entwicklung

### Voraussetzungen

- [Node.js](https://nodejs.org) 24 und [Rust](https://rustup.rs) (stable)
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Windows:** Visual Studio Build Tools mit „Desktopentwicklung mit C++“
- **Linux (Ubuntu/Debian):** `sudo apt install libwebkit2gtk-4.1-dev build-essential librsvg2-dev libxdo-dev libayatana-appindicator3-dev`

Details stehen in den [Tauri-Voraussetzungen](https://tauri.app/start/prerequisites/).

### Befehle

```bash
npm install                 # Abhängigkeiten installieren
npm run tauri dev           # App im Entwicklungsmodus starten
npm run tauri build         # Installer für das eigene System bauen

npm test                                          # Vue-Tests (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml   # Rust-Tests
```

### Aufbau

- `src-tauri/src/` – Rust-Backend: Berechnung von Stunden, Soll und Saldo, Validierung, Speicherung (SQLite), Feiertage, CSV-Export
- `src/` – Vue-Oberfläche: zeigt nur an und ruft das Backend über Tauri-Befehle auf
- `docs/` – Anleitungen zum [Release bauen](docs/release.md) und [App-Icon austauschen](docs/app-icon.md)

Releases für alle drei Systeme baut GitHub Actions, sobald ein Tag `v<version>` gepusht wird – siehe [docs/release.md](docs/release.md).

## Feiertage

Die Feiertage kommen von der [OpenHolidays API](https://www.openholidaysapi.org). Die App lädt sie für die letzten zwei Wochen und die nächsten zwei Monate und speichert sie lokal; ältere oder spätere Wochen lassen sich per Knopf nachladen.

## Lizenz

[MIT](LICENSE)
