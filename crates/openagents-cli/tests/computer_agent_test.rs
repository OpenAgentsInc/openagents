//! ACP delegation on the Computer controller (issue 113).
//!
//! The `agent` frame drives a real ACP child here, so these tests stand up a
//! real one: a Python ACP server that asks for exactly what a test tells it to
//! ask for, and reports back what answer it got. That is the only way to
//! assert the thing that matters — that a delegated agent is run *under this
//! machine's policy* rather than around it.
//!
//! Two shapes of assertion are deliberately absent. Nothing asserts
//! `x.is_empty() || !x.is_empty()`, and nothing asserts merely that "a frame
//! arrived": every case below names the reason the policy produced, the
//! journal line it wrote, and — where the agent can observe it — the answer the
//! agent was given.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use openagents_cli::acp::PermissionQuery;
use openagents_cli::computer::{
    AgentEntry, ComputerPaths, Decision, ForgeCredentials, Journal, JournalEntry, PolicyConfig,
    RefusalReason, ResolvedAgent, Tier, ToolReport, agent_catalog, agent_permission,
    forge_credentials, push_delegated, resolve_agent, serve, validate_refspec,
    write_credential_helper,
};

// ---------------------------------------------------------------------------
// the stand-in agent
// ---------------------------------------------------------------------------

/// An ACP server that does what one test told it to do.
///
/// The plan is a file named in its own argv, not an environment variable: the
/// tests run in one process, and a shared variable would make two concurrent
/// delegations decide each other's behaviour.
const STUB_AGENT: &str = r#"#!/usr/bin/env python3
import json, sys, time

with open(sys.argv[1]) as handle:
    plan = json.load(handle)

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

if plan.get("fail") == "exit":
    sys.exit(3)

notes = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "agentCapabilities": {"loadSession": bool(plan.get("load"))},
        }})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"sessionId": plan.get("session", "sess-stub")}})
    elif method == "session/set_mode":
        notes.append("mode:" + message["params"]["modeId"])
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"modeId": message["params"]["modeId"]}})
    elif method == "session/load":
        notes.append("loaded:" + message["params"]["sessionId"])
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/prompt":
        sid = message["params"]["sessionId"]
        ask = plan.get("permission")
        if ask is not None:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
                  "update": {"sessionUpdate": "tool_call", "toolCallId": "t1",
                             "kind": ask.get("kind", ""), "title": ask.get("title", "")}}})
            send({"jsonrpc": "2.0", "id": 9001, "method": "session/request_permission",
                  "params": {"sessionId": sid,
                             "toolCall": {"kind": ask.get("kind", ""),
                                          "title": ask.get("title", ""),
                                          "rawInput": ask.get("rawInput", {})},
                             "options": [
                                 {"optionId": "reject-once", "kind": "reject_once"},
                                 {"optionId": "allow-once", "kind": "allow_once"}]}})
            answer = json.loads(sys.stdin.readline())
            outcome = answer["result"]["outcome"]
            notes.append("permission:" + str(outcome.get("optionId") or outcome.get("outcome")))
        push = plan.get("push")
        if push is not None:
            send({"jsonrpc": "2.0", "id": 9002, "method": "git/push", "params": push})
            reply = json.loads(sys.stdin.readline())
            if "error" in reply:
                notes.append("push:method_not_found")
            else:
                result = reply.get("result", {})
                notes.append("push:" + ("ok" if result.get("ok") else "refused"))
        if plan.get("delay"):
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
                  "update": {"sessionUpdate": "agent_message_chunk",
                             "content": {"type": "text", "text": "working\n"}}}})
            time.sleep(plan["delay"])
        for piece in plan.get("chunks", []) + notes:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
                  "update": {"sessionUpdate": "agent_message_chunk",
                             "content": {"type": "text", "text": piece + "\n"}}}})
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"stopReason": plan.get("stop", "end_turn")}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;

fn stub_agent_path(directory: &Path, plan: &serde_json::Value) -> (PathBuf, PathBuf) {
    let path = directory.join("stub-acp-agent");
    let plan_path = directory.join("stub-acp-plan.json");
    std::fs::write(&plan_path, plan.to_string()).unwrap();
    std::fs::write(&path, STUB_AGENT).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (path, plan_path)
}

// ---------------------------------------------------------------------------
// a controller that pushes one delegation
// ---------------------------------------------------------------------------

struct StubController {
    origin: String,
    frames: Receiver<serde_json::Value>,
}

fn start_controller(
    machine_id: &str,
    event: &'static str,
    ask: serde_json::Value,
) -> StubController {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, frames) = channel();
    let topic = format!("computer:{machine_id}");

    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let Ok(mut socket) = tungstenite::accept(stream) else {
            return;
        };
        let _ = socket.read();
        let reply =
            serde_json::json!(["1", "1", topic, "phx_reply", {"status": "ok", "response": {}}]);
        let _ = socket.send(tungstenite::Message::Text(reply.to_string().into()));
        let _ = socket.read();
        let push = serde_json::json!([serde_json::Value::Null, "9", topic, event, ask]);
        let _ = socket.send(tungstenite::Message::Text(push.to_string().into()));

        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while std::time::Instant::now() < deadline {
            match socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        let terminal = value
                            .get(3)
                            .and_then(|event| event.as_str())
                            .map(|event| event == "refused" || event == "exit")
                            .unwrap_or(false);
                        if sender.send(value).is_err() {
                            return;
                        }
                        if terminal {
                            break;
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = socket.close(None);
        while socket.read().is_ok() {}
    });

    StubController {
        origin: format!("http://127.0.0.1:{port}"),
        frames,
    }
}

