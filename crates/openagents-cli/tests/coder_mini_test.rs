#[allow(dead_code)]
mod support;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
