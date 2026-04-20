pub mod control;
mod pylon;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AutopilotStatus {
    product: &'static str,
    shell: &'static str,
    rust_authority: &'static str,
    runtime_lane: &'static str,
}

#[tauri::command]
fn autopilot_status() -> AutopilotStatus {
    AutopilotStatus {
        product: "Autopilot",
        shell: "Tauri",
        rust_authority: "online",
        runtime_lane: "prototype",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(pylon::PylonManager::default())
        .setup(|app| {
            control::start_control_plane(app.handle().clone()).map_err(|error| error.into())
        })
        .invoke_handler(tauri::generate_handler![
            autopilot_status,
            pylon::pylon_detect,
            pylon::pylon_get_status,
            pylon::pylon_start,
            pylon::pylon_stop,
            pylon::pylon_restart,
            pylon::pylon_set_mode,
            pylon::pylon_open_logs,
            pylon::proof_run,
            pylon::proof_get,
            pylon::proof_doctor,
            pylon::proof_stop,
            pylon::proof_reset,
            pylon::proof_open_artifacts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
