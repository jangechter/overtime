## Devkit

<!-- Created with /devkit:devkit-setup. "—" = deliberately absent in this project, "?" = not specified yet. -->

### Commands
- (root):
  - Tests (all): `npm test && cargo test --manifest-path src-tauri/Cargo.toml`
  - Typecheck/Build: `npm run build && cargo build --manifest-path src-tauri/Cargo.toml`
- src/:
  - Test (single): `npx vitest run <file>`
- src-tauri/:
  - Test (single): `cargo test --manifest-path src-tauri/Cargo.toml <test_name>`
- Lint: —

### Test conventions
- Framework: Rust `cargo test` (Unit-Tests im Modul); Vue: Vitest + @vue/test-utils + jsdom
- Location and naming: Rust-Tests in `#[cfg(test)] mod tests` direkt in der Datei (englische Testnamen); Vue-Tests neben der Komponente als `<Name>.test.ts` (deutsche Testnamen)
- Notes: Tauri-Aufrufe werden in Vue-Tests mit einem handgeschriebenen Fake für `invoke` ersetzt, nicht mit `vi.fn` (abgelehnte Promises lösen dort einen unbehandelten Fehler aus). Kein CI bei Pushes; die Tests laufen nur im Release-Workflow (`.github/workflows/release.yml`) auf macOS, Windows und Linux. Keine Coverage-Schwelle.
- Ticket marker in tests: comment `Regression: <TICKET>` above the test

### Review focus
- Berechnung und Validierung liegen im Rust-Backend; Vue zeigt nur an.
- Fehlermeldungen für Nutzer sind auf Deutsch.
- Alte Wochen werden nie rückwirkend neu berechnet; Änderungen an Soll oder Bundesland wirken erst ab der Folgewoche.

### Do not touch
- src-tauri/gen/ – von Tauri bei jedem Build erzeugt (ignoriert)
- src-tauri/icons/ – generiert aus einem Quellbild (`tauri icon`)
- dist/, node_modules/, src-tauri/target/ – Build-Ausgabe
