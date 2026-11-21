//! Ist, Soll und Saldo einer Woche; das Soll abgeschlossener Wochen wird eingefroren.

use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;

use crate::settings::History;
use crate::store::{Absence, Entry, Holiday, Kind, Store, StoreError};
use crate::week::{format_date, monday_of, week_of};

/// Ist, Soll und Saldo einer Woche in Minuten. `target_incomplete`: Für die Woche wurden keine Feiertage
/// geladen, das Soll ist deshalb nicht um Feiertage gesenkt.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekSummary {
    pub actual_minutes: u32,
    pub target_minutes: u32,
    pub balance_minutes: i32,
    pub target_incomplete: bool,
}

/// Soll einer Woche mit den gegebenen Feiertagen: das Wochen-Soll, je Feiertag Montag bis Freitag ein Tagessoll
/// weniger, mindestens 0. So wirken ein Feiertag und ein Urlaubstag gleich auf den Saldo.
pub fn target_for(daily_minutes: u32, weekly_minutes: u32, holidays: &[Holiday]) -> u32 {
    let weekday_holidays = holidays
        .iter()
        .filter(|holiday| parse(&holiday.date).is_some_and(|day| day.weekday().number_from_monday() <= 5))
        .count() as u32;
    weekly_minutes.saturating_sub(daily_minutes.saturating_mul(weekday_holidays))
}

