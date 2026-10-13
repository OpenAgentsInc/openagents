use std::path::Path;

use acp_client::wire::PermissionRequest;
use serde_json::json;

use super::super::bridge::{Format, answer as bridge_answer};
use super::super::compose::Runner;
use super::super::outcome::{Context, HookOutcome};
use super::super::registry::{Hook, Source};
use super::*;

fn ask(value: serde_json::Value) -> PermissionRequest {
    serde_json::from_value(value).unwrap()
}

const OPTIONS: &str = r#"[{"optionId":"once","kind":"allow_once","name":"Allow once"},{"optionId":"always","kind":"allow_always","name":"Always allow"},{"optionId":"reject","kind":"reject_once","name":"Reject"}]"#;

fn options() -> serde_json::Value {
    serde_json::from_str(OPTIONS).unwrap()
}

/// A floor that refuses `rm` and holds a merge for the person.
fn floor() -> Hook {
    Hook::floor(
        "floors",
        Event::PreToolUse,
        Permission::ask("Held for the person: Coder could not check this step."),
        |call: &Call| {
            let command = call.input["command"].as_str().unwrap_or_default();
            Ok(if command.starts_with("rm ") {
                HookOutcome::deny("Refused: this removes files.")
            } else if command.contains("pr merge") {
                HookOutcome::ask("Held for the person: a merge.")
            } else {
                HookOutcome::none()
            })
        },
    )
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(floor()).unwrap();
    registry
}

#[test]
fn the_agents_tool_names_read_as_claude_codes() {
    let (tool, input) = claude_tool(
        "run_terminal_command",
        &json!({"command": "echo hi", "description": "say hi"}),
    );
    assert_eq!(tool, "Bash");
    assert_eq!(input["command"], "echo hi");
    assert_eq!(input["description"], "say hi");
    let (tool, input) = claude_tool("exec", &json!({"command": ["git", "status"]}));
    assert_eq!(
        (tool.as_str(), input["command"].as_str()),
        ("Bash", Some("git status"))
    );
    let (tool, input) = claude_tool("read_file", &json!({"target_file": "/w/.env"}));
    assert_eq!(
        (tool.as_str(), input["file_path"].as_str()),
        ("Read", Some("/w/.env"))
    );
    let (tool, input) = claude_tool("read", &json!({"file_path": "/w/a"}));
    assert_eq!(
        (tool.as_str(), input["file_path"].as_str()),
        ("Read", Some("/w/a"))
    );
    let (tool, input) = claude_tool(
        "search_replace",
        &json!({"file_path": "/w/b", "old_string": "x"}),
    );
    assert_eq!(
        (tool.as_str(), input["old_string"].as_str()),
        ("Edit", Some("x"))
    );
    let (tool, input) = claude_tool("write", &json!({"path": "/w/c"}));
    assert_eq!(
        (tool.as_str(), input["file_path"].as_str()),
        ("Write", Some("/w/c"))
    );
    let (tool, input) = claude_tool("grep", &json!({"pattern": "p"}));
    assert_eq!((tool.as_str(), input), ("grep", json!({"pattern": "p"})));
}

