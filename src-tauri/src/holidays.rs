//! Feiertage von OpenHolidays: Zeitfenster, Abruf je Bundesland, Parsen und Aktualisieren des Speichers.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Deserialize;

use crate::settings::History;
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

/// Quelle der Feiertage; in Tests durch eine Attrappe ersetzt. Liefert den Antworttext der API für ein Bundesland
/// (`subdivision`, z. B. `DE-NW`).
pub trait HolidayApi {
    fn fetch(&self, subdivision: &str, from: &str, to: &str) -> Result<String, HolidayError>;
}

pub struct OpenHolidays;

impl HolidayApi for OpenHolidays {
    fn fetch(&self, subdivision: &str, from: &str, to: &str) -> Result<String, HolidayError> {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
        let mut response = agent
            .get(request_url(subdivision, from, to))
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

fn request_url(subdivision: &str, from: &str, to: &str) -> String {
    format!(
        "https://openholidaysapi.org/PublicHolidays?countryIsoCode=DE&subdivisionCode={subdivision}&languageIsoCode=DE&validFrom={from}&validTo={to}"
    )
}

#[derive(Deserialize)]
struct ApiHoliday {
    #[serde(rename = "startDate")]
    start_date: String,
    name: Vec<ApiName>,
    #[serde(default)]
    nationwide: bool,
    #[serde(default)]
    subdivisions: Vec<ApiSubdivision>,
}

#[derive(Deserialize)]
struct ApiSubdivision {
    code: String,
}

#[derive(Deserialize)]
struct ApiName {
    language: String,
    text: String,
}

/// Liest die Antwort der API für `subdivision`. Übernommen werden bundesweite Feiertage (`nationwide`) und solche, die
/// im ganzen Bundesland gelten (sein Code steht in `subdivisions`). Feiertage nur für einen Teil des Landes (z. B.
/// `DE-BY-AU`, Augsburger Friedensfest) fallen weg. Öffentliche Feiertage sind eintägig, es zählt `startDate`.
fn parse(json: &str, subdivision: &str) -> Result<Vec<Holiday>, HolidayError> {
    let parsed: Vec<ApiHoliday> = serde_json::from_str(json).map_err(|e| HolidayError::Parse(e.to_string()))?;
    parsed
        .into_iter()
        .filter(|holiday| holiday.nationwide || holiday.subdivisions.iter().any(|part| part.code == subdivision))
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

/// Holt das Fenster um `today` von `api` und ersetzt damit die gespeicherten Feiertage des Fensters. Das Fenster wird
/// nach der Einstellungshistorie geteilt, jeder Abschnitt mit seinem Bundesland geholt; so behalten vergangene und
/// laufende Wochen nach einem Wechsel die Feiertage des alten Bundeslands. Gespeichert wird erst, wenn alle Abschnitte
/// da sind. Der Datenspeicher wird nur kurz zum Lesen der Historie und zum Speichern gesperrt.
pub fn refresh(store: &Mutex<Store>, api: &dyn HolidayApi, today: NaiveDate) -> Result<(), HolidayError> {
    let (from, to) = window(today);
    let periods = History::load(&*lock(store)?)?.subdivision_periods(&from, &to);
    let mut fetched = Vec::new();
    for (start, end, subdivision) in &periods {
        let holidays = within(parse(&api.fetch(subdivision, start, end)?, subdivision)?, start, end);
        fetched.push((start, end, holidays));
    }
    let mut store = lock(store)?;
    // Während des Abrufs (ohne Sperre) kann ein Bundeslandwechsel gespeichert worden sein; dann gehörten die Abschnitte
    // zum alten Bundesland. Geprüft wird unter derselben Sperre wie beim Speichern der Einstellungen.
    if History::load(&store)?.subdivision_periods(&from, &to) != periods {
        return Err(HolidayError::Fetch("Einstellungen während des Abrufs geändert".into()));
    }
    for (start, end, holidays) in fetched {
        store.save_holidays(start, end, &holidays)?;
    }
    Ok(())
}

/// Holt genau die Woche, in der `date` liegt, von `api` und ersetzt damit ihre gespeicherten Feiertage.
/// Eine Woche, die schon vollständig geladen ist, bleibt unberührt (kein Abruf), damit alte Wochen
/// nicht nachträglich überschrieben werden.
pub fn load_week(store: &Mutex<Store>, api: &dyn HolidayApi, date: &str) -> Result<(), HolidayError> {
    let (from, to) = week_of(date)?;
    let subdivision = {
        let store = lock(store)?;
        if store.is_loaded(&from, &to)? {
            return Ok(());
        }
        History::load(&store)?.at(&from).subdivision
    };
    let holidays = api
        .fetch(&subdivision, &from, &to)
        .and_then(|json| parse(&json, &subdivision))
        .map(|holidays| within(holidays, &from, &to))
        .map_err(|e| match e {
            HolidayError::Fetch(detail) | HolidayError::Parse(detail) => HolidayError::WeekUnavailable(detail),
            other => other,
        })?;
    let mut store = lock(store)?;
    if History::load(&store)?.at(&from).subdivision != subdivision {
        return Err(HolidayError::WeekUnavailable("Einstellungen während des Abrufs geändert".into()));
    }
    store.save_holidays(&from, &to, &holidays)?;
    Ok(())
}

/// Nur die Feiertage im angefragten Zeitraum, damit die Antwort für einen Abschnitt keinen anderen (mit anderem
/// Bundesland) überschreibt.
fn within(holidays: Vec<Holiday>, from: &str, to: &str) -> Vec<Holiday> {
    holidays.into_iter().filter(|holiday| from <= holiday.date.as_str() && holiday.date.as_str() <= to).collect()
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
        fn fetch(&self, _subdivision: &str, from: &str, to: &str) -> Result<String, HolidayError> {
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
    fn request_asks_for_public_holidays_of_the_subdivision_in_german() {
        assert_eq!(
            request_url("DE-NW", "2026-12-06", "2027-02-20"),
            "https://openholidaysapi.org/PublicHolidays?countryIsoCode=DE&subdivisionCode=DE-NW&languageIsoCode=DE&validFrom=2026-12-06&validTo=2027-02-20"
        );
        assert!(request_url("DE-BY", "2027-01-04", "2027-01-10").contains("subdivisionCode=DE-BY&"));
    }

    #[test]
    fn parse_keeps_nationwide_and_regional_holidays() {
        let json = format!("[{ALL_SAINTS},{CHRISTMAS},{NEW_YEAR}]");
        assert_eq!(
            parse(&json, "DE-NW").unwrap(),
            vec![
                holiday("2026-11-01", "Allerheiligen"),
                holiday("2026-12-25", "1. Weihnachtsfeiertag"),
                holiday("2027-01-01", "Neujahr"),
            ]
        );
    }

    const AUGSBURG: &str = r#"{"startDate":"2026-08-08","name":[{"language":"DE","text":"Friedensfest"}],"regionalScope":"Local","nationwide":false,"subdivisions":[{"code":"DE-BY-AU","shortName":"BY-AU"}]}"#;
    const ASSUMPTION: &str = r#"{"startDate":"2026-08-15","name":[{"language":"DE","text":"Mariä Himmelfahrt"}],"regionalScope":"Regional","nationwide":false,"subdivisions":[{"code":"DE-SL","shortName":"SL"},{"code":"DE-BY","shortName":"BY"}]}"#;

    #[test]
    fn parse_drops_holidays_of_other_subdivisions_and_of_only_part_of_the_subdivision() {
        let json = format!("[{CHRISTMAS},{ALL_SAINTS},{AUGSBURG},{ASSUMPTION}]");
        assert_eq!(
            parse(&json, "DE-BY").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2026-08-15", "Mariä Himmelfahrt")]
        );
        assert_eq!(
            parse(&json, "DE-NW").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2026-11-01", "Allerheiligen")]
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
            parse(&format!("[{two_names},{english_only}]"), "DE-NW").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-01", "New Year's Day")]
        );
    }

    #[test]
    fn parse_accepts_an_empty_answer() {
        assert_eq!(parse("[]", "DE-NW").unwrap(), vec![]);
    }

    #[test]
    fn parse_rejects_answers_that_are_not_a_holiday_list() {
        for body in ["", "<html>Bad Gateway</html>", "{}", r#"{"title":"Bad Request"}"#, "[{}]"] {
            assert!(matches!(parse(body, "DE-NW"), Err(HolidayError::Parse(_))), "{body}");
        }
    }

    #[test]
    fn parse_rejects_a_holiday_with_a_malformed_date() {
        let broken = CHRISTMAS.replace("2026-12-25", "25.12.2026");
        assert!(matches!(parse(&format!("[{broken}]"), "DE-NW"), Err(HolidayError::Parse(_))));
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

    // --- Bundesland ---

    /// Antwortet je Bundesland mit eigenem Text und merkt sich die Anfragen.
    struct ApiBySubdivision {
        answers: Vec<(&'static str, String)>,
        offline_for: Option<&'static str>,
        asked: RefCell<Vec<(String, String, String)>>,
    }

    impl ApiBySubdivision {
        fn new(answers: Vec<(&'static str, String)>) -> ApiBySubdivision {
            ApiBySubdivision { answers, offline_for: None, asked: RefCell::new(Vec::new()) }
        }
    }

    impl HolidayApi for ApiBySubdivision {
        fn fetch(&self, subdivision: &str, from: &str, to: &str) -> Result<String, HolidayError> {
            self.asked.borrow_mut().push((subdivision.to_string(), from.to_string(), to.to_string()));
            if self.offline_for == Some(subdivision) {
                return Err(HolidayError::Fetch("kein Netz".into()));
            }
            Ok(self.answers.iter().find(|(code, _)| *code == subdivision).map(|(_, body)| body.clone()).unwrap_or("[]".into()))
        }
    }

    const EPIPHANY: &str = r#"{"startDate":"2027-01-06","name":[{"language":"DE","text":"Heilige Drei Könige"}],"regionalScope":"Regional","nationwide":false,"subdivisions":[{"code":"DE-BW","shortName":"BW"},{"code":"DE-BY","shortName":"BY"}]}"#;

    fn bavaria_from(store: &Mutex<Store>, monday: &str) {
        let bavaria = crate::settings::Settings { subdivision: "DE-BY".into(), ..Default::default() };
        store.lock().unwrap().save_settings(monday, &bavaria).unwrap();
    }

    #[test]
    fn refresh_asks_for_each_part_of_the_window_with_the_subdivision_valid_in_it() {
        let (_dir, store) = open_temp();
        bavaria_from(&store, "2026-11-30");
        let api = ApiBySubdivision::new(vec![("DE-NW", format!("[{ALL_SAINTS}]")), ("DE-BY", format!("[{CHRISTMAS},{EPIPHANY}]"))]);
        refresh(&store, &api, date("2026-11-20")).unwrap();
        assert_eq!(
            *api.asked.borrow(),
            vec![
                ("DE-NW".to_string(), "2026-11-02".to_string(), "2026-11-29".to_string()),
                ("DE-BY".to_string(), "2026-11-30".to_string(), "2027-01-24".to_string()),
            ]
        );
        let store = store.lock().unwrap();
        assert_eq!(
            store.holidays_between("2026-11-01", "2027-01-31").unwrap(),
            vec![holiday("2026-12-25", "1. Weihnachtsfeiertag"), holiday("2027-01-06", "Heilige Drei Könige")]
        );
        assert!(!store.week_holidays("2026-11-23", "2026-11-29").unwrap().missing);
        assert!(!store.week_holidays("2026-11-30", "2026-12-06").unwrap().missing);
    }

    #[test]
    fn past_and_current_weeks_keep_the_holidays_of_the_old_subdivision_after_a_change() {
        let (_dir, store) = open_temp();
        let nrw_and_bavaria = ApiBySubdivision::new(vec![
            ("DE-NW", r#"[{"startDate":"2026-11-18","name":[{"language":"DE","text":"NRW-Feiertag"}],"nationwide":false,"subdivisions":[{"code":"DE-NW"}]}]"#.to_string()),
            ("DE-BY", r#"[{"startDate":"2026-11-18","name":[{"language":"DE","text":"Bayern-Feiertag"}],"nationwide":false,"subdivisions":[{"code":"DE-BY"}]}]"#.to_string()),
        ]);
        refresh(&store, &nrw_and_bavaria, date("2026-11-25")).unwrap();
        crate::settings::change(&store.lock().unwrap(), date("2026-11-25"), "8:00", "40:00", "DE-BY").unwrap();
        refresh(&store, &nrw_and_bavaria, date("2026-11-26")).unwrap();
        assert_eq!(store.lock().unwrap().holidays_between("2026-11-18", "2026-11-18").unwrap(), vec![holiday("2026-11-18", "NRW-Feiertag")]);
        assert!(nrw_and_bavaria.asked.borrow().iter().any(|(code, from, _)| code == "DE-BY" && from == "2026-11-30"));
    }

    #[test]
    fn refresh_stores_nothing_when_one_part_of_the_window_fails() {
        let (_dir, store) = open_temp();
        store.lock().unwrap().save_holidays("2026-11-02", "2027-01-24", &[holiday("2026-12-25", "Alt")]).unwrap();
        bavaria_from(&store, "2026-11-30");
        let mut api = ApiBySubdivision::new(vec![("DE-NW", "[]".to_string())]);
        api.offline_for = Some("DE-BY");
        assert!(matches!(refresh(&store, &api, date("2026-11-20")), Err(HolidayError::Fetch(_))));
        assert_eq!(store.lock().unwrap().holidays_between("2026-12-25", "2026-12-25").unwrap(), vec![holiday("2026-12-25", "Alt")]);
    }

    /// Speichert während des Abrufs einen Wechsel nach Bayern (wie ein Klick in den Einstellungen) und antwortet dann.
    struct ApiOvertakenBySettings<'a> {
        store: &'a Mutex<Store>,
    }

    impl HolidayApi for ApiOvertakenBySettings<'_> {
        fn fetch(&self, _subdivision: &str, _from: &str, _to: &str) -> Result<String, HolidayError> {
            crate::settings::change(&self.store.lock().unwrap(), date("2026-11-25"), "8:00", "40:00", "DE-BY").unwrap();
            Ok(format!("[{CHRISTMAS}]"))
        }
    }

    #[test]
    fn refresh_does_not_store_holidays_fetched_for_an_outdated_subdivision() {
        let (_dir, store) = open_temp();
        let error = refresh(&store, &ApiOvertakenBySettings { store: &store }, date("2026-11-25")).unwrap_err();
        assert_eq!(error.to_string(), "Feiertage konnten nicht aktualisiert werden.");
        let store = store.lock().unwrap();
        assert!(store.week_holidays("2026-12-21", "2026-12-27").unwrap().missing);
        assert!(store.holidays_between("2026-12-25", "2026-12-25").unwrap().is_empty());
    }

    #[test]
    fn load_week_does_not_store_holidays_fetched_for_an_outdated_subdivision() {
        let (_dir, store) = open_temp();
        let error = load_week(&store, &ApiOvertakenBySettings { store: &store }, "2026-12-23").unwrap_err();
        assert_eq!(error.to_string(), "Feiertage für diese Woche konnten nicht geladen werden.");
        assert!(store.lock().unwrap().week_holidays("2026-12-21", "2026-12-27").unwrap().missing);
    }

    #[test]
    fn load_week_asks_with_the_subdivision_of_that_week() {
        let (_dir, store) = open_temp();
        bavaria_from(&store, "2027-01-04");
        let api = ApiBySubdivision::new(vec![("DE-BY", format!("[{EPIPHANY}]"))]);
        load_week(&store, &api, "2027-01-06").unwrap();
        load_week(&store, &api, "2026-12-30").unwrap();
        assert_eq!(
            *api.asked.borrow(),
            vec![
                ("DE-BY".to_string(), "2027-01-04".to_string(), "2027-01-10".to_string()),
                ("DE-NW".to_string(), "2026-12-28".to_string(), "2027-01-03".to_string()),
            ]
        );
        assert_eq!(store.lock().unwrap().holidays_between("2027-01-06", "2027-01-06").unwrap(), vec![holiday("2027-01-06", "Heilige Drei Könige")]);
    }

    #[test]
    #[ignore = "braucht Netz"]
    fn live_refresh_for_bavaria_keeps_epiphany_and_drops_the_augsburg_peace_festival() {
        let (_dir, store) = open_temp();
        bavaria_from(&store, "0001-01-01");
        load_week(&store, &OpenHolidays, "2027-01-06").unwrap();
        load_week(&store, &OpenHolidays, "2026-08-08").unwrap();
        let store = store.lock().unwrap();
        assert_eq!(store.holidays_between("2027-01-04", "2027-01-10").unwrap(), vec![holiday("2027-01-06", "Heilige Drei Könige")]);
        assert!(store.holidays_between("2026-08-03", "2026-08-09").unwrap().is_empty(), "Friedensfest gilt nur in Augsburg");
        assert!(!store.week_holidays("2026-08-03", "2026-08-09").unwrap().missing);
    }

    #[test]
    fn refresh_with_an_unreadable_answer_stores_nothing() {
        let (_dir, store) = open_temp();
        let error = refresh(&store, &FakeApi::answering("<html>Bad Gateway</html>"), date("2026-12-20")).unwrap_err();
        assert_eq!(error.to_string(), "Feiertage konnten nicht aktualisiert werden.");
        assert!(!store.lock().unwrap().is_loaded("2026-12-06", "2027-02-20").unwrap());
    }
}
