//! A stand-in for the OpenAgents inference proxy, for tests that need a turn
//! to actually stream.
//!
//! It speaks the two routes `CoderRuntimeSession` calls — `POST /threads` for
//! the grant, and the proxy URL that grant points at for the completion — over
//! a real socket, with real server-sent events. Everything between the session
//! and the model is therefore the production code path; only the model is a
//! stand-in.

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A running stub. Its `base` goes to `CoderRuntimeSession::new`.
pub struct StubProxy {
    pub base: String,
}

/// Catalog the Mini tests and Explore/Plan defaults both need: GLM as the
/// default Flash pick, Gemini available so Explore can pin it.
const DEFAULT_MODELS: &str = r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true},{"id":"gemini-3.7-flash","availability":"available","default":false}]}"#;

fn grant_for_thread(request: &str, grant_url: &str) -> String {
    let fallback = "glm-5.3-flash";
    let model = request
        .split("\r\n\r\n")
        .nth(1)
        .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
        .and_then(|value| {
            value
                .get("model")
                .and_then(|model| model.as_str())
                .map(str::to_string)
        })
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    format!(
        r#"{{"thread":{{"id":"th_mini"}},"grant":{{"token":"tok_test","url":"{grant_url}","model":"{model}"}}}}"#
    )
}

/// Start a stub that streams `chunks` as one assistant message.
///
/// If `gate` is given, the stream pauses after its first chunk until that
/// receiver resolves, which is how a test observes a half-finished turn.
pub async fn start(
    chunks: Vec<&'static str>,
    gate: Option<tokio::sync::oneshot::Receiver<()>>,
) -> StubProxy {
    start_with(chunks, gate, None).await
}

/// The same stub, with a final `usage` chunk of (prompt, completion, total).
///
/// The real proxy sends usage on a chunk of its own with an empty `choices`
/// array, after the content and before `[DONE]`; this sends it the same way,
/// so what a test asserts about the status bar went through the same parse.
pub async fn start_reporting_usage(chunks: Vec<&'static str>, usage: (u64, u64, u64)) -> StubProxy {
    start_with(chunks, None, Some(usage)).await
}

/// Start a proxy that asks for one `read` call, then returns `answer`.
pub async fn start_calling_read(path: String, answer: &'static str) -> StubProxy {
    start_calling_read_with_catalog(path, answer, DEFAULT_MODELS).await
}

/// The same as [`start_calling_read`], with an explicit `GET /api/v1/models`
/// body so a test can mark a catalog id unavailable.
pub async fn start_calling_read_with_catalog(
    path: String,
    answer: &'static str,
    models: &'static str,
) -> StubProxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}/api/v1");
    let grant_url = Arc::new(format!("http://127.0.0.1:{port}/proxy"));
    let round = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let request = match read_request(&mut socket).await {
                Some(request) => request,
                None => continue,
            };
            let body = if request.starts_with("GET /api/v1/models") {
                Some(models.to_string())
            } else if request.starts_with("POST /api/v1/threads ") {
                Some(grant_for_thread(&request, &grant_url))
            } else {
                None
            };
            if let Some(body) = body {
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            if request.starts_with("POST /proxy") {
                let step = round.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let payload = if step == 0 {
                    serde_json::json!({"choices":[{"delta":{"tool_calls":[{
                        "index": 0,
                        "id": "call_read",
                        "function": {"name": "read", "arguments": serde_json::json!({"path": path}).to_string()}
                    }]}}]})
                } else {
                    serde_json::json!({"choices":[{"delta":{"content": answer}}]})
                };
                let frame = if step == 0 {
                    format!("data: {payload}\n\ndata: [DONE]\n\n")
                } else {
                    let usage = serde_json::json!({
                        "choices": [],
                        "usage": {
                            "prompt_tokens": 99,
                            "completion_tokens": 17,
                            "total_tokens": 116
                        }
                    });
                    format!("data: {payload}\n\ndata: {usage}\n\ndata: [DONE]\n\n")
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{frame}",
                    frame.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            let _ = socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}",
                )
                .await;
            let _ = socket.flush().await;
        }
    });

    StubProxy { base }
}