/// Everything the controller sent, in order, up to and including the terminal
/// frame. Reading the whole conversation is what lets a test assert that the
/// session id arrived *before* the exit, and that a refused permission still
/// produced a completed turn.
fn conversation(frames: &Receiver<serde_json::Value>) -> Vec<(String, serde_json::Value)> {
    let mut seen = Vec::new();
    while let Ok(frame) = frames.recv_timeout(Duration::from_secs(60)) {
        let event = frame
            .get(3)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let payload = frame.get(4).cloned().unwrap_or(serde_json::Value::Null);
        let terminal = event == "refused" || event == "exit";
        seen.push((event, payload));
        if terminal {
            break;
        }
    }
    seen
}

fn terminal_of(seen: &[(String, serde_json::Value)]) -> (String, serde_json::Value) {
    seen.iter()
        .rev()
        .find(|(event, _)| event == "refused" || event == "exit")
        .cloned()
        .expect("the server was left waiting: no refused and no exit ever arrived")
}

fn streamed(seen: &[(String, serde_json::Value)]) -> String {
    seen.iter()
        .filter(|(event, _)| event == "chunk")
        .filter_map(|(_, payload)| payload.get("text").and_then(|value| value.as_str()))
        .collect::<Vec<_>>()
        .join("")
}

// ---------------------------------------------------------------------------
// running one delegation end to end
// ---------------------------------------------------------------------------

struct Delegation {
    seen: Vec<(String, serde_json::Value)>,
    entries: Vec<JournalEntry>,
}

impl Delegation {
    fn journal_line(&self, decision: &str) -> Option<&JournalEntry> {
        self.entries.iter().find(|entry| entry.decision == decision)
    }
}

struct Setup {
    tier: Tier,
    declare_root: bool,
    scoped_forge_credentials: bool,
    plan: serde_json::Value,
    event: &'static str,
    ask: serde_json::Value,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            tier: Tier::Curated,
            declare_root: true,
            scoped_forge_credentials: false,
            plan: serde_json::json!({}),
            event: "agent",
            ask: serde_json::json!({}),
        }
    }
}

/// Serve one delegation against a live stub controller and a live stub agent,
/// and return everything both sides can be asked about afterwards.
fn delegate(setup: Setup) -> Delegation {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (agent, plan) = stub_agent_path(directory.path(), &setup.plan);

    let mut agents = BTreeMap::new();
    agents.insert(
        "stub".to_string(),
        AgentEntry {
            argv: vec![agent.display().to_string(), plan.display().to_string()],
            env: Vec::new(),
        },
    );
    let config = PolicyConfig {
        tier: setup.tier,
        roots: if setup.declare_root {
            vec![root.clone()]
        } else {
            Vec::new()
        },
        agents,
        scoped_forge_credentials: setup.scoped_forge_credentials,
        ..PolicyConfig::closed(ComputerPaths::in_directory(directory.path()))
    };
    let journal = Journal::at(directory.path().join("journal.ndjson"));

    let mut ask = serde_json::json!({
        "request_id": "req-agent",
        "agent_id": "stub",
        "prompt": "do the thing",
        "cwd": root.display().to_string(),
        "timeout_ms": 30_000,
    });
    if let (Some(base), Some(extra)) = (ask.as_object_mut(), setup.ask.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }

    let machine = format!("machine-{}", std::process::id());
    let stub = start_controller(&machine, setup.event, ask);

    serve(
        &stub.origin,
        &openagents_cli::auth::Secret::new("smct_stub"),
        &machine,
        &serde_json::json!({"agent_version": "test"}),
        &config,
        &journal,
        |_| {},
    );

    Delegation {
        seen: conversation(&stub.frames),
        entries: journal.read(200).unwrap(),
    }
}

// ---------------------------------------------------------------------------
// the delegation itself
// ---------------------------------------------------------------------------

