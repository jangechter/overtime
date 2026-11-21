//! CSV-Export aller gespeicherten Tage für deutsches Excel: Semikolon, UTF-8 mit BOM, Zeilenende CRLF,
//! Datum `TT.MM.JJJJ` und Stunden als Dezimalzahl mit Komma.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::NaiveDate;

use crate::store::{Entry, Kind, Store, StoreError};
use crate::target::day_entries;

/// Kopfzeile der CSV-Datei.
pub const HEADER: &str = "Datum;Start;Ende;Pause (Min.);Stunden;Art";

/// Ohne BOM öffnet Excel eine CSV-Datei nicht als UTF-8.
const BOM: &str = "\u{feff}";

/// Der Zeitraum, der alle gespeicherten Tage enthält.
const ALL_DAYS: (&str, &str) = ("0001-01-01", "9999-12-31");

#[derive(Debug)]
pub enum ExportError {
    Store(StoreError),
    /// Der im Dialog gewählte Speicherort ist kein Dateipfad.
    InvalidPath,
    /// Die Datei ließ sich nicht schreiben (z. B. keine Berechtigung).
    Write(std::io::Error),
    /// Der Datenspeicher ist durch einen früheren Fehler gesperrt.
    StoreUnavailable,
}

impl From<StoreError> for ExportError {
    fn from(e: StoreError) -> Self {
        ExportError::Store(e)
    }
}

/// Fragt mit `choose_path` nach dem Speicherort und schreibt alle gespeicherten Tage als CSV dorthin. Gibt den Pfad
/// zurück oder `None`, wenn der Dialog abgebrochen wurde; dann wird nichts gelesen oder geschrieben. Der Datenspeicher
/// wird erst nach der Auswahl gesperrt, damit Eingaben nicht warten, solange der Dialog offen ist.
pub fn export(
    store: &Mutex<Store>,
    choose_path: impl FnOnce() -> Result<Option<PathBuf>, ExportError>,
) -> Result<Option<PathBuf>, ExportError> {
    let Some(path) = choose_path()? else { return Ok(None) };
    let csv = {
        let store = store.lock().map_err(|e| {
            eprintln!("Sperre des Datenspeichers fehlgeschlagen: {e}");
            ExportError::StoreUnavailable
        })?;
        render(&day_entries(&store, ALL_DAYS.0, ALL_DAYS.1)?)
    };
    write(&path, &csv)?;
    Ok(Some(path))
}

/// Schreibt erst in `<Pfad>.tmp` und ersetzt dann die Zieldatei, damit ein fehlgeschlagener Export eine vorhandene
/// Datei (z. B. den letzten Export) nicht halb überschreibt.
fn write(path: &Path, csv: &str) -> Result<(), ExportError> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, csv).and_then(|()| std::fs::rename(&temp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        eprintln!("CSV-Export nach {} fehlgeschlagen: {e}", path.display());
        ExportError::Write(e)
    })
}

/// Die CSV-Datei mit Kopfzeile und einer Zeile pro Eintrag, in der Reihenfolge von `entries` (nach Datum sortiert).
/// Krank/Urlaub hat leere Zeiten und Pause und die angerechneten Stunden.
pub fn render(entries: &[Entry]) -> String {
    let mut csv = format!("{BOM}{HEADER}\r\n");
    for entry in entries {
        let fields = [
            format_date(&entry.date),
            entry.start.clone().unwrap_or_default(),
            entry.end.clone().unwrap_or_default(),
            entry.break_minutes.map(|minutes| minutes.to_string()).unwrap_or_default(),
            format_hours(entry.minutes),
            kind_label(entry.kind).to_string(),
        ];
        csv.push_str(&fields.join(";"));
        csv.push_str("\r\n");
    }
    csv
}

/// `2026-11-16` → `16.11.2026`; ein unlesbares Datum bleibt unverändert.
fn format_date(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|day| day.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| date.to_string())
}

