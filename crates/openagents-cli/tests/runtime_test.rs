//! What the coder runtime does on the wire, proved against real sockets.
//!
//! Every test here runs the production `CoderRuntimeSession` against a server
//! this file starts on a real port. Nothing is mocked between the session and
//! the socket, so a passing test means the bytes were right.
//!
//! Two things these tests are careful about, because the versions they replace
//! were not:
//!
//! - **Streaming is proved with a clock.** Asserting that the reply eventually
//!   contains the text is satisfied by a response assembled in one block at the
//!   end. So the server holds the rest of the stream open, and the assertion is
//!   that a chunk reached the caller measurably *before* the turn returned.
//! - **A failure is proved to be a failure.** `assert!(result.is_ok())` passed
//!   precisely while this runtime answered every refusal with a made-up grant
//!   and a sentence about an offline fallback. These assert on `Err` and on
//!   what it says.

use openagents_cli::runtime::{CoderRuntimeSession, Lane, TurnUsage};
use openagents_cli::tools::HarnessToolRegistry;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ─────────────────────────────────────────────────────────────── the server

/// What one connection should be answered with.
enum Reply {
    /// Status line, content type, and body.
    Body(u16, &'static str, String),
    /// An event stream: each frame, with an optional pause before one of them.
    Sse(Vec<String>, Option<(usize, Duration)>),
    /// Newline-delimited JSON, the shape Ollama streams.
    Ndjson(Vec<String>, Option<(usize, Duration)>),
}

/// A server that records what it was asked and answers from a script.
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
        self.requests()
            .iter()
            .map(|r| r.lines().next().unwrap_or_default().to_string())
            .collect()
    }
}

/// Start a stub whose handler picks a reply from the request text.
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
                    Reply::Sse(frames, pause) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        for (index, frame) in frames.iter().enumerate() {
                            if let Some((at, delay)) = pause {
                                if index == at {
                                    tokio::time::sleep(delay).await;
                                }
                            }
                            let _ = socket
                                .write_all(format!("data: {frame}\n\n").as_bytes())
                                .await;
                            let _ = socket.flush().await;
                        }
                        let _ = socket.write_all(b"data: [DONE]\n\n").await;
                        let _ = socket.flush().await;
                    }
                    Reply::Ndjson(lines, pause) => {
                        let _ = socket
                            .write_all(
                                b"HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\n\
                                  connection: close\r\n\r\n",
                            )
                            .await;
                        let _ = socket.flush().await;
                        for (index, line) in lines.iter().enumerate() {
                            if let Some((at, delay)) = pause {
                                if index == at {
                                    tokio::time::sleep(delay).await;
                                }
                            }
                            let _ = socket.write_all(format!("{line}\n").as_bytes()).await;
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

fn grant_body(origin: &str, model: &str) -> String {
    format!(
        r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"sig_test","url":"{origin}/api/inference/proxy","model":"{model}"}}}}"#
    )
}

fn frame(json: serde_json::Value) -> String {
    json.to_string()
}

fn tools() -> HarnessToolRegistry {
    HarnessToolRegistry::new(Some(std::env::temp_dir()))
}

fn session(lane: Lane, base: String) -> CoderRuntimeSession {
    CoderRuntimeSession::new(lane, Some(base), Some("oat_test".to_string()), tools())
}

/// A port nothing listens on, so a connection to it is refused at once.
const DEAD: &str = "http://127.0.0.1:1/api/v1";

// ──────────────────────────────────────────────────────── the streaming clock

