//! What `oa project edit`, `oa project delete`, and `oa repo create` put on
//! the wire, asserted by running the binary against a server this test owns.
//!
//! Asserting that a subcommand parses would pass against a binary that parsed
//! it and sent nothing, so every test here reads the request the stub actually
//! received: method, path, and body. The delete test lists the board before
//! and after, because a delete that answers `204` without removing anything
//! would otherwise look identical to one that worked.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::mpsc;
use std::thread;

/// One request the stub received.
#[derive(Debug, Clone)]
struct Hit {
    method: String,
    path: String,
    body: String,
}

impl Hit {
    fn route(&self) -> String {
        format!("{} {}", self.method, self.path)
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// A server that answers from a script and records every request.
///
/// The script is a list of `(status, body)` answered in order, with the last
/// entry repeating. Ordering matters here: the board list has to answer
/// differently before and after the delete, which a single canned body cannot
/// do.
struct StubServer {
    port: u16,
    hits: mpsc::Receiver<Hit>,
}

impl StubServer {
    fn start(script: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let port = listener.local_addr().expect("read the port").port();
        let (tx, hits) = mpsc::channel();
        thread::spawn(move || {
            for (answered, stream) in listener.incoming().enumerate() {
                let Ok(stream) = stream else { break };
                let index = answered.min(script.len().saturating_sub(1));
                let (code, body) = script[index].clone();
                serve_one(stream, code, &body, tx.clone());
            }
        });
        Self { port, hits }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn hits(&self) -> Vec<Hit> {
        self.hits.try_iter().collect()
    }
}

fn serve_one(mut stream: TcpStream, code: u16, body: &str, hits: mpsc::Sender<Hit>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }
        if header.trim().is_empty() {
            break;
        }
        if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut payload = vec![0u8; length];
    if length > 0 && reader.read_exact(&mut payload).is_err() {
        return;
    }
    let _ = hits.send(Hit {
        method,
        path,
        body: String::from_utf8_lossy(&payload).into_owned(),
    });
    let reason = if code == 204 { "No Content" } else { "OK" };
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

struct Run {
    stdout: String,
    stderr: String,
    status: Option<i32>,
}

fn oa(origin: &str, args: &[&str]) -> Run {
    let mut full = vec!["--api-url", origin];
    full.extend(args.iter().copied());
    let result = Command::new(env!("CARGO_BIN_EXE_oa"))
        .args(&full)
        .env("NO_COLOR", "")
        // `repo create` refuses without a credential rather than sending an
        // unauthenticated request, so the stub needs one to be reached at all.
        .env("OPENAGENTS_TOKEN", "oa_pat_stub")
        .output()
        .expect("run oa");
    Run {
        stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        status: result.status.code(),
    }
}

fn board_list(archived: bool) -> String {
    if archived {
        r#"{"projects":[{"number":5,"title":"Scratch","state":"open","archived":true}]}"#
            .to_string()
    } else {
        r#"{"projects":[]}"#.to_string()
    }
}

const ARCHIVED_BOARD: &str = r#"{"number":5,"title":"Scratch","state":"open","archived":true,"archived_at":"2026-08-26T00:00:00Z"}"#;

/// The board is listed, archived, deleted, and then gone from the list.
///
/// The two listings are the proof. The `DELETE` on its own only shows that the
/// CLI asked; the second listing shows the board is no longer there to ask
/// about.
#[test]
fn a_board_is_archived_deleted_and_absent_from_the_list_afterwards() {
    let server = StubServer::start(vec![
        (200, board_list(true)),
        (200, ARCHIVED_BOARD.to_string()),
        (204, String::new()),
        (200, board_list(false)),
    ]);
    let origin = server.origin();

    let before = oa(
        &origin,
        &[
            "--json",
            "project",
            "list",
            "--archived",
            "-R",
            "owner/repo",
        ],
    );
    assert_eq!(before.status, Some(0), "stderr: {}", before.stderr);
    assert!(
        before.stdout.contains("Scratch"),
        "the board was not listed before the delete: {}",
        before.stdout
    );

    let archive = oa(
        &origin,
        &["project", "edit", "5", "--archive", "-R", "owner/repo"],
    );
    assert_eq!(archive.status, Some(0), "stderr: {}", archive.stderr);

    let removed = oa(
        &origin,
        &["project", "delete", "5", "--yes", "-R", "owner/repo"],
    );
    assert_eq!(removed.status, Some(0), "stderr: {}", removed.stderr);
    assert!(
        removed.stdout.contains("Deleted project #5"),
        "unexpected output: {}",
        removed.stdout
    );

    let after = oa(
        &origin,
        &[
            "--json",
            "project",
            "list",
            "--archived",
            "-R",
            "owner/repo",
        ],
    );
    assert_eq!(after.status, Some(0), "stderr: {}", after.stderr);
    assert!(
        !after.stdout.contains("Scratch"),
        "the board was still listed after the delete: {}",
        after.stdout
    );

    let hits = server.hits();
    assert_eq!(
        hits.iter().map(Hit::route).collect::<Vec<_>>(),
        vec![
            "GET /api/v1/repos/owner/repo/projectsV2?archived=true".to_string(),
            "PATCH /api/v1/repos/owner/repo/projectsV2/5".to_string(),
            "DELETE /api/v1/repos/owner/repo/projectsV2/5".to_string(),
            "GET /api/v1/repos/owner/repo/projectsV2?archived=true".to_string(),
        ]
    );
    assert_eq!(hits[1].json(), serde_json::json!({ "archived": true }));
}

/// Deleting without `--yes` refuses before anything is sent.
#[test]
fn deleting_a_board_without_confirmation_sends_nothing() {
    let server = StubServer::start(vec![(204, String::new())]);
    let origin = server.origin();

    let run = oa(&origin, &["project", "delete", "5", "-R", "owner/repo"]);

    assert_ne!(run.status, Some(0));
    assert!(run.stderr.contains("--yes"), "stderr: {}", run.stderr);
    assert!(
        server.hits().is_empty(),
        "an unconfirmed delete reached the server"
    );
}

/// An edit that names no field refuses before anything is sent.
#[test]
fn editing_a_board_with_no_field_sends_nothing() {
    let server = StubServer::start(vec![(200, ARCHIVED_BOARD.to_string())]);
    let origin = server.origin();

    let run = oa(&origin, &["project", "edit", "5", "-R", "owner/repo"]);

    assert_ne!(run.status, Some(0));
    assert!(run.stderr.contains("--title"), "stderr: {}", run.stderr);
    assert!(server.hits().is_empty(), "an empty edit reached the server");
}

/// Title, description, and state travel together in one PATCH.
#[test]
fn an_edit_sends_every_field_the_caller_named() {
    let server = StubServer::start(vec![(200, ARCHIVED_BOARD.to_string())]);
    let origin = server.origin();

    let run = oa(
        &origin,
        &[
            "project",
            "edit",
            "5",
            "--title",
            "Renamed",
            "--description",
            "Why it exists",
            "--state",
            "closed",
            "-R",
            "owner/repo",
        ],
    );
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);

    let hits = server.hits();
    assert_eq!(
        hits[0].route(),
        "PATCH /api/v1/repos/owner/repo/projectsV2/5"
    );
    assert_eq!(
        hits[0].json(),
        serde_json::json!({
            "title": "Renamed",
            "description": "Why it exists",
            "state": "closed"
        })
    );
}

fn ready_repository(full_name: &str) -> String {
    let (owner, name) = full_name.split_once('/').expect("owner/name");
    format!(
        r#"{{"id":"11111111-1111-1111-1111-111111111111","name":"{name}","full_name":"{full_name}","owner":{{"id":1,"login":"{owner}","type":"User"}},"private":true,"visibility":"private","description":null,"default_branch":"main","lifecycle_state":"ready","provision_error_code":null,"clone_url":"http://example.test/{full_name}.git","html_url":"http://example.test/{full_name}","permissions":{{"admin":true,"push":true,"pull":true}},"created_at":"2026-08-26T00:00:00Z","updated_at":"2026-08-26T00:00:00Z"}}"#
    )
}

/// A named owner goes to the owner-neutral route with the owner in the body.
///
/// The bug this replaces read a slash as proof the owner is an organization
/// and posted to `/api/v1/orgs/{owner}/repos`, which is wrong whenever the
/// owner is a person. Both owners below take the same route now.
#[test]
fn a_named_owner_is_sent_to_the_server_rather_than_routed_on_a_guess() {
    for owner in ["AtlantisPleb", "OpenAgentsInc"] {
        let full_name = format!("{owner}/thing");
        let server = StubServer::start(vec![(201, ready_repository(&full_name))]);
        let origin = server.origin();

        let run = oa(&origin, &["repo", "create", &full_name]);
        assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);

        let hits = server.hits();
        assert_eq!(hits.len(), 1, "unexpected requests: {hits:?}");
        assert_eq!(hits[0].route(), "POST /api/v1/repos");
        assert!(
            !hits[0].path.contains("/orgs/"),
            "a named owner still went to the organization route"
        );
        assert_eq!(hits[0].json()["owner"], serde_json::json!(owner));
        assert_eq!(hits[0].json()["name"], serde_json::json!("thing"));
    }
}

/// A bare name carries no owner, so the server uses the caller's namespace.
#[test]
fn a_bare_name_carries_no_owner() {
    let server = StubServer::start(vec![(201, ready_repository("octavia/thing"))]);
    let origin = server.origin();

    let run = oa(&origin, &["repo", "create", "thing"]);
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);

    let hits = server.hits();
    assert_eq!(hits[0].route(), "POST /api/v1/repos");
    assert_eq!(hits[0].json().get("owner"), None);
}
