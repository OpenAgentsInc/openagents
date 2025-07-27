mod claude_code;

use claude_code::{ClaudeDiscovery, ClaudeManager, Message, ClaudeConversation, UnifiedSession};
use log::info;
use serde::{Serialize, Deserialize};
use tauri::State;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::fs;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Timelike};
use std::env;

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
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ToolUsage {
    name: String,
    count: f64, // Accept float from JavaScript, convert to int when needed
    percentage: f64,
    category: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct APMSession {
    id: String,
    project: String,
    apm: f64,
    duration: f64, // in minutes
    #[serde(rename = "messageCount")]
    message_count: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "toolCount")]
    tool_count: f64, // Accept float from JavaScript, convert to int when needed
    timestamp: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProductivityByTime {
    morning: f64,
    afternoon: f64,
    evening: f64,
    night: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct APMStats {
    #[serde(rename = "apm1h")]
    apm_1h: f64,
    #[serde(rename = "apm6h")]
    apm_6h: f64,
    #[serde(rename = "apm1d")]
    apm_1d: f64,
    #[serde(rename = "apm1w")]
    apm_1w: f64,
    #[serde(rename = "apm1m")]
    apm_1m: f64,
    #[serde(rename = "apmLifetime")]
    apm_lifetime: f64,
    #[serde(rename = "totalSessions")]
    total_sessions: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalMessages")]
    total_messages: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalToolUses")]
    total_tool_uses: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalDuration")]
    total_duration: f64, // in minutes
    #[serde(rename = "toolUsage")]
    tool_usage: Vec<ToolUsage>,
    #[serde(rename = "recentSessions")]
    recent_sessions: Vec<APMSession>,
    #[serde(rename = "productivityByTime")]
    productivity_by_time: ProductivityByTime,
}

// Historical APM data structures
#[derive(Clone, Debug, Serialize)]
struct HistoricalAPMDataPoint {
    period: String, // ISO date or week/month identifier (e.g., "2025-01-26", "2025-W04", "2025-01")
    cli_apm: f64,
    sdk_apm: f64,
    combined_apm: f64,
    total_sessions: u32,
    total_messages: u32,
    total_tools: u32,
    average_session_duration: f64,
}

#[derive(Clone, Debug, Serialize)]
struct HistoricalAPMResponse {
    data: Vec<HistoricalAPMDataPoint>,
    time_scale: String, // "daily", "weekly", "monthly"
    date_range: (String, String), // (start_date, end_date)
    view_mode: String, // "combined", "cli", "sdk"
}

// Combined APM structures for CLI + SDK data
#[derive(Serialize, Deserialize, Debug)]
struct CombinedAPMStats {
    // Combined totals
    #[serde(rename = "apm1h")]
    apm_1h: f64,
    #[serde(rename = "apm6h")]
    apm_6h: f64,
    #[serde(rename = "apm1d")]
    apm_1d: f64,
    #[serde(rename = "apm1w")]
    apm_1w: f64,
    #[serde(rename = "apm1m")]
    apm_1m: f64,
    #[serde(rename = "apmLifetime")]
    apm_lifetime: f64,
    #[serde(rename = "totalSessions")]
    total_sessions: u32,
    #[serde(rename = "totalMessages")]
    total_messages: u32,
    #[serde(rename = "totalToolUses")]
    total_tool_uses: u32,
    #[serde(rename = "totalDuration")]
    total_duration: f64, // in minutes
    #[serde(rename = "toolUsage")]
    tool_usage: Vec<ToolUsage>,
    #[serde(rename = "recentSessions")]
    recent_sessions: Vec<APMSession>,
    #[serde(rename = "productivityByTime")]
    productivity_by_time: ProductivityByTime,
    
    // Breakdown by type
    #[serde(rename = "cliStats")]
    cli_stats: APMStats,
    #[serde(rename = "sdkStats")]
    sdk_stats: APMStats,
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

// Removed skill tier functionality

fn clean_project_name(project_name: &str) -> String {
    project_name
        .replace("-Users-", "~/")
        .replace("-", "/")
        .trim_start_matches("~/")
        .to_string()
}

// Function to fetch Convex APM data
async fn fetch_convex_apm_stats() -> Result<APMStats, String> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    
    let convex_url = env::var("VITE_CONVEX_URL")
        .or_else(|_| env::var("CONVEX_URL"))
        .map_err(|_| "Convex URL not configured. Set VITE_CONVEX_URL or CONVEX_URL environment variable.".to_string())?;
    
    info!("Fetching Convex APM stats from: {}", convex_url);
    
    let client = reqwest::Client::new();
    let url = format!("{}/api/query", convex_url);
    
    let payload = serde_json::json!({
        "path": "claude:getConvexAPMStats",
        "args": {},
        "format": "json"
    });
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Convex: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Unable to read error response".to_string());
        return Err(format!("Convex request failed with status {}: {}", status, error_text));
    }
    
    let text = response.text().await
        .map_err(|e| format!("Failed to read Convex response: {}", e))?;
    
    info!("Convex response: {}", &text[..text.len().min(500)]); // Log first 500 chars
    
    // Parse the response - Convex returns the result directly
    let convex_result: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Convex response: {}", e))?;
    
    // Extract the actual data from the Convex response
    let stats_data = if convex_result.is_object() && convex_result.get("status").is_some() {
        // Handle Convex API response format: {"status": "success", "value": {...}}
        if convex_result["status"].as_str() != Some("success") {
            return Err(format!("Convex error: {}", 
                convex_result.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error")));
        }
        // Extract data from "value" field
        convex_result.get("value").unwrap_or(&convex_result)
    } else {
        // Direct response
        &convex_result
    };
    
    // Convert to APMStats struct
    let apm_stats: APMStats = serde_json::from_value(stats_data.clone())
        .map_err(|e| format!("Failed to deserialize Convex APM stats: {}", e))?;
    
    Ok(apm_stats)
}

// Function to combine CLI and SDK APM stats
fn combine_apm_stats(cli_stats: APMStats, sdk_stats: APMStats) -> CombinedAPMStats {
    info!("Combining APM stats:");
    info!("CLI - Sessions: {}, Messages: {}, Tools: {}, 1h APM: {}", 
          cli_stats.total_sessions, cli_stats.total_messages, cli_stats.total_tool_uses, cli_stats.apm_1h);
    info!("SDK - Sessions: {}, Messages: {}, Tools: {}, 1h APM: {}", 
          sdk_stats.total_sessions, sdk_stats.total_messages, sdk_stats.total_tool_uses, sdk_stats.apm_1h);
    
    // Calculate total actions for reference
    let _cli_total_actions = (cli_stats.total_messages as u32) + (cli_stats.total_tool_uses as u32);
    let _sdk_total_actions = (sdk_stats.total_messages as u32) + (sdk_stats.total_tool_uses as u32);
    
    let combine_apm = |cli_apm: f64, sdk_apm: f64| -> f64 {
        // Simply add the APM rates since they represent independent action streams
        // CLI APM + SDK APM = Combined actions per minute across both interfaces
        cli_apm + sdk_apm
    };
    
    // Combine tool usage
    let mut combined_tool_counts: HashMap<String, u32> = HashMap::new();
    
    for tool in &cli_stats.tool_usage {
        *combined_tool_counts.entry(tool.name.clone()).or_insert(0) += tool.count as u32;
    }
    
    for tool in &sdk_stats.tool_usage {
        *combined_tool_counts.entry(tool.name.clone()).or_insert(0) += tool.count as u32;
    }
    
    let combined_total_tool_uses = cli_stats.total_tool_uses + sdk_stats.total_tool_uses;
    let mut combined_tool_usage: Vec<ToolUsage> = combined_tool_counts
        .into_iter()
        .map(|(name, count)| ToolUsage {
            category: get_tool_category(&name),
            name,
            count: count as f64,
            percentage: if combined_total_tool_uses > 0.0 {
                (count as f64 / combined_total_tool_uses as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    
    // Sort tool usage by count
    combined_tool_usage.sort_by(|a, b| b.count.partial_cmp(&a.count).unwrap_or(std::cmp::Ordering::Equal));
    
    // Combine recent sessions
    let mut combined_sessions = cli_stats.recent_sessions.clone();
    combined_sessions.extend(sdk_stats.recent_sessions.clone());
    combined_sessions.sort_by(|a, b| b.apm.partial_cmp(&a.apm).unwrap_or(std::cmp::Ordering::Equal));
    combined_sessions.truncate(20); // Keep top 20
    
    // Combine productivity by time (average weighted by session count)
    let combine_productivity = |cli_prod: f64, sdk_prod: f64| -> f64 {
        let cli_sessions = cli_stats.total_sessions as f64;
        let sdk_sessions = sdk_stats.total_sessions as f64;
        let total_sessions = cli_sessions + sdk_sessions;
        
        if total_sessions == 0.0 {
            return 0.0;
        }
        
        (cli_prod * cli_sessions + sdk_prod * sdk_sessions) / total_sessions
    };
    
    let combined_apm_1h = combine_apm(cli_stats.apm_1h, sdk_stats.apm_1h);
    let combined_total_sessions = (cli_stats.total_sessions as u32) + (sdk_stats.total_sessions as u32);
    let combined_total_messages = (cli_stats.total_messages as u32) + (sdk_stats.total_messages as u32);
    let combined_total_tools = (cli_stats.total_tool_uses as u32) + (sdk_stats.total_tool_uses as u32);
    
    info!("Combined result - Sessions: {}, Messages: {}, Tools: {}, 1h APM: {}", 
          combined_total_sessions, combined_total_messages, combined_total_tools, combined_apm_1h);

    CombinedAPMStats {
        apm_1h: combined_apm_1h,
        apm_6h: combine_apm(cli_stats.apm_6h, sdk_stats.apm_6h),
        apm_1d: combine_apm(cli_stats.apm_1d, sdk_stats.apm_1d),
        apm_1w: combine_apm(cli_stats.apm_1w, sdk_stats.apm_1w),
        apm_1m: combine_apm(cli_stats.apm_1m, sdk_stats.apm_1m),
        apm_lifetime: combine_apm(cli_stats.apm_lifetime, sdk_stats.apm_lifetime),
        total_sessions: combined_total_sessions,
        total_messages: combined_total_messages,
        total_tool_uses: combined_total_tools,
        total_duration: cli_stats.total_duration + sdk_stats.total_duration,
        tool_usage: combined_tool_usage,
        recent_sessions: combined_sessions,
        productivity_by_time: ProductivityByTime {
            morning: combine_productivity(cli_stats.productivity_by_time.morning, sdk_stats.productivity_by_time.morning),
            afternoon: combine_productivity(cli_stats.productivity_by_time.afternoon, sdk_stats.productivity_by_time.afternoon),
            evening: combine_productivity(cli_stats.productivity_by_time.evening, sdk_stats.productivity_by_time.evening),
            night: combine_productivity(cli_stats.productivity_by_time.night, sdk_stats.productivity_by_time.night),
        },
        cli_stats,
        sdk_stats,
    }
}

// Historical APM data generation
async fn generate_historical_apm_data(
    time_scale: &str,
    days_back: i64,
    view_mode: &str,
) -> Result<HistoricalAPMResponse, String> {
    info!("Generating historical APM data: time_scale={}, days_back={}, view_mode={}", time_scale, days_back, view_mode);
    
    use chrono::{Utc, Duration, Datelike, NaiveDate};
    
    let home_dir = dirs_next::home_dir()
        .ok_or("Could not find home directory")?;
    
    let claude_dir = home_dir.join(".claude").join("projects");
    let mut cli_sessions_by_period: std::collections::HashMap<String, Vec<APMSession>> = std::collections::HashMap::new();
    
    // Load and group CLI sessions by time period
    if claude_dir.exists() {
        let entries = std::fs::read_dir(&claude_dir)
            .map_err(|e| format!("Failed to read Claude directory: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            
            if path.is_dir() {
                let conv_files = std::fs::read_dir(&path)
                    .map_err(|e| format!("Failed to read conversation directory: {}", e))?;
                
                for file_entry in conv_files {
                    let file_entry = file_entry.map_err(|e| format!("Failed to read file entry: {}", e))?;
                    let file_path = file_entry.path();
                    
                    if file_path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                        if let Ok(sessions) = parse_conversation_for_historical(&file_path, time_scale, days_back).await {
                            for (period, session) in sessions {
                                cli_sessions_by_period.entry(period).or_insert_with(Vec::new).push(session);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Load SDK sessions (simplified for now - we'll enhance this later)
    let sdk_sessions_by_period: std::collections::HashMap<String, Vec<APMSession>> = std::collections::HashMap::new();
    // TODO: Add SDK historical data loading from Convex
    
    // Generate data points for each time period
    let mut data_points = Vec::new();
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(days_back);
    
    // Generate periods based on time scale
    match time_scale {
        "daily" => {
            for i in 0..days_back {
                let date = start_date + Duration::days(i);
                let period = date.format("%Y-%m-%d").to_string();
                
                let empty_vec = Vec::new();
                let cli_sessions = cli_sessions_by_period.get(&period).unwrap_or(&empty_vec);
                let sdk_sessions = sdk_sessions_by_period.get(&period).unwrap_or(&empty_vec);
                
                data_points.push(create_historical_data_point(period, cli_sessions, sdk_sessions));
            }
        }
        "weekly" => {
            let weeks_back = (days_back / 7).max(1);
            for i in 0..weeks_back {
                let week_start = start_date + Duration::weeks(i);
                let year = week_start.year();
                let week = week_start.iso_week().week();
                let period = format!("{}-W{:02}", year, week);
                
                // Aggregate all sessions for this week
                let mut cli_week_sessions = Vec::new();
                let mut sdk_week_sessions = Vec::new();
                
                for day in 0..7 {
                    let date = week_start + Duration::days(day);
                    let day_period = date.format("%Y-%m-%d").to_string();
                    
                    if let Some(cli_sessions) = cli_sessions_by_period.get(&day_period) {
                        cli_week_sessions.extend(cli_sessions.iter().cloned());
                    }
                    if let Some(sdk_sessions) = sdk_sessions_by_period.get(&day_period) {
                        sdk_week_sessions.extend(sdk_sessions.iter().cloned());
                    }
                }
                
                data_points.push(create_historical_data_point(period, &cli_week_sessions, &sdk_week_sessions));
            }
        }
        "monthly" => {
            
            // Calculate actual months instead of approximating with 30 days
            let mut current_date = start_date.date_naive();
            let end_date = end_date.date_naive();
            
            while current_date <= end_date {
                let period = current_date.format("%Y-%m").to_string();
                
                // Get the actual number of days in this month
                let year = current_date.year();
                let month = current_date.month();
                let days_in_month = if month == 12 {
                    NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap().pred_opt().unwrap().day()
                } else {
                    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap().pred_opt().unwrap().day()
                };
                
                // Aggregate all sessions for this month
                let mut cli_month_sessions = Vec::new();
                let mut sdk_month_sessions = Vec::new();
                
                for day in 1..=days_in_month {
                    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                        let day_period = date.format("%Y-%m-%d").to_string();
                        
                        if let Some(cli_sessions) = cli_sessions_by_period.get(&day_period) {
                            cli_month_sessions.extend(cli_sessions.iter().cloned());
                        }
                        if let Some(sdk_sessions) = sdk_sessions_by_period.get(&day_period) {
                            sdk_month_sessions.extend(sdk_sessions.iter().cloned());
                        }
                    }
                }
                
                data_points.push(create_historical_data_point(period, &cli_month_sessions, &sdk_month_sessions));
                
                // Move to next month
                current_date = if month == 12 {
                    NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
                };
            }
        }
        _ => return Err(format!("Invalid time scale: {}", time_scale)),
    }
    
    info!("Generated {} historical data points", data_points.len());
    
    Ok(HistoricalAPMResponse {
        data: data_points,
        time_scale: time_scale.to_string(),
        date_range: (
            start_date.format("%Y-%m-%d").to_string(),
            end_date.format("%Y-%m-%d").to_string(),
        ),
        view_mode: view_mode.to_string(),
    })
}

// Helper function to calculate APM for a period
fn calculate_period_apm(sessions: &[APMSession]) -> f64 {
    if sessions.is_empty() {
        return 0.0;
    }
    
    let total_actions: f64 = sessions.iter().map(|s| s.message_count + s.tool_count).sum();
    let total_duration: f64 = sessions.iter().map(|s| s.duration).sum();
    
    if total_duration > 0.0 {
        total_actions / total_duration
    } else {
        0.0
    }
}

// Helper function to create a data point from session collections (reduces duplication)
fn create_historical_data_point(
    period: String,
    cli_sessions: &[APMSession],
    sdk_sessions: &[APMSession],
) -> HistoricalAPMDataPoint {
    let cli_apm = calculate_period_apm(cli_sessions);
    let sdk_apm = calculate_period_apm(sdk_sessions);
    
    let all_sessions: Vec<_> = cli_sessions.iter().chain(sdk_sessions.iter()).collect();
    let average_session_duration = if all_sessions.is_empty() {
        0.0
    } else {
        all_sessions.iter().map(|s| s.duration).sum::<f64>() / all_sessions.len() as f64
    };
    
    HistoricalAPMDataPoint {
        period,
        cli_apm,
        sdk_apm,
        combined_apm: cli_apm + sdk_apm,
        total_sessions: (cli_sessions.len() + sdk_sessions.len()) as u32,
        total_messages: (cli_sessions.iter().map(|s| s.message_count).sum::<f64>() + 
                        sdk_sessions.iter().map(|s| s.message_count).sum::<f64>()) as u32,
        total_tools: (cli_sessions.iter().map(|s| s.tool_count).sum::<f64>() + 
                     sdk_sessions.iter().map(|s| s.tool_count).sum::<f64>()) as u32,
        average_session_duration,
    }
}

// Helper function to parse conversation files for historical data
async fn parse_conversation_for_historical(
    file_path: &std::path::Path,
    time_scale: &str,
    days_back: i64,
) -> Result<Vec<(String, APMSession)>, String> {
    use chrono::{DateTime, Utc, Duration, Datelike};
    
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    
    let mut messages = 0;
    let mut tools = 0;
    let mut timestamps = Vec::new();
    let mut project_name = "Unknown".to_string();
    
    for line in &lines {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(ts_str) = json.get("timestamp").and_then(|v| v.as_str()) {
                if let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) {
                    timestamps.push(ts.with_timezone(&Utc));
                }
            }
            
            if let Some(msg) = json.get("message").and_then(|v| v.as_object()) {
                if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
                    messages += 1;
                }
            }
            
            if json.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                tools += 1;
            }
            
            if let Some(cwd) = json.get("cwd").and_then(|v| v.as_str()) {
                project_name = cwd.split('/').last().unwrap_or("Unknown").to_string();
            }
        }
    }
    
    if timestamps.is_empty() {
        return Ok(Vec::new());
    }
    
    let earliest = timestamps.iter().min().unwrap();
    let latest = timestamps.iter().max().unwrap();
    let cutoff_date = Utc::now() - Duration::days(days_back);
    
    // Only include sessions within our time range
    if latest < &cutoff_date {
        return Ok(Vec::new());
    }
    
    let duration = (*latest - *earliest).num_minutes() as f64;
    if duration <= 0.0 {
        return Ok(Vec::new());
    }
    
    let apm = calculate_apm(messages, tools, duration);
    
    let session = APMSession {
        id: file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        project: project_name,
        apm,
        duration,
        message_count: messages as f64,
        tool_count: tools as f64,
        timestamp: latest.to_rfc3339(),
    };
    
    // Determine which period this session belongs to
    let period = match time_scale {
        "daily" => latest.format("%Y-%m-%d").to_string(),
        "weekly" => {
            let year = latest.year();
            let week = latest.iso_week().week();
            format!("{}-W{:02}", year, week)
        }
        "monthly" => latest.format("%Y-%m").to_string(),
        _ => latest.format("%Y-%m-%d").to_string(),
    };
    
    Ok(vec![(period, session)])
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
    let mut earliest_timestamp: Option<DateTime<Utc>> = None;
    let mut latest_timestamp: Option<DateTime<Utc>> = None;
    
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
        
        // Track earliest and latest timestamps for all-time APM
        if earliest_timestamp.is_none() || *start_time < earliest_timestamp.unwrap() {
            earliest_timestamp = Some(*start_time);
        }
        if latest_timestamp.is_none() || *end_time > latest_timestamp.unwrap() {
            latest_timestamp = Some(*end_time);
        }
        
        apm_sessions.push(APMSession {
            id: session_id,
            project: project_name,
            apm: session_apm,
            duration,
            message_count: message_count as f64,
            tool_count: tool_count as f64,
            timestamp: start_time.to_rfc3339(),
        });
        
        total_messages += message_count;
        total_tools += tool_count;
        total_duration += duration;
    }
    
    // Calculate overall metrics
    let total_sessions = apm_sessions.len() as u32;
    
    // Calculate APM for different time windows
    let now = Utc::now();
    
    // Helper function to calculate APM for a time window
    let calculate_window_apm = |hours_back: i64, total_minutes: f64| -> f64 {
        let cutoff_time = now - chrono::Duration::hours(hours_back);
        let mut window_messages = 0u32;
        let mut window_tools = 0u32;
        
        for session in &apm_sessions {
            if let Ok(session_time) = DateTime::parse_from_rfc3339(&session.timestamp) {
                let session_utc = session_time.with_timezone(&Utc);
                if session_utc >= cutoff_time {
                    window_messages += session.message_count as u32;
                    window_tools += session.tool_count as u32;
                }
            }
        }
        
        (window_messages as f64 + window_tools as f64) / total_minutes
    };
    
    // Calculate APM for each time window
    let apm_1h = calculate_window_apm(1, 60.0);      // 1 hour = 60 minutes
    let apm_6h = calculate_window_apm(6, 360.0);     // 6 hours = 360 minutes  
    let apm_1d = calculate_window_apm(24, 1440.0);   // 1 day = 1440 minutes
    let apm_1w = calculate_window_apm(168, 10080.0); // 1 week = 10080 minutes
    let apm_1m = calculate_window_apm(720, 43200.0); // 30 days = 43200 minutes
    
    // Lifetime APM: counts total calendar time from first to last conversation
    let apm_lifetime = if let (Some(earliest), Some(latest)) = (earliest_timestamp, latest_timestamp) {
        let total_calendar_minutes = (latest - earliest).num_seconds() as f64 / 60.0;
        if total_calendar_minutes > 0.0 {
            (total_messages as f64 + total_tools as f64) / total_calendar_minutes
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    // Process tool usage statistics
    let total_tool_uses_for_percentage = total_tools as f64;
    let mut tool_usage: Vec<ToolUsage> = tool_counts
        .into_iter()
        .map(|(name, count)| ToolUsage {
            category: get_tool_category(&name),
            name,
            count: count as f64,
            percentage: if total_tool_uses_for_percentage > 0.0 {
                (count as f64 / total_tool_uses_for_percentage) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    
    tool_usage.sort_by(|a, b| b.count.partial_cmp(&a.count).unwrap_or(std::cmp::Ordering::Equal));
    
    // Calculate productivity by time of day
    let productivity_by_time = ProductivityByTime {
        morning: productivity_by_hour[0].iter().sum::<f64>() / productivity_by_hour[0].len().max(1) as f64,
        afternoon: productivity_by_hour[1].iter().sum::<f64>() / productivity_by_hour[1].len().max(1) as f64,
        evening: productivity_by_hour[2].iter().sum::<f64>() / productivity_by_hour[2].len().max(1) as f64,
        night: productivity_by_hour[3].iter().sum::<f64>() / productivity_by_hour[3].len().max(1) as f64,
    };
    
    Ok(APMStats {
        apm_1h,
        apm_6h,
        apm_1d,
        apm_1w,
        apm_1m,
        apm_lifetime,
        total_sessions: total_sessions as f64,
        total_messages: total_messages as f64,
        total_tool_uses: total_tools as f64,
        total_duration,
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
            info!("APM analysis completed successfully. 1h: {:.1}, 1d: {:.1}, Lifetime: {:.1}, Sessions: {}", 
                  stats.apm_1h, stats.apm_1d, stats.apm_lifetime, stats.total_sessions);
            Ok(CommandResult::success(stats))
        }
        Err(e) => {
            info!("APM analysis failed: {}", e);
            Ok(CommandResult::error(e))
        }
    }
}

#[tauri::command]
async fn analyze_combined_conversations() -> Result<CommandResult<CombinedAPMStats>, String> {
    info!("analyze_combined_conversations called");
    
    // Fetch CLI stats
    let cli_stats = match analyze_conversations().await {
        Ok(stats) => {
            info!("CLI APM analysis completed. Sessions: {}, Messages: {}, Tools: {}", 
                  stats.total_sessions, stats.total_messages, stats.total_tool_uses);
            stats
        }
        Err(e) => {
            info!("CLI APM analysis failed, using empty stats: {}", e);
            // Return empty stats if CLI analysis fails
            APMStats {
                apm_1h: 0.0,
                apm_6h: 0.0,
                apm_1d: 0.0,
                apm_1w: 0.0,
                apm_1m: 0.0,
                apm_lifetime: 0.0,
                total_sessions: 0.0,
                total_messages: 0.0,
                total_tool_uses: 0.0,
                total_duration: 0.0,
                tool_usage: Vec::new(),
                recent_sessions: Vec::new(),
                productivity_by_time: ProductivityByTime {
                    morning: 0.0,
                    afternoon: 0.0,
                    evening: 0.0,
                    night: 0.0,
                },
            }
        }
    };
    
    // Fetch SDK stats from Convex
    let sdk_stats = match fetch_convex_apm_stats().await {
        Ok(stats) => {
            info!("SDK APM analysis completed. Sessions: {}, Messages: {}, Tools: {}", 
                  stats.total_sessions, stats.total_messages, stats.total_tool_uses);
            stats
        }
        Err(e) => {
            info!("SDK APM analysis failed, using empty stats: {}", e);
            // Return empty stats if SDK analysis fails
            APMStats {
                apm_1h: 0.0,
                apm_6h: 0.0,
                apm_1d: 0.0,
                apm_1w: 0.0,
                apm_1m: 0.0,
                apm_lifetime: 0.0,
                total_sessions: 0.0,
                total_messages: 0.0,
                total_tool_uses: 0.0,
                total_duration: 0.0,
                tool_usage: Vec::new(),
                recent_sessions: Vec::new(),
                productivity_by_time: ProductivityByTime {
                    morning: 0.0,
                    afternoon: 0.0,
                    evening: 0.0,
                    night: 0.0,
                },
            }
        }
    };
    
    // Combine the stats
    let combined_stats = combine_apm_stats(cli_stats, sdk_stats);
    
    info!("Combined APM analysis completed. Total sessions: {}, Total messages: {}, Total tools: {}", 
          combined_stats.total_sessions, combined_stats.total_messages, combined_stats.total_tool_uses);
    
    Ok(CommandResult::success(combined_stats))
}

#[tauri::command]
async fn get_historical_apm_data(
    time_scale: String,
    days_back: Option<i64>,
    view_mode: Option<String>,
) -> Result<CommandResult<HistoricalAPMResponse>, String> {
    info!("get_historical_apm_data called with params:");
    info!("  time_scale: {}", time_scale);
    info!("  days_back: {:?}", days_back);
    info!("  view_mode: {:?}", view_mode);
    
    // Set defaults
    let days_back = days_back.unwrap_or(match time_scale.as_str() {
        "daily" => 30,     // Last 30 days
        "weekly" => 84,    // Last 12 weeks 
        "monthly" => 365,  // Last 12 months
        _ => 30,
    });
    
    let view_mode = view_mode.unwrap_or_else(|| "combined".to_string());
    
    // Validate parameters
    if !["daily", "weekly", "monthly"].contains(&time_scale.as_str()) {
        return Ok(CommandResult::error("Invalid time_scale. Must be 'daily', 'weekly', or 'monthly'".to_string()));
    }
    
    if !["combined", "cli", "sdk"].contains(&view_mode.as_str()) {
        return Ok(CommandResult::error("Invalid view_mode. Must be 'combined', 'cli', or 'sdk'".to_string()));
    }
    
    if days_back <= 0 || days_back > 365 {
        return Ok(CommandResult::error("days_back must be between 1 and 365".to_string()));
    }
    
    match generate_historical_apm_data(&time_scale, days_back, &view_mode).await {
        Ok(data) => {
            info!("Historical APM data generated successfully: {} data points", data.data.len());
            Ok(CommandResult::success(data))
        }
        Err(e) => {
            info!("Historical APM data generation failed: {}", e);
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
async fn trigger_claude_response(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<CommandResult<()>, String> {
    info!("trigger_claude_response called - session_id: {}, message: {}", session_id, message);
    info!("🚨 [RUST] This command triggers Claude WITHOUT creating a user message!");
    
    let manager_lock = state.manager.lock().await;
    
    if let Some(ref manager) = *manager_lock {
        info!("Manager found, triggering Claude response...");
        // This will call a new method that doesn't create a user message
        match manager.trigger_response(&session_id, message).await {
            Ok(_) => {
                info!("Claude response triggered successfully");
                Ok(CommandResult::success(()))
            },
            Err(e) => {
                info!("Error triggering Claude response: {}", e);
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
async fn get_unified_history(
    limit: usize,
    user_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResult<Vec<UnifiedSession>>, String> {
    info!("get_unified_history called with params:");
    info!("  limit: {}", limit);
    info!("  user_id: {:?}", user_id);
    
    let mut discovery = state.discovery.lock().await;
    
    match discovery.load_unified_sessions(limit, user_id).await {
        Ok(sessions) => Ok(CommandResult::success(sessions)),
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
