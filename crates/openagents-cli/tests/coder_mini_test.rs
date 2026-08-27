#[allow(dead_code)]
mod support;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use openagents_cli::coder::runtime::Control;
use openagents_cli::delegate::ChildOptions;
use openagents_cli::tools::{DelegationGate, HarnessToolRegistry, ToolCall};

#[tokio::test]
async fn a_coder_mini_run_reports_done_with_a_tool_use_count() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("finding.txt");
    std::fs::write(&source, "the finding").unwrap();
    let stub = support::start_calling_read(source.display().to_string(), "Read the finding.").await;
    let registry = HarnessToolRegistry::with_delegation(
        Some(root.path().to_path_buf()),
        DelegationGate {
            lane: "flash".to_string(),
            user_token: Some("test-token".to_string()),
            api_base: Some(stub.base),
            max_count: 2,
            child: ChildOptions::default(),
            acp_agents: Vec::new(),
            acp_spent: Arc::new(AtomicBool::new(false)),
        },
    );

    let output = registry
        .execute_tool(&ToolCall {
            id: "delegate-mini".to_string(),
            name: "delegate".to_string(),
            arguments: serde_json::json!({
                "agent": "coder-mini",
                "prompt": "Read finding.txt and report it."
            }),
        })
        .await;

    assert!(!output.is_error, "{}", output.output);
    assert!(
        output.output.starts_with("Done · 1 tool uses ·"),
        "{}",
        output.output
    );
    assert!(
        output.output.ends_with("Read the finding."),
        "{}",
        output.output
    );
}

#[tokio::test]
async fn a_coder_mini_run_streams_child_tools_into_the_parent_sink() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("finding.txt");
    std::fs::write(&source, "the finding").unwrap();
    let stub = support::start_calling_read(source.display().to_string(), "Read the finding.").await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let report = Arc::clone(&seen);
    let registry = HarnessToolRegistry::with_delegation(
        Some(root.path().to_path_buf()),
        DelegationGate {
            lane: "flash".to_string(),
            user_token: Some("test-token".to_string()),
            api_base: Some(stub.base),
            max_count: 2,
            child: ChildOptions::default(),
            acp_agents: Vec::new(),
            acp_spent: Arc::new(AtomicBool::new(false)),
        },
    )
    .with_event_sink(Arc::new(move |event| {
        report.lock().unwrap().push(event);
    }));

    let output = registry
        .execute_tool(&ToolCall {
            id: "delegate-mini".to_string(),
            name: "delegate".to_string(),
            arguments: serde_json::json!({
                "agent": "coder-mini",
                "prompt": "Read finding.txt and report it."
            }),
        })
        .await;

    assert!(!output.is_error, "{}", output.output);
    let events = seen.lock().unwrap();
    assert!(
        events.iter().any(|event| matches!(
            event,
            Control::Tool {
                call_id,
                name,
                ..
            } if call_id == "delegate-mini" && name == "read"
        )),
        "child read did not stream into the parent call: {events:?}"
    );
    assert!(
        !events.iter().any(
            |event| matches!(event, Control::ToolDone { call_id, .. } if call_id == "delegate-mini")
        ),
        "child ToolDone settled the parent box: {events:?}"
    );
}
