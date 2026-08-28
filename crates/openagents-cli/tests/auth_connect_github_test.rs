//! The contract for `oa auth connect-github` (#299).
//!
//! The CLI half of the GitHub repository-scope connect flow: an existing CLI
//! token is required (the connect page approves against the signed-in
//! account), `--headless` records a pending authorization marked
//! `github_connect`, `--resume` finishes exactly that kind of record, and the
//! scope-required refusal names the command that fixes it.

use std::io::{BufRead as _, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// One request the stub saw: method, path, and parsed body.
#[derive(Debug, Clone)]
struct SeenRequest {
    method: String,
    path: String,
    body: serde_json::Value,
}

struct StubApi {
    origin: String,
    seen: mpsc::Receiver<SeenRequest>,
}

/// Serve every request until the test ends, answering by path.
///
/// - `POST /api/v1/device/authorizations` → `201` with a device authorization
/// - `POST /api/v1/device/authorizations/token` → `200` with the connected
///   answer (no access token: the credential stays server-side)
/// - everything else → `403 github_scope_required`
fn start_stub_api(github_login: &str) -> StubApi {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, seen) = mpsc::channel();
    let github_login = github_login.to_string();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let sender = sender.clone();
            let github_login = github_login.clone();
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
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
                    if let Some((_name, value)) = trimmed
                        .split_once(':')
                        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    {
                        content_length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut raw = vec![0u8; content_length];
                if content_length > 0 {
                    std::io::Read::read_exact(&mut reader, &mut raw).unwrap();
                }
                let body: serde_json::Value = if raw.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_slice(&raw).unwrap_or(serde_json::Value::Null)
                };
                let _ = sender.send(SeenRequest {
                    method: method.clone(),
                    path: path.to_string(),
                    body,
                });

                let (status, payload) = if path == "/api/v1/device/authorizations" {
                    (
                        201,
                        serde_json::json!({
                            "device_code": "device-code-gc",
                            "user_code": "GHCN-JKLM",
                            "verification_uri": "http://device.example/connect",
                            "verification_uri_complete": "http://device.example/connect?user_code=GHCN-JKLM",
                            "expires_in": 600,
                            "interval": 1,
                        }),
                    )
                } else if path == "/api/v1/device/authorizations/token" {
                    (
                        200,
                        serde_json::json!({
                            "status": "connected",
                            "github_login": github_login,
                        }),
                    )
                } else {
                    (
                        403,
                        serde_json::json!({
                            "code": "github_scope_required",
                            "message": "Reconnect GitHub with required access",
                        }),
                    )
                };
                let payload = payload.to_string();
                let response = format!(
                    "HTTP/1.1 {status} S\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
        }
    });

    StubApi {
        origin: format!("http://127.0.0.1:{port}"),
        seen,
    }
}

fn seen(stub: &StubApi) -> SeenRequest {
    stub.seen
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the client never sent a request")
}

struct Home {
    directory: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn pending_file(&self) -> PathBuf {
        self.path()
            .join(".config")
            .join("openagents")
            .join("device-authorizations.json")
    }
}

fn run_oa(home: &Path, args: &[&str], stdin: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .env("HOME", home)
        .env_remove("OPENAGENTS_TOKEN")
        .args(args);
    match stdin {
        Some(text) => {
            command.stdin(Stdio::piped());
            let mut child = command.spawn().expect("oa runs");
            let mut pipe = child.stdin.take().unwrap();
            pipe.write_all(text.as_bytes()).expect("the token is read");
            drop(pipe);
            child.wait_with_output().expect("oa finishes")
        }
        None => command.output().expect("oa runs"),
    }
}

fn assert_success(output: &std::process::Output, args: &[&str]) -> serde_json::Value {
    assert!(
        output.status.success(),
        "oa {args:?} exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("oa --json answers with one document")
}

fn assert_failure(output: &std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "oa {args:?} was expected to refuse but exited 0: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn store_a_token(origin: &str, home: &Home) {
    let output = run_oa(
        home.path(),
        &["--api-url", origin, "auth", "login", "--token-stdin"],
        Some("oa_pat_connect_test"),
    );
    assert!(
        output.status.success(),
        "seeding a token failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn connect_github_refuses_without_a_stored_token() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    let output = run_oa(
        home.path(),
        &["--api-url", &stub.origin, "auth", "connect-github"],
        None,
    );
    let stderr = assert_failure(&output, &["auth", "connect-github"]);
    assert!(
        stderr.contains("auth login"),
        "the refusal must point at signing in first: {stderr}"
    );
}

#[test]
fn connect_github_refuses_headless_and_resume_together() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    store_a_token(&stub.origin, &home);
    let output = run_oa(
        home.path(),
        &[
            "--api-url",
            &stub.origin,
            "auth",
            "connect-github",
            "--headless",
            "--resume",
        ],
        None,
    );
    let stderr = assert_failure(
        &output,
        &["auth", "connect-github", "--headless", "--resume"],
    );
    assert!(
        stderr.contains("use only one of"),
        "the flag contract must match login's: {stderr}"
    );
}

