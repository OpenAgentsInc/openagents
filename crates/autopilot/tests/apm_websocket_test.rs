//! Integration test for APM WebSocket streaming endpoint
//!
//! Tests that the `/ws/apm` endpoint correctly streams APM metrics
//! in real-time with proper format and color coding.

use autopilot::apm::{APMSource, APMWindow};
use autopilot::apm_storage;
use autopilot::dashboard::start_dashboard;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use rusqlite::Connection;
use serde_json::Value;
use tempfile::NamedTempFile;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn test_apm_websocket_streams_metrics() {
    // Create test database with APM data
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    // Initialize database
    let conn = Connection::open(db_path).unwrap();
    apm_storage::init_apm_tables(&conn).unwrap();

    // Create test session
    let session_id = "test-ws-session";
    apm_storage::create_session(&conn, session_id, APMSource::Autopilot).unwrap();

    // Record some events
    for i in 0..10 {
        apm_storage::record_event(
            &conn,
            session_id,
            if i % 2 == 0 {
                apm_storage::APMEventType::ToolCall
            } else {
                apm_storage::APMEventType::Message
            },
            None,
        )
        .unwrap();
    }

    // End session
    let end_time = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE apm_sessions SET end_time = ? WHERE id = ?",
        rusqlite::params![end_time, session_id],
    )
    .unwrap();

    // Generate snapshot
    if let Ok(Some(snapshot)) = apm_storage::generate_session_snapshot(&conn, session_id) {
        apm_storage::save_snapshot(&conn, &snapshot).unwrap();
    }

    // Generate window snapshots
    let snapshots = apm_storage::generate_window_snapshots(&conn, APMSource::Combined).unwrap();
    for snapshot in snapshots {
        apm_storage::save_snapshot(&conn, &snapshot).unwrap();
    }

    drop(conn);

    // Start dashboard in background
    let port = 18123; // Use unique port to avoid conflicts
    tokio::spawn(async move {
        let _ = start_dashboard(db_path, port).await;
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Connect to WebSocket
    let ws_url = format!("ws://127.0.0.1:{}/ws/apm", port);
    let (ws_stream, _) = connect_async(&ws_url).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    // Read initial connection message
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
        assert_eq!(msg["message"], "APM metrics stream connected");
    } else {
        panic!("Expected connection message");
    }

    // Wait for first APM update (arrives within 5 seconds)
    let mut received_apm_update = false;

    for _ in 0..12 {
        // Wait up to 6 seconds
        tokio::select! {
            Some(Ok(Message::Text(text))) = read.next() => {
                let msg: Value = serde_json::from_str(&text).unwrap();
                if msg["type"] == "apm_update" {
                    // Validate structure
                    assert!(msg.get("timestamp").is_some());
                    assert!(msg.get("data").is_some());

                    let data = msg["data"].as_object().unwrap();

                    // Check for expected window data
                    for window in &["1h", "6h", "1d"] {
                        if let Some(window_data) = data.get(*window) {
                            // Validate fields
                            assert!(window_data.get("apm").is_some());
                            assert!(window_data.get("actions").is_some());
                            assert!(window_data.get("tier").is_some());
                            assert!(window_data.get("color").is_some());

                            // Validate tier and color are valid
                            let tier = window_data["tier"].as_str().unwrap();
                            let color = window_data["color"].as_str().unwrap();

                            assert!(matches!(tier, "Baseline" | "Active" | "Productive" | "High Performance" | "Elite"));
                            assert!(matches!(color, "gray" | "blue" | "green" | "amber" | "gold"));
                        }
                    }

                    received_apm_update = true;
                    break;
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                // Keep waiting
                continue;
            }
        }
    }

    // Send close message
    let _ = write.send(Message::Close(None)).await;

    assert!(
        received_apm_update,
        "Should have received at least one APM update"
    );
}

#[tokio::test]
async fn test_apm_websocket_connection() {
    // Create minimal test database
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path().to_str().unwrap();

    let conn = Connection::open(db_path).unwrap();
    apm_storage::init_apm_tables(&conn).unwrap();
    drop(conn);

    // Start dashboard
    let port = 18124;
    tokio::spawn(async move {
        let _ = start_dashboard(db_path, port).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Test connection
    let ws_url = format!("ws://127.0.0.1:{}/ws/apm", port);
    let result = connect_async(&ws_url).await;

    assert!(result.is_ok(), "Should be able to connect to WebSocket");

    let (ws_stream, _) = result.unwrap();
    let (_, mut read) = ws_stream.split();

    // Should receive connection message
    if let Some(Ok(Message::Text(text))) = read.next().await {
        let msg: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(msg["type"], "connected");
    } else {
        panic!("Expected connection message");
    }
}