/// The `agent` frame runs a real ACP child and reports what it did.
///
/// Before this, the answer was `{"reason": "unsupported"}`. The session id
/// arrives while the agent is still working — `OpenAgentsWeb.ComputerChannel`
/// checkpoints it mid-stream so a survivor can reattach — and the terminal
/// `exit` carries the same id, the stop reason, and a duration.
#[test]
fn test_a_delegation_streams_its_session_output_and_a_terminal_exit() {
    let run = delegate(Setup {
        plan: serde_json::json!({"session": "sess-live", "chunks": ["hello from the agent"]}),
        ..Setup::default()
    });

    let session = run
        .seen
        .iter()
        .find(|(event, _)| event == "session")
        .expect("the session id must be reported while the agent is still running");
    assert_eq!(
        session.1.get("session_id").and_then(|v| v.as_str()),
        Some("sess-live")
    );
    assert_eq!(
        session.1.get("request_id").and_then(|v| v.as_str()),
        Some("req-agent")
    );
    assert!(
        run.seen.iter().position(|(e, _)| e == "session")
            < run.seen.iter().position(|(e, _)| e == "exit"),
        "the session id is useless for reattach if it only arrives with the exit"
    );

    assert!(
        streamed(&run.seen).contains("hello from the agent"),
        "the agent's output must reach the server as it is written: {:?}",
        run.seen
    );

    let (kind, exit) = terminal_of(&run.seen);
    assert_eq!(kind, "exit");
    assert_eq!(
        exit.get("status").and_then(|v| v.as_str()),
        Some("completed")
    );
    assert_eq!(
        exit.get("session_id").and_then(|v| v.as_str()),
        Some("sess-live")
    );
    assert_eq!(
        exit.get("stop_reason").and_then(|v| v.as_str()),
        Some("end_turn")
    );
    assert_eq!(
        exit.get("request_id").and_then(|v| v.as_str()),
        Some("req-agent")
    );

    let allowed = run
        .journal_line("allowed")
        .expect("the delegation must be journaled as allowed");
    assert_eq!(allowed.argv, vec!["<agent>", "stub"]);
    assert!(
        run.entries
            .iter()
            .any(|entry| entry.outcome == "completed" && entry.request_id == "req-agent"),
        "the outcome must reach the journal: {:?}",
        run.entries
    );
}

