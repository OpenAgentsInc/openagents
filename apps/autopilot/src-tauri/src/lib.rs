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
        .invoke_handler(tauri::generate_handler![autopilot_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
