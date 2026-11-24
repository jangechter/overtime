# Release bauen (macOS, Windows, Linux)

Installer für alle drei Systeme baut GitHub Actions mit dem Workflow `.github/workflows/release.yml`. Tauri kann nur für das System bauen, auf dem es läuft. Deshalb läuft der Build auf drei Rechnern von GitHub, je einem für macOS, Windows und Linux.

## Kurzfassung

1. Version erhöhen, z. B. auf `0.2.0`, in
   - `src-tauri/tauri.conf.json` → `"version"` (maßgeblich für Installer und Tag)
   - `package.json` → `"version"`
   - `src-tauri/Cargo.toml` → `version` (danach einmal `cargo build --manifest-path src-tauri/Cargo.toml`, damit `Cargo.lock` mitzieht)
2. Committen und pushen.
3. Tag setzen und pushen:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
4. Unter **GitHub → Actions → Release** warten, bis alle drei Builds grün sind (ca. 15–25 Minuten).
5. Unter **GitHub → Releases** den Entwurf „Overtime v0.2.0“ prüfen und auf **Publish release** klicken.

## Was der Workflow macht

Für jedes System (macOS, Windows, Ubuntu 22.04) läuft dasselbe:

1. **Tag prüfen:** Der Tag muss genau `v` + Version aus `tauri.conf.json` sein. Sonst bricht der Build ab.
2. **Tests:** `npm test` und `cargo test` laufen auf jedem System. Schlägt ein Test fehl, wird für dieses System nichts gebaut.
3. **Bauen:** `tauri-action` baut die Installer und hängt sie an einen **Release-Entwurf**. Der Entwurf ist erst sichtbar, wenn du ihn veröffentlichst.

| System | Dateien im Release |
|---|---|
| macOS | `Overtime_<version>_universal.dmg` (Apple Silicon und Intel), `Overtime_universal.app.tar.gz` |
| Windows | `Overtime_<version>_x64-setup.exe` (NSIS), `Overtime_<version>_x64_en-US.msi` |
| Linux | `.deb` (Debian/Ubuntu), `.rpm` (Fedora/openSUSE), `.AppImage` (alle Distributionen) |

### Ohne Tag starten

Unter **Actions → Release → Run workflow** lässt sich der Build auch von Hand starten. Das eignet sich zum Ausprobieren. Der Entwurf zeigt dann auf den gewählten Commit. Den Tag `v<version>` legt GitHub erst beim Veröffentlichen an. Soll der Entwurf kein echtes Release werden, ihn einfach löschen. Ein Tag ist dann nie entstanden.

Gibt es für die Version schon einen Release oder Entwurf, hängt `tauri-action` die Dateien an ihn an. Gleichnamige Dateien werden dabei ersetzt.

### Wo ist der Entwurf?

Entwürfe sind nur für dich sichtbar, und zwar **ausschließlich** auf der Seite `https://github.com/jangechter/overtime/releases`, wenn du eingeloggt bist. Nicht sichtbar sind sie:

- im Kasten „Releases“ in der Seitenleiste der Repo-Startseite
- unter *Tags*
- über die öffentliche GitHub-API

## Erster Start beim Nutzer

Die Installer sind **nicht signiert**. Das ist für eine private App in Ordnung, führt aber beim ersten Start zu Warnungen:

- **macOS:** „Overtime kann nicht geöffnet werden …“ → *Systemeinstellungen → Datenschutz & Sicherheit → Dennoch öffnen*. Alternativ im Terminal: `xattr -cr /Applications/Overtime.app`
- **Windows:** „Der Computer wurde durch Windows geschützt“ → *Weitere Informationen → Trotzdem ausführen*. WebView2 ist unter Windows 10/11 vorinstalliert. Falls es fehlt, lädt der Installer es nach.
- **Linux:** keine Warnung. Ein AppImage muss vor dem Start ausführbar gemacht werden: `chmod +x Overtime_*.AppImage`

## Wo die Daten liegen

Jede Installation hat ihre eigene Datenbank `overtime.db`:

| System | Pfad |
|---|---|
| macOS | `~/Library/Application Support/dev.jan.overtime/` |
| Windows | `%APPDATA%\dev.jan.overtime\` |
| Linux | `~/.local/share/dev.jan.overtime/` |

Beim Wechsel auf einen anderen Rechner die Datei bei geschlossener App dorthin kopieren.

## Nach einem neuen Release prüfen

Windows und Linux nutzen eine andere Web-Engine als macOS (WebView2 bzw. WebKitGTK statt WebKit). Nach größeren Änderungen an der Oberfläche einmal auf beiden Systemen ansehen:

- **Ausrichtung der Tageszeilen:** Die Schrift ist dort *Segoe UI* bzw. die Systemschrift der Distribution, mit anderen Abständen.
- **Dark Mode:** Unter Linux folgt er nicht immer der Systemeinstellung, weil das vom GTK-Theme abhängt.
- **CSV-Export unter Windows:** Ist die Zieldatei noch in Excel geöffnet, schlägt der Export fehl.

## Wenn der Workflow fehlschlägt

- **„Tag … passt nicht zur Version …“:** Die Version in `tauri.conf.json` ist nicht erhöht oder der Tag ist falsch. Den Tag löschen (`git push origin --delete v0.2.0` und `git tag -d v0.2.0`), korrigieren und neu taggen.
- **Nur ein System rot:** Die anderen laufen trotzdem weiter (`fail-fast: false`), und ihre Dateien landen im Entwurf. Den Fehler beheben und den Workflow neu starten, entweder über *Re-run failed jobs* oder mit einem neuen Tag.
- **Linux-Build findet eine Bibliothek nicht:** Das Paket in der `apt-get install`-Zeile des Workflows ergänzen.

## Kosten

Für öffentliche Repos ist GitHub Actions kostenlos. Bei privaten Repos zählen Minuten auf macOS-Rechnern 10-fach und auf Windows-Rechnern 2-fach gegen das Freikontingent. Ein Release verbraucht damit grob 150–250 Freiminuten.

## Lokal bauen

Für einen schnellen Test auf dem eigenen Rechner reicht `npm run tauri build` (siehe [app-icon.md](app-icon.md#5-app-bauen-und-installieren)). Auf Windows oder Linux braucht es dafür zusätzlich:

- **Windows:** *Visual Studio Build Tools* mit „Desktopentwicklung mit C++“, Rust, Node 24
- **Linux (Ubuntu/Debian):** `sudo apt install libwebkit2gtk-4.1-dev build-essential librsvg2-dev libxdo-dev libayatana-appindicator3-dev`, Rust, Node 24
