// Tauri backend entry point.
// Wires up commands from split modules and bootstraps local sidecars.

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod convex;
mod commands;
mod subscriptions;
mod bridge;

#[tauri::command]
fn greet(name: &str) -> String { format!("Hello, {}! You've been greeted from Rust!", name) }

// Import Convex-facing commands and data types
use crate::convex::{get_thread_count, list_recent_threads, list_messages_for_thread};
use crate::subscriptions::{subscribe_recent_threads, subscribe_thread_messages};

#[derive(serde::Serialize)]
struct SimpleStatus { healthy: bool, url: String }

#[tauri::command]
fn get_local_convex_status() -> SimpleStatus {
    let port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = format!("http://127.0.0.1:{}", port);
    let healthy = is_port_open(port);
    SimpleStatus { healthy, url }
}

// Mutations and thread creation
use crate::commands::{create_thread, enqueue_run};

// (enqueue_run, create_thread moved to commands.rs)

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Try to start a local Convex backend (binary sidecar) on 127.0.0.1:3210.
            // This is offline-first and avoids the CLI `convex dev` path.
            if std::env::var("OPENAGENTS_SKIP_EMBEDDED_CONVEX").ok().as_deref() != Some("1") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    start_convex_sidecar(handle);
                });
                // Dev quality-of-life: once the backend is up, push functions in the background
                // so you don't need a separate terminal. Logs are inherited into this console.
                {
                    use tauri::Manager;
                    let handle = app.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        deploy_convex_functions_once(handle);
                    });
                }
                // Emit a local convex status event to the UI using the sidecar URL (3210)
                // so the dot reflects the embedded backend instead of the bridge's default (7788).
                {
                    use tauri::Manager;
                    let handle = app.app_handle().clone();
                    let port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
                    tauri::async_runtime::spawn(async move {
                        // Periodically monitor port and emit status transitions
                        let url = format!("http://127.0.0.1:{}", port);
                        let mut last = None;
                        loop {
                            let healthy = is_port_open(port);
                            if last != Some(healthy) {
                                let _ = handle.emit("convex:local_status", serde_json::json!({ "healthy": healthy, "url": url }));
                                last = Some(healthy);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(1000));
                        }
                    });
                }
            }
            // Start codex-bridge automatically in the background on app launch.
            tauri::async_runtime::spawn(async move {
                ensure_bridge_running().await;
            });
            // Emit a bridge:ready event as soon as the WS port is open so the UI connects once.
            {
                use tauri::Manager;
                let handle = app.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    for _ in 0..50u32 { // ~5s probe window
                        if is_port_open(8787) {
                            let _ = handle.emit("bridge:ready", ());
                            println!("[tauri/bootstrap] bridge:ready emitted");
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                });
            }
            // Maximize the main window on startup for an almost-fullscreen layout.
            #[allow(unused_must_use)]
            {
                use tauri::Manager as _;
                if let Some(win) = app.get_webview_window("main") {
                    win.maximize();
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_thread_count,
            list_recent_threads,
            list_messages_for_thread,
            subscribe_thread_messages,
            subscribe_recent_threads,
            get_local_convex_status,
            enqueue_run,
            create_thread
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use crate::bridge::{deploy_convex_functions_once, ensure_bridge_running, is_port_open, start_convex_sidecar};

// (helper/sidecar functions moved to bridge.rs)
