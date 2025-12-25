//! Bridge between ACP adapter telemetry and autopilot APM tracking
//!
//! This module provides a task that consumes ActionEvents from the ACP adapter
//! and feeds them into the autopilot APM tracking system.

use crate::apm_storage::{self, APMEventType};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use tokio::sync::mpsc;

/// Spawn a task to consume ActionEvents and store them in the APM database
///
/// # Arguments
/// * `session_id` - The APM session ID to associate events with
/// * `db_path` - Path to the APM database
/// * `event_rx` - Receiver for ActionEvents from ACP adapter
///
/// # Returns
/// A JoinHandle for the background task
///
/// # Example
/// ```ignore
/// use acp_adapter::AcpAgentConnection;
/// use autopilot::apm_telemetry_bridge::spawn_telemetry_consumer;
///
/// let (session, event_rx) = connection.new_session_with_telemetry(cwd).await?;
/// let task = spawn_telemetry_consumer(&session_id, &db_path, event_rx);
/// ```
pub fn spawn_telemetry_consumer(
    session_id: String,
    db_path: impl AsRef<Path>,
    mut event_rx: mpsc::UnboundedReceiver<acp_adapter::ActionEvent>,
) -> tokio::task::JoinHandle<()> {
    let db_path = db_path.as_ref().to_path_buf();

    tokio::spawn(async move {
        let conn = match Connection::open(&db_path) {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("ERROR: Failed to open APM database for telemetry: {}", e);
                return;
            }
        };

        // Ensure APM tables exist
        if let Err(e) = apm_storage::init_apm_tables(&conn) {
            eprintln!(
                "ERROR: Failed to initialize APM tables for telemetry: {}",
                e
            );
            return;
        }

        let mut event_count = 0;

        while let Some(event) = event_rx.recv().await {
            // Convert ActionEvent to APMEventType
            let event_type = match event.action_type.as_str() {
                // Messages
                "UserMessage" | "AssistantMessage" => APMEventType::Message,

                // Tool calls (all tool names count as tool calls)
                _ => APMEventType::ToolCall,
            };

            // Create metadata JSON
            let metadata = event.metadata.map(|m| m.to_string());

            // Record event to database
            if let Err(e) =
                apm_storage::record_event(&conn, &session_id, event_type, metadata.as_deref())
            {
                eprintln!(
                    "WARNING: Failed to record APM event {}: {}",
                    event.action_type, e
                );
            } else {
                event_count += 1;
            }
        }

        tracing::debug!(
            session_id = %session_id,
            event_count,
            "APM telemetry consumer task completed"
        );
    })
}

/// Create an APM session and spawn a telemetry consumer task
///
/// This is a convenience function that combines session creation with consumer spawning.
///
/// # Arguments
/// * `db_path` - Path to the APM database
/// * `source` - APM source (Autopilot or ClaudeCode)
/// * `event_rx` - Receiver for ActionEvents from ACP adapter
///
/// # Returns
/// A tuple of (session_id, JoinHandle) on success
pub fn create_apm_session_with_consumer(
    db_path: impl AsRef<Path>,
    source: crate::apm::APMSource,
    event_rx: mpsc::UnboundedReceiver<acp_adapter::ActionEvent>,
) -> Result<(String, tokio::task::JoinHandle<()>)> {
    let db_path = db_path.as_ref();
    let conn = Connection::open(db_path)?;

    apm_storage::init_apm_tables(&conn)?;

    // Create APM session
    let session_id = format!("apm-acp-{}", uuid::Uuid::new_v4());
    apm_storage::create_session(&conn, &session_id, source)?;

    // Spawn consumer
    let task = spawn_telemetry_consumer(session_id.clone(), db_path, event_rx);

    Ok((session_id, task))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_telemetry_consumer() {
        let db_file = NamedTempFile::new().unwrap();
        let db_path = db_file.path();

        // Initialize database
        let conn = Connection::open(db_path).unwrap();
        apm_storage::init_apm_tables(&conn).unwrap();
        drop(conn);

        // Create session
        let session_id = "test-session".to_string();
        let conn = Connection::open(db_path).unwrap();
        apm_storage::create_session(&conn, &session_id, crate::apm::APMSource::Autopilot).unwrap();
        drop(conn);

        // Create channel and send test events
        let (tx, rx) = mpsc::unbounded_channel();

        let task = spawn_telemetry_consumer(session_id.clone(), db_path, rx);

        // Send some events
        tx.send(acp_adapter::ActionEvent::success(
            &session_id,
            "UserMessage",
            0,
        ))
        .unwrap();

        tx.send(acp_adapter::ActionEvent::success(&session_id, "Read", 150))
            .unwrap();

        tx.send(acp_adapter::ActionEvent::failure(
            &session_id,
            "Bash",
            200,
            "Command failed",
        ))
        .unwrap();

        // Drop sender to close channel
        drop(tx);

        // Wait for consumer to finish
        task.await.unwrap();

        // Verify events were recorded
        let conn = Connection::open(db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 3, "Should have recorded 3 events");

        // Verify event types
        let message_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1 AND event_type = 'message'",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(message_count, 1, "Should have 1 message event");

        let tool_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1 AND event_type = 'tool_call'",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(tool_count, 2, "Should have 2 tool call events");
    }

    #[tokio::test]
    async fn test_create_session_with_consumer() {
        let db_file = NamedTempFile::new().unwrap();
        let db_path = db_file.path();

        let (tx, rx) = mpsc::unbounded_channel();

        let (session_id, task) =
            create_apm_session_with_consumer(db_path, crate::apm::APMSource::ClaudeCode, rx)
                .unwrap();

        // Session should be created in database
        let conn = Connection::open(db_path).unwrap();
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM apm_sessions WHERE id = ?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(exists, 1, "Session should exist in database");
        drop(conn);

        // Send event
        tx.send(acp_adapter::ActionEvent::success(&session_id, "Edit", 100))
            .unwrap();
        drop(tx);

        // Wait for consumer
        task.await.unwrap();

        // Verify event was recorded
        let conn = Connection::open(db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1",
                [&session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1, "Should have recorded 1 event");
    }
}
