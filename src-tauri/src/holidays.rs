//! Feiertage NRW von OpenHolidays: Zeitfenster, Abruf, Parsen und Aktualisieren des Speichers.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Deserialize;

use crate::store::{Holiday, Store, StoreError};
use crate::week::{format_date, week_of};

#[derive(Debug)]
pub enum HolidayError {
    /// Kein Netz, Zeitüberschreitung oder Fehlerstatus der API.
    Fetch(String),
    /// Antwort der API nicht lesbar.
    Parse(String),
    Store(StoreError),
    /// Das Nachladen einer einzelnen Woche ist gescheitert (kein Netz oder unlesbare Antwort).
    WeekUnavailable(String),
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

/// Das Fenster beim Start: 2 Wochen zurück bis 2 Monate voraus, nach außen auf volle Wochen (Montag bis Sonntag)
/// erweitert, damit die Randwochen nicht als „nie geladen“ gelten. Als `(von, bis)` im Format `YYYY-MM-DD`.
pub fn window(today: NaiveDate) -> (String, String) {
    let start = today - Days::new(14);
    let end = today + Months::new(2);
    let monday = start - Days::new(u64::from(start.weekday().num_days_from_monday()));
    let sunday = end + Days::new(u64::from(6 - end.weekday().num_days_from_monday()));
    (format_date(monday), format_date(sunday))
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
    fetch_and_save(store, api, &from, &to)
}

/// Holt genau die Woche, in der `date` liegt, von `api` und ersetzt damit ihre gespeicherten Feiertage.
/// Eine Woche, die schon vollständig geladen ist, bleibt unberührt (kein Abruf), damit alte Wochen
/// nicht nachträglich überschrieben werden.
pub fn load_week(store: &Mutex<Store>, api: &dyn HolidayApi, date: &str) -> Result<(), HolidayError> {
    let (from, to) = week_of(date)?;
    if lock(store)?.is_loaded(&from, &to)? {
        return Ok(());
    }
    fetch_and_save(store, api, &from, &to).map_err(|e| match e {
        HolidayError::Fetch(detail) | HolidayError::Parse(detail) => HolidayError::WeekUnavailable(detail),
        other => other,
    })
}

fn fetch_and_save(store: &Mutex<Store>, api: &dyn HolidayApi, from: &str, to: &str) -> Result<(), HolidayError> {
    let holidays = parse(&api.fetch(from, to)?)?;
    lock(store)?.save_holidays(from, to, &holidays)?;
    Ok(())
}

fn lock(store: &Mutex<Store>) -> Result<MutexGuard<'_, Store>, HolidayError> {
    store.lock().map_err(|e| {
        eprintln!("Sperre des Datenspeichers fehlgeschlagen: {e}");
        HolidayError::StoreUnavailable
    })
}

