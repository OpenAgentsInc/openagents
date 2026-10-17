use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Session,
    computers::{CommandOptions, CommandOutcome, CreateOptions, Error, RetryPolicy, WaitFor},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_0123456789abcdef0123";
const PRIVATE: &str = "private-command-file-and-error-fixture";

fn computer(state: &str) -> Value {
    json!({"id":ID,"kind":"cloud","name":PRIVATE,"size":"small","state":state,
        "desired":if state=="stopped" {"stopped"} else {"running"},"online":state=="awake","future_field":17})
}
fn command(state: &str) -> Value {
    json!({"id":COMMAND,"computer":ID,"state":state,"exit_code":if state=="succeeded" {Some(0)} else {None},
        "digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":65536,"output_expires_unix":2000000000,
        "stdout_truncated":false,"stderr_truncated":false})
}
fn capabilities(files: bool) -> Value {
    let mut value = json!({"schema":"openagents.computers.capabilities.v1","profiles":["core"],"sizes":["small"],"limits":{"max_computers":20}});
    if files {
        value["files"] = json!({"max_bytes":4194304,"max_path_bytes":1024,"max_concurrent_transfers":2,"requires_awake":true,"timeout_seconds":30});
    }
    value
}

struct Step {
    status: u16,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    delay: Duration,
}
impl Step {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&value).unwrap(),
            headers: vec![],
            delay: Duration::ZERO,
        }
    }
    fn after(mut self, value: &str) -> Self {
        self.headers.push(("retry-after".into(), value.into()));
        self
    }
    fn delayed(mut self) -> Self {
        self.delay = Duration::from_millis(250);
        self
    }
}
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    uri: String,
    key: Option<String>,
    length: Option<String>,
    hash: Option<String>,
    body: Vec<u8>,
}
#[derive(Clone)]
struct Script {
    steps: Arc<Mutex<VecDeque<Step>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}
struct Server {
    client: Client,
    script: Script,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn serve(State(script): State<Script>, request: Request<Body>) -> Response {
    assert_eq!(
        request.headers()["authorization"],
        format!("Bearer sess_{}", "a".repeat(64))
    );
    if request.uri().path() == "/v1/account" {
        return axum::Json(json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}))
            .into_response();
    }
    let (parts, body) = request.into_parts();
    let header = |name: &str| {
        parts
            .headers
            .get(name)
            .map(|s| s.to_str().unwrap().to_owned())
    };
    let body = to_bytes(body, 5 * 1024 * 1024).await.unwrap().to_vec();
    script.seen.lock().unwrap().push(Seen {
        method: parts.method.to_string(),
        uri: parts.uri.to_string(),
        key: header("idempotency-key"),
        length: header("content-length"),
        hash: header("x-content-sha256"),
        body,
    });
    let step = script
        .steps
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected HTTP attempt");
    tokio::time::sleep(step.delay).await;
    let mut response = Response::builder().status(step.status);
    for (name, value) in step.headers {
        response = response.header(name, value);
    }
    response.body(Body::from(step.body)).unwrap()
}
async fn mocked(steps: Vec<Step>, request_timeout: Duration) -> (Server, Session) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let script = Script {
        steps: Arc::new(Mutex::new(steps.into())),
        seen: Arc::new(Mutex::new(vec![])),
    };
    let router = Router::new().fallback(serve).with_state(script.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
    // The short timeout exercises the computer request, not fixture sign-in.
    let session = Client::new(deployment.clone())
        .unwrap()
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    let client = Client::with_timeouts(deployment, request_timeout, request_timeout).unwrap();
    (
        Server {
            client,
            script,
            task,
        },
        session,
    )
}
fn retries() -> RetryPolicy {
    RetryPolicy {
        max_retries: 2,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
    }
}

#[tokio::test]
async fn lost_create_response_and_restart_reuse_the_same_key_and_exact_body() {
    let (server, session) = mocked(
        vec![
            Step::json(202, computer("creating")).delayed(),
            Step::json(200, computer("awake")),
            Step::json(200, computer("stopped")),
        ],
        Duration::from_millis(40),
    )
    .await;
    let computers = server
        .client
        .computers(&session)
        .unwrap()
        .with_retries(retries())
        .unwrap();
    let request = computers.create_request(CreateOptions::default()).unwrap();
    let saved = serde_json::to_vec(&request).unwrap();
    assert!(!String::from_utf8_lossy(&saved).contains("sess_"));
    let row = computers
        .create(&request, &CancellationToken::new())
        .await
        .unwrap();
    assert!(row.awake());
    let restarted = Client::new(server.client.deployment().clone()).unwrap();
    let restored = serde_json::from_slice(&saved).unwrap();
    let row = restarted
        .computers(&session)
        .unwrap()
        .create(&restored, &CancellationToken::new())
        .await
        .unwrap();
    assert!(row.stopped()); // A keyed retry must not silently wake the stopped computer.
    let seen = server.script.seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    for attempt in seen.iter() {
        assert_eq!(attempt.key.as_deref(), Some(request.key().as_str()));
        assert_eq!(attempt.body, seen[0].body);
    }
}

