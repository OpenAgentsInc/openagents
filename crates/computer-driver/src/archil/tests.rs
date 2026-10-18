//! The driver against a mocked Archil control plane (wiremock), the same
//! approach as `crates/archil`'s own tests. Process-API behavior (exec,
//! commands, files) is covered by the live conformance run.
use super::*;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const SID: &str = "11111111-1111-1111-1111-111111111111";

fn ok(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": data }))
}

fn err(code: u16) -> ResponseTemplate {
    ResponseTemplate::new(code).set_body_json(json!({ "success": false, "error": "no" }))
}

fn sandbox(id: &str, name: &str, status: &str) -> Value {
    json!({
        "sandbox_id": id, "name": name, "status": status, "vcpu_count": 2, "mem_size_mib": 4096,
        "max_ttl_seconds": 3600, "idle_ttl_seconds": 0, "max_concurrent_execs": 8,
        "base_image": "ubuntu", "created_at": "2026-01-01T00:00:00Z",
        "last_active_at": "2026-01-01T00:00:00Z"
    })
}

/// Replays responses in order; the last repeats.
fn wiremock_seq(responses: Vec<ResponseTemplate>) -> impl wiremock::Respond {
    let next = std::sync::atomic::AtomicUsize::new(0);
    move |_: &Request| {
        let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        responses[i.min(responses.len() - 1)].clone()
    }
}

fn driver(server: &MockServer) -> ArchilDriver {
    ArchilDriver::with_base_url(Secret::new("test-key"), "aws-us-east-1", &server.uri()).unwrap()
}

fn request(operation: &str) -> CreateRequest {
    CreateRequest {
        operation: operation.into(),
        size: Some("medium".into()),
        ttl_seconds: Some(600),
        ..Default::default()
    }
}

async fn list(server: &MockServer, sandboxes: Value) {
    Mock::given(method("GET"))
        .and(path("/api/sandboxes"))
        .respond_with(ok(json!({ "sandboxes": sandboxes })))
        .mount(server)
        .await;
}

async fn get(server: &MockServer, status: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(ok(sandbox(SID, "x", status)))
        .mount(server)
        .await;
}

#[test]
fn declares_core_suspend_and_fork_but_not_snapshot() {
    let d = ArchilDriver::with_base_url(Secret::new("k"), "aws-us-east-1", "http://127.0.0.1:9")
        .unwrap();
    let caps = d.capabilities();
    caps.validate().unwrap();
    assert_eq!(caps.driver, "archil");
    assert!(caps.supports(Profile::Suspend) && caps.supports(Profile::Fork));
    assert!(!caps.supports(Profile::Snapshot));
    assert!(caps.memory_suspend && caps.command_sessions);
    assert_eq!(caps.sizes, ["small", "medium", "large"]);
    assert_eq!(
        caps.default_artifact.unwrap().kind,
        ArtifactKind::ArchilImage
    );
    assert!(!d.with_stop_suspend().capabilities().memory_suspend);
    assert!(
        ArchilDriver::new(Secret::new(" "), "aws-us-east-1").is_err(),
        "an empty key is refused before any request"
    );
}

#[tokio::test]
async fn snapshot_is_refused_as_unsupported() {
    let server = MockServer::start().await;
    let d = driver(&server);
    assert!(d.snapshot(SID, "snap").await.is_unsupported());
    assert!(d.snapshot_ready("snap").await.is_unsupported());
}

