//! Einstellungen (Tagessoll, Wochen-Soll, Bundesland) als Historie mit „gilt ab“. Eine Änderung gilt immer ab dem
//! Montag der nächsten Woche; Einträge, die schon gelten, werden nie geändert oder gelöscht. So rechnet jede Woche
//! dauerhaft mit den Werten, die in ihr galten.

use chrono::{Days, NaiveDate};
use serde::Serialize;

use crate::store::{Store, StoreError};
use crate::week::{format_date, monday_of};

/// „Gilt ab“ des Starteintrags aus der Migration: gilt seit jeher.
pub const START: &str = "0001-01-01";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub daily_minutes: u32,
    pub weekly_minutes: u32,
    /// Bundesland als Code für OpenHolidays, z. B. `DE-NW`.
    pub subdivision: String,
}

impl Default for Settings {
    /// Die Werte, mit denen bis S4a fest gerechnet wurde: 8 h, 40 h, NRW.
    fn default() -> Self {
        Settings { daily_minutes: 480, weekly_minutes: 2400, subdivision: "DE-NW".to_string() }
    }
}

/// Die Einstellungshistorie, nach „gilt ab“ sortiert; beginnt immer mit dem Starteintrag (`START`).
pub struct History(Vec<(String, Settings)>);

impl History {
    /// Lädt die Historie. Fehlt der Starteintrag (z. B. von Hand gelöscht), ist das ein Fehler statt stiller Startwerte.
    pub fn load(store: &Store) -> Result<History, StoreError> {
        let history = store.settings_history()?;
        if history.first().map(|(valid_from, _)| valid_from.as_str()) != Some(START) {
            eprintln!("Einstellungshistorie ohne Starteintrag: {history:?}");
            return Err(StoreError::MissingSettings);
        }
        Ok(History(history))
    }

    /// Die Einstellungen, die am Tag `date` (`YYYY-MM-DD`) gelten. Änderungen beginnen immer an einem Montag, deshalb
    /// gilt für alle Tage einer Woche dasselbe.
    pub fn at(&self, date: &str) -> Settings {
        // Der Starteintrag liegt vor jedem gültigen Datum, deshalb findet sich immer ein Eintrag.
        let (_, settings) = self.0.iter().rev().find(|(valid_from, _)| valid_from.as_str() <= date).unwrap_or(&self.0[0]);
        settings.clone()
    }
}

/// Eine gespeicherte Änderung, die noch nicht gilt.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub valid_from: String,
    pub settings: Settings,
}

/// Was die Einstellungsansicht zeigt: die Werte der laufenden Woche und eine noch nicht wirksame Änderung.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub current: Settings,
    pub pending: Option<Pending>,
}

#[derive(Debug)]
pub enum SettingsError {
    DailyFormat,
    DailyRange,
    WeeklyFormat,
    WeeklyRange,
    /// Es gibt schon Einstellungen für eine Woche nach dem nächsten Montag: Die Systemuhr ging zurück.
    ClockBehind,
    Store(StoreError),
}

impl From<StoreError> for SettingsError {
    fn from(e: StoreError) -> Self {
        SettingsError::Store(e)
    }
}

/// Der Montag der Woche nach `today`: ab dann gilt eine heute gespeicherte Änderung.
pub fn next_monday(today: NaiveDate) -> NaiveDate {
    monday_of(today) + Days::new(7)
}

pub fn overview(store: &Store, today: NaiveDate) -> Result<Overview, StoreError> {
    let history = History::load(store)?;
    let this_monday = format_date(monday_of(today));
    let pending = history
        .0
        .iter()
        .find(|(valid_from, _)| *valid_from > this_monday)
        .map(|(valid_from, settings)| Pending { valid_from: valid_from.clone(), settings: settings.clone() });
    Ok(Overview { current: history.at(&this_monday), pending })
}