#[tokio::test]
async fn read_retries_obey_retry_after_and_preserve_server_codes_without_logging_content() {
    let (server, session) = mocked(
        vec![
            Step::json(
                429,
                json!({"error":{"code":"quota_exhausted","message":PRIVATE}}),
            )
            .after("0"),
            Step::json(
                503,
                json!({"error":{"code":"temporarily_unavailable","message":PRIVATE}}),
            )
            .after("0"),
            Step::json(200, computer("new_future_state")),
            Step::json(
                429,
                json!({"error":{"code":"future_quota_code","message":PRIVATE}}),
            )
            .after("60"),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server
        .client
        .computers(&session)
        .unwrap()
        .with_retries(retries())
        .unwrap();
    let reference = computers.reference(ID).unwrap();
    let row = computers
        .get(&reference, &CancellationToken::new())
        .await
        .unwrap();
    assert!(!row.awake() && !row.stopped());
    assert_eq!(row.data().extra["future_field"], 17);
    assert!(!format!("{row:?}").contains(PRIVATE));
    let error = computers
        .get(&reference, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(429));
    assert_eq!(error.code(), Some("future_quota_code"));
    assert_eq!(error.retry_after(), Some(Duration::from_secs(60)));
    assert!(!format!("{error:?} {error}").contains(PRIVATE));
    assert_eq!(server.script.seen.lock().unwrap().len(), 4); // Don't shorten Retry-After to fit the bound.
}

#[tokio::test]
async fn ambiguous_command_followed_by_refusal_remains_unknown_and_never_gets_a_new_key() {
    let (server, session) = mocked(
        vec![
            Step::json(
                503,
                json!({"error":{"code":"unavailable","message":PRIVATE}}),
            ),
            Step::json(
                401,
                json!({"error":{"code":"signed_out","message":PRIVATE}}),
            ),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server
        .client
        .computers(&session)
        .unwrap()
        .with_retries(retries())
        .unwrap();
    let request = computers
        .command_request(
            &computers.reference(ID).unwrap(),
            CommandOptions {
                command: PRIVATE.into(),
                ..Default::default()
            },
        )
        .unwrap();
    let error = computers
        .execute(&request, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(&error,Error::Uncertain {key:Some(key),..} if key==request.key().as_str()));
    assert_eq!(error.status(), Some(401));
    assert_eq!(error.code(), Some("signed_out"));
    assert!(!format!("{request:?} {error:?} {error}").contains(PRIVATE));
    let seen = server.script.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].key, seen[1].key);
    assert_eq!(seen[0].body, seen[1].body);
}

#[tokio::test]
async fn explicit_intents_send_once_and_empty_posts_have_a_length() {
    let fail = || {
        Step::json(
            503,
            json!({"error":{"code":"unavailable","message":PRIVATE}}),
        )
    };
    let (server, session) =
        mocked(vec![fail(), fail(), fail(), fail()], Duration::from_secs(1)).await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let command = computers.command_reference(&reference, COMMAND).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        computers.stop(&reference, &cancel).await,
        Err(Error::Uncertain { .. })
    ));
    assert!(matches!(
        computers.wake(&reference, &cancel).await,
        Err(Error::Uncertain { .. })
    ));
    assert!(matches!(
        computers.cancel_command(&command, &cancel).await,
        Err(Error::Uncertain { .. })
    ));
    assert!(matches!(
        computers.delete(&reference, &cancel).await,
        Err(Error::Uncertain { .. })
    ));
    let seen = server.script.seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    for request in seen.iter().filter(|r| r.method == "POST") {
        assert_eq!(request.length.as_deref(), Some("0"));
        assert!(request.body.is_empty());
    }
    assert!(seen[3].uri.ends_with("?kind=cloud"));
}

