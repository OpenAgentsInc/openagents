//! Session data collection from autopilot metrics database

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Session information from metrics database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub prompt: String,
    pub duration_seconds: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub tokens_cached: i64,
    pub cost_usd: f64,
    pub issues_claimed: i64,
    pub issues_completed: i64,
    pub tool_calls: i64,
    pub tool_errors: i64,
    pub final_status: String,
}

/// Quick stats for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub sessions_today: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_duration: f64,
    pub avg_apm: f64,
}

/// Get recent sessions from metrics database
pub fn get_recent_sessions(db_path: &str, limit: i64) -> Result<Vec<SessionInfo>> {
    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, timestamp, model, prompt, duration_seconds,
                tokens_in, tokens_out, tokens_cached, cost_usd,
                issues_claimed, issues_completed, tool_calls, tool_errors, final_status
         FROM sessions
         ORDER BY timestamp DESC
         LIMIT ?1"
    )?;

    let sessions = stmt.query_map([limit], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            model: row.get(2)?,
            prompt: row.get(3)?,
            duration_seconds: row.get(4)?,
            tokens_in: row.get(5)?,
            tokens_out: row.get(6)?,
            tokens_cached: row.get(7)?,
            cost_usd: row.get(8)?,
            issues_claimed: row.get(9)?,
            issues_completed: row.get(10)?,
            tool_calls: row.get(11)?,
            tool_errors: row.get(12)?,
            final_status: row.get(13)?,
        })
    })?
    .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Get dashboard statistics
pub fn get_dashboard_stats(db_path: &str) -> Result<DashboardStats> {
    let conn = Connection::open(db_path)?;

    // Sessions today
    let sessions_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE date(timestamp) = date('now')",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    // Success rate (last 30 days)
    let (total, successful): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), SUM(CASE WHEN final_status = 'completed' THEN 1 ELSE 0 END)
         FROM sessions
         WHERE timestamp >= datetime('now', '-30 days')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));

    let success_rate = if total > 0 {
        (successful as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Total tokens (last 30 days)
    let total_tokens: i64 = conn.query_row(
        "SELECT SUM(tokens_in + tokens_out) FROM sessions
         WHERE timestamp >= datetime('now', '-30 days')",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    // Total cost (last 30 days)
    let total_cost: f64 = conn.query_row(
        "SELECT SUM(cost_usd) FROM sessions
         WHERE timestamp >= datetime('now', '-30 days')",
        [],
        |row| row.get(0),
    ).unwrap_or(0.0);

    // Average duration (last 30 days)
    let avg_duration: f64 = conn.query_row(
        "SELECT AVG(duration_seconds) FROM sessions
         WHERE timestamp >= datetime('now', '-30 days')",
        [],
        |row| row.get(0),
    ).unwrap_or(0.0);

    // Average APM (last 30 days)
    let avg_apm: f64 = conn.query_row(
        "SELECT AVG(apm) FROM sessions
         WHERE timestamp >= datetime('now', '-30 days')
         AND apm IS NOT NULL",
        [],
        |row| row.get(0),
    ).unwrap_or(0.0);

    Ok(DashboardStats {
        sessions_today,
        success_rate,
        total_tokens,
        total_cost,
        avg_duration,
        avg_apm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_recent_sessions() {
        // This test requires a valid metrics database
        // Skip if not available
        if let Ok(sessions) = get_recent_sessions("autopilot-metrics.db", 5) {
            assert!(sessions.len() <= 5);
        }
    }

    #[test]
    fn test_get_dashboard_stats() {
        // This test requires a valid metrics database
        // Skip if not available
        if let Ok(stats) = get_dashboard_stats("autopilot-metrics.db") {
            assert!(stats.success_rate >= 0.0 && stats.success_rate <= 100.0);
        }
    }
}
