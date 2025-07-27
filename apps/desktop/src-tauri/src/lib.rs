mod claude_code;
mod error;
mod state;
mod apm;
mod commands;

#[cfg(test)]
mod tests;

use log::info;
use state::AppState;

// Import all commands to make them available for tauri::generate_handler!
use commands::{
    session::{
        discover_claude, create_session, send_message, trigger_claude_response,
        get_messages, stop_session, get_active_sessions, handle_claude_event,
    },
    apm::{
        analyze_claude_conversations, analyze_combined_conversations, 
        get_historical_apm_data, get_user_apm_stats,
    },
    history::{get_history, get_unified_history},
    system::{greet, get_project_directory},
};

// Import Convex commands
use claude_code::commands::{
    test_convex_connection, get_sessions, create_convex_session,
    update_session, delete_session, get_session_by_id, get_convex_messages,
    add_message, update_message, delete_message, get_message_by_id,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("OpenAgents starting up...");
    
    let app_state = AppState::new();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            discover_claude,
            create_session,
            send_message,
            trigger_claude_response,
            get_messages,
            stop_session,
            get_active_sessions,
            get_history,
            get_unified_history,
            get_project_directory,
            handle_claude_event,
            analyze_claude_conversations,
            analyze_combined_conversations,
            get_historical_apm_data,
            get_user_apm_stats,
            // Convex commands
            test_convex_connection,
            get_sessions,
            create_convex_session,
            update_session,
            delete_session,
            get_session_by_id,
            get_convex_messages,
            add_message,
            update_message,
            delete_message,
            get_message_by_id,
        ])
        .setup(|app| {
            // During development, try to prevent window from stealing focus on hot reload
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                
                // Store initial focus state
                let window_focused = app.get_webview_window("main")
                    .map(|w| w.is_focused().unwrap_or(false))
                    .unwrap_or(false);
                
                // If window wasn't focused initially, try to minimize focus disruption
                if !window_focused {
                    if let Some(window) = app.get_webview_window("main") {
                        // Set window to not be always on top
                        let _ = window.set_always_on_top(false);
                        
                        // Set skip taskbar to false to ensure normal window behavior
                        let _ = window.set_skip_taskbar(false);
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}