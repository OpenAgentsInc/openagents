//! The end-to-end half of the live-execution audit for
//! OpenAgentsInc/openagents#89.
//!
//! Six defects were found by driving the compiled binary against the live
//! deployment rather than by reading the source. Five are fixed in
//! `c48fa5b138` and the sixth in the commit this file arrives on; each test
//! below now asserts the **fixed** behaviour, and each keeps the account of
//! what the defect was, because the reason a test exists outlives the
//! assertion.
//!
//! These run one layer out from the unit tests that accompany the fixes:
//! `tools::defect_tests` calls `run_real_shell` and `floor_char_boundary`
//! directly, while these go through `HarnessToolRegistry::execute_tool` with
//! real subprocesses, and through `CoderRuntimeSession` against a real socket.
//! Where a defect is covered at both layers the outer one is kept here and the
//! duplicate dropped — the hosted-lane transcript test lives in
//! `runtime_test.rs` as `the_second_turn_carries_what_the_first_turn_answered`,
//! so what remains here is the local lane, which that fix also changed and
//! nothing else covers.
//!
//! What the live run confirmed working, and what is therefore not re-asserted
//! here: a headless turn reaches a real model and prints its answer, a turn
//! calls `shell` and answers from the result, a two-child fan-out returns two
//! real outputs from separate git worktrees, every lane opens on the model it
//! names, and reported token counts match what `GET /api/v1/threads` records.

use openagents_cli::runtime::{CoderRuntimeSession, Lane};
use openagents_cli::tools::{
    HarnessToolRegistry, OpenAgentsCliSource, ToolCall, resolve_openagents_cli,
};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ───────────────────────────────────────────────────────────────── the stub

/// What a request should be answered with.
enum Reply {
    Json(String),
    /// Server-sent events, the shape the inference proxy streams.
    Sse(Vec<String>),
    /// Newline-delimited JSON, the shape Ollama streams.
    Ndjson(Vec<String>),
}

/// A stand-in for whichever server the session is talking to, recording what
/// it was asked.
///
/// It answers on a real socket, so everything between the session and the wire
/// is the production path and what these tests assert on is the bytes that
/// were actually sent.
struct Stub {
    base: String,
    origin: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    /// Every request this stub has taken, headers and body, oldest first.
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    /// Just the bodies of the calls that asked a model to say something.
    fn completions(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|r| r.starts_with("POST /proxy") || r.starts_with("POST /api/chat"))
            .collect()
    }
}

/// Start a stub whose reply is chosen by the request and by how many
/// completions it has already served.
fn start<H>(handler: H) -> Stub
where
    H: Fn(&str, usize, &str) -> Reply + Send + Sync + 'static,
{
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&requests);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("http://127.0.0.1:{port}");
    let base = format!("{origin}/api/v1");
    let handler_origin = origin.clone();

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let mut served = 0usize;
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let Some(request) = read_request(&mut socket).await else {
                continue;
            };
            recorder.lock().unwrap().push(request.clone());

            // The catalog, answered before the handler sees the request. A
            // switchable lane resolves its model against this at open, and a
            // stub that did not serve it would refuse at the lane instead of
            // exercising whatever the handler was written to exercise.
            let reply = if request.starts_with("GET /api/v1/models") {
                Reply::Json(
                    r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true}]}"#
                        .to_string(),
                )
            } else {
                handler(&request, served, &handler_origin)
            };
            match reply {
                Reply::Json(body) => {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
                Reply::Sse(frames) => {
                    served += 1;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n")
                        .await;
                    for frame in frames {
                        let _ = socket
                            .write_all(format!("data: {frame}\n\n").as_bytes())
                            .await;
                    }
                    let _ = socket.write_all(b"data: [DONE]\n\n").await;
                }
                Reply::Ndjson(lines) => {
                    served += 1;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\nconnection: close\r\n\r\n")
                        .await;
                    for line in lines {
                        let _ = socket.write_all(format!("{line}\n").as_bytes()).await;
                    }
                }
            }
            let _ = socket.flush().await;
        }
    });

    Stub {
        base,
        origin,
        requests,
    }
}

/// Read one request, headers and declared body, and return it as text.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&request);
        if let Some(headers_end) = text.find("\r\n\r\n") {
            let length = text
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length: ")
                        .or_else(|| line.strip_prefix("Content-Length: "))
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + length {
                break;
            }
        }
    }
    Some(String::from_utf8_lossy(&request).to_string())
}

fn grant(origin: &str) -> String {
    format!(
        r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"tok_test","url":"{origin}/proxy","model":"glm-5.3-flash"}}}}"#
    )
}

