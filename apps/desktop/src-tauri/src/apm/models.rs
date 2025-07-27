use serde::{Deserialize, Serialize};

/// Tool usage statistics
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolUsage {
    pub name: String,
    pub count: f64, // Accept float from JavaScript, convert to int when needed
    pub percentage: f64,
    pub category: String,
}

/// APM session data
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct APMSession {
    pub id: String,
    pub project: String,
    pub apm: f64,
    pub duration: f64, // in minutes
    #[serde(rename = "messageCount")]
    pub message_count: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "toolCount")]
    pub tool_count: f64, // Accept float from JavaScript, convert to int when needed
    pub timestamp: String,
}

/// Productivity metrics by time of day
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProductivityByTime {
    pub morning: f64,
    pub afternoon: f64,
    pub evening: f64,
    pub night: f64,
}

/// Main APM statistics structure
#[derive(Serialize, Deserialize, Debug)]
pub struct APMStats {
    #[serde(rename = "apm1h")]
    pub apm_1h: f64,
    #[serde(rename = "apm6h")]
    pub apm_6h: f64,
    #[serde(rename = "apm1d")]
    pub apm_1d: f64,
    #[serde(rename = "apm1w")]
    pub apm_1w: f64,
    #[serde(rename = "apm1m")]
    pub apm_1m: f64,
    #[serde(rename = "apmLifetime")]
    pub apm_lifetime: f64,
    #[serde(rename = "totalSessions")]
    pub total_sessions: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalMessages")]
    pub total_messages: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalToolUses")]
    pub total_tool_uses: f64, // Accept float from JavaScript, convert to int when needed
    #[serde(rename = "totalDuration")]
    pub total_duration: f64, // in minutes
    #[serde(rename = "toolUsage")]
    pub tool_usage: Vec<ToolUsage>,
    #[serde(rename = "recentSessions")]
    pub recent_sessions: Vec<APMSession>,
    #[serde(rename = "productivityByTime")]
    pub productivity_by_time: ProductivityByTime,
}

/// Historical APM data point
#[derive(Clone, Debug, Serialize)]
pub struct HistoricalAPMDataPoint {
    pub period: String, // ISO date or week/month identifier (e.g., "2025-01-26", "2025-W04", "2025-01")
    pub cli_apm: f64,
    pub sdk_apm: f64,
    pub combined_apm: f64,
    pub total_sessions: u32,
    pub total_messages: u32,
    pub total_tools: u32,
    pub average_session_duration: f64,
}

/// Historical APM response
#[derive(Clone, Debug, Serialize)]
pub struct HistoricalAPMResponse {
    pub data: Vec<HistoricalAPMDataPoint>,
    pub time_scale: String, // "daily", "weekly", "monthly"
    pub date_range: (String, String), // (start_date, end_date)
    pub view_mode: String, // "combined", "cli", "sdk"
}

/// Combined APM statistics for CLI + SDK data
#[derive(Serialize, Deserialize, Debug)]
pub struct CombinedAPMStats {
    // Combined totals
    #[serde(rename = "apm1h")]
    pub apm_1h: f64,
    #[serde(rename = "apm6h")]
    pub apm_6h: f64,
    #[serde(rename = "apm1d")]
    pub apm_1d: f64,
    #[serde(rename = "apm1w")]
    pub apm_1w: f64,
    #[serde(rename = "apm1m")]
    pub apm_1m: f64,
    #[serde(rename = "apmLifetime")]
    pub apm_lifetime: f64,
    #[serde(rename = "totalSessions")]
    pub total_sessions: u32,
    #[serde(rename = "totalMessages")]
    pub total_messages: u32,
    #[serde(rename = "totalToolUses")]
    pub total_tool_uses: u32,
    #[serde(rename = "totalDuration")]
    pub total_duration: f64, // in minutes
    #[serde(rename = "toolUsage")]
    pub tool_usage: Vec<ToolUsage>,
    #[serde(rename = "recentSessions")]
    pub recent_sessions: Vec<APMSession>,
    #[serde(rename = "productivityByTime")]
    pub productivity_by_time: ProductivityByTime,
    
    // Breakdown by type
    #[serde(rename = "cliStats")]
    pub cli_stats: APMStats,
    #[serde(rename = "sdkStats")]
    pub sdk_stats: APMStats,
}

/// Conversation entry for parsing
#[derive(Deserialize, Debug)]
pub struct ConversationEntry {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub message_type: Option<String>,
    pub message: Option<serde_json::Value>,
}

/// Tool use entry
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct ToolUse {
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub name: Option<String>,
    pub input: Option<serde_json::Value>,
}

impl Default for ProductivityByTime {
    fn default() -> Self {
        Self {
            morning: 0.0,
            afternoon: 0.0,
            evening: 0.0,
            night: 0.0,
        }
    }
}

impl Default for APMStats {
    fn default() -> Self {
        Self {
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
            productivity_by_time: ProductivityByTime::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_stats_default() {
        let stats = APMStats::default();
        assert_eq!(stats.apm_1h, 0.0);
        assert_eq!(stats.total_sessions, 0.0);
        assert!(stats.tool_usage.is_empty());
        assert!(stats.recent_sessions.is_empty());
    }

    #[test]
    fn test_productivity_by_time_default() {
        let productivity = ProductivityByTime::default();
        assert_eq!(productivity.morning, 0.0);
        assert_eq!(productivity.afternoon, 0.0);
        assert_eq!(productivity.evening, 0.0);
        assert_eq!(productivity.night, 0.0);
    }

    #[test]
    fn test_tool_usage_serialization() {
        let tool = ToolUsage {
            name: "Edit".to_string(),
            count: 10.0,
            percentage: 25.5,
            category: "Code Generation".to_string(),
        };
        
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"name\":\"Edit\""));
        assert!(json.contains("\"category\":\"Code Generation\""));
    }
}