/// The reply arrives while the turn is still open, not assembled at the end.
///
/// The server sends `PO`, waits 700ms, then sends `NG`. The assertion is on
/// the gap between the first chunk and the return: a batched response would
/// deliver both at once and close that gap to nothing.
#[tokio::test]
async fn a_chunk_reaches_the_caller_before_the_turn_returns() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        Reply::Sse(
            vec![
                frame(serde_json::json!({"choices":[{"delta":{"content":"PO"}}]})),
                frame(serde_json::json!({"choices":[{"delta":{"content":"NG"}}]})),
            ],
            Some((1, Duration::from_millis(700))),
        )
    });

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, Instant)>();
    let mut session = session(Lane::OxAlpha, stub.base.clone());

    let started = Instant::now();
    let answer = session
        .execute_turn("say pong", move |chunk| {
            let _ = tx.send((chunk.to_string(), Instant::now()));
        })
        .await
        .expect("the turn failed");
    let returned = Instant::now();

    let mut chunks = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        chunks.push(chunk);
    }

    assert_eq!(
        chunks
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>(),
        vec!["PO", "NG"],
        "the reply did not arrive in pieces"
    );
    assert_eq!(answer, "PONG");

    let first = chunks[0].1;
    let lead = returned.duration_since(first);
    assert!(
        lead >= Duration::from_millis(500),
        "the first chunk landed only {lead:?} before the turn returned, so this run \
         does not distinguish streaming from a batched reply (turn took {:?})",
        returned.duration_since(started)
    );
}

// ─────────────────────────────────────────────────────────────── the metering

/// A turn reports what it spent, taken from the server's own usage chunk.
#[tokio::test]
async fn a_turn_reports_the_tokens_the_server_counted() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        Reply::Sse(
            vec![
                frame(serde_json::json!({"choices":[{"delta":{"reasoning":"thinking"}}]})),
                frame(serde_json::json!({"choices":[{"delta":{"content":"PONG"}}]})),
                frame(serde_json::json!({
                    "choices": [],
                    "usage": {"prompt_tokens": 99, "completion_tokens": 17, "total_tokens": 116}
                })),
            ],
            None,
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let answer = session.execute_turn("say pong", |_| {}).await.unwrap();

    assert_eq!(answer, "PONG");
    assert_eq!(
        session.last_usage,
        TurnUsage {
            prompt_tokens: 99,
            completion_tokens: 17,
            total_tokens: 116
        }
    );
    assert_eq!(
        session.last_usage.line(),
        "99 prompt + 17 completion = 116 tokens"
    );
    // The reasoning was parsed and kept off the answer.
    assert_eq!(session.last_reasoning, "thinking");
}

// ────────────────────────────────────────────────────────────── the lifecycle

/// The thread the session opened is revoked, and the request is the server's.
#[tokio::test]
async fn the_session_revokes_its_thread_when_it_closes() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        if request.starts_with("DELETE /api/v1/threads/") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"grant":{"status":"revoked","spent":{"calls":1,"total_tokens":116}},
                    "thread":{"id":"th_test","status":"cancelled"}}"#
                    .to_string(),
            );
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"ok"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    session.execute_turn("hello", |_| {}).await.unwrap();
    let spent = session.close().await.expect("the revocation failed");

    let lines = stub.request_lines();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("DELETE /api/v1/threads/th_test")),
        "no revocation was sent; the stub saw {lines:?}"
    );
    assert_eq!(spent.map(|usage| usage.total_tokens), Some(116));

    // The revocation carried the account's own credential, not the grant's.
    let delete = stub
        .requests()
        .into_iter()
        .find(|r| r.starts_with("DELETE"))
        .unwrap();
    assert!(delete.contains("Bearer oat_test"), "{delete}");

    // And it does not fire twice.
    assert!(session.close().await.unwrap().is_none());
    assert_eq!(
        stub.request_lines()
            .iter()
            .filter(|line| line.starts_with("DELETE"))
            .count(),
        1
    );
}

/// One thread serves the whole session rather than one per turn.
#[tokio::test]
async fn a_second_turn_reuses_the_first_turns_thread() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"ok"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    session.execute_turn("one", |_| {}).await.unwrap();
    session.execute_turn("two", |_| {}).await.unwrap();

    let opens = stub
        .request_lines()
        .iter()
        .filter(|line| line.starts_with("POST /api/v1/threads"))
        .count();
    assert_eq!(opens, 1, "each turn opened its own thread");
}

// ────────────────────────────────────────────────────────────── the lane gate

