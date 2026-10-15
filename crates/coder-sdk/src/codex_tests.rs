//! Codex on app-server against a recorded conversation (#11510): the shapes
//! below are what `codex app-server` (codex-cli 0.162.1) sent on
//! 2026-10-13, trimmed to the fields Coder reads. No login, no model.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;
use crate::outcome::HookOutcome;
use crate::registry::Hook;

/// What the recorded server saw from Coder.
#[derive(Default, Debug)]
struct Seen {
    requests: Vec<(String, Value)>,
    decisions: Vec<(String, Value)>,
    steered: Vec<String>,
}

/// One turn's script: what the server sends after `turn/start`. A server
/// request waits for Coder's answer before the script goes on.
type Script = Vec<Value>;

fn approval_for_command(id: i64, item: &str, command: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "item/commandExecution/requestApproval", "params": {
        "kind": "command", "threadId": "thr-1", "turnId": "turn-1", "itemId": item,
        "startedAtMs": 1, "environmentId": "local",
        "command": format!("/bin/zsh -lc '{command}'"), "cwd": "/w",
        "commandActions": [{"type": "unknown", "command": command}],
    }})
}

fn note(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

fn command_item(item: &str, command: &str, status: &str) -> Value {
    json!({"type": "commandExecution", "id": item,
        "command": format!("/bin/zsh -lc '{command}'"), "cwd": "/w", "status": status,
        "aggregatedOutput": if status == "completed" { json!("ok\n") } else { Value::Null },
        "exitCode": if status == "completed" { json!(0) } else { Value::Null }})
}

fn completed(turn: &str, status: &str) -> Value {
    note(
        "turn/completed",
        json!({"threadId": "thr-1", "turn": {"id": turn, "status": status, "error": null}}),
    )
}

fn message(turn: &str, text: &str) -> Vec<Value> {
    let item = format!("msg-{turn}");
    vec![
        note(
            "item/started",
            json!({"item": {"type": "agentMessage", "id": item, "text": ""}}),
        ),
        note(
            "item/agentMessage/delta",
            json!({"threadId": "thr-1", "turnId": turn, "itemId": item, "delta": text}),
        ),
        note(
            "item/completed",
            json!({"item": {"type": "agentMessage", "id": item, "text": text, "phase": "final_answer"}}),
        ),
    ]
}

/// A recorded app-server: answers Coder's requests, plays each turn's
/// script after its `turn/start`, and keeps what Coder sent.
fn server(turns: Vec<Script>) -> (Codex, Arc<Mutex<Seen>>, tokio::task::JoinHandle<()>) {
    let (client, server) = tokio::io::duplex(1 << 20);
    let (client_read, client_write) = tokio::io::split(client);
    let (server_read, mut server_write) = tokio::io::split(server);
    let seen = Arc::new(Mutex::new(Seen::default()));
    let record = seen.clone();
    let task = tokio::spawn(async move {
        let mut lines = BufReader::new(server_read).lines();
        let mut turns = turns.into_iter();
        let mut turn_count = 0;
        let send = async |writer: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>,
                          message: Value| {
            let mut line = message.to_string();
            line.push('\n');
            writer.write_all(line.as_bytes()).await.unwrap();
        };
        // The script being played, and the request it waits on.
        let mut playing: std::collections::VecDeque<Value> = Default::default();
        let mut waiting: Option<Value> = None;
        loop {
            // Play the script until a server request waits for its answer.
            while waiting.is_none() {
                let Some(next) = playing.pop_front() else {
                    break;
                };
                let request = next.get("id").is_some();
                send(&mut server_write, next.clone()).await;
                if request {
                    waiting = Some(next);
                }
            }
            let Ok(Some(line)) = lines.next_line().await else {
                return;
            };
            let message: Value = serde_json::from_str(&line).unwrap();
            if let Some(method) = message["method"].as_str() {
                record
                    .lock()
                    .unwrap()
                    .requests
                    .push((method.to_owned(), message["params"].clone()));
                let Some(id) = message.get("id").cloned() else {
                    continue;
                };
                let result = match method {
                    "initialize" => json!({"userAgent": "codex/0.162.1"}),
                    "thread/start" => json!({"thread": {"id": "thr-1"}, "model": "gpt-6.1-sol"}),
                    "thread/resume" if message["params"]["threadId"] == "lost" => {
                        send(&mut server_write, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "no rollout found for thread id lost"}})).await;
                        continue;
                    }
                    // A thread id that is a path names its rollout, as
                    // Codex's answer does.
                    "thread/resume" => {
                        let id = message["params"]["threadId"].clone();
                        let path = id.as_str().filter(|id| Path::new(id).is_absolute());
                        json!({"thread": {"id": id, "path": path}, "model": "gpt-6.1-sol"})
                    }
                    "turn/start" => {
                        turn_count += 1;
                        playing.extend(turns.next().unwrap_or_default());
                        json!({"turn": {"id": format!("turn-{turn_count}")}})
                    }
                    "turn/steer" => {
                        record.lock().unwrap().steered.push(
                            message["params"]["input"][0]["text"]
                                .as_str()
                                .unwrap_or_default()
                                .to_owned(),
                        );
                        json!({"turnId": message["params"]["expectedTurnId"]})
                    }
                    "turn/interrupt" => {
                        playing.clear();
                        playing.push_back(completed(&format!("turn-{turn_count}"), "interrupted"));
                        json!({})
                    }
                    _ => json!({}),
                };
                send(
                    &mut server_write,
                    json!({"jsonrpc": "2.0", "id": id, "result": result}),
                )
                .await;
            } else if let Some(asked) = waiting.take() {
                assert_eq!(message["id"], asked["id"], "answers come in order");
                record.lock().unwrap().decisions.push((
                    asked["params"]["itemId"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    message["result"]["decision"].clone(),
                ));
            }
        }
    });
    let hooks = hooks(registry(), Budget::default()).unwrap();
    let codex = Codex::over(BufReader::new(client_read), client_write, hooks);
    (codex, seen, task)
}