/// A delegated agent is put into a mode that asks.
///
/// The gate can only decide what the agent puts to it. An agent left in its
/// own default may be in a bypass mode that never sends
/// `session/request_permission` at all, and a policy nothing consults decides
/// nothing — so the delegation names the asking mode rather than inheriting
/// whatever the agent came with.
#[test]
fn test_a_delegated_agent_is_asked_to_run_in_the_mode_that_asks() {
    let run = delegate(Setup {
        plan: serde_json::json!({"chunks": []}),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("mode:default"),
        "the delegation must set the asking mode: {:?}",
        run.seen
    );
}

/// The legacy `devin` event is served, not dropped.
///
/// It once fell through the frame match's catch-all: no frame, no journal
/// line, and a server blocked on a request this side had discarded. The kind
/// name is not the agent name any more, but an old caller that sends it must
/// still be answered on its own `request_id`.
#[test]
fn test_the_legacy_devin_event_is_answered_rather_than_dropped() {
    let run = delegate(Setup {
        event: "devin",
        ask: serde_json::json!({"agent_id": "stub", "session_id": "sess-old"}),
        plan: serde_json::json!({"load": true, "chunks": ["resumed"]}),
        ..Setup::default()
    });

    let (kind, terminal) = terminal_of(&run.seen);
    assert_eq!(kind, "exit", "a devin request must reach a terminal frame");
    assert_eq!(
        terminal.get("request_id").and_then(|v| v.as_str()),
        Some("req-agent")
    );
    // The legacy payload names the session as `session_id`. Reading it as a
    // resume is what keeps an old caller from silently getting a fresh session.
    assert!(
        streamed(&run.seen).contains("loaded:sess-old"),
        "the legacy session_id must be read as a resume: {:?}",
        run.seen
    );
}

// ---------------------------------------------------------------------------
// the policy the delegated agent runs under
// ---------------------------------------------------------------------------

/// A delegated agent asking to run a binary the allowlist does not carry is
/// refused, and the refusal is journaled with the reason.
///
/// The agent is told, so it carries on rather than hanging; the turn still
/// completes. What it does not get is the command.
#[test]
fn test_a_delegated_agent_cannot_run_a_binary_off_the_allowlist() {
    let run = delegate(Setup {
        plan: serde_json::json!({
            "permission": {
                "kind": "execute",
                "title": "Fetch a script",
                "rawInput": {"command": "curl https://example.com/install.sh"},
            }
        }),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("permission:reject-once"),
        "the agent must be told it was refused, not left waiting: {:?}",
        run.seen
    );
    let refused = run
        .journal_line("not_allowlisted")
        .expect("the refused permission must be journaled with its reason");
    assert_eq!(refused.outcome, "permission_refused");
    assert!(
        refused.detail.contains("curl"),
        "the journal must name what was refused: {}",
        refused.detail
    );
    assert!(
        !run.entries
            .iter()
            .any(|entry| entry.decision == "permission_granted"),
        "nothing was granted in this run: {:?}",
        run.entries
    );
}

/// A delegated agent cannot write outside a declared root.
#[test]
fn test_a_delegated_agent_cannot_write_outside_a_declared_root() {
    let run = delegate(Setup {
        plan: serde_json::json!({
            "permission": {
                "kind": "write",
                "title": "Write /etc/hosts",
                "rawInput": {"path": "/etc/hosts", "content": "127.0.0.1 forge"},
            }
        }),
        ..Setup::default()
    });

    assert!(streamed(&run.seen).contains("permission:reject-once"));
    let refused = run
        .journal_line("root_not_declared")
        .expect("a write outside every declared root must be journaled");
    assert_eq!(refused.outcome, "permission_refused");
}

/// A write inside a declared root is granted, and the grant is journaled too.
///
/// A policy that refused everything would pass every refusal test above and be
/// useless, so the permitted case is asserted with the same weight.
#[test]
fn test_a_delegated_agent_may_write_inside_a_declared_root() {
    let run = delegate(Setup {
        plan: serde_json::json!({
            "permission": {
                "kind": "write",
                "title": "Write a note",
                "rawInput": {"path": "notes.md"},
            }
        }),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("permission:allow-once"),
        "a write inside the root must be allowed: {:?}",
        run.seen
    );
    let granted = run
        .journal_line("permission_granted")
        .expect("a granted permission must be journaled too");
    assert!(
        granted.detail.contains("Write a note"),
        "{}",
        granted.detail
    );
}

/// Delegation does not exist below the curated tier.
#[test]
fn test_a_probe_tier_machine_refuses_delegation_outright() {
    let run = delegate(Setup {
        tier: Tier::Probe,
        ..Setup::default()
    });

    let (kind, refused) = terminal_of(&run.seen);
    assert_eq!(kind, "refused");
    assert_eq!(
        refused.get("reason").and_then(|v| v.as_str()),
        Some("tier_insufficient")
    );
    assert!(run.journal_line("tier_insufficient").is_some());
    assert!(
        !run.entries.iter().any(|entry| entry.decision == "allowed"),
        "nothing may be allowed on a probe-tier machine: {:?}",
        run.entries
    );
}

/// A working directory outside every declared root is refused before the agent
/// is started.
#[test]
fn test_a_delegation_outside_every_declared_root_is_refused() {
    let run = delegate(Setup {
        declare_root: false,
        ..Setup::default()
    });

    let (kind, refused) = terminal_of(&run.seen);
    assert_eq!(kind, "refused");
    assert_eq!(
        refused.get("reason").and_then(|v| v.as_str()),
        Some("root_not_declared")
    );
    assert!(run.journal_line("root_not_declared").is_some());
}

/// An agent this machine does not have is refused by name, and told what it
/// does have.
#[test]
fn test_an_unknown_agent_is_refused_with_the_available_ones() {
    let run = delegate(Setup {
        ask: serde_json::json!({"agent_id": "not-installed-here"}),
        ..Setup::default()
    });

    let (kind, refused) = terminal_of(&run.seen);
    assert_eq!(kind, "refused");
    assert_eq!(
        refused.get("reason").and_then(|v| v.as_str()),
        Some("agent_unavailable")
    );
    let detail = refused
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        detail.contains("not-installed-here") && detail.contains("stub"),
        "the refusal must name both what was asked for and what is here: {detail}"
    );
}

/// An agent that cannot be started still ends in a terminal frame.
///
/// This is the failure mode the whole shape exists to prevent: a server that
/// pushed a request and never heard back.
#[test]
fn test_an_agent_that_dies_immediately_still_answers_the_request() {
    let run = delegate(Setup {
        plan: serde_json::json!({"fail": "exit"}),
        ..Setup::default()
    });

    let (kind, terminal) = terminal_of(&run.seen);
    assert_eq!(kind, "exit");
    assert_eq!(
        terminal.get("request_id").and_then(|v| v.as_str()),
        Some("req-agent")
    );
    let status = terminal
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        status == "failed" || status == "unavailable",
        "an agent that exited must be reported as such, not as completed: {terminal}"
    );
    assert!(
        !terminal
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .is_empty(),
        "the failure must say what happened: {terminal}"
    );
}

// ---------------------------------------------------------------------------
// reattach
// ---------------------------------------------------------------------------

/// A resume asks the agent to load the session rather than opening a new one.
#[test]
fn test_a_resume_loads_the_named_session() {
    let run = delegate(Setup {
        ask: serde_json::json!({"resume_session_id": "sess-earlier"}),
        plan: serde_json::json!({"load": true}),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("loaded:sess-earlier"),
        "the agent must be asked to load the session: {:?}",
        run.seen
    );
    let (_kind, exit) = terminal_of(&run.seen);
    assert_eq!(
        exit.get("session_id").and_then(|v| v.as_str()),
        Some("sess-earlier"),
        "a resumed delegation reports the session it resumed"
    );
}

/// An agent that cannot load a session says so rather than opening a fresh one.
///
/// A silent new session looks like a successful resume and loses everything
/// the earlier one knew, which is worse than a refusal.
#[test]
fn test_a_resume_is_refused_when_the_agent_cannot_load_a_session() {
    let run = delegate(Setup {
        ask: serde_json::json!({"resume_session_id": "sess-earlier"}),
        plan: serde_json::json!({"load": false}),
        ..Setup::default()
    });

    let (kind, exit) = terminal_of(&run.seen);
    assert_eq!(kind, "exit");
    assert_eq!(exit.get("status").and_then(|v| v.as_str()), Some("failed"));
    assert!(
        exit.get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .contains("reattach"),
        "the refusal must say the agent cannot reattach: {exit}"
    );
}

/// A second request naming a session that is still running here reattaches to
/// it instead of starting a second agent in the same checkout.
///
/// This is what a relocated delegation does after a node loss: the caller is a
/// new process on a new `request_id`, and the agent it wants is already
/// working. Two agents editing one checkout is the outcome this prevents.
#[test]
fn test_a_reattach_moves_the_live_session_onto_the_new_request() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let (agent, plan) = stub_agent_path(
        directory.path(),
        &serde_json::json!({"session": "sess-live", "delay": 4, "chunks": ["finished"]}),
    );

    let mut agents = BTreeMap::new();
    agents.insert(
        "stub".to_string(),
        AgentEntry {
            argv: vec![agent.display().to_string(), plan.display().to_string()],
            env: Vec::new(),
        },
    );
    let config = PolicyConfig {
        tier: Tier::Curated,
        roots: vec![root.clone()],
        agents,
        ..PolicyConfig::closed(ComputerPaths::in_directory(directory.path()))
    };
    let journal = Journal::at(directory.path().join("journal.ndjson"));

    let machine = format!("machine-reattach-{}", std::process::id());
    let topic = format!("computer:{machine}");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, frames) = channel();
    let first = serde_json::json!({
        "request_id": "req-first",
        "agent_id": "stub",
        "prompt": "start the work",
        "cwd": root.display().to_string(),
        "timeout_ms": 30_000,
    });
    let second = serde_json::json!({
        "request_id": "req-second",
        "agent_id": "stub",
        "prompt": "keep going",
        "cwd": root.display().to_string(),
        "resume_session_id": "sess-live",
        "timeout_ms": 30_000,
    });

    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let Ok(mut socket) = tungstenite::accept(stream) else {
            return;
        };
        let _ = socket.read();
        let reply =
            serde_json::json!(["1", "1", topic, "phx_reply", {"status": "ok", "response": {}}]);
        let _ = socket.send(tungstenite::Message::Text(reply.to_string().into()));
        let _ = socket.read();
        let ask = serde_json::json!([serde_json::Value::Null, "9", topic, "agent", first]);
        let _ = socket.send(tungstenite::Message::Text(ask.to_string().into()));

        let mut resumed = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while std::time::Instant::now() < deadline {
            match socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    let event = value
                        .get(3)
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string();
                    // The moment the first request reports its session, the
                    // caller has been relocated: ask for the same session on a
                    // new request, the way a survivor node would.
                    if event == "session" && !resumed {
                        resumed = true;
                        let ask = serde_json::json!([
                            serde_json::Value::Null,
                            "10",
                            topic,
                            "agent",
                            second
                        ]);
                        let _ = socket.send(tungstenite::Message::Text(ask.to_string().into()));
                    }
                    let terminal = event == "refused" || event == "exit";
                    if sender.send(value).is_err() {
                        return;
                    }
                    if terminal {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = socket.close(None);
        while socket.read().is_ok() {}
    });

    serve(
        &format!("http://127.0.0.1:{port}"),
        &openagents_cli::auth::Secret::new("smct_stub"),
        &machine,
        &serde_json::json!({"agent_version": "test"}),
        &config,
        &journal,
        |_| {},
    );

    let seen = conversation(&frames);
    let sessions: Vec<&str> = seen
        .iter()
        .filter(|(event, _)| event == "session")
        .filter_map(|(_, payload)| payload.get("request_id").and_then(|v| v.as_str()))
        .collect();
    assert!(
        sessions.contains(&"req-second"),
        "the resumed request must be told which session it now owns: {seen:?}"
    );

    let (kind, terminal) = terminal_of(&seen);
    assert_eq!(kind, "exit");
    assert_eq!(
        terminal.get("request_id").and_then(|v| v.as_str()),
        Some("req-second"),
        "the delegation's output must follow the request that reattached to it: {terminal}"
    );
    assert_eq!(
        terminal.get("session_id").and_then(|v| v.as_str()),
        Some("sess-live")
    );

    let entries = journal.read(200).unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.decision == "reattached" && entry.request_id == "req-second"),
        "the reattach must be journaled: {entries:?}"
    );
    // One agent, not two: a second `allowed` line would mean a second child
    // started in the same checkout.
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.decision == "allowed" && entry.outcome == "running")
            .count(),
        1,
        "a reattach must not start a second agent: {entries:?}"
    );
}