/// One frame asking for a tool, whole rather than fragmented.
fn call_tool(id: &str, name: &str) -> String {
    serde_json::json!({
        "choices": [{ "delta": { "tool_calls": [{
            "index": 0, "id": id,
            "function": { "name": name, "arguments": "{}" }
        }]}}]
    })
    .to_string()
}

fn registry() -> (tempfile::TempDir, HarnessToolRegistry) {
    let dir = tempfile::tempdir().unwrap();
    let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
    (dir, registry)
}

// ──────────────────────────────────────────────────────────────── defect 1

/// **Output cut through a multi-byte character no longer kills the agent.**
///
/// `run_real_shell` bounded output with `&combined[..OUTPUT_LIMIT]` — a *byte*
/// index into a `String`, which panics when the index is not a character
/// boundary. Any command whose combined output crossed 30,000 bytes
/// mid-character took the whole process down in the middle of a turn: a `git
/// log`, a test run with UTF-8 output, any file with an accent or an emoji
/// past 30 kB. It aborted before the thread could be revoked, so the grant's
/// remaining budget was stranded too.
///
/// Reproduced live against the built binary before the fix:
///
/// ```text
/// $ oa coder --headless "Use the shell tool to run exactly: sh repro.sh"
/// thread 'main' panicked at crates/openagents-cli/src/tools.rs:424:98:
/// byte index 30000 is not a char boundary; it is inside '€' (bytes 29999..30002)
/// EXIT=101
/// ```
///
/// `tools::defect_tests` covers `floor_char_boundary` on its own. This runs
/// the real `/bin/sh` through the tool dispatch and asserts the whole path
/// survives: the panic showed up through a command, so a command is what
/// proves it gone.
#[tokio::test]
async fn shell_output_cut_through_a_multibyte_character_is_truncated_not_fatal() {
    let (_dir, registry) = registry();

    // 29,999 single-byte characters, then three-byte ones. Byte 30,000 lands
    // inside the first `€`, which occupies bytes 29,999-30,001.
    let command = "head -c 29999 /dev/zero | tr '\\0' 'a' && printf '€€€€€€€€€€'";
    let call = ToolCall {
        id: "call_trunc".to_string(),
        name: "shell".to_string(),
        arguments: serde_json::json!({ "command": command }),
    };

    // On its own task, so a panic is reported rather than unwinding the test.
    let output = tokio::spawn(async move { registry.execute_tool(&call).await })
        .await
        .expect("the shell tool panicked on a multi-byte truncation boundary");

    assert!(
        output
            .output
            .contains("[Output truncated: printed 30029 characters, limit is 30000]"),
        "the output should say it was cut, and by how much: {}",
        &output.output[output.output.len().saturating_sub(200)..]
    );
    assert!(
        output.output.starts_with("aaa"),
        "the kept head should be the start of the command's output"
    );
    assert!(
        !output.output.contains('\u{FFFD}'),
        "the cut left a broken character behind"
    );
    assert!(!output.is_error, "the command itself succeeded");
}

// ──────────────────────────────────────────────────────────────── defect 2

/// **The local lane records what it answered, so the next turn can see it.**
///
/// `run_tools` records an assistant turn only when that turn called a tool, so
/// a turn that simply answered never joined `self.messages`. The session is
/// reused across turns, so the model was never shown a word it had said
/// itself. Live, in the interactive session, it did not admit the gap — it
/// confabulated:
///
/// ```text
/// turn 1: "Invent a random six-letter nonsense codeword." -> QORVEN
/// turn 2: "What was the codeword you just invented?"      -> ZORBEX
/// ```
///
/// A test that asks the model to recall something from the **user's** prompt
/// passes against the defect, because user messages were always recorded. The
/// word has to be one only the assistant ever said.
///
/// The hosted lane is covered by `the_second_turn_carries_what_the_first_turn_answered`
/// in `runtime_test.rs`. The fix changed `run_local_turn` the same way, and
/// this is that half: the local lane keeps its own message list, in Ollama's
/// shape, through a separate code path.
#[tokio::test]
async fn the_local_lane_records_what_it_answered() {
    let stub = start(|request, served, _origin| {
        if request.starts_with("GET /api/tags") {
            return Reply::Json(
                r#"{"models":[{"name":"qwen3:0.6b","modified_at":"2026-08-26T00:00:00Z"}]}"#
                    .to_string(),
            );
        }
        let word = if served == 0 { "QORVEN" } else { "ZORBEX" };
        Reply::Ndjson(vec![
            serde_json::json!({
                "model": "qwen3:0.6b",
                "message": { "role": "assistant", "content": word },
                "done": false
            })
            .to_string(),
            serde_json::json!({
                "model": "qwen3:0.6b",
                "message": { "role": "assistant", "content": "" },
                "done": true, "done_reason": "stop",
                "prompt_eval_count": 10, "eval_count": 3
            })
            .to_string(),
        ])
    });

    let (_dir, tools) = registry();
    let mut session = CoderRuntimeSession::new(
        Lane::Local(String::new()),
        Some(stub.base.clone()),
        None,
        tools,
    );
    session.ollama_host = stub.origin.clone();

    let first = session
        .execute_turn("invent a six-letter codeword", |_| {})
        .await
        .unwrap();
    assert_eq!(first, "QORVEN");

    session
        .execute_turn("what was the codeword?", |_| {})
        .await
        .unwrap();

    let chats = stub.completions();
    assert_eq!(chats.len(), 2, "expected one chat call per turn");
    assert!(
        chats[1].contains("QORVEN"),
        "the second turn did not carry the first turn's answer, so the local \
         model cannot see what it said: {}",
        chats[1]
    );
    assert!(
        session
            .messages
            .iter()
            .any(|m| m.role == "assistant" && m.content.as_deref() == Some("QORVEN")),
        "the answer is missing from the session's own transcript"
    );
}

