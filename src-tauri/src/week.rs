//! Kalenderwochen (Montag bis Sonntag).

use chrono::{Datelike, Days, NaiveDate};

use crate::store::{check_date, StoreError};

pub(crate) fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Der Montag der Woche, in der `day` liegt.
pub fn monday_of(day: NaiveDate) -> NaiveDate {
    day - Days::new(u64::from(day.weekday().num_days_from_monday()))
}

/// Montag und Sonntag der Woche, in der `date` liegt, als `(von, bis)` im Format `YYYY-MM-DD`.
pub fn week_of(date: &str) -> Result<(String, String), StoreError> {
    check_date(date)?;
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| StoreError::InvalidDate)?;
    let monday = monday_of(day);
    Ok((format_date(monday), format_date(monday + Days::new(6))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_of_returns_monday_to_sunday_for_any_day_of_the_week() {
        for day in ["2026-11-16", "2026-11-18", "2026-11-22"] {
            assert_eq!(week_of(day).unwrap(), ("2026-11-16".to_string(), "2026-11-22".to_string()), "{day}");
        }
    }

    #[test]
    fn week_of_spans_the_year_boundary() {
        assert_eq!(week_of("2027-01-01").unwrap(), ("2026-12-28".to_string(), "2027-01-03".to_string()));
    }

    #[test]
    fn week_of_rejects_a_malformed_date() {
        for day in ["", "morgen", "2026-02-31", "16.11.2026", "2026-1-5"] {
            assert!(matches!(week_of(day), Err(StoreError::InvalidDate)), "{day}");
        }
    }
}