fn parse(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// Angerechnete Minuten eines Krank- oder Urlaubstags: das Tagessoll Montag bis Freitag. An einem Feiertag gilt der
/// Feiertag (Soll schon gesenkt) und am Wochenende gibt es kein Soll, dort zählt die Markierung 0.
pub fn absence_minutes(daily_minutes: u32, date: &str, holidays: &[Holiday]) -> u32 {
    let weekday = parse(date).is_some_and(|day| day.weekday().number_from_monday() <= 5);
    let holiday = holidays.iter().any(|holiday| holiday.date == date);
    if weekday && !holiday {
        daily_minutes
    } else {
        0
    }
}

/// Einträge von `from` bis `to` mit den Minuten, die ins Ist zählen. Krank/Urlaub wird bei jedem Aufruf mit den dann
/// gespeicherten Feiertagen und dem Tagessoll seiner Woche angerechnet, damit im Voraus eingetragener Urlaub nach dem
/// Laden der Feiertage stimmt und eine Änderung des Tagessolls vergangene Wochen nicht berührt.
pub fn day_entries(store: &Store, from: &str, to: &str) -> Result<Vec<Entry>, StoreError> {
    let mut entries = store.between(from, to)?;
    let holidays = store.holidays_between(from, to)?;
    let history = History::load(store)?;
    for entry in entries.iter_mut().filter(|entry| entry.kind != Kind::Work) {
        entry.minutes = absence_minutes(history.at(&entry.date).daily_minutes, &entry.date, &holidays);
    }
    Ok(entries)
}

/// Markiert `date` als krank oder Urlaub (ersetzt einen Eintrag) und gibt die angerechneten Minuten zurück.
pub fn mark_absence(store: &Store, date: &str, absence: Absence) -> Result<u32, StoreError> {
    store.save_absence(date, absence)?;
    let daily_minutes = History::load(store)?.at(date).daily_minutes;
    Ok(absence_minutes(daily_minutes, date, &store.holidays_between(date, date)?))
}

/// Ist, Soll und Saldo der Woche, in der `date` liegt. Ein eingefrorenes Soll hat Vorrang vor der Berechnung,
/// sonst gelten Tages- und Wochen-Soll dieser Woche.
pub fn week_summary(store: &Store, date: &str) -> Result<WeekSummary, StoreError> {
    let (from, to) = week_of(date)?;
    let actual_minutes = day_entries(store, &from, &to)?.iter().map(|entry| entry.minutes).sum();
    let settings = History::load(store)?.at(&from);
    let (target_minutes, target_incomplete) = match store.week_target(&from)? {
        Some(frozen) => (frozen, false),
        None => {
            let week = store.week_holidays(&from, &to)?;
            if week.missing {
                (settings.weekly_minutes, true)
            } else {
                (target_for(settings.daily_minutes, settings.weekly_minutes, &week.holidays), false)
            }
        }
    };
    let balance_minutes = (i64::from(actual_minutes) - i64::from(target_minutes)) as i32;
    Ok(WeekSummary { actual_minutes, target_minutes, balance_minutes, target_incomplete })
}

/// Friert das Soll aller abgeschlossenen Wochen ein, für die Feiertage geladen wurden und noch kein Soll
/// gespeichert ist, jeweils mit den Einstellungen dieser Woche. Ein gespeichertes Soll wird nie überschrieben.
pub fn freeze_past_weeks(store: &Store, today: NaiveDate) -> Result<(), StoreError> {
    let current_monday = monday_of(today);
    let history = History::load(store)?;
    for (from, to) in store.loaded_ranges()? {
        let (Some(from), Some(to)) = (parse(&from), parse(&to)) else { continue };
        let mut monday = if monday_of(from) == from { from } else { monday_of(from) + Days::new(7) };
        while monday < current_monday && monday + Days::new(6) <= to {
            let (start, end) = (format_date(monday), format_date(monday + Days::new(6)));
            let holidays = store.holidays_between(&start, &end)?;
            let settings = history.at(&start);
            store.freeze_week_target(&start, target_for(settings.daily_minutes, settings.weekly_minutes, &holidays))?;
            monday = monday + Days::new(7);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::settings::Settings;

    /// Startwerte der Einstellungen: 8 h am Tag, 40 h in der Woche.
    const DAY: u32 = 480;
    const WEEK: u32 = 2400;

    fn settings(daily_minutes: u32, weekly_minutes: u32) -> Settings {
        Settings { daily_minutes, weekly_minutes, subdivision: "DE-NW".into() }
    }

    fn holiday(date: &str) -> Holiday {
        Holiday { date: date.into(), name: "Feiertag".into() }
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn open_temp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, store)
    }

    /// Trägt `days` Tage mit je `minutes` Arbeitszeit ab Montag 2026-11-16 ein (Stunden ganz, Pause 0).
    fn work(store: &Store, days: u32, hours: u32) {
        for offset in 0..days {
            let date = format!("2026-11-{}", 16 + offset);
            store.save(&date, "06:00", &format!("{:02}:00", 6 + hours), 0.0).unwrap();
        }
    }

    // --- target_for ---

    #[test]
    fn target_without_holidays_is_five_daily_targets() {
        assert_eq!(target_for(DAY, WEEK, &[]), 2400);
    }

    #[test]
    fn each_weekday_holiday_lowers_the_target_by_one_daily_target() {
        assert_eq!(target_for(DAY, WEEK, &[holiday("2026-12-25")]), 1920);
        assert_eq!(target_for(DAY, WEEK, &[holiday("2026-12-24"), holiday("2026-12-25")]), 1440);
    }

    #[test]
    fn holidays_on_monday_and_friday_count_like_the_days_between() {
        assert_eq!(target_for(DAY, WEEK, &[holiday("2026-12-21"), holiday("2026-12-25")]), 1440);
    }

    #[test]
    fn holiday_on_a_weekend_does_not_lower_the_target() {
        assert_eq!(target_for(DAY, WEEK, &[holiday("2026-12-26"), holiday("2026-12-27")]), 2400);
    }

    #[test]
    fn target_uses_the_given_daily_target() {
        assert_eq!(target_for(360, 1800, &[]), 1800);
        assert_eq!(target_for(360, 1800, &[holiday("2026-12-25")]), 1440);
    }

    #[test]
    fn a_separate_weekly_target_is_lowered_by_one_daily_target_per_holiday() {
        assert_eq!(target_for(480, 1920, &[]), 1920);
        assert_eq!(target_for(480, 1920, &[holiday("2026-12-25")]), 1440);
        assert_eq!(target_for(480, 1920, &[holiday("2026-12-26")]), 1920, "Samstag");
    }

    #[test]
    fn the_target_never_drops_below_zero() {
        let three = [holiday("2026-12-23"), holiday("2026-12-24"), holiday("2026-12-25")];
        assert_eq!(target_for(480, 960, &three), 0);
    }

    // --- week_summary ---

    #[test]
    fn four_days_of_eight_hours_in_a_week_with_one_holiday_balance_to_zero() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-20")]).unwrap();
        work(&store, 4, 8);
        let summary = week_summary(&store, "2026-11-18").unwrap();
        assert_eq!(
            summary,
            WeekSummary { actual_minutes: 1920, target_minutes: 1920, balance_minutes: 0, target_incomplete: false }
        );
    }

    #[test]
    fn overtime_gives_a_positive_balance() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        for (offset, hours) in [9, 9, 9, 9, 9].iter().enumerate() {
            store.save(&format!("2026-11-{}", 16 + offset), "06:00", &format!("{:02}:00", 6 + hours), 0.0).unwrap();
        }
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes, summary.balance_minutes), (2700, 2400, 300));
    }

    #[test]
    fn too_few_hours_give_a_negative_balance() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, 5, 7);
        let summary = week_summary(&store, "2026-11-22").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes, summary.balance_minutes), (2100, 2400, -300));
    }

    #[test]
    fn week_without_entries_has_the_full_target_as_negative_balance() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.balance_minutes), (0, -2400));
    }

    #[test]
    fn week_without_loaded_holidays_keeps_the_full_target_and_is_marked_incomplete() {
        let (_dir, store) = open_temp();
        let summary = week_summary(&store, "2027-03-02").unwrap();
        assert_eq!(
            summary,
            WeekSummary { actual_minutes: 0, target_minutes: 2400, balance_minutes: -2400, target_incomplete: true }
        );
    }

    #[test]
    fn loaded_week_without_holidays_is_not_marked_incomplete() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        assert!(!week_summary(&store, "2026-11-16").unwrap().target_incomplete);
    }

    #[test]
    fn summary_rejects_a_malformed_date() {
        let (_dir, store) = open_temp();
        assert!(matches!(week_summary(&store, "morgen"), Err(StoreError::InvalidDate)));
    }

    #[test]
    fn frozen_target_wins_over_the_current_holidays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!(summary.target_minutes, 1920);
        assert!(!summary.target_incomplete);
    }

    #[test]
    fn actual_is_zero_without_entries() {
        let (_dir, store) = open_temp();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap().actual_minutes, 0);
    }

    #[test]
    fn actual_sums_the_stored_minutes_of_the_week_only() {
        let (_dir, store) = open_temp();
        store.save("2026-11-15", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.save("2026-11-18", "09:00", "12:15", 0.0).unwrap();
        store.save("2026-11-22", "10:00", "11:00", 0.0).unwrap();
        store.save("2026-11-23", "08:00", "12:00", 0.0).unwrap();
        assert_eq!(week_summary(&store, "2026-11-18").unwrap().actual_minutes, 480 + 195 + 60);
    }

    #[test]
    fn actual_follows_replacing_and_deleting_entries() {
        let (_dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        store.save("2026-11-17", "08:00", "12:00", 0.0).unwrap();
        store.save("2026-11-16", "08:00", "10:00", 0.0).unwrap();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap().actual_minutes, 120 + 240);
        store.delete("2026-11-17").unwrap();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap().actual_minutes, 120);
    }

    #[test]
    fn actual_across_the_year_boundary() {
        let (_dir, store) = open_temp();
        for date in ["2026-12-27", "2026-12-28", "2027-01-01", "2027-01-03", "2027-01-04"] {
            store.save(date, "08:00", "12:00", 0.0).unwrap();
        }
        assert_eq!(week_summary(&store, "2027-01-01").unwrap().actual_minutes, 3 * 240);
    }

    #[test]
    fn stored_work_minutes_are_not_recalculated_for_the_actual() {
        let (dir, store) = open_temp();
        store.save("2026-11-16", "08:00", "16:30", 30.0).unwrap();
        rusqlite::Connection::open(dir.path().join("overtime.db"))
            .unwrap()
            .execute("UPDATE entries SET minutes = 123 WHERE date = '2026-11-16'", [])
            .unwrap();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap().actual_minutes, 123);
    }

    // --- Krank/Urlaub ---

    #[test]
    fn a_marked_weekday_counts_the_daily_target() {
        assert_eq!(absence_minutes(DAY, "2026-11-16", &[]), 480);
        assert_eq!(absence_minutes(DAY, "2026-11-20", &[]), 480);
        assert_eq!(absence_minutes(360, "2026-11-18", &[]), 360);
    }

    #[test]
    fn a_marked_holiday_counts_nothing() {
        assert_eq!(absence_minutes(DAY, "2026-12-25", &[holiday("2026-12-25")]), 0);
        assert_eq!(absence_minutes(DAY, "2026-12-24", &[holiday("2026-12-25")]), 480, "nur der Feiertag selbst");
    }

    #[test]
    fn a_marked_weekend_day_counts_nothing() {
        assert_eq!(absence_minutes(DAY, "2026-11-21", &[]), 0);
        assert_eq!(absence_minutes(DAY, "2026-11-22", &[]), 0);
    }

    #[test]
    fn vacation_and_sick_day_with_three_worked_days_balance_to_zero() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        store.save_absence("2026-11-16", Absence::Vacation).unwrap();
        store.save_absence("2026-11-17", Absence::Sick).unwrap();
        for date in ["2026-11-18", "2026-11-19", "2026-11-20"] {
            store.save(date, "08:00", "16:30", 30.0).unwrap();
        }
        assert_eq!(
            week_summary(&store, "2026-11-16").unwrap(),
            WeekSummary { actual_minutes: 2400, target_minutes: 2400, balance_minutes: 0, target_incomplete: false }
        );
    }

    #[test]
    fn removing_a_marking_takes_its_hours_out_of_the_actual() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        store.save_absence("2026-11-16", Absence::Vacation).unwrap();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap().actual_minutes, 480);
        store.delete("2026-11-16").unwrap();
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes, summary.balance_minutes), (0, 2400, -2400));
    }

    #[test]
    fn a_marked_holiday_changes_neither_actual_nor_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        let before = week_summary(&store, "2026-12-21").unwrap();
        store.save_absence("2026-12-25", Absence::Vacation).unwrap();
        assert_eq!(week_summary(&store, "2026-12-21").unwrap(), before);
        assert_eq!(before.target_minutes, 1920);
    }

    #[test]
    fn a_marked_weekend_changes_neither_actual_nor_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        store.save_absence("2026-11-21", Absence::Sick).unwrap();
        store.save_absence("2026-11-22", Absence::Vacation).unwrap();
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes), (0, 2400));
    }

    #[test]
    fn vacation_entered_before_its_holidays_were_loaded_follows_the_loaded_holidays() {
        let (_dir, mut store) = open_temp();
        for date in ["2027-03-29", "2027-03-30", "2027-03-31", "2027-04-01", "2027-04-02"] {
            store.save_absence(date, Absence::Vacation).unwrap();
        }
        let before = week_summary(&store, "2027-03-29").unwrap();
        assert_eq!(
            before,
            WeekSummary { actual_minutes: 2400, target_minutes: 2400, balance_minutes: 0, target_incomplete: true }
        );
        store.save_holidays("2027-03-29", "2027-04-04", &[holiday("2027-03-29"), holiday("2027-04-02")]).unwrap();
        assert_eq!(
            week_summary(&store, "2027-03-29").unwrap(),
            WeekSummary { actual_minutes: 1440, target_minutes: 1440, balance_minutes: 0, target_incomplete: false }
        );
    }

    #[test]
    fn marking_a_day_of_a_frozen_week_keeps_its_frozen_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        store.save_absence("2026-11-16", Absence::Sick).unwrap();
        let summary = week_summary(&store, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes), (480, 1920));
    }

    #[test]
    fn day_entries_credit_marked_days_and_keep_worked_days() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        store.save("2026-12-21", "08:00", "12:00", 0.0).unwrap();
        store.save_absence("2026-12-22", Absence::Sick).unwrap();
        store.save_absence("2026-12-25", Absence::Vacation).unwrap();
        store.save_absence("2026-12-26", Absence::Vacation).unwrap();
        let credited: Vec<(String, Kind, u32)> = day_entries(&store, "2026-12-21", "2026-12-27")
            .unwrap()
            .into_iter()
            .map(|entry| (entry.date, entry.kind, entry.minutes))
            .collect();
        assert_eq!(
            credited,
            vec![
                ("2026-12-21".to_string(), Kind::Work, 240),
                ("2026-12-22".to_string(), Kind::Sick, 480),
                ("2026-12-25".to_string(), Kind::Vacation, 0),
                ("2026-12-26".to_string(), Kind::Vacation, 0),
            ]
        );
    }

    #[test]
    fn marking_returns_the_credited_minutes() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        assert_eq!(mark_absence(&store, "2026-12-24", Absence::Sick).unwrap(), 480);
        assert_eq!(mark_absence(&store, "2026-12-25", Absence::Vacation).unwrap(), 0);
        assert_eq!(mark_absence(&store, "2026-12-26", Absence::Vacation).unwrap(), 0);
        assert!(matches!(mark_absence(&store, "morgen", Absence::Sick), Err(StoreError::InvalidDate)));
    }

    // --- Einstellungen pro Woche ---

    #[test]
    fn each_week_uses_the_targets_valid_in_it() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-16", "2026-12-06", &[holiday("2026-12-01")]).unwrap();
        store.save_settings("2026-11-30", &settings(360, 1800)).unwrap();
        assert_eq!(week_summary(&store, "2026-11-18").unwrap().target_minutes, 2400);
        assert_eq!(week_summary(&store, "2026-11-25").unwrap().target_minutes, 2400);
        assert_eq!(week_summary(&store, "2026-12-02").unwrap().target_minutes, 1800 - 360, "Feiertag am 01.12.");
    }

    #[test]
    fn a_week_without_loaded_holidays_shows_its_weekly_target_unlowered() {
        let (_dir, store) = open_temp();
        store.save_settings("2027-03-01", &settings(480, 1920)).unwrap();
        let summary = week_summary(&store, "2027-03-02").unwrap();
        assert_eq!((summary.target_minutes, summary.target_incomplete), (1920, true));
    }

    #[test]
    fn a_sick_day_counts_the_daily_target_of_its_own_week() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-16", "2026-12-06", &[]).unwrap();
        store.save_settings("2026-11-30", &settings(360, 1800)).unwrap();
        assert_eq!(mark_absence(&store, "2026-11-27", Absence::Sick).unwrap(), 480);
        assert_eq!(mark_absence(&store, "2026-11-30", Absence::Sick).unwrap(), 360);
        let minutes: Vec<u32> = day_entries(&store, "2026-11-23", "2026-12-06").unwrap().iter().map(|e| e.minutes).collect();
        assert_eq!(minutes, [480, 360]);
        assert_eq!(week_summary(&store, "2026-11-23").unwrap().actual_minutes, 480);
        assert_eq!(week_summary(&store, "2026-11-30").unwrap().actual_minutes, 360);
    }

    #[test]
    fn a_later_change_leaves_the_balance_of_past_weeks_unchanged() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-12-13", &[]).unwrap();
        store.save_absence("2026-11-17", Absence::Vacation).unwrap();
        work(&store, 3, 8);
        let before = week_summary(&store, "2026-11-16").unwrap();
        freeze_past_weeks(&store, date("2026-11-25")).unwrap();
        crate::settings::change(&store, date("2026-11-25"), "6:00", "30:00", "DE-NW").unwrap();
        assert_eq!(week_summary(&store, "2026-11-16").unwrap(), before, "abgeschlossene Woche");
        assert_eq!(week_summary(&store, "2026-11-23").unwrap().target_minutes, 2400, "laufende Woche");
        assert_eq!(week_summary(&store, "2026-11-30").unwrap().target_minutes, 1800, "nächste Woche");
    }

    #[test]
    fn weeks_on_both_sides_of_a_subdivision_change_are_frozen_with_their_own_holidays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        store.save_holidays("2026-11-30", "2027-01-24", &[holiday("2026-12-08")]).unwrap();
        assert_eq!(store.loaded_ranges().unwrap().len(), 2, "aneinandergrenzende Abschnitte bleiben getrennt");
        freeze_past_weeks(&store, date("2026-12-14")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
        assert_eq!(frozen(&store, "2026-11-23"), Some(2400), "letzte Woche vor dem Wechsel");
        assert_eq!(frozen(&store, "2026-11-30"), Some(2400), "erste Woche nach dem Wechsel");
        assert_eq!(frozen(&store, "2026-12-07"), Some(1920));
        assert_eq!(frozen(&store, "2026-12-14"), None, "die Woche von heute");
    }

    #[test]
    fn freezing_uses_the_targets_of_each_week_not_todays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-16", "2026-12-06", &[holiday("2026-12-01")]).unwrap();
        store.save_settings("2026-11-23", &settings(480, 1920)).unwrap();
        store.save_settings("2026-11-30", &settings(360, 1800)).unwrap();
        freeze_past_weeks(&store, date("2026-12-07")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-23"), Some(1920));
        assert_eq!(frozen(&store, "2026-11-30"), Some(1440));
    }

    // --- freeze_past_weeks ---

    fn frozen(store: &Store, week_start: &str) -> Option<u32> {
        store.week_target(week_start).unwrap()
    }

    #[test]
    fn past_weeks_with_loaded_holidays_are_frozen_with_their_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-18")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None, "die Woche von heute ist noch offen");
        assert_eq!(frozen(&store, "2026-11-23"), None, "künftige Wochen sind offen");
    }

    #[test]
    fn a_week_ending_today_is_still_the_current_week() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        freeze_past_weeks(&store, date("2026-11-22")).unwrap();
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }

    #[test]
    fn the_week_after_a_finished_week_freezes_it_on_monday() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-23")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn a_frozen_target_is_never_overwritten() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-17"), holiday("2026-11-18")]).unwrap();
        store.save_settings("2026-11-16", &settings(360, 1800)).unwrap();
        freeze_past_weeks(&store, date("2026-12-07")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn weeks_without_loaded_holidays_are_not_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        freeze_past_weeks(&store, date("2027-03-01")).unwrap();
        assert_eq!(frozen(&store, "2026-10-26"), None, "nie geladen");
        assert_eq!(frozen(&store, "2026-11-30"), None, "nie geladen");
    }

    #[test]
    fn a_week_loaded_later_is_frozen_at_the_next_freeze() {
        let (_dir, mut store) = open_temp();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), None);
        store.save_holidays("2026-11-16", "2026-11-22", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn a_week_only_partly_inside_a_loaded_range_is_not_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-04", "2026-11-18", &[]).unwrap();
        freeze_past_weeks(&store, date("2027-03-01")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), None);
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }

    #[test]
    fn separately_loaded_weeks_are_each_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-08", &[]).unwrap();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        freeze_past_weeks(&store, date("2027-01-04")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), Some(2400));
        assert_eq!(frozen(&store, "2026-12-21"), Some(1920));
        assert_eq!(frozen(&store, "2026-12-14"), None);
    }

    #[test]
    fn freezing_across_the_year_boundary() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2027-01-10", &[holiday("2026-12-25"), holiday("2027-01-01")]).unwrap();
        freeze_past_weeks(&store, date("2027-01-11")).unwrap();
        assert_eq!(frozen(&store, "2026-12-21"), Some(1920));
        assert_eq!(frozen(&store, "2026-12-28"), Some(1920));
        assert_eq!(frozen(&store, "2027-01-04"), Some(2400));
    }

    #[test]
    fn freezing_without_any_loaded_range_does_nothing() {
        let (_dir, store) = open_temp();
        freeze_past_weeks(&store, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }
}