#[test]
fn an_ask_reads_as_the_hooks_call_by_its_kind() {
    let cwd = Path::new("/w");
    // OpenCode's command ask carries the command.
    let command = call(
        &ask(
            json!({"toolCall": {"toolCallId": "c1", "title": "echo hi > a", "kind": "execute", "rawInput": {"command": "echo hi > a"}}, "options": options()}),
        ),
        None,
        cwd,
    );
    assert_eq!(command.tool.as_deref(), Some("Bash"));
    assert_eq!(command.input["command"], "echo hi > a");
    assert_eq!(command.input["cwd"], "/w");
    // Cursor's carries none: its own update's input is used, else the title.
    let request = ask(
        json!({"toolCall": {"toolCallId": "t1", "title": "`gh pr merge 5`", "kind": "execute"}, "options": options()}),
    );
    let seen = json!({"command": "gh pr merge 5 --squash"});
    assert_eq!(
        call(&request, Some(&seen), cwd).input["command"],
        "gh pr merge 5 --squash"
    );
    assert_eq!(call(&request, None, cwd).input["command"], "gh pr merge 5");
    // An edit names its file in its locations.
    let edit = call(
        &ask(
            json!({"toolCall": {"title": "/w/README.md", "kind": "edit", "locations": [{"path": "/w/README.md"}], "rawInput": {"filepath": "/w/README.md", "diff": "+x"}}, "options": options()}),
        ),
        None,
        cwd,
    );
    assert_eq!(edit.tool.as_deref(), Some("Edit"));
    assert_eq!(edit.input["file_path"], "/w/README.md");
    // OpenCode's read ask names no file until it is allowed.
    let read = call(
        &ask(
            json!({"toolCall": {"title": "read", "kind": "read", "rawInput": {}}, "options": options()}),
        ),
        None,
        cwd,
    );
    assert_eq!(read.tool.as_deref(), Some("Read"));
    assert!(read.input.get("file_path").is_none());
}

#[test]
fn the_answer_is_the_agents_own_one_time_option() {
    let request = ask(json!({"toolCall": {"kind": "execute"}, "options": options()}));
    let pick = |permission: Permission| answer(&permission, &request);
    assert_eq!(
        pick(Permission::NoOpinion),
        PermissionAnswer::Selected("once".into())
    );
    assert_eq!(
        pick(Permission::Allow),
        PermissionAnswer::Selected("once".into())
    );
    assert_eq!(
        pick(Permission::deny("no")),
        PermissionAnswer::Selected("reject".into())
    );
    assert_eq!(
        pick(Permission::ask("held")),
        PermissionAnswer::Selected("reject".into())
    );
    let bare = ask(json!({"toolCall": {"kind": "execute"}, "options": []}));
    assert_eq!(
        answer(&Permission::Allow, &bare),
        PermissionAnswer::Cancelled
    );
}

#[test]
fn only_a_gated_agent_keeps_a_floor_and_the_others_say_so_plainly() {
    for agent in ["opencode", "grok-build", "devin-cli"] {
        let (_, refused) = hooks(registry(), Budget::default(), agent, agent);
        assert!(refused.is_empty(), "{agent}");
        assert!(gated(agent), "{agent}");
    }
    assert!(hooked("grok-build") && hooked("devin-cli") && !hooked("opencode"));
    let (_, refused) = hooks(registry(), Budget::default(), "cursor", "Cursor");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].hook, "floors");
    assert!(
        refused[0].message.starts_with("Cursor can't stop a step"),
        "{}",
        refused[0].message
    );
    assert!(!gated("cursor") && !gated("amp"));
}

#[tokio::test]
async fn an_ungated_agents_asks_are_still_answered_by_its_hooks() {
    let (hooks, refused) = hooks(registry(), Budget::default(), "cursor", "Cursor");
    assert_eq!(refused.len(), 1);
    let request = ask(
        json!({"toolCall": {"toolCallId": "t", "title": "`rm -rf build`", "kind": "execute"}, "options": options()}),
    );
    let applied = hooks.fire(call(&request, None, Path::new("/w"))).await;
    assert_eq!(
        refusal(&applied).as_deref(),
        Some("Refused: this removes files.")
    );
    assert_eq!(
        answer(&applied.permission, &request),
        PermissionAnswer::Selected("reject".into())
    );
    // With nobody to answer, a held step is refused with its step.
    let merge = ask(
        json!({"toolCall": {"title": "gh pr merge 5", "kind": "execute"}, "options": options()}),
    );
    let applied = hooks.fire(call(&merge, None, Path::new("/w"))).await;
    assert_eq!(
        refusal(&applied).as_deref(),
        Some("Held for the person: a merge.")
    );
}

