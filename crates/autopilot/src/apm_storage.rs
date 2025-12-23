//! APM event storage and persistence
//!
//! Stores APM events (tool calls, messages, git commands) to SQLite
//! for historical analysis and trend tracking.

use crate::apm::{APMSnapshot, APMSource, APMWindow};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use tracing::{debug, error, info};

/// Current schema version for APM tables
#[allow(dead_code)]
const APM_SCHEMA_VERSION: i32 = 1;

/// APM event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum APMEventType {
    /// User or assistant message
    Message,
    /// Tool call invocation
    ToolCall,
    /// Git command execution
    GitCommand,
    /// File operation (read, write, edit)
    FileOperation,
    /// Other action
    Other,
}

impl APMEventType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Message => "message",
            Self::ToolCall => "tool_call",
            Self::GitCommand => "git_command",
            Self::FileOperation => "file_operation",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "message" => Some(Self::Message),
            "tool_call" => Some(Self::ToolCall),
            "git_command" => Some(Self::GitCommand),
            "file_operation" => Some(Self::FileOperation),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// APM event record
#[derive(Debug, Clone)]
pub struct APMEvent {
    pub id: String,
    pub session_id: String,
    pub source: APMSource,
    pub event_type: APMEventType,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<String>, // JSON metadata
}

/// Initialize APM tables in the database
pub fn init_apm_tables(conn: &Connection) -> Result<()> {
    debug!("Initializing APM tables");

    // Check current version
    let version = get_apm_schema_version(conn)?;
    debug!("Current APM schema version: {}", version);

    if version < 1 {
        migrate_apm_v1(conn)?;
    }

    Ok(())
}

fn get_apm_schema_version(conn: &Connection) -> Result<i32> {
    // Create version table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS apm_schema_version (version INTEGER NOT NULL)",
        [],
    )?;

    let version: Option<i32> = conn
        .query_row(
            "SELECT version FROM apm_schema_version LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(version.unwrap_or(0))
}

fn set_apm_schema_version(conn: &Connection, version: i32) -> Result<()> {
    debug!("Setting APM schema version to {}", version);
    conn.execute("DELETE FROM apm_schema_version", [])?;
    conn.execute(
        "INSERT INTO apm_schema_version (version) VALUES (?)",
        [version],
    )?;
    Ok(())
}

fn migrate_apm_v1(conn: &Connection) -> Result<()> {
    info!("Running APM migration v1");
    conn.execute_batch(
        r#"
        -- APM sessions table
        CREATE TABLE IF NOT EXISTS apm_sessions (
            id TEXT NOT NULL PRIMARY KEY,
            source TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT,
            created_at TEXT NOT NULL,
            CHECK(id != ''),
            CHECK(source IN ('autopilot', 'claude_code', 'combined'))
        );

        CREATE INDEX IF NOT EXISTS idx_apm_sessions_source ON apm_sessions(source);
        CREATE INDEX IF NOT EXISTS idx_apm_sessions_start ON apm_sessions(start_time);

        -- APM events table
        CREATE TABLE IF NOT EXISTS apm_events (
            id TEXT NOT NULL PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES apm_sessions(id),
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            metadata TEXT,
            created_at TEXT NOT NULL,
            CHECK(id != ''),
            CHECK(event_type IN ('message', 'tool_call', 'git_command', 'file_operation', 'other'))
        );

        CREATE INDEX IF NOT EXISTS idx_apm_events_session ON apm_events(session_id);
        CREATE INDEX IF NOT EXISTS idx_apm_events_timestamp ON apm_events(timestamp);
        CREATE INDEX IF NOT EXISTS idx_apm_events_type ON apm_events(event_type);

        -- APM snapshots table (pre-calculated APM values)
        CREATE TABLE IF NOT EXISTS apm_snapshots (
            id TEXT NOT NULL PRIMARY KEY,
            timestamp TEXT NOT NULL,
            source TEXT NOT NULL,
            window TEXT NOT NULL,
            apm REAL NOT NULL,
            actions INTEGER NOT NULL,
            duration_minutes REAL NOT NULL,
            messages INTEGER NOT NULL,
            tool_calls INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK(id != ''),
            CHECK(source IN ('autopilot', 'claude_code', 'combined')),
            CHECK(window IN ('session', '1h', '6h', '1d', '1w', '1m', 'lifetime'))
        );

        CREATE INDEX IF NOT EXISTS idx_apm_snapshots_source ON apm_snapshots(source);
        CREATE INDEX IF NOT EXISTS idx_apm_snapshots_window ON apm_snapshots(window);
        CREATE INDEX IF NOT EXISTS idx_apm_snapshots_timestamp ON apm_snapshots(timestamp);
        "#,
    )?;

    set_apm_schema_version(conn, 1).map_err(|e| {
        error!("Failed to set APM schema version to 1: {}", e);
        e
    })?;
    info!("APM migration v1 completed successfully");
    Ok(())
}

