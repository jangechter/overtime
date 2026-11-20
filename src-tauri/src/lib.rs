pub mod hours;
pub mod store;

use std::sync::Mutex;

use tauri::Manager;

use store::{Entry, Store};

struct AppStore(Mutex<Store>);

fn lock<'a>(state: &'a tauri::State<AppStore>) -> Result<std::sync::MutexGuard<'a, Store>, String> {
    state.0.lock().map_err(|e| {
        eprintln!("Sperre des Datenspeichers fehlgeschlagen: {e}");
        "Interner Fehler: Der Datenspeicher ist nicht verfügbar. Bitte die App neu starten.".to_string()
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = Store::open(&dir.join("overtime.db")).map_err(|e| e.to_string())?;
            app.manage(AppStore(Mutex::new(store)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_entry, delete_entry, load_entries])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
