mod claude_code;

use claude_code::{ClaudeDiscovery, ClaudeManager, Message, ClaudeConversation};
use log::info;
use serde::{Serialize, Deserialize};
use tauri::State;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::fs;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Timelike};

// State wrapper for Tauri
pub struct AppState {
    discovery: Arc<Mutex<ClaudeDiscovery>>,
    manager: Arc<Mutex<Option<ClaudeManager>>>,
}

#[derive(Serialize)]
struct CommandResult<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> CommandResult<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

// APM Analysis Structures
#[derive(Serialize, Deserialize, Debug)]
struct ToolUsage {
    name: String,
    count: u32,
    percentage: f64,
    category: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct APMSession {
    id: String,
    project: String,
    apm: f64,
    duration: f64, // in minutes
    #[serde(rename = "messageCount")]
    message_count: u32,
    #[serde(rename = "toolCount")]
    tool_count: u32,
    timestamp: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProductivityByTime {
    morning: f64,
    afternoon: f64,
    evening: f64,
    night: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct APMStats {
    #[serde(rename = "overallAPM")]
    overall_apm: f64,
    #[serde(rename = "currentSessionAPM")]
    current_session_apm: f64,
    #[serde(rename = "totalSessions")]
    total_sessions: u32,
    #[serde(rename = "totalMessages")]
    total_messages: u32,
    #[serde(rename = "totalToolUses")]
    total_tool_uses: u32,
    #[serde(rename = "totalDuration")]
    total_duration: f64, // in minutes
    #[serde(rename = "skillTier")]
    skill_tier: String,
    #[serde(rename = "tierColor")]
    tier_color: String,
    #[serde(rename = "toolUsage")]
    tool_usage: Vec<ToolUsage>,
    #[serde(rename = "recentSessions")]
    recent_sessions: Vec<APMSession>,
    #[serde(rename = "productivityByTime")]
    productivity_by_time: ProductivityByTime,
}

#[derive(Deserialize, Debug)]
struct ConversationEntry {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "type")]
    message_type: Option<String>,
    message: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct ToolUse {
    #[serde(rename = "type")]
    tool_type: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

// APM Analysis Implementation
fn get_tool_category(tool_name: &str) -> String {
    match tool_name {
        "Edit" | "MultiEdit" | "Write" => "Code Generation".to_string(),
        "Read" | "LS" | "Glob" => "File Operations".to_string(),
        "Bash" => "System Operations".to_string(),
        "Grep" | "WebSearch" | "WebFetch" => "Search".to_string(),
        "TodoWrite" | "TodoRead" => "Planning".to_string(),
        _ => "Other".to_string(),
    }
}

fn calculate_apm(message_count: u32, tool_count: u32, duration_minutes: f64) -> f64 {
    if duration_minutes <= 0.0 {
        return 0.0;
    }
    (message_count as f64 + tool_count as f64) / duration_minutes
}

fn get_skill_tier(apm: f64) -> (String, String) {
    if apm >= 200.0 {
        ("Elite".to_string(), "text-purple-400".to_string())
    } else if apm >= 100.0 {
        ("Professional".to_string(), "text-red-400".to_string())
    } else if apm >= 50.0 {
        ("Productive".to_string(), "text-orange-400".to_string())
    } else if apm >= 25.0 {
        ("Active".to_string(), "text-yellow-400".to_string())
    } else if apm >= 10.0 {
        ("Casual".to_string(), "text-green-400".to_string())
    } else {
        ("Novice".to_string(), "text-amber-600".to_string())
    }
}

fn clean_project_name(project_name: &str) -> String {
    project_name
        .replace("-Users-", "~/")
        .replace("-", "/")
        .trim_start_matches("~/")
        .to_string()
}

async fn analyze_conversations() -> Result<APMStats, String> {
    let home_dir = dirs_next::home_dir()
        .ok_or("Could not find home directory")?;
    
    let claude_dir = home_dir.join(".claude").join("projects");
    
    if !claude_dir.exists() {
        return Err("Claude projects directory not found. Please ensure Claude Code is installed and has been used.".to_string());
    }
    
    let mut all_sessions: HashMap<String, (Vec<ConversationEntry>, String)> = HashMap::new();
    let mut tool_counts: HashMap<String, u32> = HashMap::new();
    
    // Scan for project directories
    let project_dirs = fs::read_dir(&claude_dir)
        .map_err(|e| format!("Failed to read projects directory: {}", e))?;
    
    for project_entry in project_dirs {
        let project_entry = project_entry
            .map_err(|e| format!("Failed to read project entry: {}", e))?;
        
        if !project_entry.file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?
            .is_dir() {
            continue;
        }
        
        let project_name = project_entry.file_name().to_string_lossy().to_string();
        let cleaned_project_name = clean_project_name(&project_name);
        
        // Look for .jsonl files in this project directory
        let jsonl_files = fs::read_dir(project_entry.path())
            .map_err(|e| format!("Failed to read project directory {}: {}", project_name, e))?;
        
        for file_entry in jsonl_files {
            let file_entry = file_entry
                .map_err(|e| format!("Failed to read file entry: {}", e))?;
            
            let file_path = file_entry.path();
            if file_path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            
            // Read and parse JSONL file
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;
            
            let mut session_entries: Vec<ConversationEntry> = Vec::new();
            
            for (line_num, line) in content.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<ConversationEntry>(line) {
                    Ok(entry) => session_entries.push(entry),
                    Err(e) => {
                        info!("Warning: Failed to parse line {} in {}: {}", line_num + 1, file_path.display(), e);
                        continue;
                    }
                }
            }
            
            if !session_entries.is_empty() {
                // Group by session ID
                let mut session_groups: HashMap<String, Vec<ConversationEntry>> = HashMap::new();
                
                for entry in session_entries {
                    if let Some(session_id) = &entry.session_id {
                        session_groups.entry(session_id.clone())
                            .or_insert_with(Vec::new)
                            .push(entry);
                    }
                }
                
                for (session_id, entries) in session_groups {
                    all_sessions.insert(session_id, (entries, cleaned_project_name.clone()));
                }
            }
        }
    }
    
    if all_sessions.is_empty() {
        return Err("No conversation data found. Please ensure you have used Claude Code to create some conversations.".to_string());
    }
    
    // Analyze each session
    let mut apm_sessions: Vec<APMSession> = Vec::new();
    let mut total_messages = 0u32;
    let mut total_tools = 0u32;
    let mut total_duration = 0.0f64;
    let mut productivity_by_hour: [Vec<f64>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()]; // morning, afternoon, evening, night
    
    for (session_id, (entries, project_name)) in all_sessions {
        if entries.len() < 2 {
            continue; // Skip sessions with too few entries
        }
        
        let mut message_count = 0u32;
        let mut tool_count = 0u32;
        let mut timestamps: Vec<DateTime<Utc>> = Vec::new();
        
        for entry in &entries {
            if let Some(msg_type) = &entry.message_type {
                if msg_type == "user" || msg_type == "assistant" {
                    message_count += 1;
                }
                
                if msg_type == "assistant" {
                    if let Some(message) = &entry.message {
                        if let Some(content_array) = message.get("content").and_then(|c| c.as_array()) {
                            for content_item in content_array {
                                if let Some(tool_use) = content_item.as_object() {
                                    if tool_use.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                        if let Some(tool_name) = tool_use.get("name").and_then(|n| n.as_str()) {
                                            tool_count += 1;
                                            *tool_counts.entry(tool_name.to_string()).or_insert(0) += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            if let Some(timestamp_str) = &entry.timestamp {
                if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                    timestamps.push(timestamp.with_timezone(&Utc));
                }
            }
        }
        
        if timestamps.len() < 2 {
            continue; // Skip sessions without valid timestamps
        }
        
        timestamps.sort();
        let start_time = timestamps.first().unwrap();
        let end_time = timestamps.last().unwrap();
        let duration = (*end_time - *start_time).num_seconds() as f64 / 60.0; // Convert to minutes
        
        if duration <= 0.0 {
            continue;
        }
        
        let session_apm = calculate_apm(message_count, tool_count, duration);
        
        // Track productivity by time of day
        let hour = start_time.hour();
        let time_slot = if hour >= 6 && hour < 12 { 0 } // morning
        else if hour >= 12 && hour < 18 { 1 } // afternoon  
        else if hour >= 18 && hour < 24 { 2 } // evening
        else { 3 }; // night
        
        productivity_by_hour[time_slot].push(session_apm);
        
        apm_sessions.push(APMSession {
            id: session_id,
            project: project_name,
            apm: session_apm,
            duration,
            message_count,
            tool_count,
            timestamp: start_time.to_rfc3339(),
        });
        
        total_messages += message_count;
        total_tools += tool_count;
        total_duration += duration;
    }
    
    // Calculate overall metrics
    let total_sessions = apm_sessions.len() as u32;
    let overall_apm = if total_duration > 0.0 {
        (total_messages as f64 + total_tools as f64) / total_duration
    } else {
        0.0
    };
    
    // Get current session APM (most recent session)
    apm_sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let current_session_apm = apm_sessions.first().map(|s| s.apm).unwrap_or(0.0);
    
    // Calculate skill tier
    let (skill_tier, tier_color) = get_skill_tier(overall_apm);
    
    // Process tool usage statistics
    let total_tool_uses_for_percentage = total_tools as f64;
    let mut tool_usage: Vec<ToolUsage> = tool_counts
        .into_iter()
        .map(|(name, count)| ToolUsage {
            category: get_tool_category(&name),
            name,
            count,
            percentage: if total_tool_uses_for_percentage > 0.0 {
                (count as f64 / total_tool_uses_for_percentage) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    
    tool_usage.sort_by(|a, b| b.count.cmp(&a.count));
    
    // Calculate productivity by time of day
    let productivity_by_time = ProductivityByTime {
        morning: productivity_by_hour[0].iter().sum::<f64>() / productivity_by_hour[0].len().max(1) as f64,
        afternoon: productivity_by_hour[1].iter().sum::<f64>() / productivity_by_hour[1].len().max(1) as f64,
        evening: productivity_by_hour[2].iter().sum::<f64>() / productivity_by_hour[2].len().max(1) as f64,
        night: productivity_by_hour[3].iter().sum::<f64>() / productivity_by_hour[3].len().max(1) as f64,
    };
    
    Ok(APMStats {
        overall_apm,
        current_session_apm,
        total_sessions,
        total_messages,
        total_tool_uses: total_tools,
        total_duration,
        skill_tier,
        tier_color,
        tool_usage,
        recent_sessions: apm_sessions.into_iter().take(20).collect(), // Limit to 20 most recent
        productivity_by_time,
    })
}

#[tauri::command]
async fn analyze_claude_conversations() -> Result<CommandResult<APMStats>, String> {
    info!("analyze_claude_conversations called");
    
    match analyze_conversations().await {
        Ok(stats) => {
            info!("APM analysis completed successfully. Overall APM: {:.1}, Sessions: {}", 
                  stats.overall_apm, stats.total_sessions);
            Ok(CommandResult::success(stats))
        }
        Err(e) => {
            info!("APM analysis failed: {}", e);
            Ok(CommandResult::error(e))
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn discover_claude(state: State<'_, AppState>) -> Result<CommandResult<String>, String> {
    let mut discovery = state.discovery.lock().await;
    
    match discovery.discover_binary().await {
        Ok(path) => {
            // Also try to discover data directory
            let _ = discovery.discover_data_directory().await;
            
            // Initialize the manager with the binary path
            let mut manager = ClaudeManager::new();
            manager.set_binary_path(path.clone());
            
            let mut manager_lock = state.manager.lock().await;
            *manager_lock = Some(manager);
            
            Ok(CommandResult::success(path.to_string_lossy().to_string()))
        }
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[tauri::command]
async fn create_session(
    project_path: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<CommandResult<String>, String> {
    info!("create_session called - project_path: {}", project_path);
    let mut manager_lock = state.manager.lock().await;
    
    if let Some(ref mut manager) = *manager_lock {
        // Set the app handle if not already set
        manager.set_app_handle(app_handle);
        info!("Manager found, creating session...");
        match manager.create_session(project_path).await {
            Ok(session_id) => {
                info!("Session created successfully with ID: {}", session_id);
                Ok(CommandResult::success(session_id))
            },
            Err(e) => {
                info!("Error creating session: {}", e);
                Ok(CommandResult::error(e.to_string()))
            },
        }
    } else {
        info!("Manager not initialized");
        Ok(CommandResult::error("Claude Code not initialized. Please discover Claude first.".to_string()))
    }
}

#[tauri::command]
async fn send_message(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    info!("send_message called - session_id: {}, message: {}", session_id, message);
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        info!("Manager found, sending message...");
        match manager.send_message(&session_id, message).await {
            Ok(_) => {
                info!("Message sent successfully");
                Ok(CommandResult::success(()))
            },
            Err(e) => {
                info!("Error sending message: {}", e);
                Ok(CommandResult::error(e.to_string()))
            },
        }
    } else {
        info!("Manager not initialized");
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<Message>>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.get_messages(&session_id).await {
            Ok(messages) => Ok(CommandResult::success(messages)),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn stop_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        match manager.stop_session(&session_id).await {
            Ok(_) => Ok(CommandResult::success(())),
            Err(e) => Ok(CommandResult::error(e.to_string())),
        }
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_active_sessions(
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<(String, String)>>, String> {
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        let sessions = manager.get_active_sessions().await;
        Ok(CommandResult::success(sessions))
    } else {
        Ok(CommandResult::error("Claude Code not initialized".to_string()))
    }
}

#[tauri::command]
async fn get_history(
    limit: usize,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<ClaudeConversation>>, String> {
    let discovery = state.discovery.lock().await;
    
    match discovery.load_conversations(limit).await {
        Ok(conversations) => Ok(CommandResult::success(conversations)),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[tauri::command]
fn get_project_directory() -> Result<CommandResult<String>, String> {
    // Try to find git repository root first
    if let Ok(output) = std::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !git_root.is_empty() {
                return Ok(CommandResult::success(git_root));
            }
        }
    }
    
    // Fall back to current directory if not in a git repo
    match std::env::current_dir() {
        Ok(path) => Ok(CommandResult::success(path.to_string_lossy().to_string())),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}

#[tauri::command]
async fn handle_claude_event(
    event_type: String,
    _payload: serde_json::Value,
    _state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    // This command is for future use when we need the frontend to send specific events
    // For now, it's a placeholder that can be extended
    match event_type.as_str() {
        "claude:send_message" => {
            // The existing send_message command already handles this
            Ok(CommandResult::success(()))
        }
        _ => Ok(CommandResult::error(format!("Unknown event type: {}", event_type)))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("OpenAgents starting up...");
    
    let app_state = AppState {
        discovery: Arc::new(Mutex::new(ClaudeDiscovery::new())),
        manager: Arc::new(Mutex::new(None)),
    };
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            discover_claude,
            create_session,
            send_message,
            get_messages,
            stop_session,
            get_active_sessions,
            get_history,
            get_project_directory,
            handle_claude_event,
            analyze_claude_conversations,
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