/// Speichert Tagessoll und Wochen-Soll (Text `H:MM`), gültig ab dem Montag der nächsten Woche. Eine noch nicht wirksame
/// Änderung wird ersetzt; entsprechen die Werte denen der laufenden Woche, wird sie zurückgenommen. Das Bundesland
/// bleibt, wie es ab nächster Woche gilt.
pub fn change(store: &Store, today: NaiveDate, daily: &str, weekly: &str) -> Result<Overview, SettingsError> {
    let daily_minutes = parse_duration(daily).ok_or(SettingsError::DailyFormat)?;
    if !(1..=1440).contains(&daily_minutes) {
        return Err(SettingsError::DailyRange);
    }
    let weekly_minutes = parse_duration(weekly).ok_or(SettingsError::WeeklyFormat)?;
    if !(1..=10080).contains(&weekly_minutes) {
        return Err(SettingsError::WeeklyRange);
    }
    let history = History::load(store)?;
    let next = format_date(next_monday(today));
    // Mit richtiger Uhr gibt es nach dem nächsten Montag keinen Eintrag. Sonst ging die Uhr zurück, und Speichern könnte
    // einen Eintrag ersetzen oder löschen, der schon gilt.
    if history.0.iter().any(|(valid_from, _)| *valid_from > next) {
        eprintln!("Einstellungen nach {next} vorhanden, Speichern verweigert");
        return Err(SettingsError::ClockBehind);
    }
    let current = history.at(&format_date(monday_of(today)));
    let changed = Settings { daily_minutes, weekly_minutes, subdivision: history.at(&next).subdivision };
    if changed == current {
        store.delete_settings(&next)?;
    } else {
        store.save_settings(&next, &changed)?;
    }
    Ok(overview(store, today)?)
}