// ---------------------------------------------------------------------------
// scoped forge credentials
// ---------------------------------------------------------------------------

/// A credential the owner has not enabled locally governs nothing.
///
/// The server withholds the credential unless the Computers page checkbox is
/// ticked; this machine requires the same thing said in its own configuration,
/// because the machine is what decides what runs here. The agent's push is
/// answered — with `method not found` — rather than left hanging, and the
/// refusal is journaled.
#[test]
fn test_a_delegated_push_is_refused_when_the_local_switch_is_off() {
    let run = delegate(Setup {
        scoped_forge_credentials: false,
        ask: serde_json::json!({
            "assignment_credential": "oa_assignment_notarealtoken",
            "assignment_repository": "OpenAgentsInc/openagents",
            "assignment_branch": "work/1",
        }),
        plan: serde_json::json!({"push": {"remote": "origin", "refspec": "work/1"}}),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("push:method_not_found"),
        "an unserved push must be answered, not left hanging: {:?}",
        run.seen
    );
    let refused = run
        .journal_line("credentials_refused")
        .expect("the withheld credential must be journaled");
    assert!(
        refused.detail.contains("not enabled"),
        "the journal must say why: {}",
        refused.detail
    );
    assert!(
        !run.entries
            .iter()
            .any(|entry| entry.decision == "push_completed"),
        "nothing may have been pushed: {:?}",
        run.entries
    );
    assert_no_token_anywhere(&run, "oa_assignment_notarealtoken");
}

/// With the switch on, the credential is accepted and the push is attempted —
/// and refused, because the checkout's remote is not the assigned repository.
///
/// A scoped credential that would push to whatever remote the checkout happens
/// to have is not scoped.
#[test]
fn test_a_delegated_push_refuses_a_remote_that_is_not_the_assigned_repository() {
    let run = delegate(Setup {
        scoped_forge_credentials: true,
        ask: serde_json::json!({
            "assignment_credential": "oa_assignment_notarealtoken",
            "assignment_repository": "OpenAgentsInc/openagents",
            "assignment_branch": "work/1",
        }),
        plan: serde_json::json!({"push": {"remote": "origin", "refspec": "work/1"}}),
        ..Setup::default()
    });

    assert!(
        streamed(&run.seen).contains("push:refused"),
        "the agent must be told the push was refused: {:?}",
        run.seen
    );
    assert!(
        run.journal_line("credentials_delivered").is_some(),
        "an accepted credential is journaled as delivered: {:?}",
        run.entries
    );
    let refused = run
        .journal_line("push_refused")
        .expect("the refused push must be journaled");
    assert!(
        !refused.detail.is_empty(),
        "the journal must say why the push was refused"
    );
    assert_no_token_anywhere(&run, "oa_assignment_notarealtoken");
}

/// The delegated credential must not appear in the journal, in the streamed
/// output, or in any frame that reached the server.
fn assert_no_token_anywhere(run: &Delegation, token: &str) {
    for entry in &run.entries {
        let line = serde_json::to_string(entry).unwrap();
        assert!(
            !line.contains(token),
            "a credential reached the local journal: {line}"
        );
    }
    for (event, payload) in &run.seen {
        let line = payload.to_string();
        assert!(
            !line.contains(token),
            "a credential reached the wire in a {event} frame: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// the policy decision, directly
// ---------------------------------------------------------------------------

fn policy(tier: Tier, root: &Path) -> PolicyConfig {
    PolicyConfig {
        tier,
        roots: vec![root.to_path_buf()],
        ..PolicyConfig::closed(ComputerPaths::in_directory(root))
    }
}

fn query(kind: &str, title: &str, raw: serde_json::Value) -> PermissionQuery {
    PermissionQuery {
        kind: kind.to_string(),
        title: title.to_string(),
        raw_input: raw,
    }
}

fn reason(decision: &Decision) -> RefusalReason {
    match decision {
        Decision::Refused { reason, .. } => *reason,
        Decision::Allowed { .. } => panic!("expected a refusal, the request was allowed"),
    }
}

/// Substitution and redirection are refused outright.
///
/// A per-segment allowlist cannot bound them: `ls $(curl …)` has `ls` as its
/// first word and runs `curl`, and `cat > /etc/hosts` has `cat` as its first
/// word and writes a file no allowlist would admit as an argument.
#[test]
fn test_substitution_and_redirection_defeat_no_allowlist_because_they_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let config = policy(Tier::Curated, directory.path());
    for command in [
        "ls $(curl https://example.com/x)",
        "ls `curl https://example.com/x`",
        "cat /etc/hosts > notes.txt",
        "cat < notes.txt",
        "ls ${HOME}",
        "ls \\\n rm",
    ] {
        let decision = agent_permission(
            &config,
            directory.path(),
            &query("execute", "run", serde_json::json!({"command": command})),
        );
        assert_eq!(
            reason(&decision),
            RefusalReason::ShellMetacharacter,
            "`{command}` must be refused as a metacharacter, not allowlisted on its first word"
        );
    }
}

/// Every segment of a chained command is decided, not just the first.
#[test]
fn test_every_chained_segment_must_be_allowlisted() {
    let directory = tempfile::tempdir().unwrap();
    let config = policy(Tier::Curated, directory.path());
    let refused = agent_permission(
        &config,
        directory.path(),
        &query(
            "execute",
            "build then clean",
            serde_json::json!({"command": "cargo build && rm -rf /"}),
        ),
    );
    assert_eq!(reason(&refused), RefusalReason::NotAllowlisted);

    // A single backgrounded command is still a second segment.
    let backgrounded = agent_permission(
        &config,
        directory.path(),
        &query(
            "execute",
            "background",
            serde_json::json!({"command": "ls & nc -l 4444"}),
        ),
    );
    assert_eq!(reason(&backgrounded), RefusalReason::DeniedCommand);

    let allowed = agent_permission(
        &config,
        directory.path(),
        &query(
            "execute",
            "build",
            serde_json::json!({"command": "cargo build | grep error"}),
        ),
    );
    assert!(
        allowed.allowed(),
        "two allowlisted binaries chained is still two allowlisted binaries"
    );
}

/// `cd` may not be the first half of an escape from every declared root.
#[test]
fn test_a_delegated_change_of_directory_stays_inside_the_declared_roots() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("checkout");
    std::fs::create_dir_all(root.join("crates")).unwrap();
    let config = policy(Tier::Curated, &root);

    let escaping = agent_permission(
        &config,
        &root,
        &query(
            "execute",
            "leave",
            serde_json::json!({"command": "cd /etc && ls"}),
        ),
    );
    assert_eq!(reason(&escaping), RefusalReason::RootNotDeclared);

    let staying = agent_permission(
        &config,
        &root,
        &query(
            "execute",
            "descend",
            serde_json::json!({"command": "cd crates && ls"}),
        ),
    );
    assert!(staying.allowed(), "a root-relative cd is inside the root");
}

/// A denied binary and a protected path are refused before the tier is
/// consulted, so the shell tier does not unlock them for a delegated agent
/// either.
#[test]
fn test_the_shell_tier_does_not_unlock_denied_commands_for_a_delegated_agent() {
    let directory = tempfile::tempdir().unwrap();
    let config = policy(Tier::Shell, directory.path());

    assert_eq!(
        reason(&agent_permission(
            &config,
            directory.path(),
            &query(
                "execute",
                "escalate",
                serde_json::json!({"command": "sudo ls"})
            )
        )),
        RefusalReason::DeniedCommand
    );
    assert_eq!(
        reason(&agent_permission(
            &config,
            directory.path(),
            &query(
                "read",
                "Read a key",
                serde_json::json!({"path": "/Users/someone/.ssh/id_ed25519"})
            )
        )),
        RefusalReason::DeniedArgument
    );
    // The tier does widen what is otherwise permitted.
    assert!(
        agent_permission(
            &config,
            directory.path(),
            &query(
                "execute",
                "anything",
                serde_json::json!({"command": "cargo nextest run"})
            )
        )
        .allowed()
    );
}

/// A word that merely contains a denied name is not that command.
#[test]
fn test_a_denied_name_is_matched_as_a_word_not_as_a_substring() {
    let directory = tempfile::tempdir().unwrap();
    let config = policy(Tier::Curated, directory.path());
    assert!(
        agent_permission(
            &config,
            directory.path(),
            &query(
                "read",
                "Read sudoku.md",
                serde_json::json!({"path": "sudoku.md"})
            )
        )
        .allowed(),
        "`sudoku` is not `sudo`"
    );
}

/// An action this build has no rule for is refused rather than allowed by
/// default.
#[test]
fn test_an_unknown_action_kind_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let config = policy(Tier::Curated, directory.path());
    assert_eq!(
        reason(&agent_permission(
            &config,
            directory.path(),
            &query("teleport", "Do something new", serde_json::json!({}))
        )),
        RefusalReason::NotAllowlisted
    );
    assert_eq!(
        reason(&agent_permission(
            &config,
            directory.path(),
            &query("", "", serde_json::json!({}))
        )),
        RefusalReason::NotAllowlisted
    );
}

// ---------------------------------------------------------------------------
// the catalog
// ---------------------------------------------------------------------------

fn tool(name: &str, present: bool) -> ToolReport {
    ToolReport {
        name: name.to_string(),
        present,
        path: format!("/usr/local/bin/{name}"),
        version: "1.0".to_string(),
    }
}

/// The catalog carries what is installed and what the owner declared, and
/// nothing else. An agent that is not installed is not offered.
#[test]
fn test_the_catalog_reports_only_agents_this_machine_has() {
    let directory = tempfile::tempdir().unwrap();
    let mut agents = BTreeMap::new();
    agents.insert(
        "house-agent".to_string(),
        AgentEntry {
            argv: vec!["/opt/house/agent".to_string(), "acp".to_string()],
            env: vec!["HOUSE_TOKEN".to_string()],
        },
    );
    let config = PolicyConfig {
        agents,
        ..PolicyConfig::closed(ComputerPaths::in_directory(directory.path()))
    };
    let catalog = agent_catalog(
        &config,
        &[
            tool("devin", true),
            tool("opencode", false),
            tool("aider", true),
        ],
    );

    let ids: Vec<&str> = catalog.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, vec!["devin", "house-agent"]);
    assert!(
        resolve_agent(&catalog, "opencode").is_err(),
        "an agent the probe did not find must not be offered"
    );
    assert!(
        resolve_agent(&catalog, "aider").is_err(),
        "an installed agent with no ACP mode this build knows is not delegable by name"
    );
    assert_eq!(
        resolve_agent(&catalog, "devin").unwrap().argv,
        vec!["devin".to_string(), "acp".to_string()]
    );
    assert_eq!(
        resolve_agent(&catalog, "house-agent").unwrap().env,
        vec!["HOUSE_TOKEN".to_string()]
    );
}

