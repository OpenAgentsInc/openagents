use serde_json::json;

use super::super::outcome::HookOutcome;
use super::super::registry::{Hook, Source};
use super::*;

fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// A rewrite that wraps `cargo test`, and a floor that denies a merge in
/// whatever command it is finally given.
fn hooks() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(Hook::floor(
            "floors",
            Event::PreToolUse,
            Permission::ask("confirm"),
            |call: &Call| {
                let command = call.input["command"].as_str().unwrap_or_default();
                Ok(if command.contains("gh pr merge") {
                    HookOutcome::deny(&format!("held: {command}"))
                } else if command.contains("deploy") {
                    HookOutcome::ask("deploy needs the person")
                } else {
                    HookOutcome::none()
                })
            },
        ))
        .unwrap();
    registry
        .register(
            Hook::new(
                "squeeze",
                Event::PreToolUse,
                Source::BuiltIn,
                |call: &Call| {
                    let command = call.input["command"].as_str().unwrap_or_default();
                    Ok(if command.starts_with("cargo test") {
                        HookOutcome::rewrite(json!({"command": format!("squeeze -- {command}")}))
                    } else {
                        HookOutcome::none()
                    })
                },
            )
            .matching("Bash"),
        )
        .unwrap();
    registry
        .register(Hook::new(
            "note",
            Event::PostToolUse,
            Source::BuiltIn,
            |_| {
                Ok(HookOutcome::context(Context::new(
                    "monitor",
                    "3 tests failed",
                    1,
                )))
            },
        ))
        .unwrap();
    registry
        .register(Hook::new("done", Event::Stop, Source::Decision, |_| {
            Ok(HookOutcome::stopping(Stop::Block {
                reason: "run the tests first".into(),
            }))
        }))
        .unwrap();
    registry
}

fn bash(command: &str) -> Value {
    json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
        "tool_input": {"command": command}})
}

fn backend() -> Claude {
    Claude::new(hooks(), Budget::default()).unwrap()
}

#[test]
fn one_hook_per_event_matching_the_union_of_tools() {
    let options = backend().install(QueryOptions::new(), PreTool::Hook);
    let pre = &options.hooks[&HookEvent::PreToolUse];
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0].matcher, None, "the floors fire on every tool");
    assert_eq!(options.hooks[&HookEvent::PostToolUse].len(), 1);
    assert_eq!(options.hooks[&HookEvent::Stop].len(), 1);
    assert!(!options.hooks.contains_key(&HookEvent::SessionStart));
    // On the permission channel PreToolUse is the callback's.
    let options = backend().install(QueryOptions::new(), PreTool::Permission);
    assert!(!options.hooks.contains_key(&HookEvent::PreToolUse));
    assert_eq!(
        union_matcher([Some("Bash"), Some("Edit|Bash")].into_iter()).as_deref(),
        Some("Bash|Edit")
    );
}

