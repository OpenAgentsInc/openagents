#[allow(dead_code)]
mod support;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use openagents_cli::coder::runtime::Control;
use openagents_cli::delegate::ChildOptions;
use openagents_cli::delegate_result::{DelegateAgentResult, DelegateStatus, WorktreeOutcome};
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
    let result = DelegateAgentResult::parse(&output.output)
        .unwrap_or_else(|| panic!("expected result JSON: {}", output.output));
    assert_eq!(result.status, DelegateStatus::Done);
    assert_eq!(result.agent, "coder-mini");
    assert_eq!(result.total_tool_uses, 1);
    assert_eq!(result.total_tokens, 116);
    assert_eq!(result.model.as_deref(), Some("glm-5.3-flash"));
    assert_eq!(result.session_id.as_deref(), Some("th_mini"));
    assert_eq!(result.worktree, WorktreeOutcome::Unused);
    let raw: serde_json::Value = serde_json::from_str(&output.output).unwrap();
    assert!(
        raw.get("worktree").is_none(),
        "a run without isolation must omit worktree: {}",
        output.output
    );
    assert_eq!(result.report, "Read the finding.");
    let shown = result.render();
    assert!(shown.starts_with("Done · 1 tool uses ·"), "{shown}");
    assert!(
        shown.contains("116 tokens") || shown.contains("0.1k tokens"),
        "{shown}"
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

fn init_git_repo(root: &std::path::Path) {
    for argv in [
        vec!["init", "--quiet", "-b", "main"],
        vec!["config", "user.email", "test@example.test"],
        vec!["config", "user.name", "Test"],
        vec!["add", "."],
        vec![
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "root",
        ],
    ] {
        let done = std::process::Command::new("git")
            .args(&argv)
            .current_dir(root)
            .output()
            .expect("run git");
        assert!(done.status.success(), "git {argv:?}: {done:?}");
    }
}

fn worktree_list(root: &std::path::Path) -> String {
    let done = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(root)
        .output()
        .expect("git worktree list");
    String::from_utf8_lossy(&done.stdout).into_owned()
}

fn mini_gate(_root: &std::path::Path, api_base: String) -> DelegationGate {
    DelegationGate {
        lane: "flash".to_string(),
        user_token: Some("test-token".to_string()),
        api_base: Some(api_base),
        max_count: 2,
        child: ChildOptions::default(),
        acp_agents: Vec::new(),
        acp_spent: Arc::new(AtomicBool::new(false)),
    }
}

#[tokio::test]
async fn an_unresolvable_model_refuses_before_any_child_starts() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "hello\n").unwrap();
    init_git_repo(root.path());
    let before = worktree_list(root.path());
    let stub = support::start_calling_read(
        root.path().join("README.md").display().to_string(),
        "unused",
    )
    .await;
    let registry = HarnessToolRegistry::with_delegation(
        Some(root.path().to_path_buf()),
        mini_gate(root.path(), stub.base),
    );

    let output = registry
        .execute_tool(&ToolCall {
            id: "delegate-mini".to_string(),
            name: "delegate".to_string(),
            arguments: serde_json::json!({
                "agent": "coder-mini",
                "tools": "read-write",
                "isolation": "worktree",
                "model": "not-a-served-model",
                "prompt": "edit README"
            }),
        })
        .await;

    assert!(output.is_error, "{}", output.output);
    assert!(
        output.output.contains("not-a-served-model"),
        "{}",
        output.output
    );
    assert!(
        !output.output.contains("worktree kept"),
        "{}",
        output.output
    );
    assert_eq!(worktree_list(root.path()), before);
}

#[tokio::test]
async fn an_unchanged_worktree_is_removed() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("finding.txt"), "the finding").unwrap();
    init_git_repo(root.path());
    let before = worktree_list(root.path());
    let stub = support::start_calling_read("finding.txt".to_string(), "Read the finding.").await;
    let registry = HarnessToolRegistry::with_delegation(
        Some(root.path().to_path_buf()),
        mini_gate(root.path(), stub.base),
    );

    let output = registry
        .execute_tool(&ToolCall {
            id: "delegate-mini".to_string(),
            name: "delegate".to_string(),
            arguments: serde_json::json!({
                "agent": "coder-mini",
                "tools": "read-write",
                "isolation": "worktree",
                "prompt": "Read finding.txt and report it."
            }),
        })
        .await;

    assert!(!output.is_error, "{}", output.output);
    let result = DelegateAgentResult::parse(&output.output)
        .unwrap_or_else(|| panic!("expected result JSON: {}", output.output));
    assert_eq!(result.status, DelegateStatus::Done);
    assert_eq!(result.worktree, WorktreeOutcome::Removed);
    let raw: serde_json::Value = serde_json::from_str(&output.output).unwrap();
    assert!(
        raw.get("worktree").unwrap().is_null(),
        "removed worktree must be JSON null, not omitted: {}",
        output.output
    );
    assert!(
        result.render().contains("worktree removed (no changes)"),
        "{}",
        result.render()
    );
    assert_eq!(result.model.as_deref(), Some("glm-5.3-flash"));
    assert_eq!(result.session_id.as_deref(), Some("th_mini"));
    assert_eq!(worktree_list(root.path()), before);
}

#[tokio::test]
async fn a_changed_worktree_is_kept_and_named() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "hello\n").unwrap();
    init_git_repo(root.path());
    let stub = support::start_calling_write(
        "README.md".to_string(),
        "# hello\n\nA hello section.\n".to_string(),
        "Added a hello section.",
    )
    .await;
    let registry = HarnessToolRegistry::with_delegation(
        Some(root.path().to_path_buf()),
        mini_gate(root.path(), stub.base),
    );

    let output = registry
        .execute_tool(&ToolCall {
            id: "delegate-mini".to_string(),
            name: "delegate".to_string(),
            arguments: serde_json::json!({
                "agent": "coder-mini",
                "tools": "read-write",
                "isolation": "worktree",
                "prompt": "edit README to add a hello section"
            }),
        })
        .await;

    assert!(!output.is_error, "{}", output.output);
    let result = DelegateAgentResult::parse(&output.output)
        .unwrap_or_else(|| panic!("expected result JSON: {}", output.output));
    let kept = result
        .worktree
        .as_kept()
        .unwrap_or_else(|| panic!("missing worktree: {}", output.output));
    assert!(
        kept.branch
            .as_deref()
            .is_some_and(|branch| branch.starts_with("agent-")),
        "kept branch was {:?}",
        kept.branch
    );
    let path = kept.path.as_str();
    let listed = worktree_list(root.path());
    assert!(
        listed.contains(path),
        "worktree list missing {path}: {listed}"
    );
    let status = std::process::Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output()
        .expect("git status");
    assert!(
        !String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "kept worktree has no changes"
    );
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", path])
        .current_dir(root.path())
        .output();
}
