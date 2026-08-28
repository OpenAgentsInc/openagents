//! The Agent Client Protocol client, against a server that speaks it.
//!
//! The stand-in is an ACP server in twenty lines of Python: it reads
//! newline-delimited JSON-RPC on stdin and writes it on stdout, exactly as
//! `devin acp` does. What is under test is this crate's half of that
//! conversation — the handshake order, the session, the permission answer the
//! child has nobody else to give, the update parsing, and the kill.
//!
//! The client this replaces built one `initialize` request as a struct and
//! never sent it anywhere.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use openagents_cli::acp::{AcpEvent, AcpFailure, AcpHarness, PermissionMode, first_allow_option};
use tokio::sync::watch;

const SERVER: &str = r#"#!/usr/bin/env python3
import json, sys, time

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

sys.stderr.write("this line is not protocol and must be ignored\n")
sys.stdout.write("neither is this one\n")
sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"protocolVersion": 1}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"sessionId": "sess_" + message["params"]["cwd"].split("/")[-1]}})
    elif method == "session/set_mode":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"modeId": message["params"]["modeId"]}})
    elif method == "session/prompt":
        sid = message["params"]["sessionId"]
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
              "update": {"sessionUpdate": "tool_call", "toolCallId": "t1",
                         "kind": "execute", "title": "Ran ls"}}})
        # The agent asks. A delegated child has nobody to ask, so the client
        # answers, and the server proves it by only continuing once it has.
        send({"jsonrpc": "2.0", "id": 9001, "method": "session/request_permission",
              "params": {"sessionId": sid, "options": [
                  {"optionId": "reject-once", "kind": "reject_once"},
                  {"optionId": "allow-always", "kind": "allow_always"}]}})
        answer = json.loads(sys.stdin.readline())
        chosen = answer["result"]["outcome"]["optionId"]
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
              "update": {"sessionUpdate": "usage_update",
                         "_meta": {"cognition.ai/inputTokens": 120,
                                   "cognition.ai/outputTokens": 34}}}})
        for piece in ["the answer ", "in two ", "pieces, permitted by " + chosen]:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
                  "update": {"sessionUpdate": "agent_message_chunk",
                             "content": {"type": "text", "text": piece}}}})
            time.sleep(0.2)
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;

