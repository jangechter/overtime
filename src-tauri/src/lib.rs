pub mod holidays;
pub mod hours;
pub mod store;
pub mod target;
pub mod week;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use holidays::OpenHolidays;
use store::{Absence, Entry, Store, WeekHolidays};

pub(crate) const STORE_UNAVAILABLE: &str = "Interner Fehler: Der Datenspeicher ist nicht verfügbar. Bitte die App neu starten.";

/// Der Datenspeicher oder, wenn die Datenbank beim Start nicht geöffnet werden konnte, die deutsche Meldung dazu.
/// So startet die App trotzdem, und jeder Befehl gibt die Meldung zurück, die dann im Fenster erscheint.
struct AppStore(Result<OpenedStore, String>);

/// Der geöffnete Datenspeicher und ob das Einfrieren des Solls geklappt hat (`frozen`).
struct OpenedStore {
    store: Arc<Mutex<Store>>,
    frozen: bool,
}

/// Der Datenspeicher für den Feiertags-Abruf, der gespeicherte Feiertage der letzten Wochen ersetzt.
/// Ohne eingefrorenes Soll wird nicht abgerufen, sonst flösse die Änderung in das Soll abgeschlossener Wochen ein.
fn store_for_refresh(opened: &Result<OpenedStore, String>) -> Result<Arc<Mutex<Store>>, String> {
    let opened = opened.as_ref().map_err(|message| message.clone())?;
    if !opened.frozen {
        return Err("Das Soll abgeschlossener Wochen konnte nicht gesichert werden, deshalb werden die Feiertage erst beim nächsten Start aktualisiert.".to_string());
    }
    Ok(Arc::clone(&opened.store))
}

/// Öffnet die Datenbank und friert danach das Soll der abgeschlossenen Wochen ein. Das passiert vor dem ersten Befehl
/// der Oberfläche, also vor dem Feiertags-Abruf, der gespeicherte Feiertage der letzten Wochen ersetzen kann.
/// Scheitert nur das Einfrieren, startet die App trotzdem, ruft aber keine Feiertage ab (siehe `store_for_refresh`).
fn open_store(path: &std::path::Path, today: chrono::NaiveDate) -> Result<OpenedStore, String> {
    let store = Store::open(path).map_err(|e| {
        eprintln!("Datenbank konnte nicht geöffnet werden: {e:?}");
        e.to_string()
    })?;
    let frozen = match target::freeze_past_weeks(&store, target::DAILY_TARGET_MINUTES, today) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("Soll abgeschlossener Wochen konnte nicht eingefroren werden: {e:?}");
            false
        }
    };
    Ok(OpenedStore { store: Arc::new(Mutex::new(store)), frozen })
}

fn lock<'a>(state: &'a tauri::State<AppStore>) -> Result<std::sync::MutexGuard<'a, Store>, String> {
    let opened = state.0.as_ref().map_err(|message| message.clone())?;
    opened.store.lock().map_err(|e| {
        eprintln!("Sperre des Datenspeichers fehlgeschlagen: {e}");
        STORE_UNAVAILABLE.to_string()
    })
}

