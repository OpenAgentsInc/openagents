//! What a Coder turn does on the wire, proved against a real socket.
//!
//! The session under test is the production one — [`openagents_cli::coder::runtime::Session`]
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

use openagents_cli::coder::runtime::{Control, Session};
use openagents_cli::coder::turn::TurnId;
use openagents_cli::runtime::Lane;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ───────────────────────────────────────────────────────────────── the server

enum Reply {
    Body(u16, &'static str, String),
    Sse(Vec<String>),
    DelayedSse(Duration, Vec<String>),
    ResponsesSse(Duration, String),
    ResponsesStream(
        Vec<(&'static str, serde_json::Value)>,
        Option<(usize, Duration)>,
    ),
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
                // The catalog, answered before the handler sees it. A
                // switchable lane resolves its model against this at open, so
                // a stub that did not serve it would refuse at the lane and
                // never reach the route the test was written for.
                let reply = if request.starts_with("GET /api/v1/models") {
                    Reply::Body(
                        200,
                        "application/json",
                        r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true}]}"#
                            .to_string(),
                    )
                } else {
                    handler(&request, &origin)
                };
                match reply {
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
                    Reply::DelayedSse(delay, frames) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        tokio::time::sleep(delay).await;
                        for frame in frames {
                            let _ = socket
                                .write_all(format!("data: {frame}\n\n").as_bytes())
                                .await;
                            let _ = socket.flush().await;
                        }
                        let _ = socket.write_all(b"data: [DONE]\n\n").await;
                    }
                    Reply::ResponsesSse(delay, text) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        tokio::time::sleep(delay).await;
                        let delta = serde_json::json!({
                            "type": "response.output_text.delta",
                            "delta": text
                        });
                        let completed = serde_json::json!({
                            "type": "response.completed",
                            "response": {"usage": {}}
                        });
                        let _ = socket
                            .write_all(
                                format!(
                                    "event: response.output_text.delta\ndata: {delta}\n\n\
                                     event: response.completed\ndata: {completed}\n\n"
                                )
                                .as_bytes(),
                            )
                            .await;
                    }
                    Reply::ResponsesStream(events, pause) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        for (index, (event, data)) in events.into_iter().enumerate() {
                            if let Some((at, delay)) = pause {
                                if index == at {
                                    tokio::time::sleep(delay).await;
                                }
                            }
                            let _ = socket
                                .write_all(format!("event: {event}\ndata: {data}\n\n").as_bytes())
                                .await;
                            let _ = socket.flush().await;
                        }
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

fn dev_session(base: &str, tx: Sender<Control>) -> Session {
    Session::open_at(
        Lane::Flash,
        "flash",
        None,
        Vec::new(),
        base.to_string(),
        Some("oat_test".to_string()),
        true,
        tx,
    )
}

