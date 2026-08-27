//! What `/logout` does, proved against a real socket and a real credential
//! store.
//!
//! A `/logout` that prints a line and does nothing passes any test that only
//! checks the command is listed, so nothing here looks at the command list.
//! These assert the two effects that are the whole point:
//!
//! - **The stored token is gone.** Against a real
//!   [`openagents_cli::auth::CredentialStore`], confined to a directory so the
//!   owner's credential file is never touched, read back through the same
//!   `find_token` every other command reads it through.
//! - **The thread was ended by reporting, not abandoned.** The stub records
//!   every request, and the assertion is that
//!   `POST /api/v1/threads/{id}/report` arrived — a session whose credential
//!   vanishes must not leave an open thread holding its grant's remaining
//!   budget (issues #106, #107).
//!
//! And one adversarial case: a server that refuses the report. The credential
//! still goes, the thread is still disposed of rather than leaked, and the
//! notice says a report did not happen instead of claiming one did.

use openagents_cli::auth::{CredentialStore, Secret};
use openagents_cli::coder::runtime::{Control, Session};
use openagents_cli::runtime::Lane;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ───────────────────────────────────────────────────────────────── the server

/// What the stub answers with: a body, or a stream of SSE frames.
enum Reply {
    Body(u16, String),
    Sse(Vec<String>),
}