fn stand_in(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oa-acp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn harness(name: &str, body: &str) -> AcpHarness {
    AcpHarness {
        command: stand_in(name, body).to_string_lossy().to_string(),
        args: Vec::new(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    }
}

/// A whole turn: handshake, session, permission, updates, answer.
#[tokio::test]
async fn a_turn_over_acp_streams_and_answers() {
    let harness = harness("acp-server", SERVER);
    let cwd = std::env::temp_dir().join(format!("oa-acp-cwd-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(Instant, AcpEvent)>::new()));
    let sink = std::sync::Arc::clone(&seen);
    let (_stop, mut cancel) = watch::channel(false);

    let started = Instant::now();
    let answer = harness
        .run(
            "do the thing",
            &cwd,
            move |event| sink.lock().unwrap().push((Instant::now(), event)),
            &mut cancel,
        )
        .await
        .expect("the turn failed");

    // The permission answer is in the text, so the answer proves the client
    // both replied to the request and picked the option that allows.
    assert_eq!(
        answer,
        "the answer in two pieces, permitted by allow-always"
    );

    let events = seen.lock().unwrap();
    let session = events
        .iter()
        .find_map(|(_, event)| match event {
            AcpEvent::Session { id } => Some(id.clone()),
            _ => None,
        })
        .expect("no session was reported");
    assert!(session.starts_with("sess_"), "{session}");

    assert!(
        events.iter().any(|(_, event)| matches!(
            event,
            AcpEvent::Tool { title, .. } if title == "Ran ls"
        )),
        "the tool call was not reported"
    );
    assert!(
        events.iter().any(|(_, event)| matches!(
            event,
            AcpEvent::Tokens {
                input: 120,
                output: 34
            }
        )),
        "the token counts were not reported"
    );

    // Streamed, not delivered at the end: the first piece arrives well before
    // the last, because the server sleeps between them.
    let chunks: Vec<Instant> = events
        .iter()
        .filter_map(|(at, event)| match event {
            AcpEvent::Text { .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(chunks.len(), 3, "the answer did not arrive in pieces");
    let spread = chunks[2].duration_since(chunks[0]);
    assert!(
        spread >= Duration::from_millis(300),
        "three pieces arrived {spread:?} apart, which is one delivery and not three"
    );
    assert!(started.elapsed() < Duration::from_secs(30));
}

/// An agent that is not there is a failure that says so.
#[tokio::test]
async fn a_missing_agent_is_reported_as_missing() {
    let harness = AcpHarness {
        command: "/nonexistent/no-such-agent".to_string(),
        args: Vec::new(),
        mode: None,
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let failure = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect_err("a missing binary was reported as a finished turn");
    assert!(
        matches!(&failure, AcpFailure::Unstartable(why) if why.contains("not on PATH")),
        "{failure}"
    );
}

/// An agent that exits without answering is a failure, not an empty answer.
#[tokio::test]
async fn an_agent_that_exits_early_is_a_failure() {
    let harness = harness("acp-quitter", "#!/bin/sh\necho 'not json'\nexit 0\n");
    let (_stop, mut cancel) = watch::channel(false);
    let failure = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect_err("an agent that said nothing reported an answer");
    assert!(
        matches!(&failure, AcpFailure::Refused(why) if why.contains("exited before it answered")),
        "{failure}"
    );
}

/// Stopping the fan-out stops the agent, and does not wait for it.
#[tokio::test]
async fn a_cancelled_turn_stops_the_agent() {
    let harness = harness(
        "acp-slow",
        r#"#!/bin/sh
read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
sleep 120
"#,
    );
    let (stop, mut cancel) = watch::channel(false);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        let _ = stop.send(true);
    });

    let at = Instant::now();
    let failure = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect_err("a cancelled turn reported an answer");
    assert!(matches!(failure, AcpFailure::Cancelled), "{failure}");
    assert!(
        at.elapsed() < Duration::from_secs(20),
        "cancelling took {:?}; the stand-in sleeps for two minutes",
        at.elapsed()
    );
}

/// Claude's adapter gets its own mode ids on the wire, not Devin's.
#[tokio::test]
async fn claude_set_mode_sends_the_adapter_mode_id() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

mode = ""
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"protocolVersion": 1}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_claude"}})
    elif method == "session/set_mode":
        mode = message["params"]["modeId"]
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"modeId": mode}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": message["params"]["sessionId"],
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": mode}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-claude-mode", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "claude".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("do the thing", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "bypassPermissions");

    let readonly = AcpHarness {
        command: stand_in("acp-claude-plan", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "claude".to_string(),
        mode: Some(PermissionMode::ReadOnly),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = readonly
        .run("do the thing", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "plan");

    let other = AcpHarness {
        command: stand_in("acp-devin-mode", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "devin".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = other
        .run("do the thing", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "bypass");
}

/// The permission answer prefers an option that allows.
#[test]
fn the_allowing_option_is_the_one_chosen() {
    let asked = serde_json::json!({"options": [
        {"optionId": "no", "kind": "reject_once"},
        {"optionId": "yes", "kind": "allow_always"}
    ]});
    assert_eq!(first_allow_option(&asked).as_deref(), Some("yes"));

    // Nothing that allows: the first named option, so the agent is answered
    // rather than left waiting.
    let none = serde_json::json!({"options": [{"optionId": "only", "kind": "reject_once"}]});
    assert_eq!(first_allow_option(&none).as_deref(), Some("only"));

    assert_eq!(first_allow_option(&serde_json::json!({})), None);
}

const GROK_AUTH_SERVER: &str = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

authenticated = ""
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "cached_token"}, {"id": "xai.api_key"}, {"id": "grok.com"}],
            "_meta": {"defaultAuthMethodId": "cached_token", "headless": True}
        }})
    elif method == "authenticate":
        params = message.get("params") or {}
        meta = params.get("_meta") or {}
        if meta.get("headless") is not True:
            send({"jsonrpc": "2.0", "id": message["id"], "error": {"message": "headless required"}})
            continue
        authenticated = params.get("methodId") or ""
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        if not authenticated:
            send({"jsonrpc": "2.0", "id": message["id"], "error": {"message": "auth_required"}})
            continue
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_grok"}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": message["params"]["sessionId"],
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "authed:" + authenticated}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;

#[tokio::test]
async fn grok_authenticates_with_cached_token_before_opening_a_session() {
    let harness = AcpHarness {
        command: stand_in("acp-grok-auth", GROK_AUTH_SERVER)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "authed:cached_token");
}

#[tokio::test]
async fn grok_uses_the_api_key_method_when_the_env_has_one() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

authenticated = ""
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "xai.api_key"}, {"id": "cached_token"}]
        }})
    elif method == "authenticate":
        authenticated = (message.get("params") or {}).get("methodId") or ""
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_key"}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "sess_key",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": authenticated}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-grok-key", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        env: Some(vec![("XAI_API_KEY".to_string(), "test-key".to_string())]),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "xai.api_key");
}