/// A floor like Coder's: a merge is held, anything else runs. It reads the
/// command through Codex's shell wrapper.
fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(Hook::floor(
            "floors",
            Event::PreToolUse,
            Permission::ask("Coder could not check this step."),
            |call: &Call| {
                let command = call.input["command"].as_str().unwrap_or_default();
                Ok(if command.contains("gh pr merge") {
                    HookOutcome::deny("Held for the person: this would merge a pull request.")
                } else if call.tool.as_deref() == Some(PATCH_TOOL)
                    && call.input["input"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("*** Update File: /shared/")
                {
                    HookOutcome::deny("Refused: /shared is the shared checkout.")
                } else {
                    HookOutcome::none()
                })
            },
        ))
        .unwrap();
    registry
}

async fn open(codex: &mut Codex) -> Vec<Output> {
    let mut outputs = Vec::new();
    codex.initialize().await.unwrap();
    codex
        .open(
            &Open {
                cwd: "/w".into(),
                developer_instructions: Some("Briefing.".into()),
                ..Open::default()
            },
            &mut |output| outputs.push(output),
        )
        .await
        .unwrap();
    outputs
}

/// A question, an edit with checks, a held merge: each approval request is
/// answered by the hooks, the held one declined with its reason told to
/// the agent in the same turn, and the thread runs under approvals, never
/// the bypass flags.
#[tokio::test]
async fn approvals_are_answered_by_the_hooks() {
    let mut turn = vec![
        note(
            "item/started",
            json!({"item": command_item("exec-1", "cargo test", "inProgress")}),
        ),
        approval_for_command(0, "exec-1", "cargo test"),
        note(
            "item/completed",
            json!({"item": command_item("exec-1", "cargo test", "completed")}),
        ),
        note(
            "item/started",
            json!({"item": {"type": "fileChange", "id": "exec-2", "status": "inProgress",
                "changes": [{"path": "/w/README.md", "kind": {"type": "update", "move_path": null}, "diff": "@@\n-hi\n+hello\n"}]}}),
        ),
        json!({"jsonrpc": "2.0", "id": 1, "method": "item/fileChange/requestApproval", "params": {
            "threadId": "thr-1", "turnId": "turn-1", "itemId": "exec-2", "startedAtMs": 2, "reason": null, "grantRoot": null}}),
        note(
            "item/started",
            json!({"item": {"type": "fileChange", "id": "exec-3", "status": "inProgress",
                "changes": [{"path": "/shared/README.md", "kind": {"type": "update", "move_path": null}, "diff": "@@\n-hi\n+hello\n"}]}}),
        ),
        json!({"jsonrpc": "2.0", "id": 2, "method": "item/fileChange/requestApproval", "params": {
            "threadId": "thr-1", "turnId": "turn-1", "itemId": "exec-3", "startedAtMs": 3, "reason": null, "grantRoot": null}}),
        approval_for_command(3, "exec-4", "gh pr merge 5"),
        note(
            "item/completed",
            json!({"item": command_item("exec-4", "gh pr merge 5", "declined")}),
        ),
        note(
            "thread/tokenUsage/updated",
            json!({"threadId": "thr-1", "turnId": "turn-1", "tokenUsage": {"total": {"inputTokens": 10, "cachedInputTokens": 0, "outputTokens": 5, "reasoningOutputTokens": 0, "totalTokens": 15}, "last": {"inputTokens": 10, "cachedInputTokens": 0, "outputTokens": 5, "reasoningOutputTokens": 0, "totalTokens": 15}}}),
        ),
    ];
    turn.extend(message(
        "turn-1",
        "Tests pass; README updated. The merge is yours to do.",
    ));
    turn.push(completed("turn-1", "completed"));
    let (mut codex, seen, _task) = server(vec![turn]);
    let mut outputs = open(&mut codex).await;
    let ended = codex
        .turn(
            "Run the tests, update README, merge #5.",
            None,
            None,
            &mut |output| {
                outputs.push(output);
            },
        )
        .await
        .unwrap();
    assert_eq!(ended.status, "completed");
    assert_eq!(
        ended.text,
        "Tests pass; README updated. The merge is yours to do."
    );
    assert_eq!(ended.usage["total"]["totalTokens"], 15);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.decisions,
        [
            ("exec-1".into(), json!("accept")),
            ("exec-2".into(), json!("accept")),
            ("exec-3".into(), json!("decline")),
            ("exec-4".into(), json!("decline")),
        ]
    );
    assert_eq!(
        seen.steered,
        [
            "Refused: /shared is the shared checkout.",
            "Held for the person: this would merge a pull request.",
        ]
    );
    // The thread runs under approvals, never Codex's bypass.
    let start = &seen
        .requests
        .iter()
        .find(|(method, _)| method == "thread/start")
        .unwrap()
        .1;
    assert_eq!(start["approvalPolicy"], "untrusted");
    assert_eq!(start["sandbox"], "danger-full-access");
    assert_eq!(start["developerInstructions"], "Briefing.");
    assert!(outputs.contains(&Output::Thread("thr-1".into())));
    assert!(outputs.contains(&Output::Model("gpt-6.1-sol".into())));
    assert!(outputs.iter().any(|output| matches!(output,
        Output::Tool { name, input, running: false, .. } if name == "Run" && input["command"] == "cargo test")));
    let held = outputs
        .iter()
        .filter_map(|output| match output {
            Output::Gated(applied) => Some(applied),
            _ => None,
        })
        .find(|applied| matches!(applied.permission, Permission::Deny { .. }))
        .unwrap();
    assert_eq!(held.tool.as_deref(), Some(PATCH_TOOL));
}