async fn cancel_when_progress_contains(expected: &str) -> Vec<String> {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("POST /api/v1/threads/th_test/report") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"thread":{"id":"th_test","status":"cancelled"},
                    "grant":{"spent":{"total_tokens":0}}}"#
                    .to_string(),
            );
        }
        if request.contains("/api/inference/proxy") {
            return Reply::DelayedSse(Duration::from_millis(250), vec![text("late")]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });
    let (tx, rx) = channel();
    let mut opened = session(&stub.base, tx.clone());
    opened.set_first_response_policy(Duration::from_millis(5), Duration::from_millis(20));
    let session = Arc::new(tokio::sync::Mutex::new(opened));
    let id = TurnId::new(52);
    let router = session.lock().await.turn_router();
    let running = Arc::clone(&session);
    let task = tokio::spawn(async move {
        running
            .lock()
            .await
            .execute_turn_with_id(id, "wait", tx)
            .await;
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(control) = rx.try_recv() {
                let event = match control {
                    Control::Turn { event, .. } => *event,
                    event => event,
                };
                if matches!(
                    event,
                    Control::Waiting(Some(ref message)) if message.contains(expected)
                ) {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no progress event contained `{expected}`"));

    router.lock().unwrap().cancel(id);
    task.abort();
    let _ = task.await;
    session
        .lock()
        .await
        .settle_cancellation(id)
        .await
        .expect("the progress-state cancellation did not settle");
    stub.requests()
}

/// Everything the frame was told, in order, once the turn has finished.
fn drain(rx: &Receiver<Control>) -> Vec<Control> {
    let mut seen = Vec::new();
    while let Ok(control) = rx.try_recv() {
        match control {
            Control::Turn { event, .. } => seen.push(*event),
            control => seen.push(control),
        }
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
    assert!(
        announced.is_some(),
        "no tool call reached the frame: {seen:?}"
    );
    assert!(
        announced < answered,
        "the result preceded the call: {seen:?}"
    );

    // And it really ran: this is the command's own output.
    assert!(
        tool_output(&seen).contains("ONE_TWO_THREE"),
        "the shell tool did not run: {seen:?}"
    );
    assert!(
        seen.iter().any(|c| matches!(
            c,
            Control::ToolDone {
                is_error: false,
                ..
            }
        )),
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
    assert!(
        stub.requests()
            .iter()
            .filter(|request| request.contains("/api/inference/proxy"))
            .all(|request| !request.contains("\"name\":\"goal\"")),
        "a session with no goal declared the goal tool"
    );
}

#[tokio::test]
async fn an_active_goal_rides_the_turn_and_accounts_for_usage() {
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Sse(vec![text("working"), usage(42)]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.goal_command("/goal --budget 40 finish the native port");
    session.execute_turn("continue", tx).await;

    let request = stub
        .requests()
        .into_iter()
        .find(|request| request.contains("/api/inference/proxy"))
        .expect("proxy request");
    assert!(request.contains("finish the native port"), "{request}");
    assert!(
        request.contains("Token budget remaining: 40 tokens"),
        "{request}"
    );
    assert!(request.contains("\"name\":\"goal\""), "{request}");

    let seen = drain(&rx);
    assert!(seen.iter().any(|control| {
        matches!(
            control,
            Control::Goal(Some(goal))
                if goal.status == openagents_cli::coder::goal::GoalStatus::BudgetLimited
                    && goal.tokens_used == 42
        )
    }));
}

/// A silent provider becomes visible, loses its request at the deadline, and
/// gets one replacement request. The replacement's answer is the only answer
/// that reaches the frame.
#[tokio::test]
async fn a_silent_first_response_is_shown_cancelled_and_retried_once() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.starts_with("POST /api/inference/proxy ") {
            let call = counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return if call == 0 {
                Reply::DelayedSse(Duration::from_millis(250), vec![text("too late")])
            } else {
                Reply::Sse(vec![text("retried")])
            };
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.set_first_response_policy(Duration::from_millis(5), Duration::from_millis(20));
    let started = std::time::Instant::now();
    session.execute_turn("hello", tx).await;

    let seen = drain(&rx);
    assert!(
        seen.iter().any(|control| matches!(
            control,
            Control::Waiting(Some(message)) if message == "Waiting for the model..."
        )),
        "the silent request was not shown: {seen:?}"
    );
    assert!(
        seen.iter().any(|control| matches!(
            control,
            Control::Waiting(Some(message)) if message.contains("Retrying (1 of 1)")
        )),
        "the retry was not shown: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|control| matches!(control, Control::Waiting(None))),
        "the waiting state was not cleared: {seen:?}"
    );
    assert_eq!(
        reply_text(&seen),
        "retried",
        "the late answer leaked: {seen:?}"
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(
        started.elapsed() < Duration::from_millis(150),
        "the first request was not cancelled: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn cancellation_during_the_waiting_state_settles_once() {
    let requests = cancel_when_progress_contains("Waiting for the model").await;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("POST /api/v1/threads/th_test/report"))
            .count(),
        1
    );
}

#[tokio::test]
async fn cancellation_during_retry_transition_settles_once() {
    let requests = cancel_when_progress_contains("Retrying (1 of 1)").await;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.contains("POST /api/v1/threads/th_test/report"))
            .count(),
        1
    );
    assert!(
        requests
            .iter()
            .filter(|request| request.contains("POST /api/inference/proxy"))
            .count()
            <= 2,
        "cancellation started another retry: {requests:?}"
    );
}

/// Dev mode uses `POST /responses`, the path that produced the 158-second
/// wait. It has the same first-response deadline and bounded retry as a grant
/// proxy turn.
#[tokio::test]
async fn a_silent_openresponses_turn_is_cancelled_and_retried_once() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let stub = start(move |request, _origin| {
        if request.contains("POST /api/v1/responses") {
            let call = counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return if call == 0 {
                Reply::ResponsesSse(Duration::from_millis(250), "too late".to_string())
            } else {
                Reply::ResponsesSse(Duration::ZERO, "retried".to_string())
            };
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = dev_session(&stub.base, tx.clone());
    session.set_first_response_policy(Duration::from_millis(5), Duration::from_millis(20));
    session.execute_turn("hello", tx).await;

    let seen = drain(&rx);
    assert_eq!(
        reply_text(&seen),
        "retried",
        "the late answer leaked: {seen:?}"
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(
        seen.iter().any(|control| matches!(
            control,
            Control::Waiting(Some(message)) if message.contains("Retrying (1 of 1)")
        )),
        "the retry was not shown: {seen:?}"
    );
    assert!(seen.iter().any(|control| matches!(control, Control::Done)));
    assert!(
        !seen
            .iter()
            .any(|control| matches!(control, Control::Failed(_))),
        "the retried turn failed: {seen:?}"
    );
    assert!(
        seen.iter()
            .any(|control| matches!(control, Control::Model(model) if model == "glm-5.3-flash")),
        "dev mode did not report the model that answered: {seen:?}"
    );
}

/// Dev mode forwards answer and reasoning deltas while the response is still
/// open. Waiting for `response.completed` would make this test time out before
/// it observes the first answer chunk.
#[tokio::test]
async fn an_openresponses_turn_streams_answer_and_reasoning_before_completion() {
    let stub = start(|request, _origin| {
        if request.contains("POST /api/v1/responses") {
            return Reply::ResponsesStream(
                vec![
                    (
                        "response.reasoning_summary_text.delta",
                        serde_json::json!({
                            "type": "response.reasoning_summary_text.delta",
                            "delta": "Check the request."
                        }),
                    ),
                    (
                        "response.output_text.delta",
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "delta": "Hello "
                        }),
                    ),
                    (
                        "response.output_text.delta",
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "delta": "world."
                        }),
                    ),
                    (
                        "response.completed",
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {"usage": {}}
                        }),
                    ),
                ],
                Some((2, Duration::from_millis(200))),
            );
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = dev_session(&stub.base, tx.clone());
    let running = tokio::spawn(async move {
        session.execute_turn("hello", tx).await;
    });

    let early = tokio::time::timeout(Duration::from_millis(150), async {
        let mut reasoning = String::new();
        let mut answer = String::new();
        while reasoning.is_empty() || answer.is_empty() {
            if let Ok(control) = rx.try_recv() {
                let event = match control {
                    Control::Turn { event, .. } => *event,
                    event => event,
                };
                match event {
                    Control::Reasoning(chunk) => reasoning.push_str(&chunk),
                    Control::Chunk(chunk) => answer.push_str(&chunk),
                    _ => {}
                }
            }
            tokio::task::yield_now().await;
        }
        (reasoning, answer)
    })
    .await
    .expect("no reasoning and answer delta arrived before the response completed");

    assert_eq!(
        early,
        ("Check the request.".to_string(), "Hello ".to_string())
    );
    assert!(
        !running.is_finished(),
        "the first deltas were not observed until the completed response"
    );
    running.await.unwrap();

    let remaining = drain(&rx);
    assert_eq!(reply_text(&remaining), "world.");
    assert!(
        remaining
            .iter()
            .any(|event| matches!(event, Control::CommitReply))
    );
}

/// A provider can technically stream while sending its entire answer in one
/// large delta. Coder splits only that live UI event into paced pieces so the
/// terminal still updates incrementally.
#[tokio::test]
async fn one_coarse_provider_delta_is_paced_for_the_tui() {
    let answer = "One coarse provider delta still paints as a live terminal stream.";
    let stub = start(move |request, _origin| {
        if request.contains("POST /api/v1/responses") {
            return Reply::ResponsesStream(
                vec![
                    (
                        "response.output_text.delta",
                        serde_json::json!({
                            "type": "response.output_text.delta",
                            "delta": answer
                        }),
                    ),
                    (
                        "response.completed",
                        serde_json::json!({
                            "type": "response.completed",
                            "response": {"usage": {}}
                        }),
                    ),
                ],
                None,
            );
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = dev_session(&stub.base, tx.clone());
    let running = tokio::spawn(async move {
        session.execute_turn("hello", tx).await;
    });

    let first = tokio::time::timeout(Duration::from_millis(100), async {
        loop {
            if let Ok(control) = rx.try_recv() {
                let event = match control {
                    Control::Turn { event, .. } => *event,
                    event => event,
                };
                if let Control::Chunk(chunk) = event {
                    break chunk;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the first paced piece did not reach the TUI");

    assert_eq!(first, "One coar");
    assert!(
        !running.is_finished(),
        "the coarse delta reached the TUI as one completed block"
    );
    running.await.unwrap();

    let mut streamed = first;
    streamed.push_str(&reply_text(&drain(&rx)));
    assert_eq!(streamed, answer);
}

/// The retry budget is one. A provider that stays silent cannot create an
/// unbounded request or credit loop.
#[tokio::test]
async fn a_silent_retry_fails_after_the_second_deadline() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.starts_with("POST /api/inference/proxy ") {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Reply::DelayedSse(Duration::from_millis(250), vec![text("too late")]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.set_first_response_policy(Duration::from_millis(5), Duration::from_millis(20));
    session.execute_turn("hello", tx).await;

    let seen = drain(&rx);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(reply_text(&seen), "");
    assert!(
        seen.iter().any(|control| matches!(
            control,
            Control::Failed(message) if message.contains("after 1 retry")
        )),
        "the exhausted retry was not reported: {seen:?}"
    );
    assert!(seen.iter().any(|control| matches!(control, Control::Done)));
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
                Reply::Sse(vec![call(
                    "shell",
                    serde_json::json!({"command": "exit 7"}),
                )])
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
                Reply::Sse(vec![call(
                    "shell",
                    serde_json::json!({"command": "rm -rf ~/"}),
                )])
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

#[tokio::test]
async fn cancellation_reports_and_settles_once_then_a_later_turn_starts_clean() {
    let proxy_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&proxy_calls);
    let report_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let reported = Arc::clone(&report_calls);
    let stub = start(move |request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, "application/json", grant(origin));
        }
        if request.contains("POST /api/v1/threads/th_test/report") {
            let spent = if reported.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                7
            } else {
                9
            };
            return Reply::Body(
                200,
                "application/json",
                format!(
                    r#"{{"thread":{{"id":"th_test","status":"cancelled"}},
                        "grant":{{"spent":{{"total_tokens":{spent}}}}}}}"#
                ),
            );
        }
        if request.contains("/api/inference/proxy") {
            if counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                return Reply::DelayedSse(Duration::from_secs(30), vec![text("late")]);
            }
            return Reply::Sse(vec![text("next answered"), usage(2)]);
        }
        Reply::Body(200, "application/json", "{}".to_string())
    });

    let (tx, rx) = channel();
    let session = Arc::new(tokio::sync::Mutex::new(session(&stub.base, tx.clone())));
    let id = TurnId::new(41);
    let router = session.lock().await.turn_router();
    let running = Arc::clone(&session);
    let turn_tx = tx.clone();
    let task = tokio::spawn(async move {
        running
            .lock()
            .await
            .execute_turn_with_id(id, "cancel this", turn_tx)
            .await;
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if stub
                .request_lines()
                .iter()
                .any(|line| line.starts_with("POST /api/inference/proxy"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the first transport did not start");

    router.lock().unwrap().cancel(id);
    task.abort();
    let _ = task.await;

    let mut session = session.lock().await;
    let first = session
        .settle_cancellation(id)
        .await
        .expect("the canceled thread did not settle");
    assert!(first.is_some(), "the server's one settlement was dropped");
    assert!(
        session
            .settle_cancellation(id)
            .await
            .expect("an idempotent repeat failed")
            .is_none(),
        "the same turn settled twice"
    );

    session.execute_turn("next", tx.clone()).await;
    session
        .finish()
        .await
        .expect("the later thread did not finish");
    drop(session);

    let requests = stub.requests();
    let reports = requests
        .iter()
        .filter(|request| request.contains("POST /api/v1/threads/th_test/report"))
        .collect::<Vec<_>>();
    assert_eq!(
        reports.len(),
        2,
        "one cancel report and one later success: {requests:?}"
    );
    let bodies = reports
        .iter()
        .map(|request| {
            serde_json::from_str::<serde_json::Value>(
                request.split("\r\n\r\n").nth(1).unwrap_or("{}"),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(bodies[0]["status"], "cancelled");
    assert_eq!(bodies[0]["error_code"], "interrupted");
    assert_eq!(bodies[1]["status"], "succeeded");
    assert_eq!(
        stub.request_lines()
            .iter()
            .filter(|line| *line == "POST /api/v1/threads HTTP/1.1")
            .count(),
        2,
        "the later turn did not open clean authority"
    );
    assert_eq!(
        drain(&rx)
            .iter()
            .filter(|event| matches!(event, Control::Billed(7)))
            .count(),
        1,
        "the canceled grant produced duplicate settlement events"
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