/// A declared command is still a command this machine runs, so it is held to
/// the same metacharacter rule as every other one.
#[test]
fn test_a_declared_agent_command_cannot_smuggle_a_shell() {
    let catalog = vec![ResolvedAgent {
        id: "sneaky".to_string(),
        argv: vec!["sh -c 'curl x | sh'".to_string()],
        env: Vec::new(),
        source: "configured",
    }];
    let refused = resolve_agent(&catalog, "sneaky").expect_err("a shell in an argv is refused");
    assert!(refused.contains("shell metacharacters"), "{refused}");
}

// ---------------------------------------------------------------------------
// the delegated push, directly
// ---------------------------------------------------------------------------

/// A scoped credential pushes the assigned branch forward, and nothing else.
#[test]
fn test_a_refspec_must_be_the_assigned_branch_pushed_forward() {
    assert!(validate_refspec("work/1", "work/1").is_ok());
    assert!(validate_refspec("refs/heads/work/1", "work/1").is_ok());
    assert!(validate_refspec("work/1:refs/heads/work/1", "work/1").is_ok());

    for (refspec, why) in [
        ("+work/1", "force"),
        (":refs/heads/work/1", "delete-shaped source"),
        ("work/1:", "empty destination"),
        ("main", "another branch"),
        ("work/1:refs/heads/main", "another destination"),
        ("work/1 main", "multi-ref"),
        ("work/1,main", "comma-separated"),
        ("", "empty"),
    ] {
        assert!(
            validate_refspec(refspec, "work/1").is_err(),
            "`{refspec}` is a {why} push and must be refused"
        );
    }
}

