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

/// Start a stub that streams `chunks` as one assistant message.
///
/// If `gate` is given, the stream pauses after its first chunk until that
/// receiver resolves, which is how a test observes a half-finished turn.
pub async fn start(
    chunks: Vec<&'static str>,
    gate: Option<tokio::sync::oneshot::Receiver<()>>,
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

            if request.starts_with("POST /api/v1/threads") {
                let body = format!(
                    r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"tok_test","url":"{grant_url}","model":"ox-alpha"}}}}"#
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
