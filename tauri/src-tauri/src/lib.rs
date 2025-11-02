// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_bridge_token() -> Option<String> {
    use std::path::PathBuf;
    // Prefer OPENAGENTS_HOME if set (points to ~/.openagents). Otherwise derive from HOME/USERPROFILE.
    let base = if let Ok(home) = std::env::var("OPENAGENTS_HOME") {
        PathBuf::from(home)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".openagents")
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        PathBuf::from(profile).join(".openagents")
    } else {
        return None;
    };
    let p = base.join("bridge.json");
    let data = std::fs::read_to_string(&p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    v.get("token").and_then(|x| x.as_str()).map(|s| s.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            std::env::set_var("WINIT_UNIX_BACKEND", "x11");
            std::env::set_var("GDK_BACKEND", "x11");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_bridge_token])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