#[test]
fn the_bridge_reads_an_agents_hook_input_and_sends_only_what_it_can_use() {
    let mut registry = registry();
    // A rewrite the agent's tool could not read.
    registry
        .register(Hook::new(
            "squeeze",
            Event::PreToolUse,
            Source::BuiltIn,
            |call: &Call| {
                let mut input = call.input.clone();
                input["command"] = json!(format!(
                    "{} | tail",
                    input["command"].as_str().unwrap_or_default()
                ));
                Ok(HookOutcome {
                    rewrite: Some(input),
                    ..HookOutcome::none()
                })
            },
        ))
        .unwrap();
    let mut runner = Runner::new(registry, Budget::default());
    let grok = |tool: &str, input: serde_json::Value| json!({"hook_event_name": "PreToolUse", "cwd": "/w", "tool_name": tool, "tool_input": input});
    let denied = bridge_answer(
        &mut runner,
        Format::Acp,
        &grok("run_terminal_command", json!({"command": "rm -rf target"})),
    )
    .unwrap();
    assert_eq!(denied["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(
        denied["hookSpecificOutput"]["permissionDecisionReason"],
        "Refused: this removes files."
    );
    // Devin's `exec`, held for the person: nobody can answer inside it.
    let held = bridge_answer(
        &mut runner,
        Format::Acp,
        &grok("exec", json!({"command": "gh pr merge 5"})),
    )
    .unwrap();
    assert_eq!(held["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        held["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .starts_with("Held for the person")
    );
    // A plain command goes as the agent asked: the rewrite is not sent.
    assert_eq!(
        bridge_answer(
            &mut runner,
            Format::Acp,
            &grok("exec", json!({"command": "ls"}))
        ),
        None
    );
    assert_eq!(
        super::super::bridge::normalized(Format::Acp, &grok("exec", json!({"command": "ls"})))["tool_input"]
            ["cwd"],
        "/w"
    );
}

#[tokio::test]
async fn the_host_drives_the_turns_events() {
    let mut registry = Registry::new();
    registry
        .register(Hook::new(
            "brief",
            Event::SessionStart,
            Source::BuiltIn,
            |_: &Call| {
                Ok(HookOutcome {
                    context: Some(Context::new("coder", "Work in your worktree.", 0)),
                    ..HookOutcome::none()
                })
            },
        ))
        .unwrap();
    registry
        .register(Hook::new(
            "prompt",
            Event::UserPromptSubmit,
            Source::BuiltIn,
            |call: &Call| {
                Ok(if call.input["prompt"] == "pay" {
                    HookOutcome::deny("Payments are the person's.")
                } else {
                    HookOutcome::none()
                })
            },
        ))
        .unwrap();
    registry
        .register(Hook::new(
            "tests",
            Event::Stop,
            Source::BuiltIn,
            |_: &Call| {
                Ok(HookOutcome {
                    stop: Stop::Block {
                        reason: "Run the tests first.".into(),
                    },
                    ..HookOutcome::none()
                })
            },
        ))
        .unwrap();
    let (hooks, _) = hooks(registry, Budget::default(), "opencode", "OpenCode");
    let mut turns = Turns::new(hooks);
    let first = turns.prompt(Path::new("/w"), "fix it").await.unwrap();
    assert!(
        first.contains("Work in your worktree.") && first.ends_with("fix it"),
        "{first}"
    );
    assert_eq!(
        turns.prompt(Path::new("/w"), "again").await.unwrap(),
        "again"
    );
    assert_eq!(
        turns.prompt(Path::new("/w"), "pay").await.unwrap_err(),
        "Payments are the person's."
    );
    for _ in 0..MAX_CONTINUED {
        assert_eq!(
            turns.stopped("done").await.as_deref(),
            Some("Run the tests first.")
        );
    }
    assert_eq!(turns.stopped("done").await, None);
    turns.end("close").await;
}
