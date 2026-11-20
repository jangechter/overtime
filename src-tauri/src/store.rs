//! Speicherung der Tageseinträge in SQLite.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::hours::{worked_minutes, HoursError};

#[derive(Debug)]
pub enum StoreError {
    Hours(HoursError),
    InvalidDate,
    Db(rusqlite::Error),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub date: String,
    pub start: String,
    pub end: String,
    pub break_minutes: u32,
    pub minutes: u32,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                date TEXT PRIMARY KEY,
                start TEXT NOT NULL,
                end TEXT NOT NULL,
                break_minutes INTEGER NOT NULL
            )",
        )?;
        Ok(Store { conn })
    }

    /// Speichert den Eintrag eines Tages (ersetzt einen vorhandenen) und gibt die Arbeitszeit in Minuten zurück.
    pub fn save(&self, date: &str, start: &str, end: &str, break_minutes: f64) -> Result<u32, StoreError> {
        check_date(date)?;
        let minutes = worked_minutes(start, end, break_minutes)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO entries (date, start, end, break_minutes) VALUES (?1, ?2, ?3, ?4)",
            (date, start, end, break_minutes as u32),
        )?;
        Ok(minutes)
    }

    pub fn delete(&self, date: &str) -> Result<(), StoreError> {
        check_date(date)?;
        self.conn.execute("DELETE FROM entries WHERE date = ?1", [date])?;
        Ok(())
    }

    /// Einträge von `from` bis `to` (beide einschließlich), nach Datum sortiert.
    pub fn between(&self, from: &str, to: &str) -> Result<Vec<Entry>, StoreError> {
        check_date(from)?;
        check_date(to)?;
        let mut statement = self.conn.prepare(
            "SELECT date, start, end, break_minutes FROM entries WHERE date BETWEEN ?1 AND ?2 ORDER BY date",
        )?;
        let rows = statement.query_map([from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, u32>(3)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (date, start, end, break_minutes) = row?;
            let minutes = worked_minutes(&start, &end, f64::from(break_minutes))?;
            entries.push(Entry { date, start, end, break_minutes, minutes });
        }
        Ok(entries)
    }
}

/// Existierendes Kalenderdatum im Format `YYYY-MM-DD`.
fn check_date(date: &str) -> Result<(), StoreError> {
    let bytes = date.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !shape_ok {
        return Err(StoreError::InvalidDate);
    }
    let year: u32 = date[0..4].parse().map_err(|_| StoreError::InvalidDate)?;
    let month: u32 = date[5..7].parse().map_err(|_| StoreError::InvalidDate)?;
    let day: u32 = date[8..10].parse().map_err(|_| StoreError::InvalidDate)?;
    let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap => 29,
        2 => 28,
        _ => return Err(StoreError::InvalidDate),
    };
    if !(1..=days_in_month).contains(&day) {
        return Err(StoreError::InvalidDate);
    }
    Ok(())
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        eprintln!("Datenbankfehler: {e}");
        StoreError::Db(e)
    }
}

impl From<HoursError> for StoreError {
    fn from(e: HoursError) -> Self {
        StoreError::Hours(e)
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Hours(e) => e.fmt(f),
            StoreError::InvalidDate => f.write_str("Ungültiges Datum, erwartet wird JJJJ-MM-TT."),
            StoreError::Db(_) => f.write_str(
                "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(date: &str, start: &str, end: &str, break_minutes: u32, minutes: u32) -> Entry {
        Entry { date: date.into(), start: start.into(), end: end.into(), break_minutes, minutes }
    }

    fn open_temp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn saved_entry_can_be_read_back_with_hours() {
        let (_dir, store) = open_temp();
        assert_eq!(store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap(), 480);
        assert_eq!(
            store.between("2026-11-16", "2026-11-16").unwrap(),
            vec![entry("2026-11-16", "08:00", "16:30", 30, 480)]
        );
    }

    #[test]
    fn saving_again_replaces_the_old_entry() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.save("2026-11-16", "09:00", "17:00", 0.0).unwrap();
        assert_eq!(
            store.between("2026-11-16", "2026-11-16").unwrap(),
            vec![entry("2026-11-16", "09:00", "17:00", 0, 480)]
        );
    }

    #[test]
    fn entries_survive_reopening_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        Store::open(&path).unwrap().save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.between("2026-11-16", "2026-11-16").unwrap().len(), 1);
    }

    #[test]
    fn between_returns_only_the_range_sorted_by_date() {
        let (_dir, store) = open_temp();
        store.save("2026-11-18", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-15", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-16", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-23", "08:00", "12:00", 0.0).unwrap();
        let dates: Vec<String> = store.between("2026-11-16", "2026-11-22").unwrap().into_iter().map(|e| e.date).collect();
        assert_eq!(dates, vec!["2026-11-16", "2026-11-18"]);
    }

    #[test]
    fn deleted_entry_is_gone() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.delete("2026-11-16").unwrap();
        assert!(store.between("2026-11-16", "2026-11-16").unwrap().is_empty());
    }

    #[test]
    fn deleting_a_missing_entry_is_not_an_error() {
        let (_dir, store) = open_temp();
        assert!(store.delete("2026-11-16").is_ok());
    }

    #[test]
    fn invalid_entry_is_rejected_and_not_stored() {
        let (_dir, store) = open_temp();
        assert!(matches!(
            store.save("2026-11-16", "16:00", "08:00", 0.0),
            Err(StoreError::Hours(HoursError::EndNotAfterStart))
        ));
        assert!(store.between("2026-11-16", "2026-11-16").unwrap().is_empty());
    }

    #[test]
    fn invalid_entry_does_not_overwrite_the_stored_one() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        assert!(store.save("2026-11-16", "16:00", "08:00", 0.0).is_err());
        assert_eq!(store.between("2026-11-16", "2026-11-16").unwrap()[0].end, "16:30");
    }

    #[test]
    fn malformed_date_is_rejected() {
        let (_dir, store) = open_temp();
        for date in ["16.11.2026", "2026-13-01", "2026-11-32", "2026-1-1", "morgen", "2026-02-31", "2026-04-31", "2026-02-29", "2100-02-29"] {
            assert!(matches!(store.save(date, "08:00", "16:00", 0.0), Err(StoreError::InvalidDate)), "{date}");
        }
    }

    #[test]
    fn leap_day_is_accepted() {
        let (_dir, store) = open_temp();
        assert!(store.save("2028-02-29", "08:00", "16:00", 0.0).is_ok());
        assert!(store.save("2000-02-29", "08:00", "16:00", 0.0).is_ok());
    }

    #[test]
    fn database_error_message_is_german_and_hides_technical_detail() {
        let message = StoreError::Db(rusqlite::Error::InvalidQuery).to_string();
        assert_eq!(message, "Die Datenbank konnte nicht gelesen oder geschrieben werden. Bitte die App neu starten.");
    }

    #[test]
    fn week_across_the_year_boundary_is_loaded_completely() {
        let (_dir, store) = open_temp();
        for date in ["2026-12-27", "2026-12-28", "2026-12-31", "2027-01-01", "2027-01-03", "2027-01-04"] {
            store.save(date, "08:00", "12:00", 0.0).unwrap();
        }
        let dates: Vec<String> = store.between("2026-12-28", "2027-01-03").unwrap().into_iter().map(|e| e.date).collect();
        assert_eq!(dates, vec!["2026-12-28", "2026-12-31", "2027-01-01", "2027-01-03"]);
    }
}
