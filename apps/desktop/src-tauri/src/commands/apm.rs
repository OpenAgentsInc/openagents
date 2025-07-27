//! APM-related Tauri commands

use log::info;
use crate::error::CommandResult;
use crate::apm::{
    APMStats, CombinedAPMStats, HistoricalAPMResponse, APMAnalyzer,
    generate_historical_apm_data, combine_apm_stats, fetch_convex_apm_stats
};

#[tauri::command]
pub async fn analyze_claude_conversations() -> Result<CommandResult<APMStats>, String> {
    info!("analyze_claude_conversations called");
    
    let analyzer = APMAnalyzer::new();
    match analyzer.analyze_conversations().await {
        Ok(stats) => {
            info!("APM analysis completed successfully. 1h: {:.1}, 1d: {:.1}, Lifetime: {:.1}, Sessions: {}", 
                  stats.apm_1h, stats.apm_1d, stats.apm_lifetime, stats.total_sessions);
            Ok(CommandResult::success(stats))
        }
        Err(e) => {
            info!("APM analysis failed: {}", e);
            Ok(CommandResult::error(e.to_string()))
        }
    }
}

#[tauri::command]
pub async fn analyze_combined_conversations() -> Result<CommandResult<CombinedAPMStats>, String> {
    info!("analyze_combined_conversations called");
    
    // Fetch CLI stats
    let analyzer = APMAnalyzer::new();
    let cli_stats = match analyzer.analyze_conversations().await {
        Ok(stats) => {
            info!("CLI APM analysis completed. Sessions: {}, Messages: {}, Tools: {}", 
                  stats.total_sessions, stats.total_messages, stats.total_tool_uses);
            stats
        }
        Err(e) => {
            info!("CLI APM analysis failed, using empty stats: {}", e);
            // Return empty stats if CLI analysis fails
            APMStats::default()
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
            APMStats::default()
        }
    };
    
    // Combine the stats
    let combined_stats = combine_apm_stats(cli_stats, sdk_stats);
    
    info!("Combined APM analysis completed. Total sessions: {}, Total messages: {}, Total tools: {}", 
          combined_stats.total_sessions, combined_stats.total_messages, combined_stats.total_tool_uses);
    
    Ok(CommandResult::success(combined_stats))
}

#[tauri::command]
pub async fn get_historical_apm_data(
    time_scale: String,
    days_back: Option<i64>,
    view_mode: Option<String>,
) -> Result<CommandResult<HistoricalAPMResponse>, String> {
    info!("get_historical_apm_data called with params:");
    info!("  timeScale: {}", time_scale);
    info!("  daysBack: {:?}", days_back);
    info!("  viewMode: {:?}", view_mode);
    
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
        return Ok(CommandResult::error("Invalid timeScale. Must be 'daily', 'weekly', or 'monthly'".to_string()));
    }
    
    if !["combined", "cli", "sdk"].contains(&view_mode.as_str()) {
        return Ok(CommandResult::error("Invalid viewMode. Must be 'combined', 'cli', or 'sdk'".to_string()));
    }
    
    if days_back <= 0 || days_back > 365 {
        return Ok(CommandResult::error("daysBack must be between 1 and 365".to_string()));
    }
    
    match generate_historical_apm_data(&time_scale, days_back, &view_mode).await {
        Ok(data) => {
            info!("Historical APM data generated successfully: {} data points", data.data.len());
            Ok(CommandResult::success(data))
        }
        Err(e) => {
            info!("Historical APM data generation failed: {}", e);
            Ok(CommandResult::error(e.to_string()))
        }
    }
}