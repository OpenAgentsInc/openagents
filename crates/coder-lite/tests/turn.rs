//! What a coder-lite turn does on the wire, proved against a real socket.
//!
//! The session under test is the production one — [`coder_lite::runtime::Session`]
//! over `openagents_cli`'s thread, grant, and proxy path — talking to a server
//! this file starts on a real port. Nothing is mocked between the session and
//! the socket.
//!
//! Two properties are what these are for:
//!
//! - **A tool really runs.** The stub asks for `shell`, and the assertion is
//!   that the command's own output came back through the frame's channel. A
//!   test that only checked the tool was *declared* passes on a session that
//!   runs nothing.
//! - **A refusal is a refusal.** The version of this runtime that these guard
//!   against answered a rejected request with a fabricated grant and a
//!   sentence about an offline fallback. So the refusal tests assert on
//!   `Failed` and on what it says, and assert that no reply text arrived.

use coder_lite::runtime::{Control, Session};
use openagents_cli::runtime::Lane;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ───────────────────────────────────────────────────────────────── the server

enum Reply {
    Body(u16, &'static str, String),
    Sse(Vec<String>),
}

struct Stub {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    /// Every request this stub has taken, headers and body, most recent last.
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    fn request_lines(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.lines().next().unwrap_or_default().to_string())
            .collect()
    }
}

fn start<H>(handler: H) -> Stub
where
    H: Fn(&str, &str) -> Reply + Send + Sync + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    let base = format!("{origin}/api/v1");
    let requests = Arc::new(Mutex::new(Vec::new()));

    let seen = Arc::clone(&requests);
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let seen = Arc::clone(&seen);
            let handler = Arc::clone(&handler);
            let origin = origin.clone();
            tokio::spawn(async move {
                let Some(request) = read_request(&mut socket).await else {
                    return;
                };
                seen.lock().unwrap().push(request.clone());
                match handler(&request, &origin) {
                    Reply::Body(status, content_type, body) => {
                        let head = format!(
                            "HTTP/1.1 {status} X\r\ncontent-type: {content_type}\r\n\
                             content-length: {}\r\nconnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = socket.write_all(head.as_bytes()).await;
                        let _ = socket.write_all(body.as_bytes()).await;
                    }
                    Reply::Sse(frames) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        for frame in frames {
                            let _ = socket
                                .write_all(format!("data: {frame}\n\n").as_bytes())
                                .await;
                            let _ = socket.flush().await;
                        }
                        let _ = socket.write_all(b"data: [DONE]\n\n").await;
                    }
                }
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            });
        }
    });

    Stub { base, requests }
}

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
        r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"sig_test","url":"{origin}/api/inference/proxy","model":"gemini-3.7-flash"}}}}"#
    )
}

/// One OpenAI-shaped chunk carrying text.
fn text(piece: &str) -> String {
    serde_json::json!({"choices":[{"delta":{"content": piece}}]}).to_string()
}

/// One chunk asking for a tool.
fn call(name: &str, arguments: serde_json::Value) -> String {
    serde_json::json!({"choices":[{"delta":{"tool_calls":[{
        "index": 0,
        "id": "call_1",
        "function": {"name": name, "arguments": arguments.to_string()}
    }]}}]})
    .to_string()
}

fn usage(total: u64) -> String {
    serde_json::json!({"choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":total}}).to_string()
}

// ───────────────────────────────────────────────────────────────── the harness

fn session(base: &str, tx: Sender<Control>) -> Session {
    Session::open_at(
        Lane::Flash,
        "flash",
        None,
        // No ACP agents: whether one happens to be installed on the machine
        // running the tests is not this test's business.
        Vec::new(),
        base.to_string(),
        Some("oat_test".to_string()),
        false,
        tx,
    )
}

/// Everything the frame was told, in order, once the turn has finished.
fn drain(rx: &Receiver<Control>) -> Vec<Control> {
    let mut seen = Vec::new();
    while let Ok(control) = rx.try_recv() {
        seen.push(control);
    }
    seen
}

fn reply_text(seen: &[Control]) -> String {
    seen.iter()
        .filter_map(|c| match c {
            Control::Chunk(chunk) => Some(chunk.as_str()),
            _ => None,
        })
        .collect()
}

