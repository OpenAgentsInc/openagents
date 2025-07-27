use std::collections::HashMap;
use chrono::{Utc, Duration, Datelike, NaiveDate};
use log::info;

use crate::error::{AppError, AppResult};
use super::models::{APMSession, HistoricalAPMDataPoint, HistoricalAPMResponse};
use super::utils::calculate_apm;

/// Generate historical APM data for visualization
pub async fn generate_historical_apm_data(
    time_scale: &str,
    days_back: i64,
    view_mode: &str,
) -> AppResult<HistoricalAPMResponse> {
    info!("Generating historical APM data: time_scale={}, days_back={}, view_mode={}", time_scale, days_back, view_mode);
    
    let home_dir = dirs_next::home_dir()
        .ok_or_else(|| AppError::ApmError("Could not find home directory".to_string()))?;
    
    let claude_dir = home_dir.join(".claude").join("projects");
    let mut cli_sessions_by_period: HashMap<String, Vec<APMSession>> = HashMap::new();
    
    // Load and group CLI sessions by time period
    if claude_dir.exists() {
        let entries = std::fs::read_dir(&claude_dir)
            .map_err(|e| AppError::ApmError(format!("Failed to read Claude directory: {}", e)))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| AppError::ApmError(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_dir() {
                let conv_files = std::fs::read_dir(&path)
                    .map_err(|e| AppError::ApmError(format!("Failed to read conversation directory: {}", e)))?;
                
                for file_entry in conv_files {
                    let file_entry = file_entry.map_err(|e| AppError::ApmError(format!("Failed to read file entry: {}", e)))?;
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
    let sdk_sessions_by_period: HashMap<String, Vec<APMSession>> = HashMap::new();
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
        _ => return Err(AppError::ValidationError(format!("Invalid time scale: {}", time_scale))),
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

/// Helper function to calculate APM for a period
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

/// Helper function to create a data point from session collections
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

/// Helper function to parse conversation files for historical data
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
                project_name = std::path::Path::new(cwd)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_period_apm() {
        let sessions = vec![
            APMSession {
                id: "1".to_string(),
                project: "test".to_string(),
                apm: 15.0,
                duration: 10.0,
                message_count: 100.0,
                tool_count: 50.0,
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            APMSession {
                id: "2".to_string(),
                project: "test".to_string(),
                apm: 20.0,
                duration: 5.0,
                message_count: 75.0,
                tool_count: 25.0,
                timestamp: "2024-01-01T01:00:00Z".to_string(),
            },
        ];
        
        let apm = calculate_period_apm(&sessions);
        // Total actions: 100 + 50 + 75 + 25 = 250
        // Total duration: 10 + 5 = 15
        // APM: 250 / 15 = 16.67
        assert!((apm - 16.67).abs() < 0.01);
    }

    #[test]
    fn test_create_historical_data_point() {
        let cli_sessions = vec![
            APMSession {
                id: "1".to_string(),
                project: "test".to_string(),
                apm: 15.0,
                duration: 10.0,
                message_count: 100.0,
                tool_count: 50.0,
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
        ];
        
        let sdk_sessions = vec![
            APMSession {
                id: "2".to_string(),
                project: "test".to_string(),
                apm: 20.0,
                duration: 5.0,
                message_count: 75.0,
                tool_count: 25.0,
                timestamp: "2024-01-01T01:00:00Z".to_string(),
            },
        ];
        
        let data_point = create_historical_data_point("2024-01-01".to_string(), &cli_sessions, &sdk_sessions);
        
        assert_eq!(data_point.period, "2024-01-01");
        assert_eq!(data_point.total_sessions, 2);
        assert_eq!(data_point.total_messages, 175);
        assert_eq!(data_point.total_tools, 75);
        assert_eq!(data_point.average_session_duration, 7.5);
    }
}