//! Soll und Saldo einer Woche; das Soll abgeschlossener Wochen wird eingefroren.

use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;

use crate::store::{Holiday, Store, StoreError};
use crate::week::{format_date, monday_of, week_of};

/// Tagessoll Montag bis Freitag in Minuten (8 h). Wird als Parameter durchgereicht, damit die Einstellungen
/// aus Plan B es später ersetzen können.
pub const DAILY_TARGET_MINUTES: u32 = 480;

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

/// Soll einer Woche mit den gegebenen Feiertagen: fünf Tagessolle, je Feiertag Montag bis Freitag eines weniger.
pub fn target_for(daily_minutes: u32, holidays: &[Holiday]) -> u32 {
    let weekday_holidays = holidays
        .iter()
        .filter(|holiday| parse(&holiday.date).is_some_and(|day| day.weekday().number_from_monday() <= 5))
        .count() as u32;
    daily_minutes * 5u32.saturating_sub(weekday_holidays)
}

fn parse(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// Ist, Soll und Saldo der Woche, in der `date` liegt. Ein eingefrorenes Soll hat Vorrang vor der Berechnung.
pub fn week_summary(store: &Store, daily_minutes: u32, date: &str) -> Result<WeekSummary, StoreError> {
    let (from, to) = week_of(date)?;
    let actual_minutes = store.week_total(&from, &to)?;
    let (target_minutes, target_incomplete) = match store.week_target(&from)? {
        Some(frozen) => (frozen, false),
        None => {
            let week = store.week_holidays(&from, &to)?;
            if week.missing {
                (target_for(daily_minutes, &[]), true)
            } else {
                (target_for(daily_minutes, &week.holidays), false)
            }
        }
    };
    let balance_minutes = (i64::from(actual_minutes) - i64::from(target_minutes)) as i32;
    Ok(WeekSummary { actual_minutes, target_minutes, balance_minutes, target_incomplete })
}

/// Friert das Soll aller abgeschlossenen Wochen ein, für die Feiertage geladen wurden und noch kein Soll
/// gespeichert ist. Ein gespeichertes Soll wird nie überschrieben.
pub fn freeze_past_weeks(store: &Store, daily_minutes: u32, today: NaiveDate) -> Result<(), StoreError> {
    let current_monday = monday_of(today);
    for (from, to) in store.loaded_ranges()? {
        let (Some(from), Some(to)) = (parse(&from), parse(&to)) else { continue };
        let mut monday = if monday_of(from) == from { from } else { monday_of(from) + Days::new(7) };
        while monday < current_monday && monday + Days::new(6) <= to {
            let (start, end) = (format_date(monday), format_date(monday + Days::new(6)));
            let holidays = store.holidays_between(&start, &end)?;
            store.freeze_week_target(&start, target_for(daily_minutes, &holidays))?;
            monday = monday + Days::new(7);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u32 = DAILY_TARGET_MINUTES;

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
        assert_eq!(target_for(DAY, &[]), 2400);
    }

    #[test]
    fn each_weekday_holiday_lowers_the_target_by_one_daily_target() {
        assert_eq!(target_for(DAY, &[holiday("2026-12-25")]), 1920);
        assert_eq!(target_for(DAY, &[holiday("2026-12-24"), holiday("2026-12-25")]), 1440);
    }

    #[test]
    fn holidays_on_monday_and_friday_count_like_the_days_between() {
        assert_eq!(target_for(DAY, &[holiday("2026-12-21"), holiday("2026-12-25")]), 1440);
    }

    #[test]
    fn holiday_on_a_weekend_does_not_lower_the_target() {
        assert_eq!(target_for(DAY, &[holiday("2026-12-26"), holiday("2026-12-27")]), 2400);
    }

    #[test]
    fn target_uses_the_given_daily_target() {
        assert_eq!(target_for(360, &[]), 1800);
        assert_eq!(target_for(360, &[holiday("2026-12-25")]), 1440);
    }

    // --- week_summary ---

    #[test]
    fn four_days_of_eight_hours_in_a_week_with_one_holiday_balance_to_zero() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-20")]).unwrap();
        work(&store, 4, 8);
        let summary = week_summary(&store, DAY, "2026-11-18").unwrap();
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
        let summary = week_summary(&store, DAY, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes, summary.balance_minutes), (2700, 2400, 300));
    }

    #[test]
    fn too_few_hours_give_a_negative_balance() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, 5, 7);
        let summary = week_summary(&store, DAY, "2026-11-22").unwrap();
        assert_eq!((summary.actual_minutes, summary.target_minutes, summary.balance_minutes), (2100, 2400, -300));
    }

    #[test]
    fn week_without_entries_has_the_full_target_as_negative_balance() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        let summary = week_summary(&store, DAY, "2026-11-16").unwrap();
        assert_eq!((summary.actual_minutes, summary.balance_minutes), (0, -2400));
    }

    #[test]
    fn week_without_loaded_holidays_keeps_the_full_target_and_is_marked_incomplete() {
        let (_dir, store) = open_temp();
        let summary = week_summary(&store, DAY, "2027-03-02").unwrap();
        assert_eq!(
            summary,
            WeekSummary { actual_minutes: 0, target_minutes: 2400, balance_minutes: -2400, target_incomplete: true }
        );
    }

    #[test]
    fn loaded_week_without_holidays_is_not_marked_incomplete() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        assert!(!week_summary(&store, DAY, "2026-11-16").unwrap().target_incomplete);
    }

    #[test]
    fn summary_rejects_a_malformed_date() {
        let (_dir, store) = open_temp();
        assert!(matches!(week_summary(&store, DAY, "morgen"), Err(StoreError::InvalidDate)));
    }

    #[test]
    fn frozen_target_wins_over_the_current_holidays() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-30")).unwrap();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        let summary = week_summary(&store, DAY, "2026-11-16").unwrap();
        assert_eq!(summary.target_minutes, 1920);
        assert!(!summary.target_incomplete);
    }

    // --- freeze_past_weeks ---

    fn frozen(store: &Store, week_start: &str) -> Option<u32> {
        store.week_target(week_start).unwrap()
    }

    #[test]
    fn past_weeks_with_loaded_holidays_are_frozen_with_their_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-18")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None, "die Woche von heute ist noch offen");
        assert_eq!(frozen(&store, "2026-11-23"), None, "künftige Wochen sind offen");
    }

    #[test]
    fn a_week_ending_today_is_still_the_current_week() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-22")).unwrap();
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }

    #[test]
    fn the_week_after_a_finished_week_freezes_it_on_monday() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-23")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn a_frozen_target_is_never_overwritten() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-30")).unwrap();
        store.save_holidays("2026-11-02", "2026-11-29", &[holiday("2026-11-17"), holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, 360, date("2026-12-07")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn weeks_without_loaded_holidays_are_not_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        freeze_past_weeks(&store, DAY, date("2027-03-01")).unwrap();
        assert_eq!(frozen(&store, "2026-10-26"), None, "nie geladen");
        assert_eq!(frozen(&store, "2026-11-30"), None, "nie geladen");
    }

    #[test]
    fn a_week_loaded_later_is_frozen_at_the_next_freeze() {
        let (_dir, mut store) = open_temp();
        freeze_past_weeks(&store, DAY, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), None);
        store.save_holidays("2026-11-16", "2026-11-22", &[holiday("2026-11-18")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), Some(1920));
    }

    #[test]
    fn a_week_only_partly_inside_a_loaded_range_is_not_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-04", "2026-11-18", &[]).unwrap();
        freeze_past_weeks(&store, DAY, date("2027-03-01")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), None);
        assert_eq!(frozen(&store, "2026-11-09"), Some(2400));
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }

    #[test]
    fn separately_loaded_weeks_are_each_frozen() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-08", &[]).unwrap();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2027-01-04")).unwrap();
        assert_eq!(frozen(&store, "2026-11-02"), Some(2400));
        assert_eq!(frozen(&store, "2026-12-21"), Some(1920));
        assert_eq!(frozen(&store, "2026-12-14"), None);
    }

    #[test]
    fn freezing_across_the_year_boundary() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2027-01-10", &[holiday("2026-12-25"), holiday("2027-01-01")]).unwrap();
        freeze_past_weeks(&store, DAY, date("2027-01-11")).unwrap();
        assert_eq!(frozen(&store, "2026-12-21"), Some(1920));
        assert_eq!(frozen(&store, "2026-12-28"), Some(1920));
        assert_eq!(frozen(&store, "2027-01-04"), Some(2400));
    }

    #[test]
    fn freezing_without_any_loaded_range_does_nothing() {
        let (_dir, store) = open_temp();
        freeze_past_weeks(&store, DAY, date("2026-11-30")).unwrap();
        assert_eq!(frozen(&store, "2026-11-16"), None);
    }
}