#[tokio::test]
async fn create_sends_the_named_shape_and_a_retry_finds_it_instead_of_creating_again() {
    let server = MockServer::start().await;
    let d = driver(&server);
    let name = d.name("c", &["op-1"]);
    let first = Mock::given(method("GET"))
        .and(path("/api/sandboxes"))
        .respond_with(ok(json!({ "sandboxes": [] })))
        .up_to_n_times(1)
        .mount_as_scoped(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .and(query_param("wait", "false"))
        .respond_with(move |r: &Request| {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            assert_eq!(body["vcpu_count"], 4);
            assert_eq!(body["mem_size_mib"], 8192);
            assert_eq!(body["idle_ttl_seconds"], 0);
            assert_eq!(body["max_ttl_seconds"], 600);
            assert!(body.get("mounts").is_none(), "no separate disk");
            ok(sandbox(SID, body["name"].as_str().unwrap(), "pending"))
        })
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(d.create(&request("op-1")).await, Outcome::done(SID.into()));
    drop(first);
    // The reply was lost; the same operation now finds the named sandbox.
    list(
        &server,
        json!([
            sandbox("other", "oa-c-other", "running"),
            sandbox(SID, &name, "pending")
        ]),
    )
    .await;
    assert_eq!(d.create(&request("op-1")).await, Outcome::done(SID.into()));
}

#[tokio::test]
async fn create_refusals_are_definite_and_server_errors_unknown() {
    let server = MockServer::start().await;
    list(&server, json!([])).await;
    let d = driver(&server);
    let post = Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .respond_with(err(400))
        .mount_as_scoped(&server)
        .await;
    assert!(matches!(
        d.create(&request("a")).await,
        Outcome::Failed { .. }
    ));
    drop(post);
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .respond_with(err(500))
        .mount(&server)
        .await;
    assert!(matches!(
        d.create(&request("b")).await,
        Outcome::Unknown { .. }
    ));
    assert!(matches!(
        d.create(&CreateRequest {
            size: Some("huge".into()),
            ..request("c")
        })
        .await,
        Outcome::Failed { .. }
    ));
}

#[tokio::test]
async fn observe_maps_states_and_a_missing_sandbox_is_gone() {
    let server = MockServer::start().await;
    let d = driver(&server);
    Mock::given(method("GET"))
        .and(path("/api/sandboxes/missing"))
        .respond_with(err(404))
        .mount(&server)
        .await;
    assert_eq!(
        d.observe("missing").await.value().unwrap().state,
        State::Gone
    );
    get(&server, "paused").await;
    assert_eq!(d.observe(SID).await.value().unwrap().state, State::Stopped);
    assert!(matches!(d.observe("../x").await, Outcome::Failed { .. }));
    for (status, want) in [
        ("pending", State::Starting),
        ("running", State::Running),
        ("pausing", State::Stopping),
        ("deleting", State::Stopping),
        ("exited", State::Stopped),
        ("deleted", State::Gone),
        ("failed", State::Failed),
    ] {
        let s: SandboxStatus = serde_json::from_value(json!(status)).unwrap();
        assert_eq!(state(s), want, "{status}");
    }
}

#[tokio::test]
async fn a_paused_computer_runs_nothing_and_takes_no_files() {
    let server = MockServer::start().await;
    get(&server, "paused").await;
    let d = driver(&server);
    let out = d.exec(SID, &Exec::new("true")).await;
    assert!(matches!(out, Outcome::Failed { ref reason } if reason.contains("isn't running")));
    assert!(matches!(
        d.put_file(SID, "a", b"x").await,
        Outcome::Failed { .. }
    ));
    assert!(matches!(d.get_file(SID, "a").await, Outcome::Failed { .. }));
    assert!(matches!(
        d.put_file(SID, "../etc/passwd", b"x").await,
        Outcome::Failed { .. }
    ));
}

#[tokio::test]
async fn a_refused_delete_stops_and_retries_and_a_missing_sandbox_is_done() {
    let server = MockServer::start().await;
    get(&server, "running").await;
    Mock::given(method("POST"))
        .and(path(format!("/api/sandboxes/{SID}/stop")))
        .respond_with(ok(sandbox(SID, "x", "stopped")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(wiremock_seq(vec![err(409), ResponseTemplate::new(204)]))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes/gone"))
        .respond_with(err(404))
        .mount(&server)
        .await;
    let d = driver(&server);
    assert_eq!(d.delete(SID).await, Outcome::done(()));
    assert_eq!(d.delete("gone").await, Outcome::done(()));
}

#[tokio::test]
async fn a_computer_with_forks_is_not_stopped_or_deleted() {
    let server = MockServer::start().await;
    get(&server, "running").await;
    Mock::given(method("POST"))
        .respond_with(err(500))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/sandboxes/{SID}")))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "success": false, "error": "has forks", "code": "sandbox_has_forks" }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let out = driver(&server).delete(SID).await;
    assert!(matches!(out, Outcome::Failed { ref reason } if reason.contains("copies")));
}

#[tokio::test]
async fn suspend_pauses_and_resume_starts_a_stopped_sandbox() {
    let server = MockServer::start().await;
    get(&server, "running").await;
    Mock::given(method("POST"))
        .and(path(format!("/api/sandboxes/{SID}/pause")))
        .respond_with(ok(sandbox(SID, "x", "paused")))
        .expect(1)
        .mount(&server)
        .await;
    let d = driver(&server);
    assert!(d.suspend(SID).await.is_done());

    let server = MockServer::start().await;
    get(&server, "stopped").await;
    Mock::given(method("POST"))
        .and(path(format!("/api/sandboxes/{SID}/start")))
        .respond_with(ok(sandbox(SID, "x", "running")))
        .expect(1)
        .mount(&server)
        .await;
    let d = driver(&server);
    assert!(d.resume(SID).await.is_done());
    // Already stopped: suspend is done without a request.
    assert!(d.suspend(SID).await.is_done());
}

#[tokio::test]
async fn resume_of_a_running_sandbox_is_done_without_a_request() {
    let server = MockServer::start().await;
    get(&server, "running").await;
    Mock::given(method("POST"))
        .respond_with(err(500))
        .expect(0)
        .mount(&server)
        .await;
    assert!(driver(&server).resume(SID).await.is_done());
}

#[tokio::test]
async fn a_repeated_fork_returns_the_named_child() {
    let server = MockServer::start().await;
    let d = driver(&server);
    let name = d.name("f", &[SID, "fork-1"]);
    list(&server, json!([sandbox("child-1", &name, "running")])).await;
    Mock::given(method("POST"))
        .respond_with(err(500))
        .expect(0)
        .mount(&server)
        .await;
    assert_eq!(d.fork(SID, "fork-1").await, Outcome::done("child-1".into()));
    assert_ne!(name, d.name("f", &[SID, "fork-2"]));
}

#[test]
fn secrets_travel_as_base64_lines_ending_in_a_blank_line() {
    let a = "A".to_owned();
    let input = secret_input([(&a, "x y\n'z")].into_iter());
    assert_eq!(input, format!("A={}\n\n", BASE64.encode("x y\n'z")));
}

#[test]
fn the_secret_reader_exports_values_without_writing_them() {
    let home = tempfile::tempdir().unwrap();
    let script = format!("{SECRET_READER}printf %s \"$TOKEN\"");
    let mut child = std::process::Command::new("sh")
        .args(["-c", &script])
        .current_dir(home.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    let name = "TOKEN".to_owned();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(secret_input([(&name, "va'l ue")].into_iter()).as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "va'l ue");
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
}