// ──────────────────────────────────────────────────────────────── defect 3

/// **The final step answers instead of exposing another tool.**
///
/// `run_thread_turn` loops `for _ in 0..MAX_TOOL_STEPS` and assigned
/// `final_answer` only on the step that came back without tool calls. A model
/// that asked for a tool on all thirty steps fell out of the bottom and the
/// function returned `Ok(String::new())` — the same `Ok` a finished turn
/// returns, carrying nothing. `run_headless_coder` printed `Turn result:`
/// followed by a blank line and exited 0, and the interactive session settled
/// `TurnEvent::Done("")` as an answered turn.
///
/// The runtime now reserves the last step for synthesis. This keeps the
/// original protection against an empty success without printing an internal
/// tool-budget failure into the conversation.
#[tokio::test]
async fn a_turn_that_reaches_its_last_step_synthesizes() {
    // Every step asks for a tool and never stops asking. The name is one no
    // registry has, so the loop spends no time running anything.
    let stub = start(|request, served, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Json(grant(origin));
        }
        if request.contains(r#""tools":[]"#) {
            return Reply::Sse(vec![
                serde_json::json!({"choices":[{"delta":{"content":"Finished from the gathered evidence."}}]})
                    .to_string(),
            ]);
        }
        Reply::Sse(vec![call_tool(
            &format!("call_{served}"),
            "a_tool_that_does_not_exist",
        )])
    });

    let (_dir, tools) = registry();
    let mut session = CoderRuntimeSession::new(
        Lane::default(),
        Some(stub.base.clone()),
        Some("tok_user".to_string()),
        tools,
    );

    let answer = session
        .execute_turn("loop forever", |_| {})
        .await
        .expect("the reserved synthesis step failed");

    assert_eq!(answer, "Finished from the gathered evidence.");
    assert_eq!(
        stub.completions().len(),
        30,
        "the final request should use the reserved thirtieth model step"
    );
    let last = stub.completions().pop().expect("a final request");
    assert!(
        last.contains(r#""tools":[]"#),
        "tools remained on the final step"
    );
}

// ──────────────────────────────────────────────────────────────── defect 4

/// **The `openagents` tool prefers `PATH` and falls back to this binary.**
///
/// The tool is declared as "Run the OpenAgents CLI commands (issue, project,
/// repo, auth, etc.)" and was implemented as `Command::new("openagents")` — a
/// bare name resolved through `PATH`, with nothing behind it. On the machine
/// this was verified on that reached `openagents v0.4.0`, the TypeScript CLI,
/// while the agent running the tool was `oa 0.1.0`; on a machine carrying only
/// the Rust CLI there is no `openagents` on `PATH` at all and every call
/// failed with `No such file or directory`.
///
/// The contract now: prefer `PATH`, because the CLI installed under that name
/// covers more subcommands than this binary does; fall back to
/// `current_exe()`, so the tool still works where only the Rust binary exists;
/// name which one ran in the result, so a model reading `unknown command`
/// knows which CLI said it; and if neither resolves, return an error rather
/// than a success carrying nothing.
#[tokio::test]
async fn the_openagents_tool_prefers_path_and_falls_back_to_this_binary() {
    let dir = tempfile::tempdir().unwrap();
    let shim = dir.path().join("openagents");
    std::fs::write(&shim, "#!/bin/sh\necho SHIM-ON-PATH\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    // The only test in this file that touches the environment.
    let original = std::env::var("PATH").unwrap_or_default();

    // With something named `openagents` on PATH, that is what runs.
    unsafe {
        std::env::set_var("PATH", format!("{}:{original}", dir.path().display()));
    }
    let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));
    let output = registry
        .execute_tool(&ToolCall {
            id: "call_oa".to_string(),
            name: "openagents".to_string(),
            arguments: serde_json::json!({ "args": ["--version"] }),
        })
        .await;
    let on_path = resolve_openagents_cli();

    // With nothing named `openagents` anywhere, this binary answers for it.
    unsafe {
        std::env::set_var("PATH", "");
    }
    let fallback = resolve_openagents_cli();

    unsafe {
        std::env::set_var("PATH", original);
    }

    assert!(
        output.output.contains("SHIM-ON-PATH"),
        "the CLI on PATH should have run: {}",
        output.output
    );
    assert!(
        output.output.contains("[ran the `openagents` CLI on PATH:")
            && output.output.contains(&shim.display().to_string()),
        "the result should name the program that answered: {}",
        output.output
    );
    assert!(!output.is_error, "the shim exited zero");

    let (found, source) = on_path.expect("a shim on PATH must resolve");
    assert_eq!(source, OpenAgentsCliSource::Path);
    assert_eq!(found, shim);

    let (found, source) = fallback.expect("an empty PATH must still resolve to this binary");
    assert_eq!(
        source,
        OpenAgentsCliSource::ThisBinary,
        "with nothing on PATH the tool should fall back rather than fail"
    );
    assert_eq!(found, std::env::current_exe().unwrap());
}

