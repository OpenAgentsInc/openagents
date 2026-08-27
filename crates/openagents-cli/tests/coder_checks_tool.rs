//! The `check` tool: named scopes, and failure attribution against a baseline.
//!
//! `.openagents/checks.json` is where a repository says what "verify" means
//! at each width, and the tool is the model's way to ask for one by name
//! instead of assembling suite invocations mid-loop. A failure gets checked
//! against the session's known-failures baseline — taken from clean-tree
//! runs — so "is this mine" is a file read, not a second sweep.

use openagents_cli::checks::{CheckScope, ChecksConfig, FailureBaseline, KnownFailure};
use openagents_cli::coder::tui::now_ms;
use openagents_cli::tools::HarnessToolRegistry;
use openagents_cli::tools::ToolCall;

/// Declares one failing scope (`diff`) and one passing scope (`full`).
fn a_failing_command_registry(root: &std::path::Path) -> HarnessToolRegistry {
    let dir = root.join(".openagents");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        root.join(".openagents/checks.json"),
        r#"{
            "scopes": {
                "diff": { "run": ["exit 3"] },
                "full": { "description": "everything", "run": ["true"] }
            }
        }"#,
    )
    .unwrap();
    HarnessToolRegistry::new(Some(root.to_path_buf()))
}

fn run_check(registry: HarnessToolRegistry, scope: &str) -> openagents_cli::tools::ToolOutput {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(registry.execute_tool(&ToolCall {
        id: "c".to_string(),
        name: "check".to_string(),
        arguments: serde_json::json!({ "scope": scope }),
    }))
}

#[test]
fn the_check_tool_is_declared_only_where_scopes_exist() {
    let bare = tempfile::tempdir().unwrap();
    let registry = HarnessToolRegistry::new(Some(bare.path().to_path_buf()));
    assert!(
        !registry
            .list_tools()
            .iter()
            .any(|tool| tool.name == "check"),
        "a repository without checks.json declares no check tool"
    );

    let declared = tempfile::tempdir().unwrap();
    let registry = a_failing_command_registry(declared.path());
    assert!(
        registry
            .list_tools()
            .iter()
            .any(|tool| tool.name == "check"),
        "a repository with scopes declares the tool"
    );
}

#[test]
fn an_unknown_scope_names_what_is_declared() {
    let root = tempfile::tempdir().unwrap();
    let registry = a_failing_command_registry(root.path());
    let output = run_check(registry, "package");
    assert!(output.is_error);
    assert!(output.output.contains("diff"), "names declared scopes");
    assert!(output.output.contains("full"));
}

#[test]
fn a_failing_scope_reports_failure_and_attribution() {
    let root = tempfile::tempdir().unwrap();
    let registry = a_failing_command_registry(root.path());
    let output = run_check(registry, "diff");
    assert!(output.is_error);
    assert!(output.output.contains("FAILED"), "{}", output.output);
    assert!(
        output.output.contains("new — no clean-tree baseline"),
        "without a baseline the failure is labelled new: {}",
        output.output
    );
}

#[test]
fn a_baseline_failure_is_labelled_inherited() {
    let root = tempfile::tempdir().unwrap();
    let registry = a_failing_command_registry(root.path());
    let session = tempfile::tempdir().unwrap();
    let registry = registry.keeping_session_logs(session.path().to_path_buf());
    let mut baseline = FailureBaseline::default();
    baseline.record_failures(
        None,
        vec![KnownFailure {
            scope: "diff".to_string(),
            command: "exit 3".to_string(),
            recorded_at_ms: now_ms(),
            summary: "seeded".to_string(),
        }],
    );
    baseline.store(session.path());

    let output = run_check(registry, "diff");
    assert!(output.is_error);
    assert!(
        output
            .output
            .contains("inherited — it already failed on a clean tree"),
        "a baseline failure reads as inherited: {}",
        output.output
    );
}

#[test]
fn the_config_round_trips_through_the_public_api() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".openagents");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        root.path().join(".openagents/checks.json"),
        r#"{ "scopes": { "diff": { "run": ["true"] } } }"#,
    )
    .unwrap();
    let config = ChecksConfig::load(root.path()).unwrap().unwrap();
    let scope: &CheckScope = config.scope("diff").unwrap();
    assert_eq!(scope.run, vec!["true".to_string()]);
}