/// A lane nothing admits is refused by name, with what the deployment serves.
///
/// `Lane::from_str` used to answer any unrecognised name with `Lane::OxAlpha`,
/// so `--lane bogus` ran the default and said nothing about it.
#[tokio::test]
async fn an_unadmitted_lane_is_refused_with_the_ones_that_work() {
    let stub = start(|request, _origin| {
        if request.starts_with("GET /api/v1/models") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"default":"gemini-3.7-flash","models":[
                    {"id":"gemini-3.7-flash","availability":"available","default":true},
                    {"id":"ox-alpha","availability":"available","default":false}]}"#
                    .to_string(),
            );
        }
        Reply::Body(500, "application/json", "{}".to_string())
    });

    let mut session = session(Lane::from_str("bogus"), stub.base.clone());
    let error = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("an unadmitted lane ran a turn");
    let message = error.to_string();

    assert!(message.contains("bogus"), "{message}");
    assert!(message.contains("ox-alpha"), "{message}");
    assert!(message.contains("gemini-3.7-flash"), "{message}");
    for tier in ["auto", "flash", "pro", "ollama:<model>"] {
        assert!(
            message.contains(tier),
            "the refusal does not name {tier}: {message}"
        );
    }
    // And no thread was opened for a lane that cannot run.
    assert!(
        !stub
            .request_lines()
            .iter()
            .any(|line| line.starts_with("POST /api/v1/threads")),
        "a refused lane still opened a thread"
    );
}

/// A model in the catalog whose provider is not configured is refused too.
#[tokio::test]
async fn a_served_but_unconfigured_model_is_refused() {
    let stub = start(|request, _origin| {
        if request.starts_with("GET /api/v1/models") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"models":[
                    {"id":"quiet-one","availability":"unavailable","default":false},
                    {"id":"ox-alpha","availability":"available","default":true}]}"#
                    .to_string(),
            );
        }
        Reply::Body(500, "application/json", "{}".to_string())
    });

    let mut session = session(Lane::from_str("quiet-one"), stub.base.clone());
    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("an unavailable model ran a turn")
        .to_string();
    assert!(message.contains("provider is not configured"), "{message}");
    assert!(message.contains("ox-alpha"), "{message}");
}