#[test]
fn the_floors_judge_the_rewritten_command_on_claude_code() {
    let options = backend().install(QueryOptions::new(), PreTool::Hook);
    let hook = options.hooks[&HookEvent::PreToolUse][0].hooks[0].clone();
    let value = |input| serde_json::to_value(block(hook.call(input, None)).unwrap()).unwrap();
    // Squeeze rewrites; the floors have nothing against the result.
    let squeezed = value(bash("cargo test -p x"));
    assert_eq!(
        squeezed["hookSpecificOutput"]["updatedInput"]["command"],
        "squeeze -- cargo test -p x"
    );
    assert!(
        squeezed["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none()
    );
    // Both fire: the floors see the rewritten command and deny it.
    let denied = value(bash("cargo test && gh pr merge 3"));
    assert_eq!(denied["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(
        denied["hookSpecificOutput"]["permissionDecisionReason"],
        "held: squeeze -- cargo test && gh pr merge 3"
    );
    assert!(denied["hookSpecificOutput"].get("updatedInput").is_none());
    // Nothing to say lets the call through unchanged.
    let plain = value(bash("ls"));
    assert_eq!(plain, json!({"continue": true}));
}

#[test]
fn an_ask_goes_to_the_asker_or_is_held() {
    let held = block(backend().fire(call(Event::PreToolUse, &bash("deploy prod"))));
    assert_eq!(held.permission, Permission::deny("deploy needs the person"));
    let approving: Asker = Arc::new(|_| Box::pin(async { Permission::NoOpinion }));
    let approved = block(
        backend()
            .asking(approving)
            .fire(call(Event::PreToolUse, &bash("deploy prod"))),
    );
    assert_eq!(approved.permission, Permission::NoOpinion);
}

#[test]
fn context_stop_and_end_read_as_claude_code_expects() {
    let options = backend().install(QueryOptions::new(), PreTool::Hook);
    let post = options.hooks[&HookEvent::PostToolUse][0].hooks[0].clone();
    let out = block(post.call(
        json!({"tool_name": "Bash", "tool_input": {}, "tool_response": "x"}),
        None,
    ))
    .unwrap();
    let out = serde_json::to_value(out).unwrap();
    assert_eq!(
        out["hookSpecificOutput"],
        json!({"hookEventName": "PostToolUse", "additionalContext": "[monitor] 3 tests failed"})
    );
    let stop = options.hooks[&HookEvent::Stop][0].hooks[0].clone();
    let out = serde_json::to_value(block(stop.call(json!({}), None)).unwrap()).unwrap();
    assert_eq!(out["decision"], "block");
    assert_eq!(out["reason"], "run the tests first");
    let mut ended = unchanged(&Call::new(Event::PostToolBatch, Value::Null));
    ended.stop = Stop::End;
    let out = output(&ended);
    assert_eq!(out.continue_execution, Some(false));
    assert!(out.stop_reason.is_some());
}

#[test]
fn the_permission_channel_carries_rewrites_and_interrupts_on_end() {
    let mut applied = unchanged(&Call::tool(
        Event::PreToolUse,
        "Bash",
        json!({"command": "a"}),
    ));
    applied.input = json!({"command": "b"});
    let same = |a: PermissionResult, b: PermissionResult| {
        assert_eq!(
            serde_json::to_value(a).unwrap(),
            serde_json::to_value(b).unwrap()
        );
    };
    same(
        permission_result(&applied),
        PermissionResult::allow(json!({"command": "b"})),
    );
    applied.permission = Permission::deny("outside the worktree");
    same(
        permission_result(&applied),
        PermissionResult::deny("outside the worktree"),
    );
    applied.stop = Stop::End;
    same(
        permission_result(&applied),
        PermissionResult::deny_and_interrupt("outside the worktree"),
    );
    let handler = backend().permission_handler();
    let result = block(handler.can_use_tool(
        "Bash",
        &json!({"command": "cargo test"}),
        None,
        None,
        None,
        "t1",
        None,
    ))
    .unwrap();
    same(
        result,
        PermissionResult::allow(json!({"command": "squeeze -- cargo test"})),
    );
}

#[test]
fn the_box_applies_the_recipe_row() {
    let options = SessionBox {
        system_prompt: Some("You are lean.".into()),
        tools: Some(vec!["Bash".into(), "Read".into()]),
        setting_sources: Some(Vec::new()),
        model: Some("claude-haiku-5-5".into()),
        ..SessionBox::for_recipe(
            route_contract::recipe::CLAUDE_SESSION,
            Some(route_contract::recipe::TaskClass::Question),
            Some("high"),
        )
    }
    .apply(QueryOptions::new());
    assert_eq!(options.effort, Some(claude_agent_sdk::EffortLevel::Low));
    assert_eq!(options.model.as_deref(), Some("claude-haiku-5-5"));
    assert!(matches!(
        options.tools,
        Some(claude_agent_sdk::ToolsConfig::Names(ref names)) if names.len() == 2
    ));
    assert_eq!(options.setting_sources, Some(Vec::new()));
    assert_eq!(
        effort_level("XHigh"),
        Some(claude_agent_sdk::EffortLevel::Xhigh)
    );
    assert_eq!(effort_level("loud"), None);
}
