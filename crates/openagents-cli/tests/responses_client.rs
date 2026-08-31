//! End-to-end checks that `openagents responses` renders a Coder stream.
//!
//! Each test serves one canned Server-Sent Events body from a stub origin,
//! captures the request the CLI sent, and asserts on the rendered output:
//! text deltas print in order, tool calls and usage print one line each, a
//! typed failure exits nonzero on stderr, reconnect flags pass through the
//! request body, and a bearer stored for the origin is sent without ever
//! being printed.

use std::process::{Command, Output};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

const COMPLETE_STREAM: &str = concat!(
    "data: {\"type\":\"response.resumed\",\"seq\":0,\"run\":\"3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10\",\"replay_from\":0}\n\n",
    "data: {\"type\":\"response.reasoning_summary_text.delta\",\"seq\":1,\"delta\":\"Read the failing test first.\"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"seq\":2,\"delta\":\"I will \"}\n\n",
    "data: {\"type\":\"response.output_text.delta\",\"seq\":3,\"delta\":\"fix the test.\"}\n\n",
    "data: {\"type\":\"response.some_future_event\",\"seq\":4}\n\n",
    "data: {\"type\":\"response.output_item.done\",\"seq\":5,\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\": \\\"src/lib.rs\\\"}\"}}\n\n",
    "data: {\"type\":\"response.output_item.done\",\"seq\":6,\"item\":{\"type\":\"function_call_output\",\"call_id\":\"call_1\",\"output\":\"fn main() {}\"}}\n\n",
    "data: {\"type\":\"response.completed\",\"seq\":7,\"response\":{\"id\":\"3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10\",\"usage\":{\"input_tokens\":1204,\"output_tokens\":356}}}\n\n",
);

const FAILED_STREAM: &str = concat!(
    "data: {\"type\":\"response.output_text.delta\",\"seq\":1,\"delta\":\"Starting.\"}\n\n",
    "data: {\"type\":\"response.failed\",\"seq\":2,\"response\":{\"id\":\"3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10\",\"error\":{\"code\":\"insufficient_credit\",\"message\":\"The spending grant is exhausted.\"}}}\n\n",
);

#[tokio::test]
async fn responses_renders_a_multi_event_stream_in_order() {
    let (origin, request) = serve_one(COMPLETE_STREAM).await;
    let home = tempfile::tempdir().unwrap();
    let output = run_binary(&["responses", "hello", "--origin", &origin], home.path()).await;
    let request = request.await.expect("the stub captured the request");

    assert!(
        request.starts_with("POST /v1/responses "),
        "expected a Coder Responses POST, got: {request}"
    );
    assert!(
        request.contains(r#""content":"hello""#),
        "expected the prompt in the request body: {request}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = "resumed: run=3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10 replay_from=0\n\
                    thinking: Read the failing test first.\n\
                    I will fix the test.\n\
                    tool: read_file {\"path\": \"src/lib.rs\"}\n\
                    tool output: fn main() {}\n\
                    completed: input_tokens=1204 output_tokens=356\n";
    assert_eq!(stdout, expected, "rendered stream mismatch");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn responses_failed_exits_nonzero_with_the_typed_message_on_stderr() {
    let (origin, _request) = serve_one(FAILED_STREAM).await;
    let home = tempfile::tempdir().unwrap();
    let output = run_binary(&["responses", "hello", "--origin", &origin], home.path()).await;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("The spending grant is exhausted."),
        "expected the typed error message on stderr, got: {stderr}"
    );
    assert!(
        stdout.contains("Starting."),
        "text before the failure still renders: {stdout}"
    );
    assert!(
        !stdout.contains("The spending grant is exhausted."),
        "the error belongs on stderr, not stdout: {stdout}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "a failed run must exit nonzero"
    );
}

#[tokio::test]
async fn responses_passes_reconnect_flags_through_the_request_body() {
    let (origin, request) = serve_one(COMPLETE_STREAM).await;
    let home = tempfile::tempdir().unwrap();
    let output = run_binary(
        &[
            "responses",
            "continue",
            "--origin",
            &origin,
            "--response",
            "3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10",
            "--starting-after",
            "41",
        ],
        home.path(),
    )
    .await;
    let request = request.await.expect("the stub captured the request");

    assert!(
        request.contains(r#""previous_response_id":"3f6d2a51-9c4e-4b1a-8f2e-0d5b7c1a9e10""#),
        "expected the response id in the request body: {request}"
    );
    assert!(
        request.contains(r#""starting_after":41"#),
        "expected the resume sequence in the request body: {request}"
    );
    assert_eq!(output.status.code(), Some(0));
}

#[tokio::test]
async fn responses_sends_the_stored_bearer_for_the_origin_and_never_prints_it() {
    let (origin, request) = serve_one(COMPLETE_STREAM).await;
    let home = tempfile::tempdir().unwrap();
    let credential_directory = home.path().join(".openagents");
    std::fs::create_dir_all(&credential_directory).unwrap();
    std::fs::write(
        credential_directory.join("credentials.json"),
        serde_json::json!({
            "version": 1,
            "tokens": { origin.as_str(): "oa_pat_coder_stream_secret" },
        })
        .to_string(),
    )
    .unwrap();

    let output = run_binary(&["responses", "hello", "--origin", &origin], home.path()).await;
    let request = request.await.expect("the stub captured the request");

    assert!(
        request.contains("authorization: Bearer oa_pat_coder_stream_secret"),
        "expected the stored bearer on the request: {request}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("oa_pat_coder_stream_secret"),
        "the bearer leaked to stdout: {stdout}"
    );
    assert!(
        !stderr.contains("oa_pat_coder_stream_secret"),
        "the bearer leaked to stderr: {stderr}"
    );
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[tokio::test]
async fn responses_sends_no_bearer_when_the_origin_has_no_stored_credential() {
    let (origin, request) = serve_one(COMPLETE_STREAM).await;
    let home = tempfile::tempdir().unwrap();
    let output = run_binary(&["responses", "hello", "--origin", &origin], home.path()).await;
    let request = request.await.expect("the stub captured the request");

    assert!(
        !request.to_lowercase().contains("authorization:"),
        "an origin with no stored credential sends no bearer: {request}"
    );
    assert_eq!(output.status.code(), Some(0));
}

/// Run the CLI against the stub with a private home directory and colour off,
/// so the rendered stream is byte-for-byte comparable.
async fn run_binary(args: &[&str], home: &std::path::Path) -> Output {
    let args: Vec<String> = args.iter().map(|value| value.to_string()).collect();
    let home = home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(&args)
            .env("NO_COLOR", "1")
            .env("HOME", &home)
            .env_remove("OPENAGENTS_TOKEN")
            .output()
            .expect("run openagents")
    })
    .await
    .unwrap()
}

/// Serve one request: capture what the CLI sent, answer with `body` as an
/// event stream. The captured request comes back through the returned
/// receiver so assertions run in the test, not in the spawned task.
async fn serve_one(body: &'static str) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept one connection");
        let request = read_request(&mut socket).await.expect("read the request");
        let _ = sender.send(request);
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
    });
    (format!("http://127.0.0.1:{port}"), receiver)
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
