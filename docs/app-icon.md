# App-Icon austauschen

Alle Icons in `src-tauri/icons/` werden aus **einem** Quellbild erzeugt: `app-icon.png` im Projekt-Root. Die Dateien in `src-tauri/icons/` nie von Hand bearbeiten.

## Kurzfassung

```bash
swift scripts/app-icon.swift ~/Downloads/neues-logo.png   # → app-icon.png
npm run tauri icon app-icon.png                           # → src-tauri/icons/
rm -rf src-tauri/icons/ios src-tauri/icons/android
touch src-tauri/src/lib.rs                                # damit dev das neue Icon übernimmt
npm run tauri build
```

Danach die App installieren (siehe [unten](#5-app-bauen-und-installieren)), committen und pushen.

## 1. Logo vorbereiten

Das Logo sollte so aussehen:

- **PNG mit transparentem Hintergrund**: Außerhalb der Icon-Form muss alles durchsichtig sein.
- **Die Form ist ein (annähernd) Quadrat** mit abgerundeten Ecken und einem gefüllten Hintergrund. Das Skript verzerrt eine nicht quadratische Form so, dass sie quadratisch wird. Bis ca. 5 % Abweichung fällt das nicht auf.
- **Die Form ist mindestens 824 px groß**, besser 1024 px. Sonst wird sie hochskaliert und unscharf.
- **Wichtige Details liegen nicht direkt am Rand.** Das Skript schneidet ringsum ca. 2 % ab, und an den Ecken etwas mehr.

Wie viel transparenter Rand um die Form herum ist und wie rund die Ecken sind, spielt keine Rolle. Beides gleicht das Skript aus.

## 2. Logo aufbereiten

```bash
swift scripts/app-icon.swift ~/Downloads/neues-logo.png
```

Das Skript schreibt `app-icon.png` in das aktuelle Verzeichnis, also vom Projekt-Root aus aufrufen. Als zweites Argument kann ein anderer Ausgabepfad angegeben werden.

Dabei macht das Skript Folgendes:

1. Es sucht die sichtbare Form, also alle Pixel, die zu mehr als 50 % deckend sind.
2. Es skaliert die Form auf **824 × 824 px** und setzt sie mittig auf eine Leinwand von **1024 × 1024 px**.
3. Es schneidet die Form mit **Apples Eckenform** zu, einem abgerundeten Quadrat mit weich auslaufenden Ecken (Radius 185 px).
4. Es macht die Fläche innerhalb der Form **voll deckend**. KI-generierte Bilder sind oft nur zu ca. 99 % deckend.

Warnungen in der Ausgabe ernst nehmen:

- *„nicht annähernd quadratisch“*: Das Motiv wird sichtbar gestaucht. Das Logo vorher quadratisch zuschneiden.
- *„kleiner als 824 px“*: Das Icon wird unscharf. Ein größeres Original verwenden.

Danach `app-icon.png` öffnen und kurz prüfen, ob nichts Wichtiges abgeschnitten ist.

### Warum dieser Schritt nötig ist

Seit **macOS 26** prüft das System, ob ein Icon Apples Form exakt ausfüllt. Wenn nicht, verkleinert es das Icon und setzt es auf eine graue Fläche. Im Dock wirkt es dann kleiner als die anderen Apps. Nur zuzuschneiden reicht nicht: Auch Seitenverhältnis, Eckenradius und Deckkraft müssen stimmen.

## 3. Icons erzeugen

```bash
npm run tauri icon app-icon.png
rm -rf src-tauri/icons/ios src-tauri/icons/android
```

`tauri icon` überschreibt alle Dateien in `src-tauri/icons/` (`icon.icns` für macOS, `icon.ico` für Windows, PNGs in allen Größen). Zusätzlich legt es bei jedem Aufruf Icons für iOS und Android an. Die braucht die Desktop-App nicht, deshalb werden sie gleich wieder gelöscht.

An `src-tauri/tauri.conf.json` muss nichts geändert werden.

## 4. Im Dev-Modus prüfen

```bash
touch src-tauri/src/lib.rs
npm run tauri dev
```

Die Icons werden beim Kompilieren fest ins Programm eingebaut (`tauri::generate_context!()` in `src-tauri/src/lib.rs`). Cargo bemerkt aber nicht, wenn sich nur Bilddateien ändern. Ohne das `touch` startet dev weiter mit dem alten Icon.

## 5. App bauen und installieren

```bash
npm run tauri build
```

Ergebnis in `src-tauri/target/release/bundle/`:

- `macos/Overtime.app`: die App
- `dmg/Overtime_<version>_aarch64.dmg`: der Installer zum Weitergeben

Installieren (Overtime vorher beenden):

```bash
rm -rf /Applications/Overtime.app
cp -R src-tauri/target/release/bundle/macos/Overtime.app /Applications/
killall Dock
```

`killall Dock` leert den Icon-Cache, sonst zeigt macOS eventuell noch das alte Icon. Die Daten der App liegen unter `~/Library/Application Support/dev.jan.overtime/` und bleiben beim Austauschen erhalten.

Stolperfallen:

- **Nie nur `--bundles dmg` bauen.** Tauri löscht dann die `.app`. Wenn schon, dann `--bundles app,dmg`.
- **Ist eine Overtime-DMG noch eingehängt**, schlägt der DMG-Build fehl („couldn't unmount … Resource busy“). Dann die DMG im Finder auswerfen und neu bauen.
- **`cp` ohne `-R`** kopiert keine `.app`, weil eine App ein Ordner ist.

## 6. Committen

Diese Dateien gehören zusammen in einen Commit:

- `app-icon.png`
- `src-tauri/icons/`

Das ursprüngliche Logo (z. B. aus `~/Downloads`) muss nicht ins Repo, weil `app-icon.png` das aufbereitete Quellbild ist. Es lohnt sich aber, das Original aufzubewahren, falls das Icon später neu aufbereitet werden soll.
