use std::collections::HashMap;
use std::env;
use log::info;

use crate::error::{AppError, AppResult};
use crate::claude_code::EnhancedConvexClient;
use crate::claude_code::database::ConvexDatabase;
use super::models::{APMStats, CombinedAPMStats, ToolUsage, ProductivityByTime};
use super::utils::get_tool_category;

/// Fetch APM stats from Convex backend using native client
pub async fn fetch_convex_apm_stats() -> AppResult<APMStats> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    
    let convex_url = env::var("VITE_CONVEX_URL")
        .or_else(|_| env::var("CONVEX_URL"))
        .map_err(|_| AppError::ConfigError(
            "Convex URL not configured. Set VITE_CONVEX_URL or CONVEX_URL environment variable.".to_string()
        ))?;
    
    info!("Fetching Convex APM stats from: {} using native client", convex_url);
    
    // Create enhanced Convex client
    let mut client = EnhancedConvexClient::new(&convex_url, None).await
        .map_err(|e| AppError::ConvexConnectionError(format!("Failed to create Convex client: {}", e)))?;
    
    // Call the Convex function directly using the native client
    let args = serde_json::json!({});
    let apm_stats: APMStats = client.query("claude:getConvexAPMStats", args).await
        .map_err(|e| AppError::ConvexDatabaseError(format!("Failed to fetch APM stats: {}", e)))?;
    
    info!("Successfully fetched APM stats using native Convex client");
    
    Ok(apm_stats)
}

/// Combine CLI and SDK APM statistics
pub fn combine_apm_stats(cli_stats: APMStats, sdk_stats: APMStats) -> CombinedAPMStats {
    info!("Combining APM stats:");
    info!("CLI - Sessions: {}, Messages: {}, Tools: {}, 1h APM: {}", 
          cli_stats.total_sessions, cli_stats.total_messages, cli_stats.total_tool_uses, cli_stats.apm_1h);
    info!("SDK - Sessions: {}, Messages: {}, Tools: {}, 1h APM: {}", 
          sdk_stats.total_sessions, sdk_stats.total_messages, sdk_stats.total_tool_uses, sdk_stats.apm_1h);
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apm::models::APMSession;

    #[test]
    fn test_combine_apm_stats() {
        let cli_stats = APMStats {
            apm_1h: 10.0,
            apm_6h: 9.0,
            apm_1d: 8.0,
            apm_1w: 7.0,
            apm_1m: 6.0,
            apm_lifetime: 5.0,
            total_sessions: 100.0,
            total_messages: 1000.0,
            total_tool_uses: 500.0,
            total_duration: 5000.0,
            tool_usage: vec![
                ToolUsage {
                    name: "Edit".to_string(),
                    count: 200.0,
                    percentage: 40.0,
                    category: "Code Generation".to_string(),
                },
            ],
            recent_sessions: vec![
                APMSession {
                    id: "cli-1".to_string(),
                    project: "test".to_string(),
                    apm: 15.0,
                    duration: 10.0,
                    message_count: 50.0,
                    tool_count: 25.0,
                    timestamp: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
            productivity_by_time: ProductivityByTime {
                morning: 12.0,
                afternoon: 15.0,
                evening: 10.0,
                night: 5.0,
            },
        };

        let sdk_stats = APMStats {
            apm_1h: 5.0,
            apm_6h: 4.0,
            apm_1d: 3.0,
            apm_1w: 2.0,
            apm_1m: 1.0,
            apm_lifetime: 0.5,
            total_sessions: 50.0,
            total_messages: 500.0,
            total_tool_uses: 250.0,
            total_duration: 2500.0,
            tool_usage: vec![
                ToolUsage {
                    name: "Edit".to_string(),
                    count: 100.0,
                    percentage: 40.0,
                    category: "Code Generation".to_string(),
                },
                ToolUsage {
                    name: "Read".to_string(),
                    count: 150.0,
                    percentage: 60.0,
                    category: "File Operations".to_string(),
                },
            ],
            recent_sessions: vec![
                APMSession {
                    id: "sdk-1".to_string(),
                    project: "test".to_string(),
                    apm: 20.0,
                    duration: 5.0,
                    message_count: 30.0,
                    tool_count: 15.0,
                    timestamp: "2024-01-01T01:00:00Z".to_string(),
                },
            ],
            productivity_by_time: ProductivityByTime {
                morning: 8.0,
                afternoon: 10.0,
                evening: 6.0,
                night: 3.0,
            },
        };

        let combined = combine_apm_stats(cli_stats, sdk_stats);

        // Test combined APM values
        assert_eq!(combined.apm_1h, 15.0); // 10 + 5
        assert_eq!(combined.apm_6h, 13.0); // 9 + 4
        assert_eq!(combined.apm_1d, 11.0); // 8 + 3

        // Test combined totals
        assert_eq!(combined.total_sessions, 150); // 100 + 50
        assert_eq!(combined.total_messages, 1500); // 1000 + 500
        assert_eq!(combined.total_tool_uses, 750); // 500 + 250
        assert_eq!(combined.total_duration, 7500.0); // 5000 + 2500

        // Test tool usage combination
        assert_eq!(combined.tool_usage.len(), 2); // Edit and Read
        let edit_tool = combined.tool_usage.iter().find(|t| t.name == "Edit").unwrap();
        assert_eq!(edit_tool.count, 300.0); // 200 + 100

        // Test session combination
        assert_eq!(combined.recent_sessions.len(), 2);
        assert_eq!(combined.recent_sessions[0].apm, 20.0); // Sorted by APM

        // Test productivity combination (weighted average)
        // Morning: (12*100 + 8*50) / 150 = 10.67
        assert!((combined.productivity_by_time.morning - 10.67).abs() < 0.01);
    }
}