#[tauri::command]
fn save_entry(
    state: tauri::State<AppStore>,
    date: String,
    start: String,
    end: String,
    break_minutes: f64,
) -> Result<u32, String> {
    let store = lock(&state)?;
    store.save(&date, &start, &end, break_minutes).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_entry(state: tauri::State<AppStore>, date: String) -> Result<(), String> {
    let store = lock(&state)?;
    store.delete(&date).map_err(|e| e.to_string())
}

/// Markiert den Tag als krank oder Urlaub und gibt die angerechneten Minuten zurück.
#[tauri::command]
fn mark_absence(state: tauri::State<AppStore>, date: String, kind: Absence) -> Result<u32, String> {
    let store = lock(&state)?;
    target::mark_absence(&store, target::DAILY_TARGET_MINUTES, &date, kind).map_err(|e| e.to_string())
}

/// Einträge von `from` bis `to`; Krank/Urlaub mit den angerechneten Minuten.
#[tauri::command]
fn load_entries(state: tauri::State<AppStore>, from: String, to: String) -> Result<Vec<Entry>, String> {
    let store = lock(&state)?;
    target::day_entries(&store, target::DAILY_TARGET_MINUTES, &from, &to).map_err(|e| e.to_string())
}

/// Ist, Soll und Saldo der Woche, in der `date` liegt.
#[tauri::command]
fn week_summary(state: tauri::State<AppStore>, date: String) -> Result<target::WeekSummary, String> {
    let store = lock(&state)?;
    target::week_summary(&store, target::DAILY_TARGET_MINUTES, &date).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_holidays(state: tauri::State<AppStore>, from: String, to: String) -> Result<WeekHolidays, String> {
    let store = lock(&state)?;
    store.week_holidays(&from, &to).map_err(|e| e.to_string())
}

/// Holt die Feiertage für das Fenster um heute. Der Abruf läuft in einem eigenen Thread, damit das Fenster nicht einfriert.
#[tauri::command]
async fn refresh_holidays(state: tauri::State<'_, AppStore>) -> Result<(), String> {
    let store = store_for_refresh(&state.0)?;
    let today = chrono::Local::now().date_naive();
    tauri::async_runtime::spawn_blocking(move || holidays::refresh(&store, &OpenHolidays, today))
        .await
        .map_err(|e| holidays::HolidayError::Fetch(e.to_string()))
        .and_then(|result| result)
        .map_err(|e| {
            eprintln!("Feiertage konnten nicht aktualisiert werden: {e:?}");
            e.to_string()
        })
}

/// Holt die Feiertage genau der Woche, in der `date` liegt (Knopf „Feiertage laden“), ebenfalls in einem eigenen Thread.
#[tauri::command]
async fn fetch_week_holidays(state: tauri::State<'_, AppStore>, date: String) -> Result<(), String> {
    let store = state.0.as_ref().map(|opened| Arc::clone(&opened.store)).map_err(|message| message.clone())?;
    tauri::async_runtime::spawn_blocking(move || holidays::load_week(&store, &OpenHolidays, &date))
        .await
        .map_err(|e| holidays::HolidayError::WeekUnavailable(e.to_string()))
        .and_then(|result| result)
        .map_err(|e| {
            eprintln!("Feiertage der Woche konnten nicht geladen werden: {e:?}");
            e.to_string()
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(AppStore(open_store(&dir.join("overtime.db"), chrono::Local::now().date_naive())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_entry, delete_entry, mark_absence, load_entries, week_summary, load_holidays, refresh_holidays, fetch_week_holidays])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()
    }

    #[test]
    fn open_store_opens_a_fresh_database() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_store(&dir.path().join("overtime.db"), today()).is_ok());
    }

    #[test]
    fn open_store_reports_a_database_from_a_newer_app_version_in_german() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        rusqlite::Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
        let message = open_store(&path, today()).err().unwrap();
        assert_eq!(message, "Die Datenbank stammt aus einer neueren Version der App und kann nicht geöffnet werden.");
    }

    #[test]
    fn open_store_reports_an_unmigratable_entry_in_german() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE entries (date TEXT PRIMARY KEY, start TEXT NOT NULL, end TEXT NOT NULL, break_minutes INTEGER NOT NULL);
                INSERT INTO entries VALUES ('2026-11-17', '16:00', '08:00', 0);",
            )
            .unwrap();
        let message = open_store(&path, today()).err().unwrap();
        assert!(message.starts_with("Der gespeicherte Eintrag vom 2026-11-17 ist ungültig"), "{message}");
    }

    #[test]
    fn open_store_freezes_the_target_of_finished_weeks_before_anything_else_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        let holidays = [store::Holiday { date: "2026-11-18".into(), name: "Feiertag".into() }];
        Store::open(&path).unwrap().save_holidays("2026-11-02", "2026-11-29", &holidays).unwrap();

        let shared = open_store(&path, today()).unwrap().store;
        let store = shared.lock().unwrap();
        assert_eq!(store.week_target("2026-11-09").unwrap(), Some(2400));
        assert_eq!(store.week_target("2026-11-16").unwrap(), Some(1920), "Feiertag am 18.11. senkt das Soll");
        assert_eq!(store.week_target("2026-11-23").unwrap(), None, "die Woche von heute (25.11.) ist noch offen");
    }

    #[test]
    fn open_store_still_opens_when_freezing_has_nothing_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let shared = open_store(&dir.path().join("overtime.db"), today()).unwrap().store;
        assert!(shared.lock().unwrap().week_target("2026-11-09").unwrap().is_none());
    }

    /// Antwortet auf jeden Abruf mit einer leeren Feiertagsliste (die API „korrigiert“ alles weg).
    struct EmptyApi;

    impl holidays::HolidayApi for EmptyApi {
        fn fetch(&self, _from: &str, _to: &str) -> Result<String, holidays::HolidayError> {
            Ok("[]".to_string())
        }
    }

    fn prepare_db_with_holiday_in_week_of_16th(path: &std::path::Path) {
        let holidays = [store::Holiday { date: "2026-11-18".into(), name: "Feiertag".into() }];
        Store::open(path).unwrap().save_holidays("2026-11-02", "2026-11-29", &holidays).unwrap();
    }

    #[test]
    fn a_refresh_after_open_store_does_not_change_the_frozen_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        prepare_db_with_holiday_in_week_of_16th(&path);

        let opened = Ok(open_store(&path, today()).unwrap());
        let shared = store_for_refresh(&opened).unwrap();
        holidays::refresh(&shared, &EmptyApi, today()).unwrap();

        let store = shared.lock().unwrap();
        assert!(store.holidays_between("2026-11-18", "2026-11-18").unwrap().is_empty(), "der Abruf hat die Feiertage ersetzt");
        assert_eq!(store.week_target("2026-11-16").unwrap(), Some(1920));
        let summary = target::week_summary(&store, target::DAILY_TARGET_MINUTES, "2026-11-16").unwrap();
        assert_eq!(summary.target_minutes, 1920);
    }

    #[test]
    fn open_store_reports_that_freezing_worked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        prepare_db_with_holiday_in_week_of_16th(&path);
        assert!(open_store(&path, today()).unwrap().frozen);
    }

    #[test]
    fn a_failed_freeze_still_opens_the_store_but_blocks_the_holiday_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        prepare_db_with_holiday_in_week_of_16th(&path);
        rusqlite::Connection::open(&path).unwrap().execute_batch("DROP TABLE week_targets").unwrap();

        let opened = open_store(&path, today()).unwrap();
        assert!(!opened.frozen);
        assert!(opened.store.lock().unwrap().between("2026-11-16", "2026-11-22").is_ok(), "Eingaben bleiben möglich");

        let result = store_for_refresh(&Ok(opened));
        assert_eq!(
            result.err().unwrap(),
            "Das Soll abgeschlossener Wochen konnte nicht gesichert werden, deshalb werden die Feiertage erst beim nächsten Start aktualisiert."
        );
    }

    #[test]
    fn the_holiday_refresh_passes_on_the_database_message_when_the_store_could_not_be_opened() {
        let message = "Die Datenbank stammt aus einer neueren Version der App und kann nicht geöffnet werden.";
        assert_eq!(store_for_refresh(&Err(message.to_string())).err().unwrap(), message);
    }
}