impl std::fmt::Display for HolidayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HolidayError::Fetch(_) | HolidayError::Parse(_) => f.write_str("Feiertage konnten nicht aktualisiert werden."),
            HolidayError::Store(e) => e.fmt(f),
            HolidayError::WeekUnavailable(_) => f.write_str("Feiertage für diese Woche konnten nicht geladen werden."),
            HolidayError::StoreUnavailable => f.write_str(crate::STORE_UNAVAILABLE),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::Weekday;

    use super::*;
    use crate::store::WeekHolidays;

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
    fn window_reaches_two_weeks_back_and_two_months_ahead_rounded_out_to_whole_weeks() {
        // Heute Freitag 20.11.: −2 Wochen = Freitag 6.11. (Montag davor: 2.11.), +2 Monate = Mittwoch 20.1. (Sonntag danach: 24.1.)
        assert_eq!(window(date("2026-11-20")), ("2026-11-02".to_string(), "2027-01-24".to_string()));
    }

    #[test]
    fn window_always_starts_on_a_monday_and_ends_on_a_sunday() {
        for offset in 0..14 {
            let today = date("2026-11-16") + Days::new(offset);
            let (from, to) = window(today);
            assert_eq!(date(&from).weekday(), Weekday::Mon, "{today}");
            assert_eq!(date(&to).weekday(), Weekday::Sun, "{today}");
        }
    }

    #[test]
    fn window_spans_the_year_boundary() {
        assert_eq!(window(date("2026-12-20")), ("2026-11-30".to_string(), "2027-02-21".to_string()));
        assert_eq!(window(date("2027-01-05")), ("2026-12-21".to_string(), "2027-03-07".to_string()));
    }

    #[test]
    fn window_end_is_clamped_to_the_last_day_of_a_shorter_month() {
        assert_eq!(window(date("2026-12-31")).1, "2027-02-28");
        assert_eq!(window(date("2027-12-31")).1, "2028-03-05", "29.2.2028 ist ein Dienstag, die Woche endet am 5.3.");
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
        assert_eq!(*api.requests.borrow(), vec![("2026-11-30".to_string(), "2027-02-21".to_string())]);
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
    fn refresh_covers_the_edge_weeks_completely_but_not_the_weeks_beyond() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering("[]"), date("2026-11-20")).unwrap();
        let store = store.lock().unwrap();
        // Woche, in die „heute −14 Tage“ (6.11.) fällt, und Woche, in die „heute +2 Monate“ (20.1.) fällt
        assert!(!store.week_holidays("2026-11-02", "2026-11-08").unwrap().missing);
        assert!(!store.week_holidays("2027-01-18", "2027-01-24").unwrap().missing);
        assert!(store.week_holidays("2026-10-26", "2026-11-01").unwrap().missing);
        assert!(store.week_holidays("2027-01-25", "2027-01-31").unwrap().missing);
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
        let error = refresh(&store, &FakeApi::offline(), date("2027-01-05")).unwrap_err();
        assert_eq!(error.to_string(), "Feiertage konnten nicht aktualisiert werden.");
        let store = store.lock().unwrap();
        assert_eq!(
            store.holidays_between("2026-12-06", "2027-02-20").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")]
        );
        assert!(store.is_loaded("2026-12-06", "2027-02-20").unwrap());
        assert!(!store.is_loaded("2027-03-01", "2027-03-07").unwrap(), "Fenster des Fehlversuchs gilt nicht als geladen");
    }

    #[test]
    fn load_week_asks_for_exactly_that_week_and_stores_its_holidays() {
        let (_dir, store) = open_temp();
        let api = FakeApi::answering(&format!("[{CHRISTMAS}]"));
        load_week(&store, &api, "2026-12-23").unwrap();
        assert_eq!(*api.requests.borrow(), vec![("2026-12-21".to_string(), "2026-12-27".to_string())]);
        let week = store.lock().unwrap().week_holidays("2026-12-21", "2026-12-27").unwrap();
        assert_eq!(week, WeekHolidays { holidays: vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")], missing: false });
    }

    #[test]
    fn load_week_without_holidays_marks_the_week_as_loaded() {
        let (_dir, store) = open_temp();
        load_week(&store, &FakeApi::answering("[]"), "2027-03-02").unwrap();
        let week = store.lock().unwrap().week_holidays("2027-03-01", "2027-03-07").unwrap();
        assert_eq!(week, WeekHolidays { holidays: vec![], missing: false });
    }

    #[test]
    fn load_week_on_an_already_loaded_week_does_not_call_the_api_and_keeps_its_holidays() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering(&format!("[{CHRISTMAS}]")), date("2026-12-20")).unwrap();
        let api = FakeApi::answering(&format!("[{NEW_YEAR}]"));
        load_week(&store, &api, "2026-12-23").unwrap();
        assert!(api.requests.borrow().is_empty());
        assert_eq!(
            store.lock().unwrap().holidays_between("2026-12-21", "2026-12-27").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")]
        );
    }

    #[test]
    fn load_week_fetches_a_week_that_is_only_partly_loaded() {
        let (_dir, store) = open_temp();
        store.lock().unwrap().save_holidays("2027-02-22", "2027-03-03", &[]).unwrap();
        let api = FakeApi::answering("[]");
        load_week(&store, &api, "2027-03-02").unwrap();
        assert_eq!(*api.requests.borrow(), vec![("2027-03-01".to_string(), "2027-03-07".to_string())]);
        assert!(!store.lock().unwrap().week_holidays("2027-03-01", "2027-03-07").unwrap().missing);
    }

    #[test]
    fn load_week_leaves_other_weeks_untouched() {
        let (_dir, store) = open_temp();
        refresh(&store, &FakeApi::answering(&format!("[{CHRISTMAS}]")), date("2026-12-20")).unwrap();
        load_week(&store, &FakeApi::answering("[]"), "2027-03-02").unwrap();
        let store = store.lock().unwrap();
        assert_eq!(store.holidays_between("2026-12-06", "2027-02-20").unwrap(), vec![holiday("2026-12-25", "1. Weihnachtsfeiertag")]);
        assert!(!store.week_holidays("2027-03-01", "2027-03-07").unwrap().missing);
        assert!(store.week_holidays("2027-02-22", "2027-02-28").unwrap().missing, "Lücke zwischen Fenster und Woche");
    }

    #[test]
    fn load_week_without_network_stores_nothing_and_reports_a_german_message() {
        let (_dir, store) = open_temp();
        let error = load_week(&store, &FakeApi::offline(), "2027-03-02").unwrap_err();
        assert_eq!(error.to_string(), "Feiertage für diese Woche konnten nicht geladen werden.");
        assert!(store.lock().unwrap().week_holidays("2027-03-01", "2027-03-07").unwrap().missing);
    }

    #[test]
    fn load_week_with_an_unreadable_answer_stores_nothing() {
        let (_dir, store) = open_temp();
        let error = load_week(&store, &FakeApi::answering("<html>Bad Gateway</html>"), "2027-03-02").unwrap_err();
        assert_eq!(error.to_string(), "Feiertage für diese Woche konnten nicht geladen werden.");
        assert!(store.lock().unwrap().week_holidays("2027-03-01", "2027-03-07").unwrap().missing);
    }

    #[test]
    fn load_week_with_a_malformed_date_does_not_call_the_api() {
        let (_dir, store) = open_temp();
        let api = FakeApi::answering("[]");
        assert!(matches!(load_week(&store, &api, "morgen"), Err(HolidayError::Store(StoreError::InvalidDate))));
        assert!(api.requests.borrow().is_empty());
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
    #[ignore = "braucht Netz"]
    fn live_load_week_stores_the_holidays_of_a_week_outside_the_window() {
        let (_dir, store) = open_temp();
        load_week(&store, &OpenHolidays, "2026-12-23").unwrap();
        let week = store.lock().unwrap().week_holidays("2026-12-21", "2026-12-27").unwrap();
        assert_eq!(
            week,
            WeekHolidays {
                holidays: vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2026-12-26", "2. Weihnachtsfeiertag")],
                missing: false
            }
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
