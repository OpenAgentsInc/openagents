//! `oa trace upload` against the ingest route.
//!
//! The command used to be absent from this CLI and to refuse with exit 16 in the
//! TypeScript one, on the grounds that `POST /api/v1/traces` did not exist. It does
//! exist. So the thing worth asserting is no longer "does it refuse" but what it
//! actually puts on the wire, and — more importantly — what it refuses to say when
//! the server's answer does not support saying it.
//!
//! These run against a stub server on localhost. Uploading a trace is a write, and a
//! test that proves upload works by writing to the real store is not one anyone can
//! run twice.

use openagents_cli::trace_client::{read_visibility, TraceClient, TRACE_VISIBILITIES};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug, Clone)]
struct SeenRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

struct StubApi {
    base: String,
    seen: Receiver<SeenRequest>,
}

/// Serve one request with `status` and `body`, and report what was asked.
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
        let mut authorization = None;
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
                if name.eq_ignore_ascii_case("authorization") {
                    authorization = Some(value.trim().to_string());
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
            authorization,
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

fn seen(stub: &StubApi) -> SeenRequest {
    stub.seen
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the client never sent a request")
}

fn atif_document() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "ATIF-v1.7",
        "session_id": "probe",
        "steps": [{ "step_id": 1, "source": "user", "message": "hello" }]
    })
}

fn stored_body(visibility: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "trace-1",
        // The route the server points at here does not exist. The client must
        // not carry it through to the caller as a place to look.
        "url": "https://openagents.com/api/v1/traces/trace-1",
        "digest": format!("sha256:{}", "a".repeat(64)),
        "byte_size": 412,
        "visibility": visibility,
        "inserted_at": "2026-08-26T03:00:00Z"
    })
}

#[test]
fn a_visibility_outside_the_servers_set_is_refused_with_the_set() {
    for name in TRACE_VISIBILITIES {
        assert_eq!(read_visibility(name).unwrap(), name);
    }
    // The names the old flags spoke. They were never the server's vocabulary.
    for wrong in ["public", "unlisted", "owner_only", ""] {
        let refusal = read_visibility(wrong)
            .expect_err(&format!("{wrong} is not a rung the server stores at"));
        let text = refusal.to_string();
        assert!(
            text.contains("dark, pulse, ledger, glass"),
            "the refusal must name the choices; got {text}"
        );
    }
}

#[tokio::test]
async fn upload_posts_the_document_itself_with_the_visibility_named() {
    let stub = start_stub_api(201, stored_body("dark"));
    let client = TraceClient::new(&stub.base, Some("oa_pat_test".to_string()));

    let stored = client
        .upload(&atif_document(), "dark", None)
        .await
        .expect("the stub answered 201");

    let request = seen(&stub);
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/v1/traces?visibility=dark");
    assert_eq!(
        request.authorization.as_deref(),
        Some("Bearer oa_pat_test"),
        "the ingest route is account-scoped"
    );
    // The body is the document, with nothing wrapped around it.
    assert_eq!(request.body, atif_document());

    assert_eq!(stored.id, "trace-1");
    assert!(
        stored.created,
        "201 means the server did not hold it before"
    );
    assert_eq!(stored.byte_size, 412);
    assert_eq!(stored.visibility, "dark");
}

#[tokio::test]
async fn an_existing_digest_is_reported_as_existing_not_as_an_upload() {
    let stub = start_stub_api(200, stored_body("dark"));
    let client = TraceClient::new(&stub.base, None);

    let stored = client.upload(&atif_document(), "dark", None).await.unwrap();

    let _ = seen(&stub);
    // The status is the only thing that tells the two apart. Discarding it is
    // how a caller comes to believe a write happened that did not.
    assert!(
        !stored.created,
        "a 200 means the server already held this digest"
    );
}

#[tokio::test]
async fn the_attempt_binding_and_a_higher_rung_reach_the_route() {
    let stub = start_stub_api(201, stored_body("ledger"));
    let client = TraceClient::new(&stub.base, None);

    client
        .upload(&atif_document(), "ledger", Some("asg-9"))
        .await
        .unwrap();

    let request = seen(&stub);
    assert!(
        request.path.contains("visibility=ledger"),
        "path was {}",
        request.path
    );
    assert!(
        request.path.contains("assignment_id=asg-9"),
        "path was {}",
        request.path
    );
}

#[tokio::test]
async fn nothing_the_client_returns_carries_the_dead_url_the_server_sends() {
    let stub = start_stub_api(201, stored_body("dark"));
    let client = TraceClient::new(&stub.base, None);

    let stored = client.upload(&atif_document(), "dark", None).await.unwrap();
    let _ = seen(&stub);

    // `GET /api/v1/traces/:id` is not a route. Reporting the url the server
    // builds would hand the reader a 404 dressed as a receipt.
    let rendered = serde_json::to_string(&stored).unwrap();
    assert!(
        !rendered.contains("openagents.com/api/v1/traces/"),
        "the client carried the server's unreachable url through: {rendered}"
    );
}

#[tokio::test]
async fn an_accepted_status_that_names_nothing_stored_is_an_error() {
    // 201 with an empty body: the server said yes and said nothing. Reporting a
    // stored trace here is how a caller comes to believe in one that has no id.
    let stub = start_stub_api(201, serde_json::json!({}));
    let client = TraceClient::new(&stub.base, None);

    let refused = client.upload(&atif_document(), "dark", None).await;
    let _ = seen(&stub);

    let error = refused.expect_err("an id-less 201 is not a stored trace");
    let text = error.to_string();
    assert!(
        text.contains("no id or digest"),
        "the error must say what was missing; got {text}"
    );
}

#[tokio::test]
async fn a_refusal_is_an_error_carrying_what_the_server_said() {
    let stub = start_stub_api(
        422,
        serde_json::json!({
            "message": "Validation Failed",
            "errors": { "document": ["The document is not a valid ATIF v1 object."] }
        }),
    );
    let client = TraceClient::new(&stub.base, None);

    let refused = client.upload(&atif_document(), "dark", None).await;
    let _ = seen(&stub);

    let text = refused
        .expect_err("a 422 is not a stored trace")
        .to_string();
    assert!(
        text.contains("not a valid ATIF v1 object"),
        "the refusal must carry the server's own words; got {text}"
    );
}
