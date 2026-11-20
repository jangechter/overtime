pub mod holidays;
pub mod hours;
pub mod store;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use holidays::OpenHolidays;
use store::{Entry, Store, WeekHolidays};

pub(crate) const STORE_UNAVAILABLE: &str = "Interner Fehler: Der Datenspeicher ist nicht verfügbar. Bitte die App neu starten.";

/// Der Datenspeicher oder, wenn die Datenbank beim Start nicht geöffnet werden konnte, die deutsche Meldung dazu.
/// So startet die App trotzdem, und jeder Befehl gibt die Meldung zurück, die dann im Fenster erscheint.
struct AppStore(Result<Arc<Mutex<Store>>, String>);

fn open_store(path: &std::path::Path) -> Result<Arc<Mutex<Store>>, String> {
    Store::open(path).map(|store| Arc::new(Mutex::new(store))).map_err(|e| {
        eprintln!("Datenbank konnte nicht geöffnet werden: {e:?}");
        e.to_string()
    })
}

fn lock<'a>(state: &'a tauri::State<AppStore>) -> Result<std::sync::MutexGuard<'a, Store>, String> {
    let store = state.0.as_ref().map_err(|message| message.clone())?;
    store.lock().map_err(|e| {
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

#[tauri::command]
fn load_entries(state: tauri::State<AppStore>, from: String, to: String) -> Result<Vec<Entry>, String> {
    let store = lock(&state)?;
    store.between(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
fn week_total(state: tauri::State<AppStore>, from: String, to: String) -> Result<u32, String> {
    let store = lock(&state)?;
    store.week_total(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_holidays(state: tauri::State<AppStore>, from: String, to: String) -> Result<WeekHolidays, String> {
    let store = lock(&state)?;
    store.week_holidays(&from, &to).map_err(|e| e.to_string())
}

/// Holt die Feiertage für das Fenster um heute. Der Abruf läuft in einem eigenen Thread, damit das Fenster nicht einfriert.
#[tauri::command]
async fn refresh_holidays(state: tauri::State<'_, AppStore>) -> Result<(), String> {
    let store = state.0.clone()?;
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
    let store = state.0.clone()?;
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
            app.manage(AppStore(open_store(&dir.join("overtime.db"))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_entry, delete_entry, load_entries, week_total, load_holidays, refresh_holidays, fetch_week_holidays])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_store_opens_a_fresh_database() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_store(&dir.path().join("overtime.db")).is_ok());
    }

    #[test]
    fn open_store_reports_a_database_from_a_newer_app_version_in_german() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overtime.db");
        rusqlite::Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
        let message = open_store(&path).err().unwrap();
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
        let message = open_store(&path).err().unwrap();
        assert!(message.starts_with("Der gespeicherte Eintrag vom 2026-11-17 ist ungültig"), "{message}");
    }
}