/// Start a proxy that asks for one `write` call, then returns `answer`.
pub async fn start_calling_write(path: String, content: String, answer: &'static str) -> StubProxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}/api/v1");
    let grant_url = Arc::new(format!("http://127.0.0.1:{port}/proxy"));
    let round = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let request = match read_request(&mut socket).await {
                Some(request) => request,
                None => continue,
            };
            let body = if request.starts_with("GET /api/v1/models") {
                Some(DEFAULT_MODELS.to_string())
            } else if request.starts_with("POST /api/v1/threads ") {
                Some(grant_for_thread(&request, &grant_url))
            } else {
                None
            };
            if let Some(body) = body {
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            if request.starts_with("POST /proxy") {
                let step = round.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let payload = if step == 0 {
                    serde_json::json!({"choices":[{"delta":{"tool_calls":[{
                        "index": 0,
                        "id": "call_write",
                        "function": {"name": "write", "arguments": serde_json::json!({"path": path, "content": content}).to_string()}
                    }]}}]})
                } else {
                    serde_json::json!({"choices":[{"delta":{"content": answer}}]})
                };
                let frame = format!("data: {payload}\n\ndata: [DONE]\n\n");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{frame}",
                    frame.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            let _ = socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}",
                )
                .await;
            let _ = socket.flush().await;
        }
    });

    StubProxy { base }
}

async fn start_with(
    chunks: Vec<&'static str>,
    gate: Option<tokio::sync::oneshot::Receiver<()>>,
    usage: Option<(u64, u64, u64)>,
) -> StubProxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}/api/v1");
    let grant_url = Arc::new(format!("http://127.0.0.1:{port}/proxy"));

    tokio::spawn(async move {
        let mut gate = gate;
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let request = match read_request(&mut socket).await {
                Some(request) => request,
                None => continue,
            };

            // The catalog. A switchable lane resolves its model against this
            // before it opens a thread, so a stub that does not serve it
            // refuses at the lane and never reaches the routes below.
            if request.starts_with("GET /api/v1/models") {
                let body = r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true},{"id":"thinkingmachines/inkling","availability":"available","default":false},{"id":"gemini-3.7-flash","availability":"available","default":false}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            if request.starts_with("POST /api/v1/threads") {
                let body = format!(
                    r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"tok_test","url":"{grant_url}","model":"glm-5.3-flash"}}}}"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                continue;
            }

            if request.starts_with("POST /proxy") {
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n")
                    .await;
                let _ = socket.flush().await;
                for (index, chunk) in chunks.iter().enumerate() {
                    let frame = format!(
                        "data: {}\n\n",
                        serde_json::json!({ "choices": [{ "delta": { "content": chunk } }] })
                    );
                    let _ = socket.write_all(frame.as_bytes()).await;
                    let _ = socket.flush().await;
                    if index == 0 {
                        if let Some(gate) = gate.take() {
                            let _ = gate.await;
                        }
                    }
                }
                if let Some((prompt, completion, total)) = usage {
                    let frame = format!(
                        "data: {}\n\n",
                        serde_json::json!({
                            "choices": [],
                            "usage": {
                                "prompt_tokens": prompt,
                                "completion_tokens": completion,
                                "total_tokens": total,
                            }
                        })
                    );
                    let _ = socket.write_all(frame.as_bytes()).await;
                    let _ = socket.flush().await;
                }
                let _ = socket.write_all(b"data: [DONE]\n\n").await;
                let _ = socket.flush().await;
                continue;
            }

            let _ = socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                )
                .await;
        }
    });

    StubProxy { base }
}

/// Start a stub that refuses everything, the way the real host does without a
/// token.
pub async fn start_refusing() -> StubProxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}/api/v1");

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let _ = read_request(&mut socket).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: 26\r\nconnection: close\r\n\r\n{\"error\":\"token required\"}",
                )
                .await;
            let _ = socket.flush().await;
        }
    });

    StubProxy { base }
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