/// Minuten als Stunden mit zwei Nachkommastellen und Komma, kaufmännisch gerundet: 450 → `7,50`, 481 → `8,02`.
fn format_hours(minutes: u32) -> String {
    let hundredths = (u64::from(minutes) * 100 + 30) / 60;
    format!("{},{:02}", hundredths / 100, hundredths % 100)
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Work => "Arbeit",
        Kind::Sick => "Krank",
        Kind::Vacation => "Urlaub",
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Store(e) => e.fmt(f),
            ExportError::InvalidPath => {
                f.write_str("Der gewählte Speicherort kann nicht verwendet werden. Bitte einen anderen wählen.")
            }
            ExportError::Write(_) => {
                f.write_str("Die Datei konnte nicht gespeichert werden. Bitte einen anderen Speicherort wählen.")
            }
            ExportError::StoreUnavailable => f.write_str(crate::STORE_UNAVAILABLE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use crate::store::{Absence, Holiday};

    fn open_temp() -> (tempfile::TempDir, Mutex<Store>) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, Mutex::new(store))
    }

    /// Exportiert nach `export.csv` im Verzeichnis und gibt den Inhalt zurück.
    fn exported(dir: &tempfile::TempDir, store: &Mutex<Store>) -> String {
        let path = dir.path().join("export.csv");
        let chosen = path.clone();
        let result = export(store, || {
            assert!(store.try_lock().is_ok(), "der Datenspeicher ist gesperrt, solange der Dialog offen ist");
            Ok(Some(chosen))
        });
        assert_eq!(result.unwrap(), Some(path.clone()));
        String::from_utf8(std::fs::read(&path).unwrap()).expect("die Datei ist gültiges UTF-8")
    }

    /// Die Zeilen ohne BOM und ohne das leere Stück nach dem letzten Zeilenende.
    fn lines(csv: &str) -> Vec<&str> {
        let mut lines: Vec<&str> = csv.trim_start_matches(BOM).split("\r\n").collect();
        assert_eq!(lines.pop(), Some(""), "die letzte Zeile endet mit CRLF");
        lines
    }

    #[test]
    fn empty_database_exports_only_the_header() {
        let (dir, store) = open_temp();
        assert_eq!(exported(&dir, &store), format!("{BOM}Datum;Start;Ende;Pause (Min.);Stunden;Art\r\n"));
    }

    #[test]
    fn worked_day_has_german_date_times_break_and_decimal_hours() {
        let (dir, store) = open_temp();
        store.lock().unwrap().save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        assert_eq!(lines(&exported(&dir, &store))[1], "16.11.2026;08:00;16:30;30;8,00;Arbeit");
    }

    #[test]
    fn sick_and_vacation_days_have_empty_times_and_the_credited_hours() {
        let (dir, store) = open_temp();
        {
            let mut store = store.lock().unwrap();
            store.save_holidays("2026-12-21", "2026-12-27", &[Holiday { date: "2026-12-25".into(), name: "1. Weihnachtsfeiertag".into() }]).unwrap();
            store.save_absence("2026-12-21", Absence::Sick).unwrap();
            store.save_absence("2026-12-22", Absence::Vacation).unwrap();
            store.save_absence("2026-12-25", Absence::Vacation).unwrap();
        }
        assert_eq!(
            lines(&exported(&dir, &store))[1..],
            ["21.12.2026;;;;8,00;Krank", "22.12.2026;;;;8,00;Urlaub", "25.12.2026;;;;0,00;Urlaub"]
        );
    }

    #[test]
    fn days_are_ordered_by_date_across_years() {
        let (dir, store) = open_temp();
        {
            let store = store.lock().unwrap();
            store.save("2027-01-04", "08:00", "12:00", 0.0).unwrap();
            store.save_absence("2025-06-02", Absence::Vacation).unwrap();
            store.save("2026-12-31", "08:00", "12:00", 0.0).unwrap();
        }
        let csv = exported(&dir, &store);
        let dates: Vec<&str> = lines(&csv)[1..].iter().map(|line| &line[..10]).collect();
        assert_eq!(dates, ["02.06.2025", "31.12.2026", "04.01.2027"]);
    }

    #[test]
    fn every_line_has_six_fields_separated_by_semicolons() {
        let (dir, store) = open_temp();
        {
            let store = store.lock().unwrap();
            store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
            store.save_absence("2026-11-17", Absence::Sick).unwrap();
        }
        for line in lines(&exported(&dir, &store)) {
            assert_eq!(line.split(';').count(), 6, "{line}");
        }
    }

    #[test]
    fn file_starts_with_a_utf8_byte_order_mark_for_excel() {
        let (dir, store) = open_temp();
        exported(&dir, &store);
        let bytes = std::fs::read(dir.path().join("export.csv")).unwrap();
        assert_eq!(&bytes[..3], [0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn umlauts_are_written_as_utf8_also_in_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Überstunden.csv");
        write(&file, "Grüße;Größe\r\n").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), "Grüße;Größe\r\n".as_bytes());
    }

    #[test]
    fn hours_are_rounded_to_two_decimals_with_a_comma() {
        for (minutes, hours) in [(0, "0,00"), (1, "0,02"), (450, "7,50"), (480, "8,00"), (481, "8,02"), (615, "10,25"), (1439, "23,98")] {
            assert_eq!(format_hours(minutes), hours, "{minutes}");
        }
    }

    #[test]
    fn a_cancelled_dialog_writes_nothing_and_leaves_the_store_unlocked() {
        let (dir, store) = open_temp();
        store.lock().unwrap().save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        let result = export(&store, || {
            assert!(store.try_lock().is_ok(), "der Datenspeicher ist gesperrt, solange der Dialog offen ist");
            Ok(None)
        });
        assert_eq!(result.unwrap(), None);
        assert_eq!(files_in(&dir), ["overtime.db"]);
    }

    fn files_in(dir: &tempfile::TempDir) -> Vec<String> {
        let mut names: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }

    #[test]
    fn an_unusable_location_gives_a_german_message_and_writes_nothing() {
        let (dir, store) = open_temp();
        let error = export(&store, || Err(ExportError::InvalidPath)).err().unwrap();
        assert_eq!(error.to_string(), "Der gewählte Speicherort kann nicht verwendet werden. Bitte einen anderen wählen.");
        assert_eq!(files_in(&dir), ["overtime.db"]);
    }

    #[test]
    fn a_poisoned_store_gives_the_unavailable_message_and_writes_nothing() {
        let (dir, store) = open_temp();
        let _ = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _guard = store.lock().unwrap();
                    panic!("Sperre vergiften");
                })
                .join()
        });
        let path = dir.path().join("export.csv");
        let error = export(&store, || Ok(Some(path.clone()))).err().unwrap();
        assert!(matches!(error, ExportError::StoreUnavailable));
        assert_eq!(error.to_string(), crate::STORE_UNAVAILABLE);
        assert!(!path.exists());
    }

    #[test]
    fn a_failed_write_keeps_an_existing_file_and_leaves_no_temporary_file() {
        let (dir, store) = open_temp();
        let path = dir.path().join("export.csv");
        std::fs::write(&path, "letzter Export").unwrap();
        std::fs::create_dir(dir.path().join("export.csv.tmp")).unwrap();
        assert!(matches!(export(&store, || Ok(Some(path.clone()))), Err(ExportError::Write(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "letzter Export");
        assert_eq!(files_in(&dir), ["export.csv", "export.csv.tmp", "overtime.db"], "nur das vorher angelegte Verzeichnis");
    }

    #[test]
    fn a_successful_export_leaves_no_temporary_file() {
        let (dir, store) = open_temp();
        exported(&dir, &store);
        assert_eq!(files_in(&dir), ["export.csv", "overtime.db"]);
    }

    #[test]
    fn sick_days_are_exported_with_the_daily_target_of_their_week() {
        let (dir, store) = open_temp();
        {
            let store = store.lock().unwrap();
            store.save_settings("2026-11-30", &Settings { daily_minutes: 360, weekly_minutes: 1800, subdivision: "DE-NW".into() }).unwrap();
            store.save_absence("2026-11-27", Absence::Sick).unwrap();
            store.save_absence("2026-11-30", Absence::Sick).unwrap();
        }
        assert_eq!(lines(&exported(&dir, &store))[1..], ["27.11.2026;;;;8,00;Krank", "30.11.2026;;;;6,00;Krank"]);
    }

    #[test]
    fn exporting_again_replaces_the_file() {
        let (dir, store) = open_temp();
        store.lock().unwrap().save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        exported(&dir, &store);
        store.lock().unwrap().delete("2026-11-16").unwrap();
        assert_eq!(lines(&exported(&dir, &store)), [HEADER]);
    }

    #[test]
    fn an_unwritable_location_gives_a_german_message() {
        let (dir, store) = open_temp();
        let path = dir.path().join("fehlt").join("export.csv");
        let error = export(&store, || Ok(Some(path.clone()))).err().unwrap();
        assert!(matches!(error, ExportError::Write(_)));
        assert_eq!(error.to_string(), "Die Datei konnte nicht gespeichert werden. Bitte einen anderen Speicherort wählen.");
    }
}
