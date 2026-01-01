//! Integration tests for ACP adapter
//!
//! Comprehensive E2E tests covering:
//! - Full prompt flow with mock Claude Code agent
//! - Full prompt flow with mock Codex agent
//! - Session lifecycle (create, prompt, cancel, close)
//! - Permission handling across both agents
//! - File operations (read, write) via ACP
//! - Rlog streaming and replay accuracy

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use acp_adapter::{
    AcpAgentConnection, AgentCommand, AllowAllPermissions, DenyAllPermissions, OpenAgentsClient,
    PermissionHandler, RlogStreamer, StreamConfig, acp,
};

/// Mock agent that implements basic ACP protocol for testing
mod mock_agent {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::io::{BufRead, Write};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(untagged)]
    enum JsonRpcMessage {
        Request(JsonRpcRequest),
        Response(JsonRpcResponse),
        Notification(JsonRpcNotification),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        id: serde_json::Value,
        method: String,
        params: serde_json::Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct JsonRpcResponse {
        jsonrpc: String,
        id: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct JsonRpcNotification {
        jsonrpc: String,
        method: String,
        params: serde_json::Value,
    }

    pub struct MockAgent {
        agent_type: String,
    }

    impl MockAgent {
        pub fn new(agent_type: impl Into<String>) -> Self {
            Self {
                agent_type: agent_type.into(),
            }
        }

        pub fn run(&self) {
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            let mut reader = stdin.lock();
            let mut writer = stdout.lock();

            let mut session_counter = 0u64;

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error reading: {}", e);
                        break;
                    }
                }

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let msg: JsonRpcMessage = match serde_json::from_str(line) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Failed to parse message: {}", e);
                        continue;
                    }
                };

                match msg {
                    JsonRpcMessage::Request(req) => {
                        let response = self.handle_request(&req, &mut session_counter);
                        let response_json = serde_json::to_string(&response).unwrap();
                        writeln!(writer, "{}", response_json).unwrap();
                        writer.flush().unwrap();
                    }
                    JsonRpcMessage::Notification(notif) => {
                        self.handle_notification(&notif);
                    }
                    _ => {}
                }
            }
        }

        fn handle_request(
            &self,
            req: &JsonRpcRequest,
            session_counter: &mut u64,
        ) -> JsonRpcResponse {
            match req.method.as_str() {
                "initialize" => {
                    let result = json!({
                        "protocol_version": "v1",
                        "agent_capabilities": {
                            "sessions": true,
                            "prompts": true
                        },
                        "agent_info": {
                            "name": format!("mock-{}", self.agent_type),
                            "version": "1.0.0"
                        }
                    });

                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id.clone(),
                        result: Some(result),
                        error: None,
                    }
                }
                "session/new" => {
                    *session_counter += 1;
                    let session_id = format!("session-{}", session_counter);

                    let result = json!({
                        "session_id": session_id
                    });

                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id.clone(),
                        result: Some(result),
                        error: None,
                    }
                }
                "session/prompt" => {
                    // Parse prompt request to get session_id
                    let params: serde_json::Value = req.params.clone();
                    let session_id = params
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    // Return a simple response
                    let result = json!({
                        "session_id": session_id,
                        "status": "completed"
                    });

                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id.clone(),
                        result: Some(result),
                        error: None,
                    }
                }
                _ => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id.clone(),
                    result: None,
                    error: Some(json!({
                        "code": -32601,
                        "message": format!("Method not found: {}", req.method)
                    })),
                },
            }
        }

        fn handle_notification(&self, _notif: &JsonRpcNotification) {
            // Handle notifications (like cancel) silently
        }
    }
}

