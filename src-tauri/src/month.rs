//! Monatssumme und Durchschnitt pro Arbeitstag eines Kalendermonats.

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Serialize;

use crate::store::{check_date, Store, StoreError};
use crate::target::day_entries;
use crate::week::format_date;

/// Ist eines Kalendermonats in Minuten. Die Summe enthält alle Tage, der Durchschnitt nur die gezählten Arbeitstage
/// (Montag bis Freitag ohne Feiertage, mit Eintrag oder Krank/Urlaub-Markierung).
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthSummary {
    pub actual_minutes: u32,
    /// Auf ganze Minuten gerundet; `None`, wenn kein Arbeitstag gezählt wurde.
    pub average_minutes: Option<u32>,
    pub counted_days: u32,
    /// Ist am Wochenende und an Feiertagen: in der Summe, nicht im Durchschnitt.
    pub other_minutes: u32,
    /// Für mindestens einen gezählten Tag wurden keine Feiertage geladen; er könnte ein Feiertag sein.
    pub holidays_missing: bool,
}

/// Erster und letzter Tag des Monats, in dem `date` liegt, als `(von, bis)` im Format `YYYY-MM-DD`.
pub fn month_of(date: &str) -> Result<(String, String), StoreError> {
    check_date(date)?;
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| StoreError::InvalidDate)?;
    let first = day.with_day(1).ok_or(StoreError::InvalidDate)?;
    let last = first + Months::new(1) - Days::new(1);
    Ok((format_date(first), format_date(last)))
}

