//! Speicherung der Tageseinträge in SQLite.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::hours::{worked_minutes, HoursError};

#[derive(Debug)]
pub enum StoreError {
    Hours(HoursError),
    InvalidDate,
    /// Beim Umstellen einer älteren Datenbank ließ sich die Arbeitszeit dieses Eintrags (Datum) nicht berechnen.
    InvalidLegacyEntry(String),
    /// Die Datenbank hat eine höhere Schema-Version, als diese App kennt.
    NewerSchema,
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

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Holiday {
    pub date: String,
    pub name: String,
}

/// Feiertage einer Woche und ob für sie überhaupt geladen wurde (`missing`: nie geladen).
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct WeekHolidays {
    pub holidays: Vec<Holiday>,
    pub missing: bool,
}

pub struct Store {
    conn: Connection,
}

/// Aktuelle Version des Datenbankschemas (`PRAGMA user_version`). 0 heißt: neue Datenbank oder Stand bis S4b ohne Version,
/// 1: mit Spalte `minutes` (S5a), 2: mit Tabelle `week_targets` (S5b).
const SCHEMA_VERSION: i64 = 2;

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let mut conn = Connection::open(path)?;
        migrate(&mut conn)?;
        Ok(Store { conn })
    }

    /// Speichert den Eintrag eines Tages (ersetzt einen vorhandenen) und gibt die Arbeitszeit in Minuten zurück.
    pub fn save(&self, date: &str, start: &str, end: &str, break_minutes: f64) -> Result<u32, StoreError> {
        check_date(date)?;
        let minutes = worked_minutes(start, end, break_minutes)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO entries (date, start, end, break_minutes, minutes) VALUES (?1, ?2, ?3, ?4, ?5)",
            (date, start, end, break_minutes as u32, minutes),
        )?;
        Ok(minutes)
    }

    pub fn delete(&self, date: &str) -> Result<(), StoreError> {
        check_date(date)?;
        self.conn.execute("DELETE FROM entries WHERE date = ?1", [date])?;
        Ok(())
    }

    /// Einträge von `from` bis `to` (beide einschließlich), nach Datum sortiert. Die Arbeitsminuten
    /// sind die beim Speichern berechneten und werden nicht neu berechnet.
    pub fn between(&self, from: &str, to: &str) -> Result<Vec<Entry>, StoreError> {
        check_date(from)?;
        check_date(to)?;
        let mut statement = self.conn.prepare(
            "SELECT date, start, end, break_minutes, minutes FROM entries WHERE date BETWEEN ?1 AND ?2 ORDER BY date",
        )?;
        let rows = statement.query_map([from, to], |row| {
            Ok(Entry {
                date: row.get(0)?,
                start: row.get(1)?,
                end: row.get(2)?,
                break_minutes: row.get(3)?,
                minutes: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Ersetzt die Feiertage von `from` bis `to` (beide einschließlich) durch `holidays` und merkt
    /// den Zeitraum als geladen, auch wenn er keinen Feiertag enthält.
    pub fn save_holidays(&mut self, from: &str, to: &str, holidays: &[Holiday]) -> Result<(), StoreError> {
        check_date(from)?;
        check_date(to)?;
        for holiday in holidays {
            check_date(&holiday.date)?;
        }
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM holidays WHERE date BETWEEN ?1 AND ?2", [from, to])?;
        for holiday in holidays {
            tx.execute("INSERT OR REPLACE INTO holidays (date, name) VALUES (?1, ?2)", [&holiday.date, &holiday.name])?;
        }
        // Überlappende Zeiträume zu einem zusammenfassen, damit die Tabelle nicht mit jedem Starttag wächst.
        // Nur aneinandergrenzende (z. B. Sonntag/Montag) werden nicht zusammengefasst.
        let (merged_from, merged_to): (String, String) = tx.query_row(
            "SELECT MIN(from_date), MAX(to_date) FROM (
                SELECT ?1 AS from_date, ?2 AS to_date
                UNION ALL
                SELECT from_date, to_date FROM loaded_ranges WHERE from_date <= ?2 AND to_date >= ?1
            )",
            [from, to],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        tx.execute("DELETE FROM loaded_ranges WHERE from_date <= ?2 AND to_date >= ?1", [from, to])?;
        tx.execute("INSERT INTO loaded_ranges (from_date, to_date) VALUES (?1, ?2)", [&merged_from, &merged_to])?;
        tx.commit()?;
        Ok(())
    }

    /// Feiertage von `from` bis `to` (beide einschließlich), nach Datum sortiert.
    pub fn holidays_between(&self, from: &str, to: &str) -> Result<Vec<Holiday>, StoreError> {
        check_date(from)?;
        check_date(to)?;
        let mut statement =
            self.conn.prepare("SELECT date, name FROM holidays WHERE date BETWEEN ?1 AND ?2 ORDER BY date")?;
        let rows = statement.query_map([from, to], |row| Ok(Holiday { date: row.get(0)?, name: row.get(1)? }))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Alle geladenen Zeiträume `(von, bis)`, nach Beginn sortiert.
    pub fn loaded_ranges(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut statement = self.conn.prepare("SELECT from_date, to_date FROM loaded_ranges ORDER BY from_date")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Das eingefrorene Soll der Woche mit Montag `week_start` in Minuten, falls vorhanden.
    pub fn week_target(&self, week_start: &str) -> Result<Option<u32>, StoreError> {
        check_date(week_start)?;
        Ok(self
            .conn
            .query_row("SELECT target_minutes FROM week_targets WHERE week_start = ?1", [week_start], |row| row.get(0))
            .optional()?)
    }

    /// Friert das Soll der Woche mit Montag `week_start` ein. Ein vorhandenes Soll bleibt unverändert.
    pub fn freeze_week_target(&self, week_start: &str, minutes: u32) -> Result<(), StoreError> {
        check_date(week_start)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO week_targets (week_start, target_minutes) VALUES (?1, ?2)",
            (week_start, minutes),
        )?;
        Ok(())
    }

    /// Summe der gespeicherten Arbeitsminuten von `from` bis `to` (beide einschließlich).
    pub fn week_total(&self, from: &str, to: &str) -> Result<u32, StoreError> {
        check_date(from)?;
        check_date(to)?;
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(minutes), 0) FROM entries WHERE date BETWEEN ?1 AND ?2",
            [from, to],
            |row| row.get(0),
        )?)
    }

    /// Feiertage von `from` bis `to` samt Angabe, ob der Zeitraum nie geladen wurde.
    pub fn week_holidays(&self, from: &str, to: &str) -> Result<WeekHolidays, StoreError> {
        Ok(WeekHolidays { holidays: self.holidays_between(from, to)?, missing: !self.is_loaded(from, to)? })
    }

    /// Ob ein einzelner geladener Zeitraum `from` bis `to` vollständig abdeckt.
    pub fn is_loaded(&self, from: &str, to: &str) -> Result<bool, StoreError> {
        check_date(from)?;
        check_date(to)?;
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM loaded_ranges WHERE from_date <= ?1 AND to_date >= ?2)",
            [from, to],
            |row| row.get(0),
        )?)
    }
}