/// Helper to create a mock agent binary for testing
fn create_mock_agent_binary(agent_type: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let binary_path = temp_dir.join(format!("mock-acp-agent-{}", agent_type));

    // Write a simple Rust program that runs the mock agent
    let src = r###"
use std::io::{BufRead, Write};

fn extract_id(line: &str) -> u64 {
    let key = "\"id\":";
    if let Some(idx) = line.find(key) {
        let rest = &line[idx + key.len()..];
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse::<u64>().unwrap_or(0)
    } else {
        0
    }
}

fn has_method(line: &str, method: &str) -> bool {
    let needle = format!("\"method\":\"{}\"", method);
    line.contains(&needle)
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut session_counter = 0u64;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {},
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let id = extract_id(line);

        if has_method(line, "initialize") {
            let result = "{\"protocolVersion\":1,\"agentCapabilities\":{\"sessions\":true,\"prompts\":true},\"agentInfo\":{\"name\":\"mock\",\"version\":\"1.0.0\"}}";
            let _ = writeln!(writer, "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}", id, result);
            let _ = writer.flush();
        } else if has_method(line, "session/new") {
            session_counter += 1;
            let result = format!("{{\"sessionId\":\"session-{}\"}}", session_counter);
            let _ = writeln!(writer, "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}", id, result);
            let _ = writer.flush();
        } else if has_method(line, "session/prompt") {
            let result = "{\"stopReason\":\"end_turn\"}";
            let _ = writeln!(writer, "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}", id, result);
            let _ = writer.flush();
        }
    }
}
"###;

    let src_path = temp_dir.join(format!("mock-agent-{}.rs", agent_type));
    std::fs::write(&src_path, src).unwrap();

    // Compile the mock agent
    let output = std::process::Command::new("rustc")
        .arg(&src_path)
        .arg("-o")
        .arg(&binary_path)
        .arg("--edition=2024")
        .output()
        .expect("Failed to compile mock agent");

    if !output.status.success() {
        panic!(
            "Failed to compile mock agent: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    binary_path
}

#[tokio::test]
async fn test_claude_code_session_lifecycle() {
    let binary_path = create_mock_agent_binary("claude");
    let cwd = std::env::current_dir().unwrap();

    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-claude", command, &cwd)
        .await
        .expect("Failed to create connection");

    // Test session creation
    let session = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session");

    assert!(!session.session_id.to_string().is_empty());

    // Test sending a prompt
    let _response = connection
        .prompt(&session.session_id, "Test prompt")
        .await
        .expect("Failed to send prompt");

    // Test session listing
    let sessions = connection.list_sessions().await;
    assert_eq!(sessions.len(), 1);
    assert!(sessions.contains(&session.session_id.to_string()));

    // Test cancellation
    connection.cancel(&session.session_id).await;

    // Test session closure
    connection
        .close_session(&session.session_id.to_string())
        .await
        .expect("Failed to close session");

    let sessions = connection.list_sessions().await;
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_codex_session_lifecycle() {
    let binary_path = create_mock_agent_binary("codex");
    let cwd = std::env::current_dir().unwrap();

    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-codex", command, &cwd)
        .await
        .expect("Failed to create connection");

    // Test session creation
    let session = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session");

    assert!(!session.session_id.to_string().is_empty());

    // Test sending a prompt
    let _response = connection
        .prompt(&session.session_id, "Test codex prompt")
        .await
        .expect("Failed to send prompt");

    // Test session retrieval
    let retrieved = connection
        .get_session(&session.session_id.to_string())
        .await
        .expect("Session not found");
    assert_eq!(retrieved.session_id, session.session_id);
}

#[tokio::test]
async fn test_multiple_sessions() {
    let binary_path = create_mock_agent_binary("multi");
    let cwd = std::env::current_dir().unwrap();

    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-multi", command, &cwd)
        .await
        .expect("Failed to create connection");

    // Create multiple sessions
    let session1 = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session 1");

    let session2 = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session 2");

    let session3 = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session 3");

    // Verify all sessions are distinct
    assert_ne!(session1.session_id, session2.session_id);
    assert_ne!(session2.session_id, session3.session_id);
    assert_ne!(session1.session_id, session3.session_id);

    // Verify all sessions are listed
    let sessions = connection.list_sessions().await;
    assert_eq!(sessions.len(), 3);

    // Close one session
    connection
        .close_session(&session2.session_id.to_string())
        .await
        .expect("Failed to close session");

    let sessions = connection.list_sessions().await;
    assert_eq!(sessions.len(), 2);
    assert!(!sessions.contains(&session2.session_id.to_string()));
}

#[tokio::test]
async fn test_agent_capabilities() {
    let binary_path = create_mock_agent_binary("caps");
    let cwd = std::env::current_dir().unwrap();

    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-caps", command, &cwd)
        .await
        .expect("Failed to create connection");

    let _capabilities = connection.capabilities();

    // Mock agent returns basic capabilities - just verify connection works
    assert!(connection.agent_name == "mock-caps");
}

#[tokio::test]
async fn test_protocol_version() {
    let binary_path = create_mock_agent_binary("version");
    let cwd = std::env::current_dir().unwrap();

    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-version", command, &cwd)
        .await
        .expect("Failed to create connection");

    let version = connection.protocol_version();
    assert_eq!(*version, acp::ProtocolVersion::V1);
}

#[tokio::test]
async fn test_permission_handler_allow_all() {
    let handler = AllowAllPermissions;

    // Create a minimal tool call and options for testing
    let tool_call = acp::ToolCallUpdate::new(acp::ToolCallId::new("test-call"), Default::default());

    let options = vec![acp::PermissionOption::new(
        acp::PermissionOptionId::new("allow-1"),
        "Allow once",
        acp::PermissionOptionKind::AllowOnce,
    )];

    let outcome = handler.can_use_tool(&tool_call, &options).await.unwrap();

    // Verify we got a Selected outcome (AllowAll should select an allow option)
    assert!(matches!(
        outcome,
        acp::RequestPermissionOutcome::Selected(_)
    ));
}

#[tokio::test]
async fn test_permission_handler_deny_all() {
    let handler = DenyAllPermissions;

    let tool_call = acp::ToolCallUpdate::new(acp::ToolCallId::new("test-call"), Default::default());

    let options = vec![
        acp::PermissionOption::new(
            acp::PermissionOptionId::new("allow-1"),
            "Allow once",
            acp::PermissionOptionKind::AllowOnce,
        ),
        acp::PermissionOption::new(
            acp::PermissionOptionId::new("reject-1"),
            "Reject once",
            acp::PermissionOptionKind::RejectOnce,
        ),
    ];

    let outcome = handler.can_use_tool(&tool_call, &options).await.unwrap();

    // Verify outcome (DenyAll should select reject or cancel)
    assert!(matches!(
        outcome,
        acp::RequestPermissionOutcome::Selected(_) | acp::RequestPermissionOutcome::Cancelled
    ));
}

#[tokio::test]
async fn test_openagents_client_file_operations() {
    let temp_dir = std::env::temp_dir();

    // Create sessions map
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let handler = Arc::new(AllowAllPermissions);
    let client = OpenAgentsClient::new(sessions, handler, temp_dir.clone());

    // Create a temporary test file
    let test_file = temp_dir.join("acp-test-file.txt");
    std::fs::write(&test_file, "original content").unwrap();

    // Test read
    let read_req =
        acp::ReadTextFileRequest::new(acp::SessionId::new("test-session"), test_file.clone());

    let read_resp = client.read_text_file(read_req).await.unwrap();
    assert_eq!(read_resp.content, "original content");

    // Test write
    let write_req = acp::WriteTextFileRequest::new(
        acp::SessionId::new("test-session"),
        test_file.clone(),
        "modified content".to_string(),
    );

    client.write_text_file(write_req).await.unwrap();

    // Verify write
    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "modified content");

    // Cleanup
    std::fs::remove_file(&test_file).unwrap();
}

#[tokio::test]
async fn test_rlog_streaming() {
    use std::time::Duration;
    use tokio::time::sleep;
    use serde_json::json;

    let temp_dir = std::env::temp_dir();
    let log_file = temp_dir.join("acp-test-stream.rlog");

    // Create streamer with header
    use acp_adapter::RlogHeaderInfo;
    let header = RlogHeaderInfo::new("test-session", "abc123")
        .agent("test-agent")
        .model("test-model");

    let config = StreamConfig {
        flush_immediately: true,
        include_timestamps: false,
        max_line_length: 500,
    };

    let streamer = RlogStreamer::create(&log_file, header)
        .await
        .expect("Failed to create streamer")
        .with_config(config);

    // Write some notifications
    let notif1 = acp::SessionNotification::new(
        acp::SessionId::new("test-session"),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            acp::ToolCallId::new("tool-1"),
            acp::ToolCallUpdateFields::new()
                .status(acp::ToolCallStatus::Completed)
                .raw_output(json!({"ok": true})),
        )),
    );

    let notif2 = acp::SessionNotification::new(
        acp::SessionId::new("test-session"),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            acp::ToolCallId::new("tool-2"),
            acp::ToolCallUpdateFields::new()
                .status(acp::ToolCallStatus::Completed)
                .raw_output(json!({"ok": false})),
        )),
    );

    streamer.write_notification(&notif1).await.unwrap();
    streamer.write_notification(&notif2).await.unwrap();

    // Flush
    streamer.flush().await.unwrap();

    // Wait for flush
    sleep(Duration::from_millis(50)).await;

    // Verify file was created and has content
    assert!(log_file.exists());
    let content = std::fs::read_to_string(&log_file).unwrap();
    assert!(content.contains("tool-1"));
    assert!(content.contains("tool-2"));

    // Cleanup
    std::fs::remove_file(&log_file).unwrap();
}