/// Summe und Durchschnitt des Monats, in dem `date` liegt. Wochen über die Monatsgrenze werden tageweise zugeordnet.
pub fn month_summary(store: &Store, daily_minutes: u32, date: &str) -> Result<MonthSummary, StoreError> {
    let (from, to) = month_of(date)?;
    let holidays = store.holidays_between(&from, &to)?;
    let (mut actual_minutes, mut counted_minutes, mut counted_days, mut holidays_missing) = (0, 0, 0, false);
    for entry in day_entries(store, daily_minutes, &from, &to)? {
        actual_minutes += entry.minutes;
        let weekday = NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d")
            .is_ok_and(|day| day.weekday().number_from_monday() <= 5);
        if weekday && !holidays.iter().any(|holiday| holiday.date == entry.date) {
            counted_minutes += entry.minutes;
            counted_days += 1;
            holidays_missing |= !store.is_loaded(&entry.date, &entry.date)?;
        }
    }
    let average_minutes = (counted_days > 0).then(|| (counted_minutes + counted_days / 2) / counted_days);
    Ok(MonthSummary {
        actual_minutes,
        average_minutes,
        counted_days,
        other_minutes: actual_minutes - counted_minutes,
        holidays_missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Absence, Holiday};
    use crate::target::DAILY_TARGET_MINUTES;

    const DAY: u32 = DAILY_TARGET_MINUTES;

    fn open_temp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, store)
    }

    fn holiday(date: &str) -> Holiday {
        Holiday { date: date.into(), name: "Feiertag".into() }
    }

    /// Trägt an `date` `hours` volle Stunden ohne Pause ein.
    fn work(store: &Store, date: &str, hours: u32) {
        store.save(date, "06:00", &format!("{:02}:00", 6 + hours), 0.0).unwrap();
    }

    fn summary(store: &Store, date: &str) -> MonthSummary {
        month_summary(store, DAY, date).unwrap()
    }

    // --- month_of ---

    #[test]
    fn month_of_returns_the_first_and_last_day_for_any_day_of_the_month() {
        for day in ["2026-11-01", "2026-11-17", "2026-11-30"] {
            assert_eq!(month_of(day).unwrap(), ("2026-11-01".to_string(), "2026-11-30".to_string()), "{day}");
        }
        assert_eq!(month_of("2026-12-31").unwrap(), ("2026-12-01".to_string(), "2026-12-31".to_string()));
    }

    #[test]
    fn february_has_29_days_in_a_leap_year_and_28_otherwise() {
        assert_eq!(month_of("2028-02-10").unwrap().1, "2028-02-29");
        assert_eq!(month_of("2027-02-10").unwrap().1, "2027-02-28");
        assert_eq!(month_of("2100-02-10").unwrap().1, "2100-02-28");
    }

    #[test]
    fn month_of_rejects_a_malformed_date() {
        for day in ["", "morgen", "2026-02-31", "2026-11"] {
            assert!(matches!(month_of(day), Err(StoreError::InvalidDate)), "{day}");
        }
    }

    // --- month_summary ---

    #[test]
    fn empty_month_has_no_hours_and_no_average() {
        let (_dir, store) = open_temp();
        assert_eq!(
            summary(&store, "2026-11-17"),
            MonthSummary { actual_minutes: 0, average_minutes: None, counted_days: 0, other_minutes: 0, holidays_missing: false }
        );
    }

    #[test]
    fn sum_and_average_over_the_worked_days_of_the_month() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, "2026-11-02", 8);
        work(&store, "2026-11-03", 9);
        work(&store, "2026-11-04", 7);
        assert_eq!(
            summary(&store, "2026-11-30"),
            MonthSummary { actual_minutes: 1440, average_minutes: Some(480), counted_days: 3, other_minutes: 0, holidays_missing: false }
        );
    }

    #[test]
    fn days_without_an_entry_do_not_lower_the_average() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, "2026-11-02", 8);
        work(&store, "2026-11-20", 10);
        let month = summary(&store, "2026-11-01");
        assert_eq!((month.average_minutes, month.counted_days), (Some(540), 2));
    }

    #[test]
    fn average_is_rounded_to_whole_minutes() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        store.save("2026-11-02", "08:00", "16:00", 0.0).unwrap();
        store.save("2026-11-03", "08:00", "16:01", 0.0).unwrap();
        store.save("2026-11-04", "08:00", "16:01", 0.0).unwrap();
        assert_eq!(summary(&store, "2026-11-02").average_minutes, Some(481), "1442 / 3 = 480,67");
        store.save("2026-11-04", "08:00", "16:00", 0.0).unwrap();
        assert_eq!(summary(&store, "2026-11-02").average_minutes, Some(480), "1441 / 3 = 480,33");
    }

    #[test]
    fn weekend_hours_count_in_the_sum_but_not_in_the_average() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, "2026-11-06", 8);
        work(&store, "2026-11-07", 4);
        work(&store, "2026-11-08", 2);
        assert_eq!(
            summary(&store, "2026-11-06"),
            MonthSummary { actual_minutes: 840, average_minutes: Some(480), counted_days: 1, other_minutes: 360, holidays_missing: false }
        );
    }

    #[test]
    fn hours_worked_on_a_holiday_count_in_the_sum_but_not_in_the_average() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-24"), holiday("2026-12-25")]).unwrap();
        work(&store, "2026-12-23", 8);
        work(&store, "2026-12-24", 3);
        let month = summary(&store, "2026-12-01");
        assert_eq!((month.actual_minutes, month.average_minutes, month.counted_days, month.other_minutes), (660, Some(480), 1, 180));
    }

    #[test]
    fn sick_and_vacation_days_count_with_the_daily_target() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-29", &[]).unwrap();
        work(&store, "2026-11-02", 10);
        store.save_absence("2026-11-03", Absence::Sick).unwrap();
        store.save_absence("2026-11-04", Absence::Vacation).unwrap();
        let month = summary(&store, "2026-11-02");
        assert_eq!((month.actual_minutes, month.average_minutes, month.counted_days), (1560, Some(520), 3));
    }

    #[test]
    fn a_vacation_day_on_a_holiday_or_weekend_is_not_counted() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-12-21", "2026-12-27", &[holiday("2026-12-25")]).unwrap();
        work(&store, "2026-12-21", 8);
        store.save_absence("2026-12-25", Absence::Vacation).unwrap();
        store.save_absence("2026-12-26", Absence::Vacation).unwrap();
        assert_eq!(
            summary(&store, "2026-12-01"),
            MonthSummary { actual_minutes: 480, average_minutes: Some(480), counted_days: 1, other_minutes: 0, holidays_missing: false }
        );
    }

    #[test]
    fn a_week_across_the_month_boundary_is_split_by_day() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-23", "2026-12-06", &[]).unwrap();
        work(&store, "2026-11-30", 8);
        work(&store, "2026-12-01", 6);
        store.save_absence("2026-12-02", Absence::Sick).unwrap();
        let november = summary(&store, "2026-11-15");
        let december = summary(&store, "2026-12-15");
        assert_eq!((november.actual_minutes, november.counted_days), (480, 1));
        assert_eq!((december.actual_minutes, december.average_minutes, december.counted_days), (840, Some(420), 2));
    }

    #[test]
    fn the_last_day_of_a_leap_february_belongs_to_february() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2028-02-28", "2028-03-05", &[]).unwrap();
        work(&store, "2028-02-28", 8);
        work(&store, "2028-02-29", 6);
        work(&store, "2028-03-01", 4);
        let february = summary(&store, "2028-02-01");
        assert_eq!((february.actual_minutes, february.counted_days), (840, 2));
        assert_eq!(summary(&store, "2028-03-31").actual_minutes, 240);
    }

    #[test]
    fn a_counted_day_without_loaded_holidays_is_reported() {
        let (_dir, mut store) = open_temp();
        store.save_holidays("2026-11-02", "2026-11-15", &[]).unwrap();
        work(&store, "2026-11-02", 8);
        assert!(!summary(&store, "2026-11-01").holidays_missing);
        work(&store, "2026-11-28", 8);
        assert!(!summary(&store, "2026-11-01").holidays_missing, "Samstag zählt nicht, Feiertag oder nicht");
        work(&store, "2026-11-16", 8);
        let month = summary(&store, "2026-11-01");
        assert!(month.holidays_missing);
        assert_eq!(month.counted_days, 2, "der Tag zählt trotzdem mit");
    }

    #[test]
    fn month_summary_rejects_a_malformed_date() {
        let (_dir, store) = open_temp();
        assert!(matches!(month_summary(&store, DAY, "morgen"), Err(StoreError::InvalidDate)));
    }
}
