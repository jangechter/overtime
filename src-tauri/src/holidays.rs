//! Feiertage NRW von OpenHolidays: Zeitfenster, Abruf, Parsen und Aktualisieren des Speichers.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{Days, Months, NaiveDate};
use serde::Deserialize;

use crate::store::{Holiday, Store, StoreError};

#[derive(Debug)]
pub enum HolidayError {
    /// Kein Netz, Zeitüberschreitung oder Fehlerstatus der API.
    Fetch(String),
    /// Antwort der API nicht lesbar.
    Parse(String),
    Store(StoreError),
    /// Der Datenspeicher ist durch einen früheren Fehler gesperrt.
    StoreUnavailable,
}

impl From<StoreError> for HolidayError {
    fn from(e: StoreError) -> Self {
        HolidayError::Store(e)
    }
}

/// Quelle der Feiertage; in Tests durch eine Attrappe ersetzt. Liefert den Antworttext der API.
pub trait HolidayApi {
    fn fetch(&self, from: &str, to: &str) -> Result<String, HolidayError>;
}

pub struct OpenHolidays;

impl HolidayApi for OpenHolidays {
    fn fetch(&self, from: &str, to: &str) -> Result<String, HolidayError> {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
        let mut response = agent
            .get(request_url(from, to))
            .header("Accept", "application/json")
            .call()
            .map_err(|e| HolidayError::Fetch(e.to_string()))?;
        response.body_mut().read_to_string().map_err(|e| HolidayError::Fetch(e.to_string()))
    }
}