#[test]
fn headless_records_a_github_connect_pending_authorization() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    store_a_token(&stub.origin, &home);

    let value = assert_success(
        &run_oa(
            home.path(),
            &[
                "--api-url",
                stub.origin.as_str(),
                "auth",
                "connect-github",
                "--headless",
                "--json",
            ],
            None,
        ),
        &["auth connect-github --headless --json"],
    );
    assert_eq!(value["connected"], serde_json::Value::Bool(false));
    assert_eq!(
        value["authorization_pending"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        value["verification_url"],
        "http://device.example/connect?user_code=GHCN-JKLM"
    );
    assert_eq!(value["user_code"], "GHCN-JKLM");
    assert!(
        value["resume_command"]
            .as_str()
            .unwrap()
            .contains("auth connect-github --resume"),
        "the resume command names this flow, not login: {}",
        value["resume_command"]
    );

    let start = seen(&stub);
    assert_eq!(start.method, "POST");
    assert_eq!(start.path, "/api/v1/device/authorizations");
    assert_eq!(
        start.body["kind"], "github_connect",
        "the request body must select the server's connect flow: {}",
        start.body
    );
    assert!(
        start.body.get("scope").is_none(),
        "connect asks for the server's fixed scope set, not --scope: {}",
        start.body
    );

    let pending: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(home.pending_file()).expect("the pending record is on disk"),
    )
    .unwrap();
    let record = &pending["authorizations"][stub.origin.as_str()];
    assert_eq!(record["kind"], "github_connect");
    assert_eq!(record["device_code"], "device-code-gc");
}

#[test]
fn resume_completes_a_connect_pending_authorization_and_names_the_account() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    store_a_token(&stub.origin, &home);

    let headless = run_oa(
        home.path(),
        &[
            "--api-url",
            &stub.origin,
            "auth",
            "connect-github",
            "--headless",
            "--json",
        ],
        None,
    );
    assert_success(&headless, &["auth connect-github --headless --json"]);
    seen(&stub);

    let resume = run_oa(
        home.path(),
        &[
            "--api-url",
            &stub.origin,
            "auth",
            "connect-github",
            "--resume",
            "--json",
        ],
        None,
    );
    let value = assert_success(&resume, &["auth connect-github --resume --json"]);
    assert_eq!(value["connected"], serde_json::Value::Bool(true));
    assert_eq!(value["github_login"], "octavia");
    assert_eq!(value["origin"], stub.origin.as_str());

    let poll = seen(&stub);
    assert_eq!(poll.path, "/api/v1/device/authorizations/token");
    assert_eq!(poll.body["device_code"], "device-code-gc");

    assert!(
        !home.pending_file().exists(),
        "a completed authorization leaves no pending record behind"
    );
}

#[test]
fn resume_refuses_a_pending_record_a_login_wrote() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    store_a_token(&stub.origin, &home);

    // What `oa auth login --headless` writes: the same record, a different kind.
    let login_pending = serde_json::json!({
        "version": 1,
        "authorizations": {
            stub.origin.clone(): {
                "origin": stub.origin,
                "device_code": "device-code-login",
                "user_code": "LOGI-NOW",
                "verification_uri": format!("{}/device", stub.origin),
                "verification_uri_complete": format!("{}/device?user_code=LOGI-NOW", stub.origin),
                "expires_at_ms": 4_102_444_800_000_i64,
                "interval": 5,
                "kind": "device",
            }
        }
    });
    std::fs::create_dir_all(home.pending_file().parent().unwrap()).unwrap();
    std::fs::write(home.pending_file(), login_pending.to_string()).unwrap();

    let output = run_oa(
        home.path(),
        &[
            "--api-url",
            &stub.origin,
            "auth",
            "connect-github",
            "--resume",
        ],
        None,
    );
    let stderr = assert_failure(&output, &["auth connect-github --resume"]);
    assert!(
        stderr.contains("not a GitHub connect"),
        "the refusal must name the kind mismatch: {stderr}"
    );
    assert!(
        stderr.contains("auth connect-github"),
        "the refusal must point at starting the flow fresh: {stderr}"
    );
    assert!(
        home.pending_file().exists(),
        "a refused resume leaves the pending record alone"
    );
}

#[test]
fn a_scope_required_refusal_hints_the_connect_command() {
    let stub = start_stub_api("octavia");
    let home = Home::new();
    store_a_token(&stub.origin, &home);

    // The stub answers every other path with 403 github_scope_required, which
    // is exactly what `repo view` meets behind a token without repo scopes.
    let output = run_oa(
        home.path(),
        &[
            "--api-url",
            stub.origin.as_str(),
            "repo",
            "view",
            "octavia/project",
        ],
        None,
    );
    let stderr = assert_failure(&output, &["repo view octavia/project"]);
    assert!(
        stderr.contains("github_scope_required"),
        "the server's own code stays in the rendered error: {stderr}"
    );
    assert!(
        stderr.contains("auth connect-github"),
        "the fix is one line naming the connect command: {stderr}"
    );
    assert!(
        stderr.contains(&stub.origin),
        "the hint names the origin it applies to: {stderr}"
    );
}

#[test]
fn the_connect_command_for_an_origin_names_the_flags_it_needs() {
    use openagents_cli::auth::connect_github_command_for_origin;
    assert_eq!(
        connect_github_command_for_origin("https://openagents.com"),
        "oa auth connect-github"
    );
    assert_eq!(
        connect_github_command_for_origin("http://127.0.0.1:4000"),
        "oa --api-url http://127.0.0.1:4000 auth connect-github"
    );
}
