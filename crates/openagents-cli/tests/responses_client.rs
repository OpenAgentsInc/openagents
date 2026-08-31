//! End-to-end check that `openagents responses` reaches a Coder origin.
//!
//! This is the first slice of T2: prove the thin CLI Responses client posts
//! to `/v1/responses`, carries the prompt as JSON, and prints the returned
//! Server-Sent Events body.

use std::process::Command;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn responses_posts_to_a_coder_origin_and_prints_the_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(handle_one_request(listener));

    let home = tempfile::tempdir().unwrap();
    let origin = format!("http://127.0.0.1:{port}");
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(["responses", "hello", "--origin", &origin])
            .env("NO_COLOR", "")
            .env("HOME", home.path())
            .output()
            .expect("run openagents")
    })
    .await
    .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("response.failed"),
        "expected the raw SSE body in stdout, got: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn handle_one_request(listener: TcpListener) {
    let (mut socket, _) = listener.accept().await.expect("accept one connection");
    let request = read_request(&mut socket).await.expect("read the request");
    assert!(
        request.starts_with("POST /v1/responses "),
        "expected a Coder Responses POST, got: {request}"
    );
    assert!(
        request.contains(r#""content":"hello""#),
        "expected the prompt in the request body: {request}"
    );

    let body = "data: {\"type\":\"response.failed\",\"seq\":0,\"response\":{\"id\":\"\",\"error\":{\"code\":\"not_implemented\",\"message\":\"...\"}}}\n\n";
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         content-type: text/event-stream\r\n\
         content-length: {}\r\n\
         connection: close\r\n\r\n\
         {}",
        body.len(),
        body
    );
    socket.write_all(response.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();
}

async fn read_request(socket: &mut TcpStream) -> Option<String> {
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