#[tokio::test]
async fn reserved_names_and_foreign_or_changed_records_are_refused_before_network() {
    let (server, session) = mocked(vec![], Duration::from_secs(1)).await;
    let computers = server.client.computers(&session).unwrap();
    for id in ["capabilities", "my-laptop", "../capabilities", "cmp_123"] {
        assert!(matches!(computers.reference(id), Err(Error::InvalidInput)));
    }
    let reference = computers.reference(ID).unwrap();
    let mut value = serde_json::to_value(&reference).unwrap();
    value["id"] = "capabilities".into();
    assert!(matches!(
        computers
            .delete(
                &serde_json::from_value(value).unwrap(),
                &CancellationToken::new()
            )
            .await,
        Err(Error::InvalidInput)
    ));
    let request = computers.create_request(CreateOptions::default()).unwrap();
    let mut value = serde_json::to_value(request).unwrap();
    value["body"]["idempotency_key"] = "changed".into();
    assert!(matches!(
        computers
            .create(
                &serde_json::from_value(value).unwrap(),
                &CancellationToken::new()
            )
            .await,
        Err(Error::InvalidInput)
    ));
    let mut value = serde_json::to_value(reference.clone()).unwrap();
    value["account"] = "another-account".into();
    assert!(matches!(
        computers
            .get(
                &serde_json::from_value(value).unwrap(),
                &CancellationToken::new()
            )
            .await,
        Err(Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    let credential = computers
        .credential_reference(&reference, "00000000-0000-0000-0000-000000000001")
        .unwrap();
    let mut value = serde_json::to_value(&credential).unwrap();
    value["computer"]["account"] = json!("another-account");
    assert!(matches!(
        computers
            .get_credential(
                &serde_json::from_value(value).unwrap(),
                &CancellationToken::new()
            )
            .await,
        Err(Error::Session(openagents_sdk::Error::WrongAccount))
    ));
    assert!(server.script.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn local_wait_timeout_and_cancellation_never_send_remote_cancel() {
    let (server, session) = mocked(
        vec![
            Step::json(200, computer("new_future_state")),
            Step::json(200, command("future_command_state")),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let command = computers.command_reference(&reference, COMMAND).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        computers
            .wait(
                &reference,
                WaitFor::Awake,
                Duration::from_millis(30),
                &cancel
            )
            .await,
        Err(Error::WaitTimedOut)
    ));
    assert!(matches!(
        computers
            .wait_command(&command, Duration::from_millis(30), &cancel)
            .await,
        Err(Error::WaitTimedOut)
    ));
    cancel.cancel();
    assert!(matches!(
        computers.get(&reference, &cancel).await,
        Err(Error::Cancelled)
    ));
    assert!(
        server
            .script
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.method == "GET")
    );
}

fn output(stdout: &str, stderr: &str, out: u64, err: u64, terminal: bool) -> Value {
    json!({"command":command(if terminal {"succeeded"}else{"running"}),"encoding":"base64","stdout":stdout,"stderr":stderr,"next_cursor":{"stdout":out,"stderr":err},"gaps":{"stdout":false,"stderr":false},"terminal":terminal,"has_more":!terminal})
}
#[tokio::test]
async fn output_reconnect_restores_exact_byte_cursors_and_rejects_bad_pages_without_advancing() {
    let mut bad = output("", "", 2, 1, false);
    bad["gaps"]["stdout"] = true.into();
    let (server, session) = mocked(
        vec![
            Step::json(200, output("AP8=", "gA==", 2, 1, false)),
            Step::json(200, bad),
            Step::json(200, output("eA==", "", 3, 1, true)),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers
        .command_reference(&computers.reference(ID).unwrap(), COMMAND)
        .unwrap();
    let cancel = CancellationToken::new();
    let mut reader = computers.output(&reference).unwrap();
    let page = reader
        .next_page(&computers, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.stdout, [0, 255]);
    assert_eq!(page.stderr, [128]);
    let mut reader: openagents_sdk::computers::OutputReader =
        serde_json::from_slice(&serde_json::to_vec(&reader).unwrap()).unwrap();
    assert!(matches!(
        reader.next_page(&computers, &cancel).await,
        Err(Error::InvalidResponse)
    ));
    let page = reader
        .next_page(&computers, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.stdout, b"x");
    assert_eq!(page.command.outcome(), Some(CommandOutcome::Succeeded));
    assert!(
        reader
            .next_page(&computers, &cancel)
            .await
            .unwrap()
            .is_none()
    );
    let seen = server.script.seen.lock().unwrap();
    assert!(seen[1].uri.contains("stdout=2&stderr=1"));
    assert_eq!(seen[1].uri, seen[2].uri);
}

#[tokio::test]
async fn redirects_and_oversized_or_invalid_output_never_expose_content() {
    let mut redirect = Step::json(307, json!({"private":PRIVATE}));
    redirect
        .headers
        .push(("location".into(), "http://127.0.0.1:1/steal".into()));
    let mut large = Step::json(200, json!({}));
    large.body = vec![b'x'; 1024 * 1024 + 1];
    let (server, session) = mocked(
        vec![
            redirect,
            large,
            Step::json(200, output("not-base64!", "", 0, 0, true)),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        computers.get(&reference, &cancel).await,
        Err(Error::Redirect)
    ));
    assert!(matches!(
        computers.get(&reference, &cancel).await,
        Err(Error::TooLarge)
    ));
    let mut reader = computers
        .output(&computers.command_reference(&reference, COMMAND).unwrap())
        .unwrap();
    assert!(matches!(
        reader.next_page(&computers, &cancel).await,
        Err(Error::InvalidResponse)
    ));
    assert_eq!(server.script.seen.lock().unwrap().len(), 3);
}

fn file_reply(bytes: &[u8]) -> Step {
    let digest = format!("{:x}", Sha256::digest(bytes));
    Step {
        status: 200,
        body: bytes.into(),
        headers: vec![
            ("x-content-sha256".into(), digest.clone()),
            ("etag".into(), format!("\"{digest}\"")),
            ("cache-control".into(), "no-store".into()),
        ],
        delay: Duration::ZERO,
    }
}
#[tokio::test]
async fn uncertain_binary_write_is_reconciled_by_reading_without_upload_retry() {
    let bytes = b"\0\xffprivate-command-file-and-error-fixture";
    let digest = format!("{:x}", Sha256::digest(bytes));
    let (server, session) = mocked(
        vec![
            Step::json(200, capabilities(true)),
            Step::json(
                503,
                json!({"error":{"code":"unavailable","message":PRIVATE}}),
            ),
            Step::json(200, capabilities(true)),
            file_reply(bytes),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let cancel = CancellationToken::new();
    let intent = computers
        .write_intent(&reference, "nested/private.bin", bytes)
        .unwrap();
    assert!(!intent.overwrites());
    let saved = serde_json::to_vec(&intent).unwrap();
    assert!(!String::from_utf8_lossy(&saved).contains(PRIVATE));
    assert!(matches!(
        computers.write_file(&intent, bytes, &cancel).await,
        Err(Error::Uncertain { .. })
    ));
    let restored = serde_json::from_slice(&saved).unwrap();
    let receipt = computers
        .reconcile_write(&restored, &cancel)
        .await
        .unwrap()
        .unwrap();
    assert!(receipt.observed_matching);
    let seen = server.script.seen.lock().unwrap();
    assert_eq!(seen.iter().filter(|r| r.method == "PUT").count(), 1);
    let upload = &seen[1];
    assert_eq!(upload.body, bytes);
    assert_eq!(
        upload.length.as_deref(),
        Some(bytes.len().to_string().as_str())
    );
    assert_eq!(upload.hash.as_deref(), Some(digest.as_str()));
    assert!(upload.uri.contains("overwrite=false"));
}

#[tokio::test]
async fn missing_file_capability_and_bad_checksums_are_refused() {
    let mut bad = file_reply(b"binary");
    bad.headers.retain(|(name, _)| name != "cache-control");
    let (server, session) = mocked(
        vec![
            Step::json(200, capabilities(false)),
            Step::json(200, capabilities(true)),
            bad,
            Step::json(200, capabilities(true)),
            file_reply(b"other"),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        computers.read_file(&reference, "safe.bin", &cancel).await,
        Err(Error::Unsupported)
    ));
    assert!(matches!(
        computers.read_file(&reference, "safe.bin", &cancel).await,
        Err(Error::InvalidResponse)
    ));
    let intent = computers
        .write_intent(&reference, "safe.bin", b"wanted")
        .unwrap();
    assert!(
        computers
            .reconcile_write(&intent, &cancel)
            .await
            .unwrap()
            .is_none()
    );
    for path in ["../private", "/absolute", "nested/../private", "a\\b"] {
        assert!(matches!(
            computers.write_intent(&reference, path, b""),
            Err(Error::InvalidInput)
        ));
    }
    assert_eq!(server.script.seen.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn cancellation_during_dispatch_is_unknown_and_does_not_start_another_command() {
    let (server, session) = mocked(
        vec![Step::json(202, command("running")).delayed()],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let request = computers
        .command_request(
            &computers.reference(ID).unwrap(),
            CommandOptions {
                command: PRIVATE.into(),
                ..Default::default()
            },
        )
        .unwrap();
    let cancel = CancellationToken::new();
    let later = cancel.clone();
    let seen = server.script.seen.clone();
    tokio::spawn(async move {
        loop {
            if !seen.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        later.cancel();
    });
    let error = computers.execute(&request, &cancel).await.unwrap_err();
    assert!(
        matches!(&error,Error::Uncertain {cause,..} if matches!(cause.as_ref(),Error::Cancelled))
    );
    assert_eq!(server.script.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn oversized_decoded_output_and_file_checksum_mismatch_fail_closed() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let (server, session) = mocked(
        vec![
            Step::json(
                200,
                output(&STANDARD.encode(vec![0; 65537]), "", 65537, 0, true),
            ),
            Step::json(200, capabilities(true)),
            {
                let mut reply = file_reply(b"expected");
                reply.body = b"changed!".to_vec();
                reply
            },
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let cancel = CancellationToken::new();
    let mut reader = computers
        .output(&computers.command_reference(&reference, COMMAND).unwrap())
        .unwrap();
    assert!(matches!(
        reader.next_page(&computers, &cancel).await,
        Err(Error::TooLarge)
    ));
    assert!(matches!(
        computers.read_file(&reference, "file.bin", &cancel).await,
        Err(Error::InvalidResponse)
    ));
}

#[tokio::test]
async fn empty_binary_upload_and_cloud_pagination_use_the_published_contract() {
    let empty_hash = format!("{:x}", Sha256::digest([]));
    let (server, session) = mocked(
        vec![
            Step::json(
                200,
                json!({"computers":[computer("awake")],"next_cursor":"next/page"}),
            ),
            Step::json(200, json!({"computers":[],"next_cursor":null})),
            Step::json(200, capabilities(true)),
            Step::json(200, json!({"size":0,"sha256":empty_hash})),
        ],
        Duration::from_secs(1),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let cancel = CancellationToken::new();
    let first = computers.list(1, None, &cancel).await.unwrap();
    assert_eq!(first.computers.len(), 1);
    assert!(
        computers
            .list(1, first.next_cursor.as_deref(), &cancel)
            .await
            .unwrap()
            .next_cursor
            .is_none()
    );
    let intent = computers
        .write_intent(&first.computers[0].reference(), "empty.bin", b"")
        .unwrap();
    computers.write_file(&intent, b"", &cancel).await.unwrap();
    let seen = server.script.seen.lock().unwrap();
    assert!(seen[0].uri.contains("kind=cloud"));
    assert!(seen[1].uri.contains("after=next%2Fpage"));
    assert_eq!(seen[3].length.as_deref(), Some("0"));
    assert!(seen[3].body.is_empty());
}

#[tokio::test]
async fn credential_requests_keep_the_account_command_and_retry_identity() {
    let row = json!({"id":"00000000-0000-0000-0000-000000000002","computer":ID,"source":"00000000-0000-0000-0000-000000000001","rule":"messages","command_key":"one-command","state":"prepared","created_unix":1,"expires_unix":301,"egress_profile":format!("model-v1@sha256:{}","a".repeat(64))});
    let (server, session) = mocked(
        vec![
            Step::json(201, row.clone()),
            Step::json(201, row.clone()),
            Step::json(200, row),
            Step {
                status: 204,
                body: vec![],
                headers: vec![],
                delay: Duration::ZERO,
            },
        ],
        Duration::from_secs(5),
    )
    .await;
    let computers = server.client.computers(&session).unwrap();
    let reference = computers.reference(ID).unwrap();
    let command = computers
        .command_request(
            &reference,
            CommandOptions {
                command: "true".into(),
                idempotency_key: Some("one-command".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let request = computers
        .credential_request(
            &command,
            "00000000-0000-0000-0000-000000000001",
            "messages",
            300,
        )
        .unwrap();
    let saved = serde_json::to_vec(&request).unwrap();
    let cancel = CancellationToken::new();
    let first = computers
        .prepare_credential(&request, &cancel)
        .await
        .unwrap();
    let final_command = computers
        .command_with_credential(&command, &first, "OA_PARAM_MODEL_REF")
        .unwrap();
    assert_eq!(final_command.key(), command.key());
    assert_eq!(
        final_command.options().env.as_ref().unwrap()["OA_PARAM_MODEL_REF"],
        first.reference().placeholder()
    );
    let retry = computers
        .prepare_credential(&serde_json::from_slice(&saved).unwrap(), &cancel)
        .await
        .unwrap();
    assert_eq!(first.reference(), retry.reference());
    computers
        .get_credential(first.reference(), &cancel)
        .await
        .unwrap();
    computers
        .revoke_credential(first.reference(), &cancel)
        .await
        .unwrap();
    let seen = server.script.seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[0].body, seen[1].body);
}
