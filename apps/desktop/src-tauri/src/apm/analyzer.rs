use std::collections::HashMap;
use std::fs;
use chrono::{DateTime, Utc, Timelike};
use log::info;

use crate::error::{AppError, AppResult};
use super::models::*;
use super::utils::{calculate_apm, clean_project_name, get_tool_category};

/// APM Analyzer for processing Claude conversation data
pub struct APMAnalyzer;

impl APMAnalyzer {
    /// Create a new APM analyzer instance
    pub fn new() -> Self {
        Self
    }

    /// Analyze Claude conversations and generate APM statistics
    pub async fn analyze_conversations(&self) -> AppResult<APMStats> {
        let home_dir = dirs_next::home_dir()
            .ok_or_else(|| AppError::ApmError("Could not find home directory".to_string()))?;
        
        let claude_dir = home_dir.join(".claude").join("projects");
        
        if !claude_dir.exists() {
            return Err(AppError::ApmError(
                "Claude projects directory not found. Please ensure Claude Code is installed and has been used.".to_string()
            ));
        }
        
        let mut all_sessions: HashMap<String, (Vec<ConversationEntry>, String)> = HashMap::new();
        let mut tool_counts: HashMap<String, u32> = HashMap::new();
        
        // Scan for project directories
        let project_dirs = fs::read_dir(&claude_dir)
            .map_err(|e| AppError::ApmError(format!("Failed to read projects directory: {}", e)))?;
        
        for project_entry in project_dirs {
            let project_entry = project_entry
                .map_err(|e| AppError::ApmError(format!("Failed to read project entry: {}", e)))?;
            
            if !project_entry.file_type()
                .map_err(|e| AppError::ApmError(format!("Failed to get file type: {}", e)))?
                .is_dir() {
                continue;
            }
            
            let project_name = project_entry.file_name().to_string_lossy().to_string();
            let cleaned_project_name = clean_project_name(&project_name);
            
            // Look for .jsonl files in this project directory
            let jsonl_files = fs::read_dir(project_entry.path())
                .map_err(|e| AppError::ApmError(format!("Failed to read project directory {}: {}", project_name, e)))?;
            
            for file_entry in jsonl_files {
                let file_entry = file_entry
                    .map_err(|e| AppError::ApmError(format!("Failed to read file entry: {}", e)))?;
                
                let file_path = file_entry.path();
                if file_path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                
                // Read and parse JSONL file
                let content = fs::read_to_string(&file_path)
                    .map_err(|e| AppError::ApmError(format!("Failed to read file {}: {}", file_path.display(), e)))?;
                
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
            return Err(AppError::ApmError(
                "No conversation data found. Please ensure you have used Claude Code to create some conversations.".to_string()
            ));
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
}

impl Default for APMAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_analyzer_creation() {
        let analyzer = APMAnalyzer::new();
        // Just verify it creates without panic
        let _ = analyzer;
    }
}