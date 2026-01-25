#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    autopilot_desktop_backend::build_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
