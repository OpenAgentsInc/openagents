//! Demonstration of ACP adapter telemetry
//!
//! This example shows how to use the APM telemetry hooks to track
//! actions in an ACP session.

use acp_adapter::telemetry::ApmTelemetry;
use agent_client_protocol_schema as acp;

#[tokio::main]
async fn main() {
    // Create a telemetry tracker
    let (telemetry, mut rx) = ApmTelemetry::new("demo-session");

    // Spawn a task to consume and print events
    let consumer = tokio::spawn(async move {
        let mut total_actions = 0;
        let mut successful_actions = 0;
        let mut failed_actions = 0;

        while let Some(event) = rx.recv().await {
            total_actions += 1;
            if event.success {
                successful_actions += 1;
            } else {
                failed_actions += 1;
            }

            println!(
                "[{}] {} - {} ({}ms) - {}",
                event.timestamp.format("%H:%M:%S"),
                event.action_type,
                if event.success { "✓" } else { "✗" },
                event.duration_ms,
                event
                    .error
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("success")
            );
        }

        println!("\n=== APM Summary ===");
        println!("Total actions: {}", total_actions);
        println!("Successful: {}", successful_actions);
        println!("Failed: {}", failed_actions);
        println!(
            "Success rate: {:.1}%",
            (successful_actions as f64 / total_actions as f64) * 100.0
        );
    });

    // Simulate some ACP session activity
    println!("Simulating ACP session events...\n");

    let session_id = acp::SessionId::new("demo-session");

    // User message
    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(
                acp::ContentBlock::Text(acp::TextContent::new(
                    "Read the README file".to_string(),
                )),
            )),
        ))
        .await;

    // Tool call - Read (successful)
    let read_call_id = acp::ToolCallId::new("read-1");
    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCall(acp::ToolCall::new(
                read_call_id.clone(),
                "Read".to_string(),
            )),
        ))
        .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                read_call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Completed)
                    .title("Read".to_string())
                    .raw_output(serde_json::json!("# OpenAgents\n...")),
            )),
        ))
        .await;

    // Tool call - Bash (failed)
    let bash_call_id = acp::ToolCallId::new("bash-1");
    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCall(acp::ToolCall::new(
                bash_call_id.clone(),
                "Bash".to_string(),
            )),
        ))
        .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                bash_call_id,
                acp::ToolCallUpdateFields::new()
                    .status(acp::ToolCallStatus::Failed)
                    .title("Bash".to_string())
                    .raw_output(serde_json::json!("command not found: invalidcmd")),
            )),
        ))
        .await;

    // Assistant message
    telemetry
        .process_notification(&acp::SessionNotification::new(
            session_id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                acp::ContentBlock::Text(acp::TextContent::new(
                    "I found the README file.".to_string(),
                )),
            )),
        ))
        .await;

    // Drop telemetry to close the channel
    drop(telemetry);

    // Wait for consumer to finish
    consumer.await.unwrap();
}