// ──────────────────────────────────────────────────────────────── defect 5

/// **Cancelling a turn stops the command the `shell` tool started.**
///
/// `run_real_shell` spawned with neither `process_group(0)` nor
/// `kill_on_drop(true)`, while `delegate.rs`, `computer.rs` and `acp.rs` all
/// already put their children in a group. Dropping the future — which is what
/// `run_proxy_child`'s `tokio::select!` does when a fan-out is cancelled —
/// left the operating-system process running, reparented to init. On the
/// default `glm-5.3-flash` lane a child is an in-process task with no pid at all,
/// so `signals::stop_tree` is never called for it and nothing else stopped
/// what it had started.
///
/// Observed live before the fix: `oa coder --delegate --count 2` told to run
/// `sleep 300`, interrupted with `SIGINT`, printed "Stopping the fan-out;
/// children are being signalled", reported both children "stopped before
/// finishing", exited — and left two `sleep 300` processes at `PPID 1`.
///
/// Asserted without pgrep: the abandoned command must not go on to finish its
/// work, so the file it was told to create must never appear.
#[tokio::test]
async fn cancelling_a_shell_tool_call_stops_the_command_it_started() {
    let dir = tempfile::tempdir().unwrap();
    let witness = dir.path().join("the-orphan-kept-going.txt");
    let registry = HarnessToolRegistry::new(Some(dir.path().to_path_buf()));

    let call = ToolCall {
        id: "call_orphan".to_string(),
        name: "shell".to_string(),
        arguments: serde_json::json!({
            "command": format!("sleep 2 && touch '{}'", witness.display())
        }),
    };

    let handle = tokio::spawn(async move { registry.execute_tool(&call).await });
    // Long enough for the shell to be spawned, far short of its `sleep`.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    handle.abort();
    assert!(handle.await.is_err(), "the call should have been cancelled");
    assert!(
        !witness.exists(),
        "the command finished before it could be cancelled; the test is racing"
    );

    // Well past the command's own sleep.
    tokio::time::sleep(std::time::Duration::from_millis(3500)).await;

    assert!(
        !witness.exists(),
        "the shell subprocess outlived the cancelled call and finished its work"
    );
}

// ──────────────────────────────────────────────────────────────── defect 6

/// **A failed tool is reported to the model as a failure.**
///
/// `execute_tool`'s `shell` arm returned `is_error: false` unconditionally,
/// whatever the command exited with, and the `openagents` arm did the same
/// even when the program could not be spawned. Only the refusal check and an
/// unknown tool name ever set the flag, so a failing build read to the model
/// exactly like a passing one.
///
/// `tools::defect_tests` asserts this on `run_real_shell`'s own return value.
/// This asserts the layer above — that the outcome survives the dispatch into
/// `ToolOutput`, which is the field a caller actually reads.
#[tokio::test]
async fn a_failing_shell_command_is_reported_as_a_failure_through_the_tool_result() {
    let (_dir, registry) = registry();

    let failed = registry
        .execute_tool(&ToolCall {
            id: "call_fail".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({ "command": "exit 42" }),
        })
        .await;
    assert!(
        failed.output.contains("exited with code 42"),
        "the exit code belongs in the text the model reads: {}",
        failed.output
    );
    assert!(failed.is_error, "a non-zero exit must reach the caller");

    let worked = registry
        .execute_tool(&ToolCall {
            id: "call_ok".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({ "command": "echo fine" }),
        })
        .await;
    assert_eq!(worked.output, "fine");
    assert!(
        !worked.is_error,
        "a successful command must not be an error"
    );
}