struct Stub {
    base: String,
    origin: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Stub {
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
    let served = origin.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let seen = Arc::clone(&seen);
            let handler = Arc::clone(&handler);
            let origin = served.clone();
            tokio::spawn(async move {
                let Some(request) = read_request(&mut socket).await else {
                    return;
                };
                seen.lock().unwrap().push(request.clone());
                // The catalog, answered before the handler sees it. A
                // switchable lane resolves its model against this at open, so
                // a stub that did not serve it would refuse at the lane and
                // never open the thread these tests are about.
                let reply = if request.starts_with("GET /api/v1/models") {
                    Reply::Body(
                        200,
                        r#"{"models":[{"id":"glm-5.3-flash","availability":"available","default":true}]}"#
                            .to_string(),
                    )
                } else {
                    handler(&request, &origin)
                };
                match reply {
                    Reply::Body(status, body) => {
                        let head = format!(
                            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n\
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

    Stub {
        base,
        origin,
        requests,
    }
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
        r#"{{"thread":{{"id":"th_test"}},"grant":{{"token":"sig_test","url":"{origin}/api/inference/proxy","model":"stub-model"}}}}"#
    )
}

/// One OpenAI-shaped chunk carrying text.
fn text(piece: &str) -> String {
    serde_json::json!({"choices":[{"delta":{"content": piece}}]}).to_string()
}

// ──────────────────────────────────────────────────────────────── the harness

/// A store confined to a directory of this test's own. It uses the real read,
/// write, and delete paths without accessing the developer's credentials.
fn store_in(origin: &str, name: &str) -> (CredentialStore, PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "Coder-logout-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    (CredentialStore::isolated(origin, &directory), directory)
}

fn session(base: &str, tx: Sender<Control>) -> Session {
    Session::open_at(
        Lane::Flash,
        "flash",
        None,
        Vec::new(),
        base.to_string(),
        Some(TOKEN.to_string()),
        false,
        tx,
    )
}

/// Distinctive enough that a notice repeating it would be unmistakable.
const TOKEN: &str = "oat_logout_test_never_print_me";

// ────────────────────────────────────────────────────────────────── the tests

/// The whole command: a session that opened a thread ends it by reporting, the
/// stored token is gone afterwards, and the notice says which credential went.
#[tokio::test]
async fn logout_ends_the_thread_by_reporting_and_removes_the_stored_token() {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Sse(vec![text("done")]);
        }
        if request.contains("/report") {
            return Reply::Body(
                200,
                r#"{"grant":{"spent":{"total_tokens":42}}}"#.to_string(),
            );
        }
        Reply::Body(200, "{}".to_string())
    });

    // A real turn, so the session holds the thread and the grant a `/logout`
    // has to close out rather than abandon.
    let (tx, _rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("say something", tx).await;

    let (store, directory) = store_in(&stub.origin, "reports");
    store.store(&Secret::new(TOKEN)).unwrap();
    assert!(
        store.find_token().unwrap().is_some(),
        "the fixture did not store a token, so its removal would prove nothing"
    );

    let notice = openagents_cli::coder::commands::logout_at(&mut session, &store).await;

    // 1. The token is gone, read back the way every other command reads it.
    assert!(
        store.find_token().unwrap().is_none(),
        "the token is still stored after /logout: {notice}"
    );

    // 2. The thread was ended by reporting, not left open and not thrown away.
    let lines = stub.request_lines();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("POST /api/v1/threads/th_test/report")),
        "the thread was not reported: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.starts_with("DELETE ")),
        "the thread was cancelled rather than reported: {lines:?}"
    );

    // 3. The notice says what happened, and to which credential.
    assert!(notice.contains(&stub.origin), "{notice}");
    assert!(notice.contains("Removed the OpenAgents token"), "{notice}");
    assert!(notice.contains("reporting"), "{notice}");
    // What the server said it billed, as the report's own reply carried it.
    assert!(
        notice.contains("Billed by the server: 42 tokens"),
        "the spend the report came back with is not in the notice: {notice}"
    );
    assert!(
        notice.contains("stays open and unauthenticated"),
        "the notice does not say what state the session is in: {notice}"
    );
    assert!(!notice.contains(TOKEN), "the notice printed the token");

    std::fs::remove_dir_all(directory).ok();
}

/// A server that refuses the report. The thread must still be disposed of
/// rather than leaked, the credential must still go, and the notice must not
/// claim a report that did not happen.
#[tokio::test]
async fn a_refused_report_is_said_out_loud_and_still_ends_the_thread() {
    let stub = start(|request, origin| {
        if request.contains("POST /api/v1/threads ") {
            return Reply::Body(200, grant(origin));
        }
        if request.contains("/api/inference/proxy") {
            return Reply::Sse(vec![text("done")]);
        }
        if request.contains("/report") {
            return Reply::Body(500, r#"{"error":"no"}"#.to_string());
        }
        Reply::Body(200, "{}".to_string())
    });

    let (tx, _rx) = channel();
    let mut session = session(&stub.base, tx.clone());
    session.execute_turn("say something", tx).await;

    let (store, directory) = store_in(&stub.origin, "refused");
    store.store(&Secret::new(TOKEN)).unwrap();

    let notice = openagents_cli::coder::commands::logout_at(&mut session, &store).await;

    assert!(
        store.find_token().unwrap().is_none(),
        "a refused report left the credential in place: {notice}"
    );
    let lines = stub.request_lines();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("DELETE /api/v1/threads/th_test")),
        "the thread was left open after the report was refused: {lines:?}"
    );
    assert!(
        notice.contains("could not report how it ended"),
        "the notice claims an ending that did not happen: {notice}"
    );
    assert!(!notice.contains(TOKEN), "the notice printed the token");

    std::fs::remove_dir_all(directory).ok();
}

/// A session with no thread still logs out, and says so without inventing an
/// ending for a thread that was never opened.
#[tokio::test]
async fn a_session_that_never_opened_a_thread_still_drops_its_credential() {
    let stub = start(|_request, _origin| Reply::Body(200, "{}".to_string()));
    let (tx, _rx) = channel();
    let mut session = session(&stub.base, tx);

    let (store, directory) = store_in(&stub.origin, "unopened");
    store.store(&Secret::new(TOKEN)).unwrap();

    let notice = openagents_cli::coder::commands::logout_at(&mut session, &store).await;

    assert!(store.find_token().unwrap().is_none(), "{notice}");
    assert!(
        stub.request_lines().is_empty(),
        "a session with no thread talked to the server: {:?}",
        stub.request_lines()
    );
    assert!(notice.contains(&stub.origin), "{notice}");
    assert!(!notice.contains(TOKEN), "the notice printed the token");

    std::fs::remove_dir_all(directory).ok();
}
