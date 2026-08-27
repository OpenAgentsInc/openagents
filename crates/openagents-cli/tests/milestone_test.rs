//! The milestone write half of the tracker client, asserted on the request it sends.
//!
//! `create_milestone` and `delete_milestone` existed on `TrackerClient` and were
//! wired to no subcommand, and there was no way at all to put an existing issue on
//! a milestone: `issue create --milestone` was the only write. So milestones could
//! only be managed in a browser, which makes them useless to agents — and agents
//! file most of the issues here.
//!
//! These run against a stub server on localhost rather than the live tracker,
//! because they are WRITE paths: a test that proves `milestone delete` works by
//! deleting a real milestone is not a test anyone can run twice. The assertions are
//! on the method, path, and body the client actually put on the wire, which is what
//! was wrong — a route that was never called, not a response that was misread.

use openagents_cli::tracker::{RepoTarget, TrackerClient};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};

/// One request the stub server saw, reduced to what these tests assert on.
#[derive(Debug, Clone)]
struct SeenRequest {
    method: String,
    path: String,
    body: serde_json::Value,
}

struct StubApi {
    base: String,
    seen: Receiver<SeenRequest>,
}

/// Serve exactly one request with `status` and `body`, and report what was asked.
///
/// Deliberately minimal: it reads the request line, the headers, and exactly
/// `Content-Length` bytes of body. Anything it cannot parse it reports as a null
/// body rather than guessing, so a client that sent nothing is distinguishable
/// from one that sent something unreadable.
fn start_stub_api(status: u16, body: serde_json::Value) -> StubApi {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, seen) = channel();

    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream);

        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();

        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => return,
            }
            let trimmed = header.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
        }

        let mut raw = vec![0u8; content_length];
        if content_length > 0 && reader.read_exact(&mut raw).is_err() {
            return;
        }
        let parsed = if raw.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&raw).unwrap_or(serde_json::Value::Null)
        };
        let _ = sender.send(SeenRequest {
            method,
            path,
            body: parsed,
        });

        let payload = if body.is_null() {
            String::new()
        } else {
            body.to_string()
        };
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let stream = reader.get_mut();
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    StubApi {
        base: format!("http://127.0.0.1:{port}/api/v1"),
        seen,
    }
}

fn target() -> RepoTarget {
    RepoTarget::parse("octavia/project").unwrap()
}

fn seen(stub: &StubApi) -> SeenRequest {
    stub.seen
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the client never sent a request")
}

#[tokio::test]
async fn create_milestone_posts_to_the_repository_route() {
    let stub = start_stub_api(
        201,
        serde_json::json!({ "number": 7, "title": "Ship it", "state": "open" }),
    );
    let client = TrackerClient::new(&stub.base, None);

    let value = client
        .create_milestone(&target(), "Ship it", Some("the tail of #76"), None)
        .await
        .expect("the stub answered 201");

    let request = seen(&stub);
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/v1/repos/octavia/project/milestones");
    assert_eq!(
        request.body,
        serde_json::json!({ "title": "Ship it", "description": "the tail of #76" }),
        "an unset due date must be omitted, not sent as null"
    );
    // The number is the server's. It is what `issue milestone --set` has to be
    // given, so a client that invented one would be sending callers to a
    // milestone that does not exist.
    assert_eq!(value.get("number").and_then(|n| n.as_u64()), Some(7));
}

#[tokio::test]
async fn delete_milestone_uses_the_numbered_route_and_accepts_an_empty_204() {
    let stub = start_stub_api(204, serde_json::Value::Null);
    let client = TrackerClient::new(&stub.base, None);

    client
        .delete_milestone(&target(), 7)
        .await
        .expect("a 204 with no body is success, not a parse failure");

    let request = seen(&stub);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.path, "/api/v1/repos/octavia/project/milestones/7");
}

#[tokio::test]
async fn setting_a_milestone_patches_that_field_and_no_other() {
    let stub = start_stub_api(
        200,
        serde_json::json!({ "number": 129, "milestone": { "number": 7, "title": "Ship it" } }),
    );
    let client = TrackerClient::new(&stub.base, None);

    client
        .set_issue_milestone(&target(), 129, Some(7))
        .await
        .unwrap();

    let request = seen(&stub);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.path, "/api/v1/repos/octavia/project/issues/129");
    // A PATCH carrying `body` would replace the issue text.
    assert_eq!(request.body, serde_json::json!({ "milestone": 7 }));
}

#[tokio::test]
async fn clearing_a_milestone_sends_an_explicit_null_rather_than_omitting_the_key() {
    let stub = start_stub_api(200, serde_json::json!({ "number": 129, "milestone": null }));
    let client = TrackerClient::new(&stub.base, None);

    client
        .set_issue_milestone(&target(), 129, None)
        .await
        .unwrap();

    let request = seen(&stub);
    // Omitting the key would leave the milestone where it is and still answer
    // 200 — a clear that does nothing and reports success.
    assert!(
        request.body.get("milestone").is_some(),
        "the key must be present; the body was {}",
        request.body
    );
    assert!(
        request.body["milestone"].is_null(),
        "the key must carry null; the body was {}",
        request.body
    );
}

#[tokio::test]
async fn a_refused_milestone_number_is_an_error_not_a_quiet_success() {
    let stub = start_stub_api(
        422,
        serde_json::json!({
            "message": "Validation Failed",
            "code": "validation_failed",
            "errors": { "milestone": ["Milestone #99999 does not exist in this repository"] }
        }),
    );
    let client = TrackerClient::new(&stub.base, None);

    let refused = client
        .set_issue_milestone(&target(), 129, Some(99_999))
        .await;

    let error = refused.expect_err("a 422 must not be reported as a stored milestone");
    let text = error.to_string();
    assert!(
        text.contains("99999") || text.contains("Validation Failed"),
        "the refusal must name what the server rejected; got {text}"
    );
}