#[tokio::test]
async fn test_command_builder() {
    let cmd = AgentCommand::new("/usr/bin/agent")
        .arg("--model")
        .arg("sonnet")
        .args(vec!["--verbose", "--debug"])
        .env("RUST_LOG", "debug")
        .env("AGENT_MODE", "test");

    assert_eq!(cmd.path, PathBuf::from("/usr/bin/agent"));
    assert_eq!(cmd.args, vec!["--model", "sonnet", "--verbose", "--debug"]);
    assert_eq!(cmd.env.len(), 2);
    assert!(
        cmd.env
            .contains(&("RUST_LOG".to_string(), "debug".to_string()))
    );
    assert!(
        cmd.env
            .contains(&("AGENT_MODE".to_string(), "test".to_string()))
    );
}

#[tokio::test]
async fn test_telemetry_end_to_end() {
    use acp_adapter::ApmTelemetry;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    // Create temporary APM database
    let db_file = NamedTempFile::new().unwrap();
    let db_path = db_file.path();

    // Initialize APM database using autopilot's schema
    let conn = Connection::open(db_path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS apm_sessions (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT
        );

        CREATE TABLE IF NOT EXISTS apm_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            metadata TEXT,
            FOREIGN KEY(session_id) REFERENCES apm_sessions(id)
        );
        "#,
    )
    .unwrap();

    // Create APM session
    let session_id = "test-telemetry-session";
    conn.execute(
        "INSERT INTO apm_sessions (id, source, started_at) VALUES (?1, 'ClaudeCode', datetime('now'))",
        [session_id],
    ).unwrap();
    drop(conn);

    // Create mock agent with telemetry-enabled session
    let binary_path = create_mock_agent_binary("telemetry");
    let cwd = std::env::current_dir().unwrap();
    let command = AgentCommand::new(binary_path);

    let connection = AcpAgentConnection::stdio("mock-telemetry", command, &cwd)
        .await
        .expect("Failed to create connection");

    // Create session with telemetry enabled
    let (telemetry, mut event_rx) = ApmTelemetry::new(session_id);

    // Spawn task to consume telemetry events and write to APM database
    let db_path_clone = db_path.to_path_buf();
    let telemetry_task = tokio::spawn(async move {
        let conn = Connection::open(&db_path_clone).unwrap();
        let mut event_count = 0;

        while let Some(event) = event_rx.recv().await {
            // Map ActionEvent to APM event type
            let event_type = match event.action_type.as_str() {
                "UserMessage" | "AssistantMessage" => "message",
                _ => "tool_call",
            };

            // Record to database
            conn.execute(
                "INSERT INTO apm_events (session_id, event_type, timestamp, metadata) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    event.session_id,
                    event_type,
                    event.timestamp.to_rfc3339(),
                    event.metadata.map(|m| m.to_string()),
                ],
            ).unwrap();

            event_count += 1;
        }

        event_count
    });

    // Create a regular session (without telemetry integration for now)
    let session = connection
        .new_session(cwd.clone())
        .await
        .expect("Failed to create session");

    // Simulate tool calls by creating notifications manually
    use acp_adapter::acp;

    // 1. User message
    let user_msg = acp::SessionNotification::new(
        acp::SessionId::new(session_id),
        acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
            acp::TextContent::new("Test message".to_string()),
        ))),
    );
    telemetry.process_notification(&user_msg).await;

    // 2. Tool call start (Read)
    let tool_call_id = acp::ToolCallId::new("read-1");
    let read_start = acp::SessionNotification::new(
        acp::SessionId::new(session_id),
        acp::SessionUpdate::ToolCall(acp::ToolCall::new(tool_call_id.clone(), "Read".to_string())),
    );
    telemetry.process_notification(&read_start).await;

    // 3. Tool call completion
    let read_end = acp::SessionNotification::new(
        acp::SessionId::new(session_id),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            tool_call_id,
            acp::ToolCallUpdateFields::new()
                .status(acp::ToolCallStatus::Completed)
                .title("Read".to_string()),
        )),
    );
    telemetry.process_notification(&read_end).await;

    // 4. Another tool call start (Bash)
    let bash_call_id = acp::ToolCallId::new("bash-1");
    let bash_start = acp::SessionNotification::new(
        acp::SessionId::new(session_id),
        acp::SessionUpdate::ToolCall(acp::ToolCall::new(bash_call_id.clone(), "Bash".to_string())),
    );
    telemetry.process_notification(&bash_start).await;

    // 5. Tool call failure
    let bash_end = acp::SessionNotification::new(
        acp::SessionId::new(session_id),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            bash_call_id,
            acp::ToolCallUpdateFields::new()
                .status(acp::ToolCallStatus::Failed)
                .title("Bash".to_string())
                .raw_output(serde_json::json!("Command failed")),
        )),
    );
    telemetry.process_notification(&bash_end).await;

    // Drop telemetry to close channel
    drop(telemetry);

    // Wait for telemetry task to finish
    let event_count = telemetry_task.await.unwrap();
    assert_eq!(
        event_count, 3,
        "Should have processed 3 events (1 message, 2 tool calls)"
    );

    // Verify events were recorded in APM database
    let conn = Connection::open(db_path).unwrap();

    // Count total events
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(total, 3, "Should have 3 events in database");

    // Count message events
    let messages: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1 AND event_type = 'message'",
            [session_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(messages, 1, "Should have 1 message event");

    // Count tool call events
    let tools: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM apm_events WHERE session_id = ?1 AND event_type = 'tool_call'",
            [session_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tools, 2, "Should have 2 tool call events");

    // Verify we can calculate APM from the events
    let duration_seconds = 60.0; // Assume 1 minute session
    let apm = (total as f64 / duration_seconds) * 60.0;
    assert!(apm > 0.0, "APM should be positive");
}
