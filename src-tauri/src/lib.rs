pub mod hours;

#[tauri::command]
fn calculate_day(start: String, end: String, break_minutes: f64) -> Result<u32, String> {
    hours::worked_minutes(&start, &end, break_minutes).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![calculate_day])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