/// Bringt die Datenbank auf `SCHEMA_VERSION`, ganz oder gar nicht. Eine Datenbank im Schema bis S4b bekommt die
/// Spalte `minutes`, die einmal aus den Rohzeiten nachgerechnet wird; danach werden Minuten nie neu berechnet.
fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::NewerSchema);
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.transaction()?;
    if version < 1 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                date TEXT PRIMARY KEY,
                start TEXT NOT NULL,
                end TEXT NOT NULL,
                break_minutes INTEGER NOT NULL,
                minutes INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS holidays (
                date TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS loaded_ranges (
                from_date TEXT NOT NULL,
                to_date TEXT NOT NULL,
                PRIMARY KEY (from_date, to_date)
            );",
        )?;
        let has_minutes: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('entries') WHERE name = 'minutes')",
            [],
            |row| row.get(0),
        )?;
        if !has_minutes {
            tx.execute("ALTER TABLE entries ADD COLUMN minutes INTEGER NOT NULL DEFAULT 0", [])?;
            let rows = {
                let mut statement = tx.prepare("SELECT date, start, end, break_minutes FROM entries")?;
                let rows = statement.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, u32>(3)?))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            for (date, start, end, break_minutes) in rows {
                let minutes = worked_minutes(&start, &end, f64::from(break_minutes))
                    .map_err(|_| StoreError::InvalidLegacyEntry(date.clone()))?;
                tx.execute("UPDATE entries SET minutes = ?1 WHERE date = ?2", (minutes, &date))?;
            }
        }
    }
    if version < 2 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS week_targets (
                week_start TEXT PRIMARY KEY,
                target_minutes INTEGER NOT NULL
            );",
        )?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

