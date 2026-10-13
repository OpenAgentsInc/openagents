//! Coder's delegation host (#11511): call-time checks, and the parent's
//! tool list held fixed while plugins turn on and off.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use super::*;
use crate::bundled_runtime::AcpAgent;

fn agent(id: &str, name: &str) -> AcpAgent {
    AcpAgent {
        id: id.into(),
        name: name.into(),
        program: "/bin/sh".into(),
        ..serde_json::from_value(json!({"id":"x","name":"x","program":"x"})).unwrap()
    }
}

fn execution() -> ExecutionSettings {
    let mut execution = crate::App::default().agent_execution();
    execution.claude = true;
    execution.acp = true;
    execution.microcoder = true;
    execution.agents = vec![agent("codex", "Codex"), agent("opencode", "OpenCode")];
    execution
}

fn host(execution: ExecutionSettings) -> CoderHost {
    CoderHost::new(execution, None, Arc::new(AtomicBool::new(false)), None)
}

fn request(to: &str, model: Option<&str>) -> Request {
    Request {
        to: to.into(),
        model: model.map(str::to_owned),
        task: "x".into(),
        ..Request::default()
    }
}

#[test]
fn harnesses_and_models_are_checked_when_the_call_arrives() {
    let host = host(execution());
    let ids: Vec<String> = available(&execution(), false)
        .into_iter()
        .map(|harness| harness.id)
        .collect();
    assert_eq!(ids, vec!["claude-code", "codex", "opencode", "coder"]);
    assert_eq!(
        host.target(&request("auto", None)).unwrap().harness,
        "claude-code"
    );
    assert_eq!(
        host.target(&request("codex", Some("gpt-6-astra")))
            .unwrap()
            .model
            .as_deref(),
        Some("gpt-6-astra")
    );
    let error = host
        .target(&request("codex", Some("claude-opus-5-5")))
        .unwrap_err();
    assert!(error.contains("gpt-6.1-sol"), "{error}");
    let error = host
        .target(&request("opencode", Some("opencode/big-pickle")))
        .unwrap_err();
    assert!(error.contains("leave model out"), "{error}");
    // OpenCode as a harness, once installed: its models are taken.
    crate::opencode_chat::install_for_test(true);
    assert_eq!(
        host.target(&request(
            "opencode",
            Some(crate::opencode_chat::DEFAULT_MODEL)
        ))
        .unwrap()
        .model
        .as_deref(),
        Some(crate::opencode_chat::DEFAULT_MODEL)
    );
    crate::opencode_chat::install_for_test(false);
    let error = host.target(&request("cursor", None)).unwrap_err();
    assert!(error.contains("codex, opencode, coder"), "{error}");
    // Turned off after the chat started: refused at the call, not hidden.
    let mut off = execution();
    off.claude = false;
    off.microcoder = false;
    let error = self::host(off)
        .target(&request("claude-code", None))
        .unwrap_err();
    assert!(error.contains("not available"), "{error}");
    assert_eq!(
        self::host(execution())
            .target(&request("coder-loop", None))
            .unwrap()
            .harness,
        "coder"
    );
}

/// The cache check's second half: the parent's tool list is the same
/// bytes before and after a harness plugin is toggled.
#[tokio::test(flavor = "multi_thread")]
async fn toggling_a_harness_plugin_leaves_the_tool_list_alone() {
    let bytes = |execution: ExecutionSettings| async move {
        let (events, _) = tokio::sync::mpsc::unbounded_channel();
        let server = crate::claude_tools::tool_server(
            &crate::claude_tools::Tools::new(execution),
            CLAUDE,
            events,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        (
            server.list_bytes().await,
            coder_sdk::codex::dynamic_tools(&server).to_string(),
        )
    };
    let on = execution();
    let before = bytes(on.clone()).await;
    let toggles: Vec<(&str, Box<dyn Fn(&mut ExecutionSettings)>)> = vec![
        ("Coder loop off", Box::new(|e| e.microcoder = false)),
        ("local agents off", Box::new(|e| e.acp = false)),
        ("OpenCode off", Box::new(|e| e.agents[1].enabled = false)),
        (
            "an agent installed",
            Box::new(|e| e.agents.push(agent("grok-build", "Grok Build"))),
        ),
    ];
    for (toggle, apply) in toggles {
        let mut changed = on.clone();
        apply(&mut changed);
        assert_eq!(bytes(changed.clone()).await, before, "{toggle}");
        assert_eq!(
            crate::claude_tools::instructions(&crate::claude_tools::Tools::new(changed)),
            crate::claude_tools::instructions(&crate::claude_tools::Tools::new(on.clone())),
            "{toggle}"
        );
    }
    // Claude Code off also takes Solver away and narrows the routing
    // preference tool (#11516, #11518): those are their own plugins' tools,
    // not delegation's. `delegate` itself stays the same bytes.
    let mut claude_off = on.clone();
    claude_off.claude = false;
    let (events, _) = tokio::sync::mpsc::unbounded_channel();
    let delegate = |execution: ExecutionSettings| {
        crate::claude_tools::tool_server(
            &crate::claude_tools::Tools::new(execution),
            CLAUDE,
            events.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .tools()
        .iter()
        .find(|tool| tool.name == "delegate")
        .map(|tool| (tool.description.clone(), tool.schema.to_string()))
    };
    assert_eq!(delegate(claude_off), delegate(on.clone()));
    let names = crate::claude_tools::names(&on);
    assert!(names.iter().any(|name| name == "delegate"), "{names:?}");
    for gone in ["agent", "acp_subagent", "microcoder"] {
        assert!(!names.iter().any(|name| name == gone), "{gone}");
    }
}

#[test]
fn the_coder_loop_offers_the_same_delegate_tool() {
    let mut execution = execution();
    execution.shell = true;
    let definition = execution
        .defs()
        .into_iter()
        .find(|tool| tool["function"]["name"] == "delegate")
        .unwrap();
    assert_eq!(definition, coder_sdk::delegation::function_definition());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_writing_child_outside_git_is_refused_and_reported_short() {
    let dir = tempfile::tempdir().unwrap();
    let mut execution = execution();
    execution.cwd = dir.path().to_owned();
    execution.fleet = None;
    let delegator = Delegator::new(
        Arc::new(host(execution)),
        Lineage::root("run-x", CODER),
        Authority::default(),
        &dir.path().join("runs"),
    );
    let report = delegator
        .delegate(json!({"to": "codex", "task": "edit"}))
        .await
        .unwrap();
    assert_eq!(report.outcome, Outcome::Failed);
    assert!(report.error.unwrap().contains("read_only"));
    let error = delegator
        .delegate(json!({"to": "codex", "task": "x", "background": true}))
        .await
        .unwrap_err();
    assert!(error.contains("agent tray"), "{error}");
}