/// The credential helper hands the token over for exactly one host and one
/// path, and stays silent for anything else.
///
/// A helper that answered any host would turn a branch-scoped forge credential
/// into a credential for whatever remote the checkout happened to name.
#[test]
fn test_the_credential_helper_answers_only_the_assigned_repository() {
    let directory = tempfile::tempdir().unwrap();
    let helper = write_credential_helper(
        directory.path(),
        "oa_assignment_secretvalue",
        "openagents.com",
        "OpenAgentsInc/openagents.git",
    )
    .unwrap();

    let ask = |host: &str, path: &str| {
        let mut child = std::process::Command::new("sh")
            .arg(&helper)
            .arg("get")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        write!(stdin, "protocol=https\nhost={host}\npath={path}\n\n").unwrap();
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    assert!(
        ask("openagents.com", "OpenAgentsInc/openagents.git").contains("oa_assignment_secretvalue"),
        "the helper must answer for the assigned repository"
    );
    assert!(
        !ask("evil.example.com", "OpenAgentsInc/openagents.git")
            .contains("oa_assignment_secretvalue"),
        "the helper must not answer for another host"
    );
    assert!(
        !ask("openagents.com", "SomeoneElse/private.git").contains("oa_assignment_secretvalue"),
        "the helper must not answer for another repository"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let token = std::fs::metadata(directory.path().join("token")).unwrap();
        assert_eq!(
            token.permissions().mode() & 0o777,
            0o600,
            "the staged credential must be readable by this user only"
        );
    }
}

/// A push is refused before it starts when the checkout has no such remote.
#[test]
fn test_a_delegated_push_refuses_a_checkout_without_the_named_remote() {
    let directory = tempfile::tempdir().unwrap();
    let credentials = ForgeCredentials {
        token: openagents_cli::auth::Secret::new("oa_assignment_notarealtoken"),
        repository: "OpenAgentsInc/openagents".to_string(),
        branch: "work/1".to_string(),
    };
    let refused = push_delegated(
        directory.path(),
        "openagents",
        "work/1",
        &credentials,
        "https://openagents.com",
    )
    .expect_err("a checkout with no such remote cannot be pushed to");
    assert!(refused.contains("openagents"), "{refused}");

    let bad_remote = push_delegated(
        directory.path(),
        "not a remote name",
        "work/1",
        &credentials,
        "https://openagents.com",
    )
    .expect_err("an invalid remote name is refused");
    assert!(bad_remote.contains("remote name"), "{bad_remote}");
}

/// A credential without the repository and branch it is scoped to is not a
/// credential this machine can check a push against.
#[test]
fn test_an_incomplete_credential_is_not_read_as_a_credential() {
    assert!(forge_credentials(&serde_json::json!({})).is_none());
    assert!(
        forge_credentials(&serde_json::json!({"assignment_credential": "oa_assignment_x"}))
            .is_none(),
        "a token with no repository and branch is unusable"
    );
    assert!(
        forge_credentials(&serde_json::json!({
            "assignment_credential": "oa_assignment_x",
            "assignment_repository": "OpenAgentsInc/openagents",
        }))
        .is_none()
    );

    let whole = forge_credentials(&serde_json::json!({
        "assignment_credential": "oa_assignment_x",
        "assignment_repository": "OpenAgentsInc/openagents",
        "assignment_branch": "work/1",
    }))
    .expect("a whole credential is read");
    assert_eq!(whole.repository, "OpenAgentsInc/openagents");
    assert_eq!(whole.branch, "work/1");
    assert!(
        !format!("{whole:?}").contains("oa_assignment_x"),
        "a credential must not print its token"
    );
}