/// An `Ask` goes to the host's asker while Codex waits: Allow once runs
/// that call, Deny declines it, and the same turn goes on.
#[tokio::test]
async fn allow_once_and_deny_answer_while_codex_waits() {
    let mut turn = vec![
        approval_for_command(0, "exec-1", "deploy staging"),
        approval_for_command(1, "exec-2", "deploy production"),
    ];
    turn.extend(message("turn-1", "Staging is out; production was not."));
    turn.push(completed("turn-1", "completed"));
    let (codex, seen, _task) = server(vec![turn]);
    let mut registry = Registry::new();
    registry
        .register(Hook::floor(
            "floors",
            Event::PreToolUse,
            Permission::ask("Coder could not check this step."),
            |call: &Call| {
                Ok(
                    if call.input["command"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("deploy")
                    {
                        HookOutcome::ask("Deploy?")
                    } else {
                        HookOutcome::none()
                    },
                )
            },
        ))
        .unwrap();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let asker: Asker = {
        let asked = asked.clone();
        Arc::new(move |applied: Applied| {
            let command = applied.input["command"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            asked.lock().unwrap().push(command.clone());
            Box::pin(async move {
                if command.contains("production") {
                    Permission::deny("The person said no.")
                } else {
                    Permission::NoOpinion
                }
            })
        })
    };
    let mut codex = Codex {
        hooks: hooks(registry, Budget::default()).unwrap(),
        ..codex
    }
    .asking(asker);
    open(&mut codex).await;
    let ended = codex
        .turn("Deploy both.", None, None, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(ended.status, "completed");
    assert_eq!(asked.lock().unwrap().len(), 2);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.decisions,
        [
            ("exec-1".into(), json!("accept")),
            ("exec-2".into(), json!("decline"))
        ]
    );
    assert_eq!(seen.steered, ["The person said no."]);
}

/// A follow-up continues the same thread; a `Stop` block sends the agent
/// back on that thread with the reason; a thread Codex lost is an error the
/// host starts anew from.
#[tokio::test]
async fn follow_ups_and_stop_blocks_stay_on_the_thread() {
    let mut first = message("turn-1", "Done.");
    first.push(completed("turn-1", "completed"));
    let mut again = message("turn-2", "Checked, all pass.");
    again.push(completed("turn-2", "completed"));
    let mut follow = message("turn-3", "Still PELICAN.");
    follow.push(completed("turn-3", "completed"));
    let (codex, seen, _task) = server(vec![first, again, follow]);
    let mut registry = Registry::new();
    let stops = Arc::new(Mutex::new(0));
    let counted = stops.clone();
    registry
        .register(Hook::new(
            "stop-gate",
            Event::Stop,
            crate::registry::Source::BuiltIn,
            move |call: &Call| {
                let mut count = counted.lock().unwrap();
                *count += 1;
                // The first stop is sent back once; later ones end.
                Ok(if *count == 1 && call.input["stop_hook_active"] == false {
                    HookOutcome::stopping(Stop::Block {
                        reason: "Run the checks first.".into(),
                    })
                } else {
                    HookOutcome::none()
                })
            },
        ))
        .unwrap();
    let mut codex = Codex {
        hooks: hooks(registry, Budget::default()).unwrap(),
        ..codex
    };
    open(&mut codex).await;
    let ended = codex
        .turn("Remember PELICAN and fix it.", None, None, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(ended.text, "Checked, all pass.");
    assert_eq!(ended.continued, 1);
    assert_eq!(*stops.lock().unwrap(), 2);
    let ended = codex
        .turn("What word?", None, None, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(ended.text, "Still PELICAN.");
    let seen = seen.lock().unwrap();
    let turns: Vec<_> = seen
        .requests
        .iter()
        .filter(|(method, _)| method == "turn/start")
        .map(|(_, params)| {
            (
                params["threadId"].clone(),
                params["input"][0]["text"].clone(),
            )
        })
        .collect();
    assert_eq!(
        turns,
        [
            (json!("thr-1"), json!("Remember PELICAN and fix it.")),
            (json!("thr-1"), json!("Run the checks first.")),
            (json!("thr-1"), json!("What word?")),
        ]
    );
}

#[tokio::test]
async fn a_lost_thread_is_refused_and_a_resumed_one_continues() {
    let (mut codex, seen, _task) = server(vec![]);
    codex.initialize().await.unwrap();
    let lost = codex
        .open(
            &Open {
                cwd: "/w".into(),
                resume: Some("lost".into()),
                ..Open::default()
            },
            &mut |_| {},
        )
        .await
        .unwrap_err();
    assert!(lost.contains("no rollout found"), "{lost}");
    let thread = codex
        .open(
            &Open {
                cwd: "/w".into(),
                resume: Some("thr-9".into()),
                ..Open::default()
            },
            &mut |_| {},
        )
        .await
        .unwrap();
    assert_eq!(thread, "thr-9");
    assert!(seen.lock().unwrap().requests.iter().any(
        |(method, params)| method == "thread/resume" && params["approvalPolicy"] == "untrusted"
    ));
}

/// Stopping a turn interrupts it; the thread stays for the next.
#[tokio::test]
async fn an_interrupt_stops_the_turn() {
    let turn = vec![note(
        "item/started",
        json!({"item": command_item("exec-1", "sleep 100", "inProgress")}),
    )];
    let (mut codex, seen, _task) = server(vec![turn]);
    open(&mut codex).await;
    let (control, mut controls) = tokio::sync::mpsc::unbounded_channel();
    control.send(Control::Interrupt).unwrap();
    let ended = codex
        .turn("Wait.", None, Some(&mut controls), &mut |_| {})
        .await
        .unwrap();
    assert!(ended.interrupted());
    assert!(
        seen.lock()
            .unwrap()
            .requests
            .iter()
            .any(|(method, _)| method == "turn/interrupt")
    );
}

#[test]
fn codex_calls_reach_the_hooks_in_claude_codes_shapes() {
    let call = command_call(
        &json!({"command": "/bin/zsh -lc 'gh pr merge 5'", "cwd": "/w/sub"}),
        Path::new("/w"),
    );
    assert_eq!(call.tool.as_deref(), Some("Bash"));
    assert_eq!(call.input["command"], "/bin/zsh -lc 'gh pr merge 5'");
    assert_eq!(call.input["cwd"], "/w/sub");
    // The legacy request carries the words.
    let call = command_call(
        &json!({"command": ["bash", "-lc", "echo 'hi'"]}),
        Path::new("/w"),
    );
    assert_eq!(call.input["command"], "bash -lc 'echo '\\''hi'\\'''");
    assert_eq!(call.input["cwd"], "/w");
    let call = patch_call(
        &json!({}),
        &json!({"changes": [{"path": "/w/a.rs", "kind": {"type": "add"}, "diff": "x"}]}),
    );
    assert_eq!(
        call.input["input"],
        "*** Begin Patch\n*** Add File: /w/a.rs\nx\n*** End Patch"
    );
    let call = patch_call(
        &json!({"fileChanges": {"/w/b.rs": {"type": "update", "unified_diff": "@@\n"}}}),
        &Value::Null,
    );
    assert!(
        call.input["input"]
            .as_str()
            .unwrap()
            .contains("*** Update File: /w/b.rs")
    );
    assert_eq!(unwrap_display("/bin/zsh -lc 'cargo test'"), "cargo test");
    assert_eq!(unwrap_display("ls"), "ls");
    assert_eq!(
        unwrap_display(r#"/bin/zsh -lc "rg --files -g 'add.sh'""#),
        "rg --files -g 'add.sh'"
    );
    assert_eq!(
        unwrap_display(r#"/bin/zsh -lc 'echo '\''hi'\'''"#),
        "echo 'hi'"
    );
    // Codex runs no bypass on this path.
    let launch = Launch {
        config: vec!["model_reasoning_effort=\"low\"".into()],
        ..Launch::new("codex")
    };
    let args = launch.args();
    assert_eq!(args[0], "app-server");
    assert!(!args.iter().any(|arg| arg.contains("bypass")));
}

/// A hook that must deny is native here only where Codex asks first.
#[test]
fn the_tiers_admit_floors_and_refuse_denying_after_the_fact() {
    assert!(hooks(registry(), Budget::default()).is_ok());
    let mut after = Registry::new();
    after
        .register(Hook::floor(
            "late",
            Event::PostToolUse,
            Permission::deny("no"),
            |_| Ok(HookOutcome::none()),
        ))
        .unwrap();
    assert!(hooks(after, Budget::default()).is_err());
}

/// Coder's tools reach Codex as dynamic tools in process: offered at the
/// thread's start, each call answered here through the hooks.
#[tokio::test]
async fn coder_tools_are_dynamic_tools_through_the_hooks() {
    let call = |id: i64, call: &str, tool: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "item/tool/call", "params": {
            "threadId": "thr-1", "turnId": "turn-1", "callId": call, "tool": tool,
            "arguments": {"task": "look"}}})
    };
    let mut turn = vec![call(0, "c1", "agent"), call(1, "c2", "openagents_cli")];
    turn.extend(message("turn-1", "Started."));
    turn.push(completed("turn-1", "completed"));
    let (mut codex, seen, _task) = server(vec![turn]);
    let mut registry = registry();
    registry
        .register(Hook::floor(
            "cli-floor",
            Event::PreToolUse,
            Permission::ask("?"),
            |call: &Call| {
                Ok(
                    if call.tool.as_deref() == Some("mcp__coder__openagents_cli") {
                        HookOutcome::deny("Held for the person.")
                    } else {
                        HookOutcome::none()
                    },
                )
            },
        ))
        .unwrap();
    codex.hooks = hooks(registry, Budget::default()).unwrap();
    let ran = Arc::new(Mutex::new(Vec::new()));
    let tools = {
        let ran = ran.clone();
        let ran2 = ran.clone();
        ToolServer::new("coder", "1.0.0")
            .add(crate::tools::Tool::new(
                "agent",
                "Start a background agent.",
                json!({"type": "object"}),
                move |input: Value| {
                    let ran = ran.clone();
                    async move {
                        ran.lock().unwrap().push(input);
                        Ok(claude_agent_sdk::ToolResult::text("agent started"))
                    }
                },
            ))
            .add(crate::tools::Tool::new(
                "openagents_cli",
                "Run the OpenAgents CLI.",
                json!({"type": "object"}),
                move |input: Value| {
                    let ran = ran2.clone();
                    async move {
                        ran.lock().unwrap().push(input);
                        Ok(claude_agent_sdk::ToolResult::text("ran"))
                    }
                },
            ))
    };
    codex.initialize().await.unwrap();
    codex
        .open(
            &Open {
                cwd: "/w".into(),
                tools: Some(tools),
                ..Open::default()
            },
            &mut |_| {},
        )
        .await
        .unwrap();
    codex
        .turn("Start an agent.", None, None, &mut |_| {})
        .await
        .unwrap();
    // Only the tool the hooks let through ran.
    assert_eq!(*ran.lock().unwrap(), [json!({"task": "look"})]);
    let seen = seen.lock().unwrap();
    let start = &seen
        .requests
        .iter()
        .find(|(method, _)| method == "thread/start")
        .unwrap()
        .1;
    let namespace = &start["dynamicTools"][0];
    assert_eq!(namespace["type"], "namespace");
    assert_eq!(namespace["name"], "coder");
    let offered: Vec<_> = namespace["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].clone())
        .collect();
    assert_eq!(offered, [json!("agent"), json!("openagents_cli")]);
    let init = &seen.requests[0].1;
    assert_eq!(init["capabilities"]["experimentalApi"], true);
}

/// Coder's tools as one tool, `agent`, that records each call.
fn agent_tool(ran: Arc<Mutex<Vec<Value>>>) -> ToolServer {
    ToolServer::new("coder", "1.0.0").add(crate::tools::Tool::new(
        "agent",
        "Start a background agent.",
        json!({"type": "object"}),
        move |input: Value| {
            let ran = ran.clone();
            async move {
                ran.lock().unwrap().push(input);
                Ok(claude_agent_sdk::ToolResult::text("agent started"))
            }
        },
    ))
}

/// A rollout whose `session_meta` keeps `tools` (Codex's own shape,
/// codex-cli 0.162.1), or none.
fn rollout(dir: &Path, name: &str, tools: Option<Value>) -> String {
    let mut payload = json!({"id": name, "cwd": "/w"});
    if let Some(tools) = tools {
        payload["dynamic_tools"] = tools;
    }
    let path = dir.join(format!("{name}.jsonl"));
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            json!({"timestamp": "t", "type": "session_meta", "payload": payload}),
            json!({"timestamp": "t", "type": "event_msg", "payload": {}})
        ),
    )
    .unwrap();
    path.display().to_string()
}

/// A Codex thread reopened in a new process (#11525) keeps the tools it
/// started with: one that kept Coder's tools can call them again on the
/// new connection; one that started without them is refused, so the host
/// starts a thread that has them.
#[tokio::test]
async fn a_reopened_thread_calls_coders_tools_or_is_refused_without_them() {
    let dir = tempfile::tempdir().unwrap();
    let kept = rollout(
        dir.path(),
        "kept",
        Some(
            json!([{"type": "namespace", "name": "coder", "description": "Coder's own tools.",
            "tools": [{"type": "function", "name": "agent", "description": "Start a background agent.",
                "inputSchema": {"type": "object"}}]}]),
        ),
    );
    let bare = rollout(dir.path(), "bare", None);
    let call = json!({"jsonrpc": "2.0", "id": 0, "method": "item/tool/call", "params": {
        "threadId": kept, "turnId": "turn-1", "callId": "c1", "namespace": "coder",
        "tool": "agent", "arguments": {"task": "look"}}});
    let mut turn = vec![call];
    turn.extend(message("turn-1", "Started."));
    turn.push(completed("turn-1", "completed"));
    let (mut codex, _seen, _task) = server(vec![turn]);
    let ran = Arc::new(Mutex::new(Vec::new()));
    codex.initialize().await.unwrap();
    let reopen = |thread: &str| Open {
        cwd: "/w".into(),
        resume: Some(thread.to_owned()),
        tools: Some(agent_tool(ran.clone())),
        ..Open::default()
    };
    let refused = codex.open(&reopen(&bare), &mut |_| {}).await.unwrap_err();
    assert!(refused.contains("without Coder's agent tool"), "{refused}");
    assert_eq!(codex.thread(), None);
    codex.open(&reopen(&kept), &mut |_| {}).await.unwrap();
    codex
        .turn("Start an agent.", None, None, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(*ran.lock().unwrap(), [json!({"task": "look"})]);
}

/// A hook that rewrites a command (the squeezer, #11525) can't change what
/// Codex runs, so the command is declined and the agent is told the
/// rewritten one to run instead; that one runs.
#[tokio::test]
async fn a_rewritten_command_is_sent_back_to_run_as_rewritten() {
    let turn = vec![
        approval_for_command(0, "exec-1", "cargo test"),
        approval_for_command(1, "exec-2", "sq cargo-test"),
        completed("turn-1", "completed"),
    ];
    let (mut codex, seen, _task) = server(vec![turn]);
    let mut registry = registry();
    registry
        .register(
            Hook::new(
                "squeeze",
                Event::PreToolUse,
                crate::registry::Source::BuiltIn,
                |call: &Call| {
                    let command =
                        unwrap_display(call.input["command"].as_str().unwrap_or_default());
                    Ok(if command.starts_with("sq ") {
                        HookOutcome::none()
                    } else {
                        let mut input = call.input.clone();
                        input["command"] = json!("sq cargo-test");
                        HookOutcome::rewrite(input)
                    })
                },
            )
            .matching("Bash"),
        )
        .unwrap();
    codex.hooks = hooks(registry, Budget::default()).unwrap();
    open(&mut codex).await;
    codex
        .turn("Run the tests.", None, None, &mut |_| {})
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.decisions,
        [
            ("exec-1".into(), json!("decline")),
            ("exec-2".into(), json!("accept")),
        ]
    );
    assert_eq!(seen.steered.len(), 1);
    assert!(
        seen.steered[0].ends_with("run exactly this instead, unchanged: sq cargo-test"),
        "{}",
        seen.steered[0]
    );
}

/// A recorded turn, `fixtures/codex/NAME.jsonl`: what `codex app-server`
/// (codex-cli 0.162.1) sent on 2026-10-13 for "Is 9991 prime?", the ids
/// renamed to the recorded server's.
fn recorded(name: &str) -> Script {
    let path = format!("{}/fixtures/codex/{name}.jsonl", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The thinking a turn streamed, and whether `Thinking` was on when each
/// piece arrived.
fn thoughts(outputs: &[Output]) -> (String, bool) {
    let mut on = false;
    let mut always_on = true;
    let mut text = String::new();
    for output in outputs {
        match output {
            Output::Thinking(now) => on = *now,
            Output::ThinkingText(piece) => {
                always_on &= on;
                text.push_str(piece);
            }
            _ => {}
        }
    }
    (text, always_on)
}

/// The owner's "no thinking on Codex": Codex's reasoning summary streams
/// as `item/reasoning/summaryTextDelta` and reaches the host piece by
/// piece, with `Thinking` on while it does and off before the answer.
#[tokio::test]
async fn the_reasoning_summary_streams_as_thinking() {
    let (mut codex, _seen, _task) = server(vec![recorded("reasoning-summary")]);
    open(&mut codex).await;
    let mut outputs = Vec::new();
    let ended = codex
        .turn("Is 9991 prime?", None, None, &mut |output| {
            outputs.push(output)
        })
        .await
        .unwrap();
    assert_eq!(ended.status, "completed");
    let pieces = outputs
        .iter()
        .filter(|output| matches!(output, Output::ThinkingText(_)))
        .count();
    assert!(pieces > 10, "streamed in pieces, not whole: {pieces}");
    let (text, always_on) = thoughts(&outputs);
    assert!(always_on);
    assert!(text.starts_with("**Verifying a multiplication**"), "{text}");
    assert!(text.contains("97 times 103"));
    // Thinking stops before the first word of the answer.
    let first_text = outputs
        .iter()
        .position(|output| matches!(output, Output::Text(_)))
        .unwrap();
    let last_off = outputs
        .iter()
        .rposition(|output| *output == Output::Thinking(false))
        .unwrap();
    assert!(last_off < first_text);
    assert!(!ended.text.contains("Verifying"));
}

/// A reasoning item with no summary (Codex's default, or a short think)
/// shows `Thinking` and no words; a summary that came only with the
/// finished item is said whole.
#[tokio::test]
async fn hidden_reasoning_has_no_words_and_a_whole_summary_is_said_once() {
    let (mut codex, _seen, _task) = server(vec![recorded("reasoning-hidden")]);
    open(&mut codex).await;
    let mut outputs = Vec::new();
    codex
        .turn("Is 9991 prime?", None, None, &mut |output| {
            outputs.push(output)
        })
        .await
        .unwrap();
    assert!(outputs.contains(&Output::Thinking(true)));
    assert_eq!(thoughts(&outputs).0, "");

    let whole = vec![
        note(
            "item/started",
            json!({"item": {"type": "reasoning", "id": "rs-1", "summary": [], "content": []}}),
        ),
        note(
            "item/completed",
            json!({"item": {"type": "reasoning", "id": "rs-1", "summary": ["**Plan**\n\nCheck 97.", "Then answer."], "content": []}}),
        ),
    ]
    .into_iter()
    .chain(message("turn-1", "Composite."))
    .chain([completed("turn-1", "completed")])
    .collect();
    let (mut codex, _seen, _task) = server(vec![whole]);
    open(&mut codex).await;
    let mut outputs = Vec::new();
    codex
        .turn("Is 9991 prime?", None, None, &mut |output| {
            outputs.push(output)
        })
        .await
        .unwrap();
    assert_eq!(
        thoughts(&outputs).0,
        "**Plan**\n\nCheck 97.\n\nThen answer."
    );
}
