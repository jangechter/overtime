pub mod holidays;
pub mod hours;
pub mod store;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use holidays::OpenHolidays;
use store::{Entry, Store, WeekHolidays};

pub(crate) const STORE_UNAVAILABLE: &str = "Interner Fehler: Der Datenspeicher ist nicht verfügbar. Bitte die App neu starten.";

struct AppStore(Arc<Mutex<Store>>);

fn lock<'a>(state: &'a tauri::State<AppStore>) -> Result<std::sync::MutexGuard<'a, Store>, String> {
    state.0.lock().map_err(|e| {
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
fn load_holidays(state: tauri::State<AppStore>, from: String, to: String) -> Result<WeekHolidays, String> {
    let store = lock(&state)?;
    store.week_holidays(&from, &to).map_err(|e| e.to_string())
}

/// Holt die Feiertage für das Fenster um heute. Der Abruf läuft in einem eigenen Thread, damit das Fenster nicht einfriert.
#[tauri::command]
async fn refresh_holidays(state: tauri::State<'_, AppStore>) -> Result<(), String> {
    let store = Arc::clone(&state.0);
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
    let store = Arc::clone(&state.0);
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
            let store = Store::open(&dir.join("overtime.db")).map_err(|e| e.to_string())?;
            app.manage(AppStore(Arc::new(Mutex::new(store))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_entry, delete_entry, load_entries, load_holidays, refresh_holidays, fetch_week_holidays])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