/// Das Fenster beim Start: 2 Wochen zurück bis 2 Monate voraus, als `(von, bis)` im Format `YYYY-MM-DD`.
pub fn window(today: NaiveDate) -> (String, String) {
    (format_date(today - Days::new(14)), format_date(today + Months::new(2)))
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn request_url(from: &str, to: &str) -> String {
    format!(
        "https://openholidaysapi.org/PublicHolidays?countryIsoCode=DE&subdivisionCode=DE-NW&languageIsoCode=DE&validFrom={from}&validTo={to}"
    )
}

#[derive(Deserialize)]
struct ApiHoliday {
    #[serde(rename = "startDate")]
    start_date: String,
    name: Vec<ApiName>,
}

#[derive(Deserialize)]
struct ApiName {
    language: String,
    text: String,
}

/// Liest die Antwort der API. Die API liefert für NRW bundesweite (`nationwide`) und regionale Feiertage
/// in einer Liste; beide werden übernommen. Öffentliche Feiertage sind eintägig, es zählt `startDate`.
fn parse(json: &str) -> Result<Vec<Holiday>, HolidayError> {
    let parsed: Vec<ApiHoliday> = serde_json::from_str(json).map_err(|e| HolidayError::Parse(e.to_string()))?;
    parsed
        .into_iter()
        .map(|holiday| {
            let date = NaiveDate::parse_from_str(&holiday.start_date, "%Y-%m-%d")
                .ok()
                .map(format_date)
                .filter(|formatted| *formatted == holiday.start_date)
                .ok_or_else(|| HolidayError::Parse(format!("ungültiges Datum: {}", holiday.start_date)))?;
            let name = holiday
                .name
                .iter()
                .find(|name| name.language == "DE")
                .or(holiday.name.first())
                .ok_or_else(|| HolidayError::Parse(format!("Feiertag ohne Namen am {date}")))?;
            Ok(Holiday { date, name: name.text.clone() })
        })
        .collect()
}

/// Holt das Fenster um `today` von `api` und ersetzt damit die gespeicherten Feiertage des Fensters.
/// Der Datenspeicher wird erst nach dem Abruf gesperrt, damit Eingaben währenddessen nicht warten.
pub fn refresh(store: &Mutex<Store>, api: &dyn HolidayApi, today: NaiveDate) -> Result<(), HolidayError> {
    let (from, to) = window(today);
    let holidays = parse(&api.fetch(&from, &to)?)?;
    let mut store = store.lock().map_err(|e| {
        eprintln!("Sperre des Datenspeichers fehlgeschlagen: {e}");
        HolidayError::StoreUnavailable
    })?;
    store.save_holidays(&from, &to, &holidays)?;
    Ok(())
}

impl std::fmt::Display for HolidayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HolidayError::Fetch(_) | HolidayError::Parse(_) => f.write_str("Feiertage konnten nicht aktualisiert werden."),
            HolidayError::Store(e) => e.fmt(f),
            HolidayError::StoreUnavailable => f.write_str(crate::STORE_UNAVAILABLE),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const CHRISTMAS: &str = r#"{"id":"b37c8536-925e-455c-9e61-8e2b984a74d1","startDate":"2026-12-25","endDate":"2026-12-25","type":"Public","name":[{"language":"DE","text":"1. Weihnachtsfeiertag"}],"regionalScope":"Regional","temporalScope":"FullDay","nationwide":true}"#;
    const NEW_YEAR: &str = r#"{"id":"a9951b88-fbfa-4634-8b80-39c60abb2a0c","startDate":"2027-01-01","endDate":"2027-01-01","type":"Public","name":[{"language":"DE","text":"Neujahr"}],"regionalScope":"Regional","temporalScope":"FullDay","nationwide":true}"#;
    const ALL_SAINTS: &str = r#"{"id":"c359d742-f458-4962-8bf1-44e5d504f6d0","startDate":"2026-11-01","endDate":"2026-11-01","type":"Public","name":[{"language":"DE","text":"Allerheiligen"}],"regionalScope":"Regional","temporalScope":"FullDay","nationwide":false,"subdivisions":[{"code":"DE-SL","shortName":"SL"},{"code":"DE-NW","shortName":"NW"}]}"#;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn holiday(date: &str, name: &str) -> Holiday {
        Holiday { date: date.into(), name: name.into() }
    }

    fn open_temp() -> (tempfile::TempDir, Mutex<Store>) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("overtime.db")).unwrap();
        (dir, Mutex::new(store))
    }

    /// Antwortet mit festem Text oder Fehler und merkt sich die angefragten Zeiträume.
    struct FakeApi {
        answer: Result<String, ()>,
        requests: RefCell<Vec<(String, String)>>,
    }

    impl FakeApi {
        fn answering(body: &str) -> FakeApi {
            FakeApi { answer: Ok(body.to_string()), requests: RefCell::new(Vec::new()) }
        }

        fn offline() -> FakeApi {
            FakeApi { answer: Err(()), requests: RefCell::new(Vec::new()) }
        }
    }

    impl HolidayApi for FakeApi {
        fn fetch(&self, from: &str, to: &str) -> Result<String, HolidayError> {
            self.requests.borrow_mut().push((from.to_string(), to.to_string()));
            self.answer.clone().map_err(|_| HolidayError::Fetch("kein Netz".into()))
        }
    }

    #[test]
    fn window_reaches_two_weeks_back_and_two_months_ahead() {
        assert_eq!(window(date("2026-11-20")), ("2026-11-06".to_string(), "2027-01-20".to_string()));
    }

    #[test]
    fn window_spans_the_year_boundary() {
        assert_eq!(window(date("2026-12-20")), ("2026-12-06".to_string(), "2027-02-20".to_string()));
        assert_eq!(window(date("2027-01-05")), ("2026-12-22".to_string(), "2027-03-05".to_string()));
    }

    #[test]
    fn window_end_is_clamped_to_the_last_day_of_a_shorter_month() {
        assert_eq!(window(date("2026-12-31")).1, "2027-02-28");
        assert_eq!(window(date("2027-12-31")).1, "2028-02-29");
    }

    #[test]
    fn request_asks_for_public_holidays_of_nrw_in_german() {
        assert_eq!(
            request_url("2026-12-06", "2027-02-20"),
            "https://openholidaysapi.org/PublicHolidays?countryIsoCode=DE&subdivisionCode=DE-NW&languageIsoCode=DE&validFrom=2026-12-06&validTo=2027-02-20"
        );
    }

    #[test]
    fn parse_keeps_nationwide_and_regional_holidays() {
        let json = format!("[{ALL_SAINTS},{CHRISTMAS},{NEW_YEAR}]");
        assert_eq!(
            parse(&json).unwrap(),
            vec![
                holiday("2026-11-01", "Allerheiligen"),
                holiday("2026-12-25", "1. Weihnachtsfeiertag"),
                holiday("2027-01-01", "Neujahr"),
            ]
        );
    }

    #[test]
    fn parse_prefers_the_german_name_and_falls_back_to_the_first_one() {
        let two_names = CHRISTMAS.replace(
            r#""name":[{"language":"DE","text":"1. Weihnachtsfeiertag"}]"#,
            r#""name":[{"language":"EN","text":"Christmas Day"},{"language":"DE","text":"1. Weihnachtsfeiertag"}]"#,
        );
        let english_only = NEW_YEAR.replace(
            r#""name":[{"language":"DE","text":"Neujahr"}]"#,
            r#""name":[{"language":"EN","text":"New Year's Day"}]"#,
        );
        assert_eq!(
            parse(&format!("[{two_names},{english_only}]")).unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-01", "New Year's Day")]
        );
    }

    #[test]
    fn parse_accepts_an_empty_answer() {
        assert_eq!(parse("[]").unwrap(), vec![]);
    }

    #[test]
    fn parse_rejects_answers_that_are_not_a_holiday_list() {
        for body in ["", "<html>Bad Gateway</html>", "{}", r#"{"title":"Bad Request"}"#, "[{}]"] {
            assert!(matches!(parse(body), Err(HolidayError::Parse(_))), "{body}");
        }
    }

    #[test]
    fn parse_rejects_a_holiday_with_a_malformed_date() {
        let broken = CHRISTMAS.replace("2026-12-25", "25.12.2026");
        assert!(matches!(parse(&format!("[{broken}]")), Err(HolidayError::Parse(_))));
    }

    #[test]
    fn refresh_asks_for_the_window_and_stores_the_holidays() {
        let (_dir, store) = open_temp();
        let api = FakeApi::answering(&format!("[{CHRISTMAS},{NEW_YEAR}]"));
        refresh(&store, &api, date("2026-12-20")).unwrap();
        assert_eq!(*api.requests.borrow(), vec![("2026-12-06".to_string(), "2027-02-20".to_string())]);
        assert_eq!(
            store.lock().unwrap().holidays_between("2026-12-06", "2027-02-20").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-01", "Neujahr")]
        );
    }

    #[test]
    fn refresh_marks_a_week_without_holidays_as_loaded() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering(&format!("[{CHRISTMAS}]")), date("2026-12-20")).unwrap();
        let store = store.lock().unwrap();
        assert!(store.holidays_between("2026-12-07", "2026-12-13").unwrap().is_empty());
        assert!(store.is_loaded("2026-12-07", "2026-12-13").unwrap());
        assert!(!store.is_loaded("2027-03-01", "2027-03-07").unwrap());
    }

    #[test]
    fn refresh_replaces_stale_holidays_of_the_window() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering(&format!("[{CHRISTMAS}]")), date("2026-12-20")).unwrap();
        refresh(&store, &FakeApi::answering(&format!("[{NEW_YEAR}]")), date("2026-12-20")).unwrap();
        assert_eq!(
            store.lock().unwrap().holidays_between("2026-12-06", "2027-02-20").unwrap(),
            vec![holiday("2027-01-01", "Neujahr")]
        );
    }

    #[test]
    fn refresh_without_network_keeps_the_stored_holidays_and_reports_a_german_warning() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering(&format!("[{CHRISTMAS}]")), date("2026-12-20")).unwrap();
        let error = refresh(&store, &FakeApi::offline(), date("2026-12-21")).unwrap_err();
        assert_eq!(error.to_string(), "Feiertage konnten nicht aktualisiert werden.");
        let store = store.lock().unwrap();
        assert_eq!(
            store.holidays_between("2026-12-06", "2027-02-20").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")]
        );
        assert!(store.is_loaded("2026-12-06", "2027-02-20").unwrap());
        assert!(!store.is_loaded("2026-12-07", "2027-02-21").unwrap(), "Fenster des Fehlversuchs gilt nicht als geladen");
    }

    /// Echter Abruf bei OpenHolidays, daher nur mit `cargo test -- --ignored`.
    #[test]
    #[ignore = "braucht Netz"]
    fn live_refresh_stores_the_nrw_holidays_of_the_window() {
        let (_dir, store) = open_temp();
        refresh(&store, &OpenHolidays, date("2026-12-20")).unwrap();
        assert_eq!(
            store.lock().unwrap().holidays_between("2026-12-06", "2027-02-20").unwrap(),
            vec![
                holiday("2026-12-25", "1. Weihnachtsfeiertag"),
                holiday("2026-12-26", "2. Weihnachtsfeiertag"),
                holiday("2027-01-01", "Neujahr"),
            ]
        );
    }

    #[test]
    fn refresh_with_an_unreadable_answer_stores_nothing() {
        let (_dir, store) = open_temp();
        let error = refresh(&store, &FakeApi::answering("<html>Bad Gateway</html>"), date("2026-12-20")).unwrap_err();
        assert_eq!(error.to_string(), "Feiertage konnten nicht aktualisiert werden.");
        assert!(!store.lock().unwrap().is_loaded("2026-12-06", "2027-02-20").unwrap());
    }
}