/// Create a new APM session
pub fn create_session(conn: &Connection, id: &str, source: APMSource) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO apm_sessions (id, source, start_time, created_at) VALUES (?, ?, ?, ?)",
        params![id, source.as_str(), &now, &now],
    )?;
    debug!("Created APM session {}", id);
    Ok(())
}

/// End an APM session
pub fn end_session(conn: &Connection, id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE apm_sessions SET end_time = ? WHERE id = ?",
        params![&now, id],
    )?;
    debug!("Ended APM session {}", id);
    Ok(())
}

/// Record an APM event
pub fn record_event(
    conn: &Connection,
    session_id: &str,
    event_type: APMEventType,
    metadata: Option<&str>,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO apm_events (id, session_id, event_type, timestamp, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        params![&id, session_id, event_type.as_str(), &now, metadata, &now],
    )?;

    debug!("Recorded APM event {} ({:?})", id, event_type);
    Ok(id)
}

/// Save an APM snapshot
pub fn save_snapshot(conn: &Connection, snapshot: &APMSnapshot) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        r#"
        INSERT INTO apm_snapshots
        (id, timestamp, source, window, apm, actions, duration_minutes, messages, tool_calls, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        params![
            &id,
            snapshot.timestamp.to_rfc3339(),
            snapshot.source.as_str(),
            snapshot.window.as_str(),
            snapshot.apm,
            snapshot.actions,
            snapshot.duration_minutes,
            snapshot.messages,
            snapshot.tool_calls,
            &now,
        ],
    )?;

    debug!("Saved APM snapshot {} ({:?})", id, snapshot.window);
    Ok(id)
}

/// Get session event counts
pub fn get_session_stats(
    conn: &Connection,
    session_id: &str,
) -> Result<(u32, u32)> {
    let message_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM apm_events WHERE session_id = ? AND event_type = 'message'",
        params![session_id],
        |row| row.get(0),
    )?;

    let tool_call_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM apm_events WHERE session_id = ? AND event_type = 'tool_call'",
        params![session_id],
        |row| row.get(0),
    )?;

    Ok((message_count, tool_call_count))
}