fn tool_output(seen: &[Control]) -> String {
    seen.iter()
        .filter_map(|c| match c {
            Control::ToolOutput { chunk, .. } => Some(chunk.as_str()),
            _ => None,
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────── the tests

/// The whole loop: a thread is opened, the model asks for `shell`, the command
/// runs on this machine, its output goes back, and the second step answers.
#[tokio::test]
async fn a_turn_opens_a_thread_runs_a_tool_and_streams_the_answer() {
    let step = Arc::new(Mutex::new(0usize));
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            let mut at = step.lock().unwrap();
            *at += 1;
            return if *at == 1 {
                Reply::Sse(vec![call(
                    "shell",
                    serde_json::json!({"command": "printf ONE_TWO_THREE"}),
                )])
            } else {
                Reply::Sse(vec![text("ran it"), usage(42)])
            };
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("run it", tx).await;

    let seen = drain(&rx);
    assert!(
        seen.iter().any(|c| matches!(c, Control::Done)),
        "the turn never finished: {seen:?}"
    );
    assert!(
        !seen.iter().any(|c| matches!(c, Control::Failed(_))),
        "the turn failed: {seen:?}"
    );

    // The tool was announced before it answered, so the frame could show it.
    let announced = seen.iter().position(|c| matches!(c, Control::Tool { .. }));
    let answered = seen
        .iter()
        .position(|c| matches!(c, Control::ToolOutput { .. }));
    assert!(announced.is_some(), "no tool call reached the frame: {seen:?}");
    assert!(announced < answered, "the result preceded the call: {seen:?}");

    // And it really ran: this is the command's own output.
    assert!(
        tool_output(&seen).contains("ONE_TWO_THREE"),
        "the shell tool did not run: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|c| matches!(c, Control::ToolDone { is_error: false, .. })),
        "the call was not settled: {seen:?}"
    );

    assert_eq!(reply_text(&seen), "ran it");

    // What answered, as the grant pinned it — not the lane's preference.
    assert!(
        seen.iter()
            .any(|c| matches!(c, Control::Model(model) if model == "gemini-3.7-flash")),
        "the model that answered was not reported: {seen:?}"
    );
    // And what it cost, as the server reported it.
    assert!(
        seen.iter()
            .any(|c| matches!(c, Control::Usage(u) if u.total_tokens == 42)),
        "the usage was not reported: {seen:?}"
    );

    let lines = stub.request_lines();
    assert!(lines.iter().any(|l| l.starts_with("POST /api/v1/threads")));
}

/// A failing command is a failing command. Reporting it as a success is how a
/// session comes to believe a build passed.
#[tokio::test]
async fn a_tool_that_failed_is_settled_as_a_failure() {
    let step = Arc::new(Mutex::new(0usize));
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            let mut at = step.lock().unwrap();
            *at += 1;
            return if *at == 1 {
                Reply::Sse(vec![call("shell", serde_json::json!({"command": "exit 7"}))])
            } else {
                Reply::Sse(vec![text("it failed")])
            };
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("run it", tx).await;

    let seen = drain(&rx);
    assert!(
        seen.iter()
            .any(|c| matches!(c, Control::ToolDone { is_error: true, .. })),
        "a command that exited 7 was settled as a success: {seen:?}"
    );
    assert!(tool_output(&seen).contains('7'), "{seen:?}");
}

/// The gate that refuses what cannot be undone travels with the tool.
#[tokio::test]
async fn a_destructive_command_is_refused_and_the_refusal_says_why() {
    let step = Arc::new(Mutex::new(0usize));
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            let mut at = step.lock().unwrap();
            *at += 1;
            return if *at == 1 {
                Reply::Sse(vec![call("shell", serde_json::json!({"command": "rm -rf ~/"}))])
            } else {
                Reply::Sse(vec![text("understood")])
            };
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("clean up", tx).await;

    let seen = drain(&rx);
    let output = tool_output(&seen);
    assert!(
        output.contains("erase a root or a home directory"),
        "the refusal did not travel with the tool: {output}"
    );
    assert!(
        seen.iter()
            .any(|c| matches!(c, Control::ToolDone { is_error: true, .. })),
        "the refusal was settled as a success: {seen:?}"
    );
}

/// A server that will not open a thread ends the turn. It used to end it with
/// a grant the client made up out of the caller's own credential.
#[tokio::test]
async fn a_refused_thread_fails_the_turn_and_invents_no_reply() {
    let stub = start(|request, _| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(
                402,
                "application/json",
                r#"{"error":"insufficient_credit"}"#.to_string(),
            );
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("hello", tx).await;

    let seen = drain(&rx);
    let failure = seen
        .iter()
        .find_map(|c| match c {
            Control::Failed(why) => Some(why.clone()),
            _ => None,
        })
        .expect(&format!("the refusal was swallowed: {seen:?}"));
    assert!(failure.contains("402"), "{failure}");
    assert!(failure.contains("insufficient_credit"), "{failure}");
    assert_eq!(reply_text(&seen), "", "a reply was invented: {seen:?}");
    assert!(seen.iter().any(|c| matches!(c, Control::Done)));
    // No model answered, so none is named.
    assert!(
        !seen.iter().any(|c| matches!(c, Control::Model(_))),
        "a model was named where none answered: {seen:?}"
    );
}

/// A proxy that refuses mid-turn is the same rule one level down.
#[tokio::test]
async fn a_refused_proxy_fails_the_turn() {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Body(
                503,
                "application/json",
                r#"{"error":"upstream_unavailable"}"#.to_string(),
            );
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("hello", tx).await;

    let seen = drain(&rx);
    let failure = seen
        .iter()
        .find_map(|c| match c {
            Control::Failed(why) => Some(why.clone()),
            _ => None,
        })
        .expect("the refusal was swallowed");
    assert!(failure.contains("503"), "{failure}");
    assert_eq!(reply_text(&seen), "", "a reply was invented");
}

/// Leaving reports what the session did and ends the thread, and reports what
/// the server billed. A thread left open holds its grant's remaining budget.
///
/// The `DELETE` this replaces recorded a session that had answered as
/// `cancelled` (issue #106) and left it unresumable, so the assertion is on
/// both halves: the report says `succeeded`, and no revocation is sent.
#[tokio::test]
async fn leaving_reports_what_the_session_did_rather_than_cancelling_it() {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("POST /api/v1/threads/th_test/report") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"thread":{"id":"th_test","status":"succeeded"},
                    "grant":{"spent":{"total_tokens":99}}}"#
                    .to_string(),
            );
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Sse(vec![text("hi"), usage(99)]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("hello", tx).await;
    let _ = drain(&rx);

    let line = tokio::time::timeout(Duration::from_secs(10), session.finish())
        .await
        .expect("the ending hung")
        .expect("the ending failed")
        .expect("the server reported no spend");
    assert!(line.contains("99"), "{line}");

    let reported = stub
        .requests()
        .into_iter()
        .find(|r| r.contains("POST /api/v1/threads/th_test/report"))
        .expect("the session never said what it did");
    let body: serde_json::Value =
        serde_json::from_str(reported.split("\r\n\r\n").nth(1).unwrap_or("{}")).unwrap();
    assert_eq!(body["status"], "succeeded");
    assert_eq!(
        body.get("error_code"),
        None,
        "a session that answered named an error code: {body}"
    );

    assert!(
        !stub
            .request_lines()
            .iter()
            .any(|l| l.starts_with("DELETE /api/v1/threads/")),
        "leaving cancelled the thread instead of reporting: {:?}",
        stub.request_lines()
    );
}

/// A second turn keeps the first turn's thread rather than opening another.
/// Opening one per turn threw away the conversation's own budget and left a
/// trail of threads nothing revoked.
#[tokio::test]
async fn a_second_turn_reuses_the_first_turns_thread() {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Sse(vec![text("ok")]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("one", tx.clone()).await;
    session.execute_turn("two", tx).await;
    let _ = drain(&rx);

    let opens = stub
        .request_lines()
        .iter()
        .filter(|l| l.starts_with("POST /api/v1/threads "))
        .count();
    assert_eq!(opens, 1, "a thread was opened per turn: {opens}");
}
