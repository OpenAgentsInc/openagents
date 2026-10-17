#![cfg(feature = "space")]

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use computer_driver::{space::SpaceDriver, *};
use futures_util::SinkExt;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::io::AsyncWriteExt;

#[derive(Default)]
struct Records {
    machines: BTreeMap<String, String>,
    creates: BTreeMap<String, String>,
    lifecycle: BTreeMap<String, String>,
    commands: BTreeMap<String, (String, Value)>,
    // Assert the saved API payload never contains secret values.
    requests: Vec<Value>,
    fail_next: Option<u16>,
    stream_mode: &'static str,
    operation_states: std::collections::VecDeque<&'static str>,
    operation_reads: usize,
    operation_error: Option<u16>,
}
#[derive(Clone)]
struct Mock {
    root: Arc<tempfile::TempDir>,
    records: Arc<Mutex<Records>>,
}

fn reply(v: Value) -> Response {
    Json(v).into_response()
}

async fn api(
    State(m): State<Mock>,
    Path(path): Path<String>,
    method: Method,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer management-token")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let body = body.map(|Json(v)| v).unwrap_or(Value::Null);
    let mut r = m.records.lock().unwrap();
    if let Some(code) = r.fail_next.take() {
        return (
            StatusCode::from_u16(code).unwrap(),
            "do-not-echo-this-secret",
        )
            .into_response();
    }
    r.requests.push(body.clone());
    if path.starts_with("operations/") && method == Method::GET {
        r.operation_reads += 1;
        if let Some(status) = r.operation_error.take() {
            return (
                StatusCode::from_u16(status).unwrap(),
                "do-not-echo-this-secret",
            )
                .into_response();
        }
        let state = r.operation_states.pop_front().unwrap_or("succeeded");
        return reply(json!({"state":state,"terminal_error":"do-not-echo-this-secret"}));
    }
    if path == "sandboxes" && method == Method::POST {
        let key = headers.get("idempotency-key").unwrap().to_str().unwrap();
        if let Some(id) = r.creates.get(key) {
            return reply(json!({"sandbox_id":id,"operation_id":"op_create"}));
        }
        let id = format!("sbx_{}", r.creates.len());
        std::fs::create_dir(m.root.path().join(&id)).unwrap();
        r.creates.insert(key.into(), id.clone());
        r.machines.insert(id.clone(), "running".into());
        return reply(json!({"sandbox_id":id,"operation_id":"op_create"}));
    }
    let Some((_, tail)) = path.split_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let id = tail.split(['/', ':']).next().unwrap();
    if !r.machines.contains_key(id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some((_, action)) = tail.split_once(':')
        && !tail.contains('/')
    {
        let state = match action {
            "suspend" => "suspended",
            "wake" => "running",
            _ => return StatusCode::NOT_FOUND.into_response(),
        };
        let key = headers.get("idempotency-key").unwrap().to_str().unwrap();
        if let Some(operation) = r.lifecycle.get(key) {
            return reply(json!({"operation_id":operation}));
        }
        let operation = format!("op_{action}_{}", r.lifecycle.len());
        r.lifecycle.insert(key.into(), operation.clone());
        r.machines.insert(id.into(), state.into());
        return reply(json!({"operation_id":operation}));
    }
    if !tail.contains('/') {
        if method == Method::DELETE {
            r.machines.remove(id);
            return reply(json!({"operation_id":"op_delete"}));
        }
        return reply(json!({"id":id, "state":r.machines[id]}));
    }
    if path.ends_with("/commands") {
        assert_eq!(body["env"], json!({}));
        let session = format!("proc_{}", r.commands.len());
        r.commands.insert(session.clone(), (id.into(), body));
        return reply(
            json!({"session_id":session,"attach":{"url":format!("/v1/sessions/{session}/attach"),"token":"session-token"}}),
        );
    }
    // Keep every filesystem operation inside this test's own temporary root.
    let file = body["path"].as_str().unwrap_or("");
    assert!(!file.starts_with('/') && !file.split('/').any(|x| x == ".."));
    let file = m.root.path().join(id).join(file);
    if path.ends_with("/files:mkdir") {
        std::fs::create_dir_all(file).unwrap();
        return reply(json!({"ok":true}));
    }
    if path.ends_with("/files:write") {
        let bytes = BASE64
            .decode(body["data_base64"].as_str().unwrap())
            .unwrap();
        std::fs::write(file, &bytes).unwrap();
        return reply(json!({"bytes_written":bytes.len()}));
    }
    if path.ends_with("/files:read") {
        let Ok(bytes) = std::fs::read(file) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let start = (body["offset"].as_u64().unwrap() as usize).min(bytes.len());
        let end = (start + body["length"].as_u64().unwrap() as usize).min(bytes.len());
        return bytes[start..end].to_vec().into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn attach(
    State(m): State<Mock>,
    Path(id): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    assert!(
        headers.get("authorization").is_none(),
        "management token reached data plane"
    );
    ws.on_upgrade(move |socket| command(socket, m, id))
}

async fn send(socket: &mut WebSocket, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

async fn command(mut socket: WebSocket, m: Mock, session: String) {
    let Some(Ok(Message::Text(text))) = socket.recv().await else {
        panic!("missing attach");
    };
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap(),
        json!({"type":"attach","token":"session-token"})
    );
    let (id, body, mode) = {
        let r = m.records.lock().unwrap();
        let (id, body) = r.commands[&session].clone();
        (id, body, r.stream_mode)
    };
    if mode == "truncate" {
        let _ = socket.close().await;
        return;
    }
    if mode == "partial" {
        send(
            &mut socket,
            json!({"type":"stdout","data_base64":BASE64.encode(b"early output")}),
        )
        .await;
        send(
            &mut socket,
            json!({"type":"stderr","data_base64":BASE64.encode(b"early error")}),
        )
        .await;
        let _ = socket.close().await;
        return;
    }
    if mode == "error" {
        send(
            &mut socket,
            json!({"type":"error","message":"do-not-echo-this-secret"}),
        )
        .await;
        return;
    }
    let dir = m.root.path().join(id);
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(body["command"][2].as_str().unwrap())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &dir)
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    send(
        &mut socket,
        json!({"type":"process_started","process_id":"p_test"}),
    )
    .await;
    if body["stdin"] == true {
        let Some(Ok(Message::Text(text))) = socket.recv().await else {
            panic!("missing input");
        };
        let input: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(input["type"], "input");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(
                &BASE64
                    .decode(input["data_base64"].as_str().unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        let Some(Ok(Message::Text(text))) = socket.recv().await else {
            panic!("missing EOF");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap()["type"],
            "close_stdin"
        );
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().await.unwrap();
    send(
        &mut socket,
        json!({"type":"stdout","seq":1,"data_base64":BASE64.encode(out.stdout)}),
    )
    .await;
    send(
        &mut socket,
        json!({"type":"stderr","seq":2,"data_base64":BASE64.encode(out.stderr)}),
    )
    .await;
    send(
        &mut socket,
        json!({"type":"process_exited","exit_code":out.status.code().unwrap_or(-1)}),
    )
    .await;
}

async fn setup() -> (SpaceDriver, Mock, tokio::task::JoinHandle<()>) {
    let m = Mock {
        root: Arc::new(tempfile::tempdir().unwrap()),
        records: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/v1/sessions/{id}/attach", get(attach))
        .route("/v1/{*path}", any(api))
        .with_state(m.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (
        SpaceDriver::new(&url, &url, Secret::new("management-token"), "seed:latest").unwrap(),
        m,
        task,
    )
}

#[tokio::test]
async fn space_wire_contract_passes_core_and_suspend() {
    let (d, m, task) = setup().await;
    let report =
        computer_conformance::run(&d, &computer_conformance::Options::fast("space-wire")).await;
    task.abort();
    assert!(report.passed(), "{}", report.summary());
    let records = m.records.lock().unwrap();
    assert!(records.machines.is_empty());
    let creates: Vec<_> = records
        .requests
        .iter()
        .filter(|r| r.get("template").is_some())
        .collect();
    assert!(!creates.is_empty());
    assert!(
        creates.iter().all(|r| r.get("resources").is_none()),
        "a custom shape forces cold boot in SPACE"
    );
    // The conformance grep intentionally includes its search string in
    // argv. The credential-bearing command must not contain that value.
    for request in &records.requests {
        if let Some(shell) = request["command"][2].as_str()
            && shell.starts_with("eval ")
        {
            assert!(!shell.contains("s3cret-"));
            assert_eq!(request["env"], json!({}));
        }
    }
}

#[tokio::test]
async fn lifecycle_waits_for_the_operation_and_never_echoes_its_error() {
    let (d, m, task) = setup().await;
    let id = d
        .create(&CreateRequest {
            operation: "lifecycle".into(),
            ..Default::default()
        })
        .await
        .value()
        .unwrap();
    m.records
        .lock()
        .unwrap()
        .operation_states
        .extend(["queued", "running", "succeeded"]);
    assert!(d.suspend(&id).await.is_done());
    // Create completes one operation before suspend's three observations.
    assert_eq!(m.records.lock().unwrap().operation_reads, 4);
    for state in ["failed", "canceled", "expired", "future_state"] {
        m.records.lock().unwrap().operation_states.push_back(state);
        let result = d.resume(&id).await;
        assert!(matches!(result, Outcome::Unknown { .. }));
        assert!(!format!("{result:?}").contains("do-not-echo"));
    }
    for status in [403, 404, 500] {
        m.records.lock().unwrap().operation_error = Some(status);
        let result = d.resume(&id).await;
        assert!(matches!(result, Outcome::Unknown { .. }));
        assert!(!format!("{result:?}").contains("do-not-echo"));
    }
    assert!(d.delete(&id).await.is_done());
    task.abort();
}

#[tokio::test]
async fn lifecycle_retries_reuse_the_original_operation_after_lost_lookup() {
    let (driver, mock, task) = setup().await;
    let id = driver
        .create(&CreateRequest {
            operation: "stable-lifecycle".into(),
            ..Default::default()
        })
        .await
        .value()
        .unwrap();
    let driver = Arc::new(driver);
    mock.records.lock().unwrap().operation_error = Some(404);
    assert!(matches!(
        driver.suspend_operation(&id, "checkpoint-1").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(
        driver.observe(&id).await.value().unwrap().state,
        State::Stopped
    );
    assert_eq!(
        driver
            .suspend_operation(&id, "checkpoint-1")
            .await
            .value()
            .unwrap(),
        "op_suspend_0"
    );
    assert_eq!(mock.records.lock().unwrap().lifecycle.len(), 1);
    assert!(driver.resume_operation(&id, "wake-2").await.is_done());
    assert!(
        driver
            .suspend_operation(&id, "checkpoint-1")
            .await
            .is_done()
    );
    assert_eq!(
        driver.observe(&id).await.value().unwrap().state,
        State::Running,
        "an older operation retry must never apply its effect again"
    );
    assert_eq!(mock.records.lock().unwrap().lifecycle.len(), 2);
    assert!(driver.delete(&id).await.is_done());
    task.abort();
}

#[tokio::test]
async fn errors_remain_unknown_and_secrets_are_shell_quoted() {
    let (d, m, task) = setup().await;
    let id = d
        .create(&CreateRequest {
            operation: "test".into(),
            ..Default::default()
        })
        .await
        .value()
        .unwrap();
    let value = "quote'\"\n$(touch escape)\nlast\n";
    let out = d
        .exec(
            &id,
            &Exec::new("printf %s \"$TEST_SECRET\"").secret("TEST_SECRET", value),
        )
        .await
        .value()
        .unwrap();
    assert_eq!(out.stdout, value);
    assert!(
        m.records
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|v| !v["command"][2].as_str().unwrap_or("").contains(value))
    );
    assert!(!m.root.path().join(&id).join("escape").exists());
    for status in [403, 408, 429, 500] {
        m.records.lock().unwrap().fail_next = Some(status);
        let result = d
            .create(&CreateRequest {
                operation: "errors".into(),
                ..Default::default()
            })
            .await;
        assert_eq!(matches!(result, Outcome::Failed { .. }), status == 403);
        assert!(!format!("{result:?}").contains("do-not-echo"));
    }
    for mode in ["error", "truncate"] {
        m.records.lock().unwrap().stream_mode = mode;
        assert!(matches!(
            d.exec(&id, &Exec::new("true")).await,
            Outcome::Unknown { .. }
        ));
    }
    task.abort();
}

#[derive(Default)]
struct JournalFixture(Mutex<BTreeMap<String, (String, String, Option<String>)>>);

impl computer_driver::space::LifecycleJournal for JournalFixture {
    fn claim<'a>(
        &'a self,
        key: &'a str,
        resource: &'a str,
        action: &'a str,
    ) -> computer_driver::space::JournalFuture<'a, computer_driver::space::LifecycleClaim> {
        use computer_driver::space::LifecycleClaim;
        Box::pin(async move {
            let mut rows = self.0.lock().unwrap();
            if let Some((saved_resource, saved_action, operation)) = rows.get(key) {
                if saved_resource != resource || saved_action != action {
                    return Err("conflict".into());
                }
                return Ok(operation.as_ref().map_or(LifecycleClaim::Uncertain, |id| {
                    LifecycleClaim::Accepted(id.clone())
                }));
            }
            rows.insert(key.into(), (resource.into(), action.into(), None));
            Ok(LifecycleClaim::Fresh)
        })
    }
    fn accepted<'a>(
        &'a self,
        key: &'a str,
        operation: &'a str,
    ) -> computer_driver::space::JournalFuture<'a, ()> {
        Box::pin(async move {
            self.0.lock().unwrap().get_mut(key).unwrap().2 = Some(operation.into());
            Ok(())
        })
    }
    fn rejected<'a>(&'a self, _key: &'a str) -> computer_driver::space::JournalFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn saved_provider_operations_survive_mapping_expiry_and_lost_replies_never_send_again() {
    let (driver, mock, task) = setup().await;
    let id = driver
        .create(&CreateRequest {
            operation: "journal-fixture".into(),
            ..Default::default()
        })
        .await
        .value()
        .unwrap();
    let journal = Arc::new(JournalFixture::default());
    let driver = driver.with_lifecycle_journal(journal.clone());
    mock.records.lock().unwrap().operation_error = Some(404);
    assert!(matches!(
        driver.suspend_operation(&id, "first-checkpoint").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(
        journal.0.lock().unwrap()["first-checkpoint"].2.as_deref(),
        Some("op_suspend_0")
    );
    // Expired provider retry records must not turn recovery into another POST.
    mock.records.lock().unwrap().lifecycle.clear();
    let restarted = driver.clone().with_lifecycle_journal(journal.clone());
    assert!(
        restarted
            .suspend_operation(&id, "first-checkpoint")
            .await
            .is_done()
    );
    assert!(mock.records.lock().unwrap().lifecycle.is_empty());
    mock.records.lock().unwrap().operation_error = Some(404);
    assert!(matches!(
        restarted.suspend_operation(&id, "first-checkpoint").await,
        Outcome::Unknown { .. }
    ));
    assert!(mock.records.lock().unwrap().lifecycle.is_empty());

    // A lost mutation response can hide an accepted operation. Without its ID,
    // recovery remains unknown even if the provider forgot its retry mapping.
    mock.records.lock().unwrap().fail_next = Some(500);
    assert!(matches!(
        driver.resume_operation(&id, "lost-reply").await,
        Outcome::Unknown { .. }
    ));
    assert!(matches!(
        restarted.resume_operation(&id, "lost-reply").await,
        Outcome::Unknown { .. }
    ));
    assert!(mock.records.lock().unwrap().lifecycle.is_empty());
    assert!(driver.delete(&id).await.is_done());
    task.abort();
}

#[tokio::test]
async fn interrupted_space_commands_keep_bounded_partial_output_without_claiming_exit() {
    let (driver, mock, task) = setup().await;
    let id = driver
        .create(&CreateRequest {
            operation: "partial-output".into(),
            ..Default::default()
        })
        .await
        .value()
        .unwrap();
    mock.records.lock().unwrap().stream_mode = "partial";
    let exec = Exec::new("printf early");
    assert!(matches!(
        driver.exec(&id, &exec).await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(
        exec.output.read(),
        (b"early output".to_vec(), b"early error".to_vec())
    );
    exec.output.append(false, &vec![b'x'; 512 * 1024]);
    let (output, _) = exec.output.read();
    assert_eq!(output.len(), 256 * 1024);
    assert!(output.starts_with(b"early output"));
    assert!(!format!("{:?}", exec.output).contains("early output"));
    assert!(driver.delete(&id).await.is_done());
    task.abort();
}