/// Get all sessions for a source
pub fn get_sessions_by_source(
    conn: &Connection,
    source: APMSource,
) -> Result<Vec<(String, DateTime<Utc>, Option<DateTime<Utc>>)>> {
    let mut stmt = conn.prepare(
        "SELECT id, start_time, end_time FROM apm_sessions WHERE source = ? ORDER BY start_time DESC",
    )?;

    let sessions = stmt
        .query_map(params![source.as_str()], |row| {
            let id: String = row.get(0)?;
            let start_time: String = row.get(1)?;
            let end_time: Option<String> = row.get(2)?;

            Ok((
                id,
                DateTime::parse_from_rfc3339(&start_time)
                    .unwrap()
                    .with_timezone(&Utc),
                end_time.map(|t| {
                    DateTime::parse_from_rfc3339(&t)
                        .unwrap()
                        .with_timezone(&Utc)
                }),
            ))
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(sessions)
}

/// Get latest snapshot for a source and window
pub fn get_latest_snapshot(
    conn: &Connection,
    source: APMSource,
    window: APMWindow,
) -> Result<Option<APMSnapshot>> {
    let result = conn.query_row(
        r#"
        SELECT timestamp, apm, actions, duration_minutes, messages, tool_calls
        FROM apm_snapshots
        WHERE source = ? AND window = ?
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        params![source.as_str(), window.as_str()],
        |row| {
            let timestamp: String = row.get(0)?;
            Ok(APMSnapshot {
                timestamp: DateTime::parse_from_rfc3339(&timestamp)
                    .unwrap()
                    .with_timezone(&Utc),
                source,
                window,
                apm: row.get(1)?,
                actions: row.get(2)?,
                duration_minutes: row.get(3)?,
                messages: row.get(4)?,
                tool_calls: row.get(5)?,
            })
        },
    );

    match result {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Generate APM snapshot for a completed session
///
/// Calculates APM from session events and creates a snapshot.
/// Returns None if the session is still running or has no events.
pub fn generate_session_snapshot(conn: &Connection, session_id: &str) -> Result<Option<APMSnapshot>> {
    // Get session metadata
    let session_result: std::result::Result<(String, Option<String>, String), rusqlite::Error> = conn.query_row(
        "SELECT start_time, end_time, source FROM apm_sessions WHERE id = ?",
        params![session_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
            ))
        },
    );

    let (start_time_str, end_time_str, source_str) = match session_result {
        Ok(data) => data,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            debug!("Session {} not found", session_id);
            return Ok(None);
        }
        Err(e) => return Err(e),
    };

    // Session must be completed (have end_time)
    let end_time_str = match end_time_str {
        Some(t) => t,
        None => {
            debug!("Session {} not yet completed", session_id);
            return Ok(None);
        }
    };

    let start_time = DateTime::parse_from_rfc3339(&start_time_str)
        .unwrap()
        .with_timezone(&Utc);
    let end_time = DateTime::parse_from_rfc3339(&end_time_str)
        .unwrap()
        .with_timezone(&Utc);

    let source = match source_str.as_str() {
        "autopilot" => APMSource::Autopilot,
        "claude_code" => APMSource::ClaudeCode,
        "combined" => APMSource::Combined,
        _ => {
            error!("Unknown source: {}", source_str);
            return Ok(None);
        }
    };

    // Get event counts
    let (messages, tool_calls) = get_session_stats(conn, session_id)?;

    // Calculate duration
    let duration = end_time.signed_duration_since(start_time);
    let duration_minutes = duration.num_milliseconds() as f64 / 60_000.0;

    if duration_minutes <= 0.0 {
        debug!("Session {} has invalid duration: {} minutes", session_id, duration_minutes);
        return Ok(None);
    }

    // Calculate APM
    let actions = messages + tool_calls;
    let apm = actions as f64 / duration_minutes;

    let snapshot = APMSnapshot {
        timestamp: end_time,
        source,
        window: APMWindow::Session,
        apm,
        actions,
        duration_minutes,
        messages,
        tool_calls,
    };

    Ok(Some(snapshot))
}

/// Generate snapshots for time windows (1h, 6h, 1d, 1w, 1m, lifetime)
///
/// Calculates APM for various time windows from all events within the window.
/// Returns Vec of snapshots generated.
pub fn generate_window_snapshots(conn: &Connection, source: APMSource) -> Result<Vec<APMSnapshot>> {
    use crate::apm::calculate_apm_from_timestamps;

    let now = Utc::now();
    let mut snapshots = Vec::new();

    // Define windows to calculate (excluding Session which is per-session only)
    let windows = vec![
        APMWindow::Hour1,
        APMWindow::Hour6,
        APMWindow::Day1,
        APMWindow::Week1,
        APMWindow::Month1,
        APMWindow::Lifetime,
    ];

    for window in windows {
        // Get cutoff time for this window (None for lifetime)
        let cutoff = window.duration().map(|d| now - d);

        // Query events within the window
        let (messages, tool_calls, start_time, end_time) = if let Some(cutoff_time) = cutoff {
            // Time-windowed query
            let query = r#"
                SELECT
                    COUNT(CASE WHEN event_type = 'message' THEN 1 END) as messages,
                    COUNT(CASE WHEN event_type = 'tool_call' THEN 1 END) as tool_calls,
                    MIN(timestamp) as start_time,
                    MAX(timestamp) as end_time
                FROM apm_events e
                JOIN apm_sessions s ON e.session_id = s.id
                WHERE s.source = ? AND e.timestamp >= ?
            "#;

            let result: std::result::Result<(u32, u32, Option<String>, Option<String>), rusqlite::Error> = conn.query_row(
                query,
                params![source.as_str(), cutoff_time.to_rfc3339()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            );

            match result {
                Ok(data) => data,
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => return Err(e),
            }
        } else {
            // Lifetime query (all events)
            let query = r#"
                SELECT
                    COUNT(CASE WHEN event_type = 'message' THEN 1 END) as messages,
                    COUNT(CASE WHEN event_type = 'tool_call' THEN 1 END) as tool_calls,
                    MIN(timestamp) as start_time,
                    MAX(timestamp) as end_time
                FROM apm_events e
                JOIN apm_sessions s ON e.session_id = s.id
                WHERE s.source = ?
            "#;

            let result: std::result::Result<(u32, u32, Option<String>, Option<String>), rusqlite::Error> = conn.query_row(
                query,
                params![source.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            );

            match result {
                Ok(data) => data,
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => return Err(e),
            }
        };

        // Skip if no data in window
        if start_time.is_none() || end_time.is_none() {
            continue;
        }

        let start = DateTime::parse_from_rfc3339(&start_time.unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339(&end_time.unwrap())
            .unwrap()
            .with_timezone(&Utc);

        // Calculate APM
        if let Some(apm) = calculate_apm_from_timestamps(messages, tool_calls, start, end) {
            let duration = end.signed_duration_since(start);
            let duration_minutes = duration.num_milliseconds() as f64 / 60_000.0;

            let snapshot = APMSnapshot {
                timestamp: now,
                source,
                window,
                apm,
                actions: messages + tool_calls,
                duration_minutes,
                messages,
                tool_calls,
            };

            snapshots.push(snapshot);
        }
    }

    Ok(snapshots)
}

/// Regenerate all APM snapshots from existing session data
///
/// This function:
/// 1. Deletes all existing snapshots
/// 2. Generates session snapshots for all completed sessions
/// 3. Generates window snapshots for all sources
///
/// Returns the total number of snapshots generated.
pub fn regenerate_all_snapshots(conn: &Connection) -> Result<usize> {
    info!("Regenerating all APM snapshots");

    // Delete existing snapshots
    conn.execute("DELETE FROM apm_snapshots", [])?;
    debug!("Deleted existing snapshots");

    let mut total_count = 0;

    // Generate session snapshots for all completed sessions
    let sessions: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM apm_sessions WHERE end_time IS NOT NULL ORDER BY start_time"
        )?;
        let sessions = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        sessions
    };

    for session_id in &sessions {
        if let Some(snapshot) = generate_session_snapshot(conn, session_id)? {
            save_snapshot(conn, &snapshot)?;
            total_count += 1;
        }
    }

    debug!("Generated {} session snapshots", total_count);

    // Generate window snapshots for each source
    for source in [APMSource::Autopilot, APMSource::ClaudeCode, APMSource::Combined] {
        let window_snapshots = generate_window_snapshots(conn, source)?;
        for snapshot in window_snapshots {
            save_snapshot(conn, &snapshot)?;
            total_count += 1;
        }
    }

    info!("Regenerated {} total snapshots", total_count);
    Ok(total_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_apm_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_init_apm_tables() {
        let conn = setup_test_db();

        // Verify tables exist
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='apm_sessions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='apm_events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_create_and_end_session() {
        let conn = setup_test_db();
        let session_id = "test-session-1";

        create_session(&conn, session_id, APMSource::Autopilot).unwrap();

        let source: String = conn
            .query_row(
                "SELECT source FROM apm_sessions WHERE id = ?",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "autopilot");

        end_session(&conn, session_id).unwrap();

        let end_time: Option<String> = conn
            .query_row(
                "SELECT end_time FROM apm_sessions WHERE id = ?",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(end_time.is_some());
    }

    #[test]
    fn test_record_event() {
        let conn = setup_test_db();
        let session_id = "test-session-2";

        create_session(&conn, session_id, APMSource::ClaudeCode).unwrap();

        let event_id = record_event(&conn, session_id, APMEventType::Message, None).unwrap();
        assert!(!event_id.is_empty());

        record_event(&conn, session_id, APMEventType::ToolCall, Some(r#"{"tool":"bash"}"#))
            .unwrap();

        let (messages, tool_calls) = get_session_stats(&conn, session_id).unwrap();
        assert_eq!(messages, 1);
        assert_eq!(tool_calls, 1);
    }

    #[test]
    fn test_save_and_get_snapshot() {
        let conn = setup_test_db();

        let snapshot = APMSnapshot {
            timestamp: Utc::now(),
            source: APMSource::Autopilot,
            window: APMWindow::Session,
            apm: 19.5,
            actions: 195,
            duration_minutes: 10.0,
            messages: 45,
            tool_calls: 150,
        };

        save_snapshot(&conn, &snapshot).unwrap();

        let retrieved = get_latest_snapshot(&conn, APMSource::Autopilot, APMWindow::Session)
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.apm, 19.5);
        assert_eq!(retrieved.actions, 195);
    }

    #[test]
    fn test_get_sessions_by_source() {
        let conn = setup_test_db();

        create_session(&conn, "session-1", APMSource::Autopilot).unwrap();
        create_session(&conn, "session-2", APMSource::Autopilot).unwrap();
        create_session(&conn, "session-3", APMSource::ClaudeCode).unwrap();

        let sessions = get_sessions_by_source(&conn, APMSource::Autopilot).unwrap();
        assert_eq!(sessions.len(), 2);

        let sessions = get_sessions_by_source(&conn, APMSource::ClaudeCode).unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(APMEventType::Message.as_str(), "message");
        assert_eq!(APMEventType::ToolCall.as_str(), "tool_call");
        assert_eq!(APMEventType::GitCommand.as_str(), "git_command");
    }

    #[test]
    fn test_event_type_from_str() {
        assert_eq!(
            APMEventType::from_str("message"),
            Some(APMEventType::Message)
        );
        assert_eq!(
            APMEventType::from_str("tool_call"),
            Some(APMEventType::ToolCall)
        );
        assert_eq!(APMEventType::from_str("invalid"), None);
    }
}