/// A tier opens on the catalog id it names, and the grant's model is reported.
#[tokio::test]
async fn a_tier_opens_its_thread_on_the_model_it_names() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(
                200,
                "application/json",
                grant_body(origin, "gemini-3.7-flash"),
            );
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"hi"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::from_str("flash"), stub.base.clone());
    session.execute_turn("hello", |_| {}).await.unwrap();

    let open = stub
        .requests()
        .into_iter()
        .find(|r| r.starts_with("POST /api/v1/threads"))
        .unwrap();
    assert!(open.contains(r#""model":"gemini-3.7-flash""#), "{open}");
    // The lane the server admits, not a model name in the lane field.
    assert!(open.contains(r#""lane":"thread""#), "{open}");
    assert_eq!(session.last_model.as_deref(), Some("gemini-3.7-flash"));
    assert_eq!(
        session.last_grant.as_ref().map(|g| g.model.as_str()),
        Some("gemini-3.7-flash")
    );
}

/// `auto` names no model, so the deployment's own default answers.
#[tokio::test]
async fn the_auto_lane_names_no_model_at_all() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(
                200,
                "application/json",
                grant_body(origin, "gemini-3.7-flash"),
            );
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"hi"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::from_str("auto"), stub.base.clone());
    session.execute_turn("hello", |_| {}).await.unwrap();

    let open = stub
        .requests()
        .into_iter()
        .find(|r| r.starts_with("POST /api/v1/threads"))
        .unwrap();
    let body = open.split("\r\n\r\n").nth(1).unwrap_or_default();
    assert!(!body.contains("\"model\""), "auto pinned a model: {body}");
    // What answered is still reported, because the grant said so.
    assert_eq!(session.last_model.as_deref(), Some("gemini-3.7-flash"));
}

// ───────────────────────────────────────────────────────────── no fabrication

/// A refused thread is an error, not a grant this process invented.
#[tokio::test]
async fn a_refused_thread_ends_the_turn() {
    let stub = start(|_request, _origin| {
        Reply::Body(
            422,
            "application/json",
            r#"{"errors":{"model":["\"bogus\" is not an admitted model."]},"status":422}"#
                .to_string(),
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("a 422 thread request produced a completed turn")
        .to_string();

    assert!(message.contains("422"), "{message}");
    assert!(message.contains("not an admitted model"), "{message}");
    assert!(!message.contains("offline fallback"), "{message}");
    assert!(session.last_grant.is_none(), "a grant was invented anyway");
}

/// A thread that opens without a grant is an error too.
///
/// The local lane opens exactly such a thread — it mints no grant, because
/// nothing carries its model to a provider. Reaching the proxy with a token
/// taken from that reply is what the old code did with the caller's PAT.
#[tokio::test]
async fn a_thread_with_no_grant_ends_the_turn() {
    let stub = start(|_request, _origin| {
        Reply::Body(
            200,
            "application/json",
            r#"{"thread":{"id":"th_test","status":"open"}}"#.to_string(),
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("a thread with no grant produced a completed turn")
        .to_string();
    assert!(message.contains("minted no inference grant"), "{message}");
}

/// A refused proxy call is an error, with the server's own words.
#[tokio::test]
async fn a_refused_proxy_call_ends_the_turn() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        Reply::Body(
            402,
            "application/json",
            r#"{"error":"the grant's budget is spent"}"#.to_string(),
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("a 402 from the proxy produced a completed turn")
        .to_string();
    assert!(message.contains("402"), "{message}");
    assert!(message.contains("budget is spent"), "{message}");
}

/// An unreachable proxy is an error, not an empty success.
#[tokio::test]
async fn an_unreachable_host_ends_the_turn() {
    let mut session = session(Lane::OxAlpha, DEAD.to_string());
    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("an unreachable host produced a completed turn")
        .to_string();
    assert!(!message.is_empty());
    assert!(session.last_grant.is_none());
}

// ─────────────────────────────────────────────────────────── the tool loop

/// The model's tool call runs, and its result goes back on the next request.
#[tokio::test]
async fn a_tool_call_runs_and_its_output_returns_to_the_model() {
    let round = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&round);
    let stub = start(move |request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        let step = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if step == 0 {
            let call = serde_json::json!({"choices": [{"delta": {"tool_calls": [{
                "index": 0,
                "id": "call_a",
                "function": {"name": "shell", "arguments": "{\"command\":\"echo marker-9f3\"}"}
            }]}}]});
            return Reply::Sse(vec![frame(call)], None);
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"the marker is marker-9f3"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let answer = session.execute_turn("run it", |_| {}).await.unwrap();
    assert_eq!(answer, "the marker is marker-9f3");

    let proxy_calls: Vec<String> = stub
        .requests()
        .into_iter()
        .filter(|r| r.starts_with("POST /api/inference/proxy"))
        .collect();
    assert_eq!(proxy_calls.len(), 2, "the loop did not take a second step");
    assert!(
        proxy_calls[1].contains("marker-9f3") && proxy_calls[1].contains(r#""role":"tool""#),
        "the tool's output did not go back as a tool message:\n{}",
        proxy_calls[1]
    );
    assert!(
        proxy_calls[1].contains("call_a"),
        "the tool result did not name the call it answers"
    );
}

// ───────────────────────────────────────────────────────────── the local lane

fn ollama_stub() -> Stub {
    start(|request, _origin| {
        if request.starts_with("GET /api/tags") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"models":[
                    {"name":"older:1b","modified_at":"2026-01-01T00:00:00Z"},
                    {"name":"qwen3:0.6b","modified_at":"2026-08-25T15:08:15Z"}]}"#
                    .to_string(),
            );
        }
        Reply::Ndjson(
            vec![
                serde_json::json!({"message":{"role":"assistant","thinking":"brief"},"done":false})
                    .to_string(),
                serde_json::json!({"message":{"role":"assistant","content":"PO"},"done":false})
                    .to_string(),
                serde_json::json!({"message":{"role":"assistant","content":"NG"},"done":false})
                    .to_string(),
                serde_json::json!({
                    "message": {"role":"assistant","content":""},
                    "done": true,
                    "prompt_eval_count": 19,
                    "eval_count": 28
                })
                .to_string(),
            ],
            Some((2, Duration::from_millis(700))),
        )
    })
}

/// The local lane answers with the OpenAgents host unreachable, and streams.
///
/// `api_base` points at a closed port for the whole turn: a single request to
/// openagents.com would fail the turn, so a pass is proof that none was made.
#[tokio::test]
async fn the_local_lane_answers_with_the_proxy_unreachable() {
    let ollama = ollama_stub();
    let mut session = session(Lane::from_str("ollama:qwen3:0.6b"), DEAD.to_string());
    session.ollama_host = ollama.base.trim_end_matches("/api/v1").to_string();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, Instant)>();
    let answer = session
        .execute_turn("say pong", move |chunk| {
            let _ = tx.send((chunk.to_string(), Instant::now()));
        })
        .await
        .expect("the local turn failed");
    let returned = Instant::now();

    assert_eq!(answer, "PONG");
    assert_eq!(session.last_model.as_deref(), Some("ollama:qwen3:0.6b"));
    // No grant, and none invented: the model is on this machine.
    assert!(session.last_grant.is_none());
    assert_eq!(session.last_reasoning, "brief");
    assert_eq!(
        session.last_usage,
        TurnUsage {
            prompt_tokens: 19,
            completion_tokens: 28,
            total_tokens: 47
        }
    );

    let mut chunks = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        chunks.push(chunk);
    }
    assert_eq!(
        chunks.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        vec!["PO", "NG"]
    );
    let lead = returned.duration_since(chunks[0].1);
    assert!(
        lead >= Duration::from_millis(500),
        "the first chunk landed only {lead:?} before the turn returned, so this run \
         does not distinguish streaming from a batched reply"
    );

    // The whole exchange was with the local server.
    let lines = ollama.request_lines();
    assert!(
        lines.iter().any(|l| l.starts_with("GET /api/tags")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("POST /api/chat")),
        "{lines:?}"
    );
}

/// A short name resolves to the installed model whose family it names.
#[tokio::test]
async fn a_family_name_resolves_to_the_installed_model() {
    let ollama = ollama_stub();
    let mut session = session(Lane::from_str("ollama:qwen3"), DEAD.to_string());
    session.ollama_host = ollama.base.trim_end_matches("/api/v1").to_string();

    session.execute_turn("hello", |_| {}).await.unwrap();
    assert_eq!(session.last_model.as_deref(), Some("ollama:qwen3:0.6b"));

    let chat = ollama
        .requests()
        .into_iter()
        .find(|r| r.starts_with("POST /api/chat"))
        .unwrap();
    assert!(chat.contains(r#""model":"qwen3:0.6b""#), "{chat}");
}

/// With no model named, the most recently pulled one answers.
#[tokio::test]
async fn the_bare_local_lane_takes_the_most_recent_model() {
    let ollama = ollama_stub();
    let mut session = session(Lane::from_str("local"), DEAD.to_string());
    session.ollama_host = ollama.base.trim_end_matches("/api/v1").to_string();

    session.execute_turn("hello", |_| {}).await.unwrap();
    assert_eq!(session.last_model.as_deref(), Some("ollama:qwen3:0.6b"));
}

/// A name no installed model matches is refused, with what is installed.
#[tokio::test]
async fn a_missing_local_model_is_refused_by_name() {
    let ollama = ollama_stub();
    let mut session = session(Lane::from_str("ollama:llama9"), DEAD.to_string());
    session.ollama_host = ollama.base.trim_end_matches("/api/v1").to_string();

    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("a missing local model answered anyway")
        .to_string();
    assert!(message.contains("llama9"), "{message}");
    assert!(message.contains("qwen3:0.6b"), "{message}");
}

/// No Ollama server is a failed turn, not a fallback to the hosted lane.
#[tokio::test]
async fn no_local_server_ends_the_turn() {
    let mut session = session(Lane::from_str("local"), DEAD.to_string());
    session.ollama_host = "http://127.0.0.1:1".to_string();

    let message = session
        .execute_turn("hello", |_| {})
        .await
        .expect_err("a missing Ollama server produced a completed turn")
        .to_string();
    assert!(message.contains("no Ollama server answered"), "{message}");
    assert!(message.contains("choose a hosted lane"), "{message}");
}

/// A local tool call runs and returns to the local model.
#[tokio::test]
async fn the_local_lane_runs_tools_and_feeds_the_result_back() {
    let round = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&round);
    let ollama = start(move |request, _origin| {
        if request.starts_with("GET /api/tags") {
            return Reply::Body(
                200,
                "application/json",
                r#"{"models":[{"name":"qwen3:0.6b","modified_at":"2026-08-25T15:08:15Z"}]}"#
                    .to_string(),
            );
        }
        let step = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if step == 0 {
            return Reply::Ndjson(
                vec![serde_json::json!({
                    "message": {"role":"assistant","content":"","tool_calls":[{
                        "function": {"name":"shell","arguments":{"command":"echo marker-4c1"}}
                    }]},
                    "done": false
                })
                .to_string()],
                None,
            );
        }
        Reply::Ndjson(
            vec![serde_json::json!({
                "message": {"role":"assistant","content":"marker-4c1"},
                "done": true,
                "prompt_eval_count": 5,
                "eval_count": 3
            })
            .to_string()],
            None,
        )
    });

    let mut session = session(Lane::from_str("local"), DEAD.to_string());
    session.ollama_host = ollama.base.trim_end_matches("/api/v1").to_string();
    let answer = session.execute_turn("run it", |_| {}).await.unwrap();
    assert_eq!(answer, "marker-4c1");

    let chats: Vec<String> = ollama
        .requests()
        .into_iter()
        .filter(|r| r.starts_with("POST /api/chat"))
        .collect();
    assert_eq!(chats.len(), 2, "the local loop did not take a second step");
    assert!(
        chats[1].contains("marker-4c1") && chats[1].contains(r#""role":"tool""#),
        "the tool output did not go back to the local model:\n{}",
        chats[1]
    );
    assert!(
        chats[1].contains(r#""tool_name":"shell""#),
        "the tool result did not name its tool for Ollama:\n{}",
        chats[1]
    );
}

/// The model must be able to see what it said itself.
///
/// `run_tools` records an assistant turn only when that turn called a tool, so
/// a turn that simply answered was never added to `self.messages`. The session
/// is reused across turns, so asked in turn two what it said in turn one, the
/// model had nothing to read and confabulated a fresh answer instead:
///
///     turn 1: "invent a six-letter codeword" -> QORVEN
///     turn 2: "what was the codeword?"       -> ZORBEX
///
/// This asserts the recording rather than the model's behaviour: the second
/// turn's request body must carry the first turn's answer. A test that asked
/// the model to recall something from the *user's* prompt would pass against
/// the defect, because user messages were always recorded.
#[tokio::test]
async fn the_second_turn_carries_what_the_first_turn_answered() {
    let stub = start(|request, origin| {
        if request.starts_with("POST /api/v1/threads") {
            return Reply::Body(200, "application/json", grant_body(origin, "ox-alpha"));
        }
        Reply::Sse(
            vec![frame(
                serde_json::json!({"choices":[{"delta":{"content":"QORVEN"}}]}),
            )],
            None,
        )
    });

    let mut session = session(Lane::OxAlpha, stub.base.clone());
    let first = session
        .execute_turn("invent a codeword", |_| {})
        .await
        .unwrap();
    assert_eq!(first, "QORVEN");
    session
        .execute_turn("what was the codeword?", |_| {})
        .await
        .unwrap();

    // The last proxy request is the second turn's. It must contain the answer
    // the first turn produced.
    let bodies: Vec<String> = stub
        .requests()
        .into_iter()
        .filter(|r| !r.starts_with("POST /api/v1/threads"))
        .collect();
    assert_eq!(bodies.len(), 2, "expected one proxy call per turn");
    assert!(
        bodies[1].contains("QORVEN"),
        "the second turn did not carry the first turn's answer, so the model \
         cannot see what it said: {}",
        bodies[1]
    );
}