/// Existierendes Kalenderdatum im Format `YYYY-MM-DD`.
pub(crate) fn check_date(date: &str) -> Result<(), StoreError> {
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
            StoreError::InvalidLegacyEntry(date) => write!(
                f,
                "Der gespeicherte Eintrag vom {date} ist ungültig, die Datenbank konnte nicht auf das neue Format umgestellt werden."
            ),
            StoreError::NewerSchema => {
                f.write_str("Die Datenbank stammt aus einer neueren Version der App und kann nicht geöffnet werden.")
            }
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

    fn holiday(date: &str, name: &str) -> Holiday {
        Holiday { date: date.into(), name: name.into() }
    }

    #[test]
    fn saved_holidays_are_read_back_within_the_range_sorted_by_date() {
        let (_dir, mut store) = open_temp();
        store
            .save_holidays(
                "2026-12-06",
                "2027-02-20",
                &[holiday("2027-01-01", "Neujahr"), holiday("2026-12-25", "1. Weihnachtsfeiertag")],
            )
            .unwrap();
        assert_eq!(
            store.holidays_between("2026-12-21", "2027-01-03").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-01", "Neujahr")]
        );
        assert_eq!(store.holidays_between("2026-12-26", "2026-12-31").unwrap(), vec![]);
    }

    #[test]
    fn saving_a_window_again_replaces_its_holidays_but_keeps_those_outside() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-05-01", "2026-05-31", &[holiday("2026-05-14", "Christi Himmelfahrt")]).unwrap();
        store.save_holidays("2026-12-06", "2026-12-31", &[holiday("2026-12-25", "Alt")]).unwrap();
        store.save_holidays("2026-12-06", "2026-12-31", &[holiday("2026-12-26", "Neu")]).unwrap();
        assert_eq!(
            store.holidays_between("2026-01-01", "2026-12-31").unwrap(),
            vec![holiday("2026-05-14", "Christi Himmelfahrt"), holiday("2026-12-26", "Neu")]
        );
    }

    #[test]
    fn holidays_and_loaded_ranges_survive_reopening_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        Store::open(&path)
            .unwrap()
            .save_holidays("2026-12-06", "2027-02-20", &[holiday("2026-12-25", "1. Weihnachtsfeiertag")])
            .unwrap();
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.holidays_between("2026-12-25", "2026-12-25").unwrap().len(), 1);
        assert!(reopened.is_loaded("2026-12-14", "2026-12-20").unwrap());
    }

    #[test]
    fn week_without_holidays_counts_as_loaded_inside_a_saved_window() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2027-01-20", &[holiday("2026-12-25", "1. Weihnachtsfeiertag")]).unwrap();
        assert!(store.is_loaded("2026-11-09", "2026-11-15").unwrap());
        assert!(store.is_loaded("2026-11-06", "2027-01-20").unwrap());
    }

    #[test]
    fn range_that_was_never_loaded_does_not_count_as_loaded() {
        let (_dir, mut store) = open_temp();
        assert!(!store.is_loaded("2026-11-09", "2026-11-15").unwrap());
        store.save_holidays("2026-11-06", "2027-01-20", &[]).unwrap();
        assert!(!store.is_loaded("2027-03-01", "2027-03-07").unwrap());
        assert!(!store.is_loaded("2026-11-02", "2026-11-08").unwrap(), "Woche ragt vor das Fenster");
        assert!(!store.is_loaded("2027-01-18", "2027-01-24").unwrap(), "Woche ragt hinter das Fenster");
    }

    #[test]
    fn week_that_was_never_loaded_is_reported_as_missing() {
        let (_dir, store) = open_temp();
        assert_eq!(
            store.week_holidays("2027-03-01", "2027-03-07").unwrap(),
            WeekHolidays { holidays: vec![], missing: true }
        );
    }

    #[test]
    fn loaded_week_without_holidays_is_not_reported_as_missing() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2027-01-20", &[holiday("2026-12-25", "1. Weihnachtsfeiertag")]).unwrap();
        assert_eq!(
            store.week_holidays("2026-11-09", "2026-11-15").unwrap(),
            WeekHolidays { holidays: vec![], missing: false }
        );
    }

    #[test]
    fn loaded_week_returns_its_holidays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2027-01-20", &[holiday("2026-12-25", "1. Weihnachtsfeiertag")]).unwrap();
        assert_eq!(
            store.week_holidays("2026-12-21", "2026-12-27").unwrap(),
            WeekHolidays { holidays: vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")], missing: false }
        );
    }

    #[test]
    fn week_reaching_beyond_the_loaded_window_is_reported_as_missing_but_keeps_its_holidays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2027-01-20", &[holiday("2027-01-01", "Neujahr")]).unwrap();
        assert_eq!(
            store.week_holidays("2027-01-18", "2027-01-24").unwrap(),
            WeekHolidays { holidays: vec![], missing: true }
        );
        assert_eq!(
            store.week_holidays("2026-12-28", "2027-01-03").unwrap(),
            WeekHolidays { holidays: vec![holiday("2027-01-01", "Neujahr")], missing: false }
        );
    }

    fn loaded_range_count(store: &Store) -> u32 {
        store.conn.query_row("SELECT COUNT(*) FROM loaded_ranges", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn overlapping_windows_are_merged_into_one_loaded_range() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2027-01-01", "2027-01-10", &[]).unwrap();
        assert!(!store.is_loaded("2027-01-04", "2027-01-15").unwrap());
        store.save_holidays("2027-01-08", "2027-01-20", &[]).unwrap();
        assert!(store.is_loaded("2027-01-04", "2027-01-15").unwrap(), "Woche liegt nur in der Vereinigung beider Fenster");
        assert_eq!(loaded_range_count(&store), 1);
    }

    #[test]
    fn daily_sliding_window_keeps_a_single_loaded_range() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2027-01-20", &[]).unwrap();
        store.save_holidays("2026-11-07", "2027-01-21", &[]).unwrap();
        store.save_holidays("2026-11-08", "2027-01-22", &[]).unwrap();
        assert_eq!(loaded_range_count(&store), 1);
        assert!(store.is_loaded("2026-11-06", "2027-01-22").unwrap());
    }

    #[test]
    fn windows_that_do_not_overlap_stay_separate_ranges() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-06", "2026-11-20", &[]).unwrap();
        store.save_holidays("2027-03-01", "2027-03-14", &[]).unwrap();
        assert_eq!(loaded_range_count(&store), 2);
        assert!(!store.is_loaded("2026-11-18", "2027-03-02").unwrap(), "die Lücke dazwischen ist nicht geladen");
    }

    #[test]
    fn merging_ranges_keeps_the_holidays_of_all_windows() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-06", "2026-12-28", &[holiday("2026-12-25", "1. Weihnachtsfeiertag")]).unwrap();
        store.save_holidays("2026-12-28", "2027-02-20", &[holiday("2027-01-01", "Neujahr")]).unwrap();
        assert_eq!(
            store.holidays_between("2026-12-01", "2027-02-28").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-01", "Neujahr")]
        );
    }

    #[test]
    fn saving_holidays_with_a_malformed_date_is_rejected_and_stores_nothing() {
        let (_dir, mut store) = open_temp();
        assert!(matches!(store.save_holidays("morgen", "2027-02-20", &[]), Err(StoreError::InvalidDate)));
        assert!(matches!(
            store.save_holidays("2026-12-06", "2027-02-20", &[holiday("25.12.2026", "Weihnachten")]),
            Err(StoreError::InvalidDate)
        ));
        assert!(!store.is_loaded("2026-12-06", "2027-02-20").unwrap());
    }

    fn raw(path: &Path) -> Connection {
        Connection::open(path).unwrap()
    }

    fn schema_version(path: &Path) -> i64 {
        raw(path).query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap()
    }

    fn has_minutes_column(path: &Path) -> bool {
        let conn = raw(path);
        let mut statement = conn.prepare("PRAGMA table_info(entries)").unwrap();
        let names: Vec<String> = statement.query_map([], |row| row.get(1)).unwrap().map(|name| name.unwrap()).collect();
        names.iter().any(|name| name == "minutes")
    }

    /// Datenbank im Schema bis S4b: `entries` ohne `minutes`, keine Schema-Version.
    fn legacy_db(path: &Path, rows: &[(&str, &str, &str, u32)]) {
        let conn = raw(path);
        conn.execute_batch(
            "CREATE TABLE entries (date TEXT PRIMARY KEY, start TEXT NOT NULL, end TEXT NOT NULL, break_minutes INTEGER NOT NULL);
            CREATE TABLE holidays (date TEXT PRIMARY KEY, name TEXT NOT NULL);
            CREATE TABLE loaded_ranges (from_date TEXT NOT NULL, to_date TEXT NOT NULL, PRIMARY KEY (from_date, to_date));
            INSERT INTO holidays VALUES ('2026-12-25', '1. Weihnachtsfeiertag');
            INSERT INTO loaded_ranges VALUES ('2026-11-30', '2027-02-21');",
        )
        .unwrap();
        for (date, start, end, break_minutes) in rows {
            conn.execute(
                "INSERT INTO entries (date, start, end, break_minutes) VALUES (?1, ?2, ?3, ?4)",
                (date, start, end, break_minutes),
            )
            .unwrap();
        }
    }

    #[test]
    fn stored_minutes_are_read_back_without_being_recalculated() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.conn.execute("UPDATE entries SET minutes = 123 WHERE date = '2026-11-16'", []).unwrap();
        assert_eq!(store.between("2026-11-16", "2026-11-16").unwrap()[0].minutes, 123);
    }

    #[test]
    fn week_total_is_zero_without_entries() {
        let (_dir, store) = open_temp();
        assert_eq!(store.week_total("2026-11-16", "2026-11-22").unwrap(), 0);
    }

    #[test]
    fn week_total_sums_the_stored_minutes_of_the_week_only() {
        let (_dir, store) = open_temp();
        store.save("2026-11-15", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.save("2026-11-18", "09:00", "12:15", 0.0).unwrap();
        store.save("2026-11-22", "10:00", "11:00", 0.0).unwrap();
        store.save("2026-11-23", "08:00", "12:00", 0.0).unwrap();
        assert_eq!(store.week_total("2026-11-16", "2026-11-22").unwrap(), 480 + 195 + 60);
    }

    #[test]
    fn week_total_follows_replacing_and_deleting_entries() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.save("2026-11-17", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-16", "08:00", "10:00", 0.0).unwrap();
        assert_eq!(store.week_total("2026-11-16", "2026-11-22").unwrap(), 120 + 240);
        store.delete("2026-11-17").unwrap();
        assert_eq!(store.week_total("2026-11-16", "2026-11-22").unwrap(), 120);
    }

    #[test]
    fn week_total_across_the_year_boundary() {
        let (_dir, store) = open_temp();
        for date in ["2026-12-27", "2026-12-28", "2027-01-01", "2027-01-03", "2027-01-04"] {
            store.save(date, "08:00", "12:00", 0.0).unwrap();
        }
        assert_eq!(store.week_total("2026-12-28", "2027-01-03").unwrap(), 3 * 240);
    }

    #[test]
    fn week_total_rejects_a_malformed_date() {
        let (_dir, store) = open_temp();
        assert!(matches!(store.week_total("morgen", "2026-11-22"), Err(StoreError::InvalidDate)));
    }

    #[test]
    fn fresh_database_gets_the_current_schema_version_and_the_minutes_column() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        Store::open(&path).unwrap();
        assert_eq!(schema_version(&path), 2);
        assert!(has_minutes_column(&path));
    }

    #[test]
    fn database_in_the_old_schema_is_migrated_with_recalculated_minutes_and_keeps_its_other_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        legacy_db(&path, &[("2026-11-16", "08:00", "16:30", 30), ("2026-11-17", "09:00", "12:00", 0)]);
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.between("2026-11-16", "2026-11-22").unwrap(),
            vec![entry("2026-11-16", "08:00", "16:30", 30, 480), entry("2026-11-17", "09:00", "12:00", 0, 180)]
        );
        assert_eq!(store.week_total("2026-11-16", "2026-11-22").unwrap(), 660);
        assert_eq!(store.holidays_between("2026-12-25", "2026-12-25").unwrap(), vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")]);
        assert!(store.is_loaded("2026-12-01", "2027-02-01").unwrap());
        assert_eq!(schema_version(&path), 2);
    }

    #[test]
    fn migration_runs_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        legacy_db(&path, &[("2026-11-16", "08:00", "16:30", 30)]);
        Store::open(&path).unwrap();
        raw(&path).execute("UPDATE entries SET minutes = 999", []).unwrap();
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.between("2026-11-16", "2026-11-16").unwrap()[0].minutes, 999);
    }

    #[test]
    fn migration_with_an_invalid_row_fails_in_german_and_leaves_the_database_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        legacy_db(&path, &[("2026-11-16", "08:00", "16:30", 30), ("2026-11-17", "16:00", "08:00", 0)]);
        let error = Store::open(&path).err().unwrap();
        assert!(matches!(error, StoreError::InvalidLegacyEntry(ref date) if date == "2026-11-17"));
        assert_eq!(
            error.to_string(),
            "Der gespeicherte Eintrag vom 2026-11-17 ist ungültig, die Datenbank konnte nicht auf das neue Format umgestellt werden."
        );
        assert_eq!(schema_version(&path), 0);
        assert!(!has_minutes_column(&path));
    }

    #[test]
    fn database_from_a_newer_app_version_is_not_opened_and_not_touched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        legacy_db(&path, &[]);
        raw(&path).pragma_update(None, "user_version", 99).unwrap();
        let error = Store::open(&path).err().unwrap();
        assert!(matches!(error, StoreError::NewerSchema));
        assert_eq!(error.to_string(), "Die Datenbank stammt aus einer neueren Version der App und kann nicht geöffnet werden.");
        assert_eq!(schema_version(&path), 99);
        assert!(!has_minutes_column(&path));
    }

    #[test]
    fn loaded_ranges_are_listed_by_start() {
        let (_dir, mut store) = open_temp();
        assert!(store.loaded_ranges().unwrap().is_empty());
        store.save_holidays("2027-03-01", "2027-03-07", &[]).unwrap();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        assert_eq!(
            store.loaded_ranges().unwrap(),
            vec![("2026-11-02".to_string(), "2026-11-29".to_string()), ("2027-03-01".to_string(), "2027-03-07".to_string())]
        );
    }

    #[test]
    fn week_without_a_frozen_target_has_none() {
        let (_dir, store) = open_temp();
        assert_eq!(store.week_target("2026-11-16").unwrap(), None);
    }

    #[test]
    fn frozen_target_is_read_back_and_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        Store::open(&path).unwrap().freeze_week_target("2026-11-16", 1920).unwrap();
        assert_eq!(Store::open(&path).unwrap().week_target("2026-11-16").unwrap(), Some(1920));
    }

    #[test]
    fn freezing_again_keeps_the_first_target() {
        let (_dir, store) = open_temp();
        store.freeze_week_target("2026-11-16", 1920).unwrap();
        store.freeze_week_target("2026-11-16", 2400).unwrap();
        assert_eq!(store.week_target("2026-11-16").unwrap(), Some(1920));
    }

    #[test]
    fn freezing_rejects_a_malformed_date() {
        let (_dir, store) = open_temp();
        assert!(matches!(store.freeze_week_target("morgen", 2400), Err(StoreError::InvalidDate)));
        assert!(matches!(store.week_target("morgen"), Err(StoreError::InvalidDate)));
    }

    #[test]
    fn database_in_the_s5a_schema_gets_the_targets_table_and_keeps_its_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        raw(&path)
            .execute_batch(
                "CREATE TABLE entries (date TEXT PRIMARY KEY, start TEXT NOT NULL, end TEXT NOT NULL, break_minutes INTEGER NOT NULL, minutes INTEGER NOT NULL);
                CREATE TABLE holidays (date TEXT PRIMARY KEY, name TEXT NOT NULL);
                CREATE TABLE loaded_ranges (from_date TEXT NOT NULL, to_date TEXT NOT NULL, PRIMARY KEY (from_date, to_date));
                INSERT INTO entries VALUES ('2026-11-16', '08:00', '16:30', 30, 999);
                PRAGMA user_version = 1;",
            )
            .unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(store.between("2026-11-16", "2026-11-16").unwrap()[0].minutes, 999, "Minuten werden nicht neu berechnet");
        store.freeze_week_target("2026-11-16", 1920).unwrap();
        assert_eq!(store.week_target("2026-11-16").unwrap(), Some(1920));
        assert_eq!(schema_version(&path), 2);
    }
}