/// Stunden und Minuten als `H:MM` (auch `HH:MM` oder `HHH:MM`) oder ganze Stunden (`8`) in Minuten.
fn parse_duration(text: &str) -> Option<u32> {
    let text = text.trim();
    let (hours, minutes) = text.split_once(':').unwrap_or((text, "00"));
    let digits = |s: &str, lengths: std::ops::RangeInclusive<usize>| {
        lengths.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(hours, 1..=3) || !digits(minutes, 2..=2) {
        return None;
    }
    let minutes: u32 = minutes.parse().ok()?;
    if minutes > 59 {
        return None;
    }
    Some(hours.parse::<u32>().ok()? * 60 + minutes)
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::DailyFormat => f.write_str("Bitte das Tagessoll als Stunden und Minuten eingeben, z. B. 8:00."),
            SettingsError::DailyRange => f.write_str("Das Tagessoll muss zwischen 0:01 und 24:00 h liegen."),
            SettingsError::WeeklyFormat => {
                f.write_str("Bitte das Wochen-Soll als Stunden und Minuten eingeben, z. B. 40:00.")
            }
            SettingsError::WeeklyRange => f.write_str("Das Wochen-Soll muss zwischen 0:01 und 168:00 h liegen."),
            SettingsError::ClockBehind => f.write_str(
                "Es gibt schon Einstellungen für eine spätere Woche, die Systemuhr geht offenbar falsch. Bitte Datum und Uhrzeit prüfen.",
            ),
            SettingsError::Store(e) => e.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, store)
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn settings(daily_minutes: u32, weekly_minutes: u32) -> Settings {
        Settings { daily_minutes, weekly_minutes, subdivision: "DE-NW".into() }
    }

    fn pending(valid_from: &str, daily: u32, weekly: u32) -> Option<Pending> {
        Some(Pending { valid_from: valid_from.into(), settings: settings(daily, weekly) })
    }

    // --- next_monday ---

    #[test]
    fn a_change_applies_from_the_monday_of_the_next_week() {
        for day in ["2026-11-23", "2026-11-25", "2026-11-29"] {
            assert_eq!(next_monday(date(day)), date("2026-11-30"), "{day}");
        }
    }

    #[test]
    fn a_change_at_the_end_of_the_year_applies_in_the_new_year() {
        assert_eq!(next_monday(date("2026-12-30")), date("2027-01-04"));
        assert_eq!(next_monday(date("2027-01-03")), date("2027-01-04"));
    }

    // --- History::at ---

    #[test]
    fn each_day_gets_the_settings_valid_in_its_week() {
        let (_dir, store) = open_temp();
        store.save_settings("2026-11-30", &settings(360, 1800)).unwrap();
        store.save_settings("2027-01-04", &settings(480, 1920)).unwrap();
        let history = History::load(&store).unwrap();
        assert_eq!(history.at("2026-11-29"), Settings::default());
        assert_eq!(history.at("2026-11-30"), settings(360, 1800));
        assert_eq!(history.at("2026-12-31"), settings(360, 1800));
        assert_eq!(history.at("2027-01-04"), settings(480, 1920));
        assert_eq!(history.at("2020-01-01"), Settings::default(), "Wochen vor jeder Änderung");
    }

    // --- overview ---

    #[test]
    fn without_a_change_the_start_values_apply_and_nothing_is_pending() {
        let (_dir, store) = open_temp();
        assert_eq!(overview(&store, date("2026-11-25")).unwrap(), Overview { current: Settings::default(), pending: None });
    }

    // --- change ---

    #[test]
    fn a_change_is_pending_until_next_monday_and_then_applies() {
        let (_dir, store) = open_temp();
        let saved = change(&store, date("2026-11-25"), "8:00", "32:00").unwrap();
        assert_eq!(saved, Overview { current: Settings::default(), pending: pending("2026-11-30", 480, 1920) });
        assert_eq!(overview(&store, date("2026-11-29")).unwrap().current, Settings::default(), "die laufende Woche bleibt");
        assert_eq!(overview(&store, date("2026-11-30")).unwrap(), Overview { current: settings(480, 1920), pending: None });
    }

    #[test]
    fn a_change_saved_on_a_monday_still_waits_for_the_next_monday() {
        let (_dir, store) = open_temp();
        let saved = change(&store, date("2026-11-23"), "6:00", "30:00").unwrap();
        assert_eq!(saved.pending, pending("2026-11-30", 360, 1800));
    }

    #[test]
    fn a_second_change_in_the_same_week_replaces_the_first() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-11-24"), "8:00", "32:00").unwrap();
        let saved = change(&store, date("2026-11-26"), "7:00", "35:00").unwrap();
        assert_eq!(saved.pending, pending("2026-11-30", 420, 2100));
        assert_eq!(store.settings_history().unwrap().len(), 2);
    }

    #[test]
    fn changing_back_to_the_current_values_cancels_the_pending_change() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-11-24"), "8:00", "32:00").unwrap();
        let saved = change(&store, date("2026-11-26"), "8:00", "40:00").unwrap();
        assert_eq!(saved, Overview { current: Settings::default(), pending: None });
        assert_eq!(store.settings_history().unwrap(), vec![(START.to_string(), Settings::default())]);
    }

    #[test]
    fn a_change_that_already_applies_is_never_altered() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-11-25"), "8:00", "32:00").unwrap();
        change(&store, date("2026-12-02"), "6:00", "30:00").unwrap();
        change(&store, date("2026-12-03"), "8:00", "32:00").unwrap();
        let history = History::load(&store).unwrap();
        assert_eq!(history.at("2026-11-30"), settings(480, 1920));
        assert_eq!(history.at("2026-12-07"), settings(480, 1920), "die zurückgenommene Änderung gilt nie");
        assert_eq!(store.settings_history().unwrap().len(), 2);
    }

    #[test]
    fn several_changes_over_the_year_boundary_each_apply_from_their_monday() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-12-23"), "8:00", "32:00").unwrap();
        change(&store, date("2026-12-31"), "6:00", "30:00").unwrap();
        let history = History::load(&store).unwrap();
        assert_eq!(history.at("2026-12-27"), Settings::default());
        assert_eq!(history.at("2026-12-28"), settings(480, 1920));
        assert_eq!(history.at("2027-01-03"), settings(480, 1920));
        assert_eq!(history.at("2027-01-04"), settings(360, 1800));
    }

    #[test]
    fn a_change_keeps_the_subdivision_valid_from_next_week() {
        let (_dir, store) = open_temp();
        store.save_settings("2026-11-30", &Settings { subdivision: "DE-BY".into(), ..Settings::default() }).unwrap();
        let saved = change(&store, date("2026-11-25"), "8:00", "32:00").unwrap();
        assert_eq!(saved.pending.unwrap().settings.subdivision, "DE-BY");
    }

    #[test]
    fn a_clock_set_back_cannot_replace_or_cancel_settings_that_already_apply() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-11-25"), "8:00", "32:00").unwrap();
        change(&store, date("2026-12-02"), "6:00", "30:00").unwrap();
        let before = store.settings_history().unwrap();
        for (daily, weekly) in [("7:00", "35:00"), ("8:00", "40:00")] {
            let error = change(&store, date("2026-11-25"), daily, weekly).err().unwrap();
            assert!(matches!(error, SettingsError::ClockBehind), "{daily} / {weekly}");
            assert_eq!(
                error.to_string(),
                "Es gibt schon Einstellungen für eine spätere Woche, die Systemuhr geht offenbar falsch. Bitte Datum und Uhrzeit prüfen."
            );
        }
        assert_eq!(store.settings_history().unwrap(), before);
    }

    #[test]
    fn with_several_later_entries_the_earliest_is_shown_as_pending() {
        let (_dir, store) = open_temp();
        store.save_settings("2026-12-07", &settings(360, 1800)).unwrap();
        store.save_settings("2026-11-30", &settings(480, 1920)).unwrap();
        assert_eq!(overview(&store, date("2026-11-25")).unwrap().pending, pending("2026-11-30", 480, 1920));
    }

    #[test]
    fn a_pending_change_across_the_month_and_year_boundary_is_replaced_and_then_applies() {
        let (_dir, store) = open_temp();
        change(&store, date("2026-12-29"), "8:00", "32:00").unwrap();
        let saved = change(&store, date("2027-01-02"), "7:00", "35:00").unwrap();
        assert_eq!(saved.pending, pending("2027-01-04", 420, 2100));
        assert_eq!(overview(&store, date("2027-01-04")).unwrap(), Overview { current: settings(420, 2100), pending: None });
    }

    #[test]
    fn a_history_without_the_start_entry_is_an_error_instead_of_silent_defaults() {
        let (_dir, store) = open_temp();
        store.save_settings("2026-11-30", &settings(360, 1800)).unwrap();
        store.delete_settings(START).unwrap();
        let error = History::load(&store).err().unwrap();
        assert!(matches!(error, StoreError::MissingSettings));
        assert_eq!(
            error.to_string(),
            "Die gespeicherten Einstellungen sind unvollständig, deshalb kann nicht gerechnet werden. Bitte die Datenbank prüfen."
        );
        assert!(matches!(crate::target::week_summary(&store, "2026-12-02"), Err(StoreError::MissingSettings)));
        assert!(matches!(change(&store, date("2026-11-25"), "8:00", "40:00"), Err(SettingsError::Store(StoreError::MissingSettings))));
    }

    #[test]
    fn durations_are_read_as_hours_and_minutes() {
        let (_dir, store) = open_temp();
        for (daily, weekly, expected) in [
            ("8:00", "40:00", (480, 2400)),
            ("7:30", "37:30", (450, 2250)),
            (" 8 ", "40", (480, 2400)),
            ("08:15", "168:00", (495, 10080)),
            ("0:01", "0:01", (1, 1)),
            ("24:00", "100:00", (1440, 6000)),
        ] {
            let current = change(&store, date("2026-11-25"), daily, weekly).unwrap();
            let values = current.pending.map(|p| (p.settings.daily_minutes, p.settings.weekly_minutes)).unwrap_or((480, 2400));
            assert_eq!(values, expected, "{daily} / {weekly}");
        }
    }

    #[test]
    fn malformed_durations_are_rejected_in_german_and_store_nothing() {
        let (_dir, store) = open_temp();
        for daily in ["", "8h", "8,5", "8:5", "8:60", "-1:00", "1000:00", "8:00:00", "acht"] {
            let error = change(&store, date("2026-11-25"), daily, "40:00").err().unwrap();
            assert!(matches!(error, SettingsError::DailyFormat), "{daily}");
            assert_eq!(error.to_string(), "Bitte das Tagessoll als Stunden und Minuten eingeben, z. B. 8:00.");
        }
        let error = change(&store, date("2026-11-25"), "8:00", "vierzig").err().unwrap();
        assert_eq!(error.to_string(), "Bitte das Wochen-Soll als Stunden und Minuten eingeben, z. B. 40:00.");
        assert_eq!(store.settings_history().unwrap().len(), 1);
    }

    #[test]
    fn durations_outside_the_allowed_range_are_rejected_in_german() {
        let (_dir, store) = open_temp();
        for (daily, weekly, message) in [
            ("0:00", "40:00", "Das Tagessoll muss zwischen 0:01 und 24:00 h liegen."),
            ("24:01", "40:00", "Das Tagessoll muss zwischen 0:01 und 24:00 h liegen."),
            ("8:00", "0:00", "Das Wochen-Soll muss zwischen 0:01 und 168:00 h liegen."),
            ("8:00", "168:01", "Das Wochen-Soll muss zwischen 0:01 und 168:00 h liegen."),
        ] {
            assert_eq!(change(&store, date("2026-11-25"), daily, weekly).err().unwrap().to_string(), message);
        }
        assert_eq!(store.settings_history().unwrap().len(), 1);
    }
}
