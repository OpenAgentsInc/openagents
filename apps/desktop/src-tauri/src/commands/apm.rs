//! APM-related Tauri commands

use log::info;
use crate::error::CommandResult;
use crate::apm::{
    APMStats, CombinedAPMStats, HistoricalAPMResponse, APMAnalyzer,
    generate_historical_apm_data, combine_apm_stats, fetch_convex_apm_stats
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AggregatedAPMStats {
    pub apm1h: f64,
    pub apm6h: f64,
    pub apm1d: f64,
    pub apm1w: f64,
    pub apm1m: f64,
    #[serde(rename = "apmLifetime")]
    pub apm_lifetime: f64,
    #[serde(rename = "totalActions")]
    pub total_actions: i32,
    #[serde(rename = "activeMinutes")]
    pub active_minutes: f64,
    #[serde(rename = "deviceBreakdown")]
    pub device_breakdown: Option<DeviceBreakdown>,
    pub metadata: Option<AggregatedMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceBreakdown {
    pub desktop: Option<f64>,
    pub mobile: Option<f64>,
    pub github: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AggregatedMetadata {
    #[serde(rename = "overlappingMinutes")]
    pub overlapping_minutes: Option<f64>,
    #[serde(rename = "peakConcurrency")]
    pub peak_concurrency: Option<i32>,
}

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
    
    if days_back <= 0 || days_back > 1825 {
        return Ok(CommandResult::error("daysBack must be between 1 and 1825 (5 years)".to_string()));
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

#[tauri::command]
pub async fn get_user_apm_stats() -> Result<CommandResult<AggregatedAPMStats>, String> {
    info!("get_user_apm_stats called");
    
    // For now, return mock data since we need to integrate with the Convex client
    // In a real implementation, this would call the getUserAPMStats Convex function
    
    // TODO: Integrate with EnhancedConvexClient to call getUserAPMStats
    let mock_stats = AggregatedAPMStats {
        apm1h: 2.5,
        apm6h: 1.8,
        apm1d: 1.2,
        apm1w: 0.9,
        apm1m: 0.6,
        apm_lifetime: 0.4,
        total_actions: 1250,
        active_minutes: 3125.0,
        device_breakdown: Some(DeviceBreakdown {
            desktop: Some(0.3),
            mobile: Some(0.1),
            github: Some(0.05),
        }),
        metadata: Some(AggregatedMetadata {
            overlapping_minutes: Some(45.0),
            peak_concurrency: Some(2),
        }),
    };
    
    info!("Returning mock aggregated APM stats - TODO: integrate with Convex");
    Ok(CommandResult::success(mock_stats))
}