#[tokio::test]
async fn devin_browser_auth_is_skipped_and_session_still_opens() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

seen = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    seen.append(method)
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "devin-browser"}]
        }})
    elif method == "authenticate":
        send({"jsonrpc": "2.0", "id": message["id"], "error": {"message": "browser login must not run"}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_devin"}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "sess_devin",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "methods:" + ",".join(seen)}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-devin-browser", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "devin".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert!(
        answer.starts_with("methods:"),
        "unexpected answer: {answer}"
    );
    assert!(
        !answer.contains("authenticate"),
        "Devin browser authenticate was sent: {answer}"
    );
}

#[tokio::test]
async fn grok_with_only_interactive_auth_is_refused_before_session_new() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "grok.com"}]
        }})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "error": {"message": "session opened without authenticate"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-grok-oidc", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let failure = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect_err("interactive-only Grok opened a session");
    assert!(
        matches!(&failure, AcpFailure::Refused(why) if why.contains("grok login")),
        "{failure}"
    );
}

#[tokio::test]
async fn grok_dangerous_opens_a_session_with_yolo_mode_and_no_bypass() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

seen = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    seen.append(method)
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "cached_token"}]
        }})
    elif method == "authenticate":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        yolo = ((message.get("params") or {}).get("_meta") or {}).get("yoloMode")
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_yolo"}})
        opened = "yolo" if yolo is True else "no-yolo"
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "sess_yolo",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": opened}}}})
    elif method == "session/set_mode":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "sess_yolo",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "set-mode:" + message["params"]["modeId"]}}}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-grok-yolo", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "yolo");
    assert!(!answer.contains("bypass"));
}

#[tokio::test]
async fn grok_prompt_mode_sets_ask() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

mode = ""
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "cached_token"}]
        }})
    elif method == "authenticate":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_ask"}})
    elif method == "session/set_mode":
        mode = message["params"]["modeId"]
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"modeId": mode}})
    elif method == "session/prompt":
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "sess_ask",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": mode}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "result": {}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-grok-ask", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        mode: Some(PermissionMode::Prompt),
        ..AcpHarness::default()
    };
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run("hello", &std::env::temp_dir(), |_| {}, &mut cancel)
        .await
        .expect("the turn failed");
    assert_eq!(answer, "ask");
}

#[tokio::test]
async fn grok_turn_maps_thought_tool_updates_plan_and_xai_usage() {
    let server = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": 1,
            "authMethods": [{"id": "cached_token"}]
        }})
    elif method == "authenticate":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"sessionId": "sess_turn"}})
    elif method == "session/prompt":
        sid = message["params"]["sessionId"]
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "agent_thought_chunk",
                       "content": {"type": "text", "text": "pondering"}}}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "tool_call", "kind": "read", "title": "Read a.rs"}}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "tool_call_update", "kind": "read", "status": "completed"}}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "plan"}}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "usage_update",
                       "_meta": {"x.ai/inputTokens": 9, "x.ai/outputTokens": 4}}}})
        send({"jsonrpc": "2.0", "id": 77, "method": "x.ai/ask_user_question",
              "params": {"sessionId": sid, "questions": [{"header": "go?"}]}})
        answer = json.loads(sys.stdin.readline())
        chosen = answer["result"]["outcome"]
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid,
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "done:" + chosen}}}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    else:
        send({"jsonrpc": "2.0", "id": message.get("id", 0), "error": {"code": -32601, "message": method}})
"#;
    let harness = AcpHarness {
        command: stand_in("acp-grok-turn", server)
            .to_string_lossy()
            .to_string(),
        args: Vec::new(),
        agent_id: "grok".to_string(),
        mode: Some(PermissionMode::Dangerous),
        ..AcpHarness::default()
    };
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<AcpEvent>::new()));
    let sink = std::sync::Arc::clone(&seen);
    let (_stop, mut cancel) = watch::channel(false);
    let answer = harness
        .run(
            "hello",
            &std::env::temp_dir(),
            move |event| sink.lock().unwrap().push(event),
            &mut cancel,
        )
        .await
        .expect("the turn failed");
    assert_eq!(answer, "done:cancelled");
    let events = seen.lock().unwrap();
    assert!(
        events.iter().any(|event| matches!(event, AcpEvent::Tool { kind, title } if kind == "thought" && title == "pondering")),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AcpEvent::Tool { title, .. } if title == "Read a.rs")),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AcpEvent::Tool { kind, .. } if kind == "plan")),
        "{events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            AcpEvent::Tokens {
                input: 9,
                output: 4
            }
        )),
        "{events:?}"
    );
}
