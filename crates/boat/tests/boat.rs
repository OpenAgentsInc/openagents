// Boat-specific behaviour added on top of the coder-box port: retries, the org
// header, key redaction, streaming and detached exec, and recorded responses.
mod support;

use std::path::Path;

use boat::{ApiKey, CommandFrame, Error, RetryPolicy, models::*};
use sha2::{Digest, Sha256};
use support::{Reply, serve_sequence};

const OK_SANDBOXES: &[u8] = br#"{"ok":true,"type":"sandbox.list","sandboxes":[],"pageInfo":{"hasMore":false,"limit":20,"nextCursor":null}}"#;
const CREATED: &[u8] = br#"{"ok":true,"type":"sandbox.created","status":"provisioning","sandbox":{"id":"bx_23456789","name":"t","state":"provisioning","desktopAvailable":false,"snapshotAvailable":false}}"#;

#[tokio::test]
async fn reads_retry_on_429_and_5xx_then_succeed() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(429, &[("retry-after", "0")], b""),
            Reply::new(503, &[], b""),
            Reply::new(200, &[], OK_SANDBOXES),
        ],
        |b| b,
    )
    .await;
    client
        .sandboxes(&SandboxesParams::default())
        .await
        .expect("third attempt succeeds");
    assert_eq!(job.await.expect("job").len(), 3);
}

#[tokio::test]
async fn reads_do_not_retry_client_errors() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(404, &[], b""),
            Reply::new(200, &[], OK_SANDBOXES),
        ],
        |b| b,
    )
    .await;
    assert!(matches!(
        client.sandboxes(&SandboxesParams::default()).await,
        Err(Error::Api(e)) if e.status.as_u16() == 404
    ));
    drop(client);
    assert_eq!(job.await.expect("job").len(), 1);
}

#[tokio::test]
async fn retries_stop_at_the_policy_limit() {
    let (client, job) = serve_sequence((0..4).map(|_| Reply::new(500, &[], b"")).collect(), |b| {
        b.retry(RetryPolicy {
            max_retries: 1,
            ..support::fast_retries()
        })
    })
    .await;
    assert!(client.me().await.is_err());
    drop(client);
    assert_eq!(job.await.expect("job").len(), 2);
}

#[tokio::test]
async fn a_long_retry_after_is_returned_not_waited_out() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(429, &[("retry-after", "3600")], b""),
            Reply::new(200, &[], OK_SANDBOXES),
        ],
        |b| b,
    )
    .await;
    let error = client
        .sandboxes(&SandboxesParams::default())
        .await
        .expect_err("429");
    assert!(matches!(&error, Error::Api(e) if e.retry_after.as_deref() == Some("3600")));
    drop(client);
    assert_eq!(job.await.expect("job").len(), 1);
}

#[tokio::test]
async fn keyed_create_retries_with_the_same_key_and_body() {
    let (client, job) = serve_sequence(
        vec![Reply::new(502, &[], b""), Reply::new(202, &[], CREATED)],
        |b| b,
    )
    .await;
    client
        .create(&CreateParams {
            idempotency_key: Some("job-1".into()),
            body: Some(CreateSandboxRequest {
                no_env: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .expect("create");
    let seen = job.await.expect("job");
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].headers["idempotency-key"], "job-1");
    assert_eq!(seen[1].headers["idempotency-key"], "job-1");
    assert_eq!(seen[0].body, seen[1].body);
}

#[tokio::test]
async fn unkeyed_create_is_sent_once() {
    let (client, job) = serve_sequence(
        vec![Reply::new(503, &[], b""), Reply::new(202, &[], CREATED)],
        |b| b,
    )
    .await;
    assert!(client.create(&CreateParams::default()).await.is_err());
    drop(client);
    assert_eq!(job.await.expect("job").len(), 1);
}

#[tokio::test]
async fn commands_prompts_and_stops_are_never_retried() {
    let failing = || {
        (0..3)
            .map(|_| Reply::new(503, &[], b""))
            .collect::<Vec<_>>()
    };
    let (client, job) = serve_sequence(failing(), |b| b).await;
    assert!(
        client
            .command(&CommandParams {
                sandbox_id: "bx_23456789".into(),
                body: CommandRequest {
                    command: "true".into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .is_err()
    );
    assert!(
        client
            .prompt(&PromptParams {
                sandbox_id: "bx_23456789".into(),
                ..Default::default()
            })
            .await
            .is_err()
    );
    assert!(
        client
            .stop(&StopParams {
                sandbox_id: "bx_23456789".into(),
                ..Default::default()
            })
            .await
            .is_err()
    );
    drop(client);
    let seen = job.await.expect("job");
    assert_eq!(
        seen.iter().map(|r| r.method.as_str()).collect::<Vec<_>>(),
        ["POST", "POST", "POST"]
    );
    // A body-less POST still says its length (Google's front end answers
    // 411 otherwise).
    assert_eq!(
        seen[2].headers.get("content-length").map(String::as_str),
        Some("0")
    );
}

#[tokio::test]
async fn direct_failure_is_reported_as_may_be_running() {
    let body = br#"{"ok":false,"type":"sandbox.error","status":502,"code":"boat_direct_failed","message":"x","requestId":"r","error":{"code":"boat_direct_failed","message":"x","status":502}}"#;
    let (client, job) = serve_sequence(vec![Reply::new(502, &[], body)], |b| b).await;
    let error = client
        .exec_detached(
            "bx_23456789",
            CommandRequest {
                command: "sleep 100".into(),
                ..Default::default()
            },
        )
        .await
        .expect_err("502");
    assert!(error.may_be_running());
    drop(client);
    assert_eq!(job.await.expect("job").len(), 1);
}

#[tokio::test]
async fn client_org_goes_only_to_org_scoped_operations() {
    let (client, job) = serve_sequence(
        vec![
            Reply::new(200, &[], OK_SANDBOXES),
            Reply::new(200, &[], OK_SANDBOXES),
            Reply::new(418, &[], b""),
        ],
        |b| b.org("team-1"),
    )
    .await;
    client
        .sandboxes(&SandboxesParams::default())
        .await
        .expect("list");
    client
        .sandboxes(&SandboxesParams {
            x_boat_org: Some("team-2".into()),
            ..Default::default()
        })
        .await
        .expect("list");
    let _ = client
        .get(&GetParams {
            sandbox_id: "bx_23456789".into(),
            ..Default::default()
        })
        .await;
    let seen = job.await.expect("job");
    assert_eq!(seen[0].headers["x-boat-org"], "team-1");
    assert_eq!(seen[1].headers["x-boat-org"], "team-2");
    assert!(!seen[2].headers.contains_key("x-boat-org"));
}

#[test]
fn the_api_key_never_reaches_formatted_output() {
    let secret = "boat_supersecretvalue123";
    let key = ApiKey::new(format!("  {secret}\n")).expect("key");
    assert!(key.is_boat_key());
    assert!(!format!("{key:?} {key}").contains("supersecret"));
    let client = boat::Client::builder(key.clone()).build().expect("client");
    assert!(!format!("{client:?}").contains("supersecret"));
    let builder = boat::Client::builder(key);
    assert!(!format!("{builder:?}").contains("supersecret"));
    assert!(ApiKey::new("   ").is_err());
    assert!(ApiKey::new("boat_a b").is_err());
    assert!(boat::Client::new("").is_err());
}

#[tokio::test]
async fn errors_never_contain_the_key_or_server_text() {
    let body = br#"{"ok":false,"type":"sandbox.error","status":401,"code":"unauthorized","message":"bad key boat_supersecret","requestId":"r","error":{"code":"unauthorized","message":"bad key boat_supersecret","status":401}}"#;
    let (client, job) = serve_sequence(vec![Reply::new(401, &[], body)], |b| b).await;
    let error = client.me().await.expect_err("401");
    let text = format!("{error:?} {error}");
    assert!(!text.contains("supersecret") && !text.contains("test-secret"));
    let seen = job.await.expect("job");
    assert_eq!(seen[0].headers["authorization"], "Bearer test-secret");
}

#[tokio::test]
async fn exec_stream_yields_ndjson_frames_in_order() {
    let body = concat!(
        "{\"type\":\"started\"}\n",
        "{\"type\":\"stdout\",\"data\":\"Linux \"}\n",
        "{\"type\":\"stderr\",\"data\":\"warn\"}\r\n",
        "\n",
        "{\"type\":\"stdout\",\"data\":\"box\\n\"}\n",
        "{\"type\":\"future\",\"x\":1}\n",
        "{\"type\":\"exit\",\"exitCode\":0,\"success\":true,\"timedOut\":false}"
    );
    let (client, job) = serve_sequence(
        vec![Reply::new(
            200,
            &[("content-type", "application/x-ndjson")],
            body.as_bytes(),
        )],
        |b| b,
    )
    .await;
    let stream = client
        .exec_stream(
            "bx_23456789",
            CommandRequest {
                command: "uname -a".into(),
                detached: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("stream");
    let output = stream.collect().await.expect("frames");
    assert_eq!(output.stdout, "Linux box\n");
    assert_eq!(output.stderr, "warn");
    assert_eq!(output.exit_code(), Some(0));
    assert!(!format!("{output:?}").contains("Linux"));
    let seen = job.await.expect("job");
    assert_eq!(seen[0].target, "/api/v1/sandboxes/bx_23456789/commands");
    assert_eq!(seen[0].headers["accept"], "application/x-ndjson");
    let sent: serde_json::Value = serde_json::from_slice(&seen[0].body).expect("body");
    assert_eq!(
        sent,
        serde_json::json!({"command":"uname -a","stream":true})
    );
}

#[tokio::test]
async fn exec_stream_reports_error_frames_and_line_limits() {
    let body = b"{\"type\":\"started\"}\n{\"type\":\"error\",\"error\":\"boat_starting\",\"message\":\"m\",\"retryable\":true}\n";
    let (client, _job) = serve_sequence(
        vec![Reply::new(
            200,
            &[("content-type", "application/x-ndjson")],
            body,
        )],
        |b| b,
    )
    .await;
    let mut stream = client
        .exec_stream("bx_23456789", CommandRequest::default())
        .await
        .expect("stream");
    assert!(matches!(
        stream.next().await,
        Ok(Some(CommandFrame::Started))
    ));
    assert!(matches!(
        stream.next().await,
        Ok(Some(CommandFrame::Error {
            retryable: true,
            ..
        }))
    ));
    assert!(matches!(stream.next().await, Ok(None)));

    let long = format!("{{\"type\":\"stdout\",\"data\":\"{}\"}}\n", "x".repeat(200));
    let (client, _job) = serve_sequence(
        vec![Reply::new(
            200,
            &[("content-type", "application/x-ndjson")],
            long.as_bytes(),
        )],
        |b| b,
    )
    .await;
    let mut stream = client
        .exec_stream_with_limit("bx_23456789", CommandRequest::default(), 64)
        .await
        .expect("stream");
    assert!(matches!(stream.next().await, Err(Error::StreamLineTooLong)));
}

#[tokio::test]
async fn exec_stream_accepts_a_synchronous_json_answer() {
    let body = br#"{"ok":true,"type":"command.finished","success":false,"exitCode":3,"stdout":"out","stderr":"","timedOut":false}"#;
    let (client, _job) = serve_sequence(
        vec![Reply::new(
            200,
            &[("content-type", "application/json")],
            body,
        )],
        |b| b,
    )
    .await;
    let output = client
        .exec_stream("bx_23456789", CommandRequest::default())
        .await
        .expect("stream")
        .collect()
        .await
        .expect("frames");
    assert_eq!(output.stdout, "out");
    assert_eq!(output.exit_code(), Some(3));
}

#[tokio::test]
async fn exec_detached_returns_the_process_and_wait_command_polls_it() {
    let started = br#"{"ok":true,"type":"command.started","success":true,"processId":7,"pid":70,"command":"sleep 1","startedAt":"2026-10-02T00:00:00Z"}"#;
    let running = br#"{"ok":true,"type":"command.status","processId":7,"pid":70,"success":true,"status":"running","running":true,"stdout":"","stderr":""}"#;
    let done = br#"{"ok":true,"type":"command.status","processId":7,"pid":70,"success":true,"status":"exited","running":false,"exitCode":0,"stdout":"","stderr":""}"#;
    let (client, job) = serve_sequence(
        vec![
            Reply::new(200, &[], started),
            Reply::new(200, &[], running),
            Reply::new(200, &[], done),
        ],
        |b| b,
    )
    .await;
    let process = client
        .exec_detached(
            "bx_23456789",
            CommandRequest {
                command: "sleep 1".into(),
                ..Default::default()
            },
        )
        .await
        .expect("started");
    assert_eq!(process.process_id, 7);
    let options = boat::WaitOptions {
        interval: std::time::Duration::from_millis(1),
        ..Default::default()
    };
    let status = client
        .wait_command("bx_23456789", process.process_id, &options)
        .await
        .expect("finished");
    assert!(!status.running);
    let seen = job.await.expect("job");
    let sent: serde_json::Value = serde_json::from_slice(&seen[0].body).expect("body");
    assert_eq!(sent["detached"], true);
    assert_eq!(seen[2].target, "/api/v1/sandboxes/bx_23456789/commands/7");
}

#[test]
fn the_pinned_spec_matches_its_recorded_digest() {
    let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/boat-v1.yaml"))
        .expect("spec");
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), boat::SPEC_SHA256);
    assert_eq!(boat::HOSTED_BASE_URL, "https://boat.dev/api/v1");
    assert!(boat::BASE_URL.starts_with("https://oa-boat-"));
}

#[test]
fn hosted_boat_is_refused_unless_opted_in() {
    // Only this test touches BOAT_HOSTED in this binary.
    unsafe { std::env::remove_var("BOAT_HOSTED") };
    let key = || boat::ApiKey::new("k").expect("key");
    for base in [
        "https://boat.dev/api/v1",
        "https://api.boat.dev/v1",
        "https://ascii.dev/api/box/v1",
    ] {
        let e = boat::Client::builder(key())
            .base_url(base)
            .build()
            .unwrap_err();
        assert!(e.to_string().contains("BOAT_HOSTED"), "{base}: {e}");
    }
    assert!(boat::Client::builder(key()).build().is_ok());
    assert!(
        boat::Client::builder(key())
            .base_url("https://notboat.dev/api/v1")
            .build()
            .is_ok()
    );
    unsafe { std::env::set_var("BOAT_HOSTED", "1") };
    assert!(
        boat::Client::builder(key())
            .base_url(boat::HOSTED_BASE_URL)
            .build()
            .is_ok()
    );
    unsafe { std::env::remove_var("BOAT_HOSTED") };
}

#[test]
fn every_operation_has_a_retry_class_and_only_reads_and_keyed_creates_retry() {
    let inventory: Vec<serde_json::Value> =
        serde_json::from_str(boat::OPERATIONS).expect("inventory");
    assert_eq!(inventory.len(), 69);
    for op in &inventory {
        let expected = match (op["method"].as_str(), op["operation_id"].as_str()) {
            (Some("GET"), _) => "read",
            (_, Some("create" | "fork")) => "if_idempotency_key",
            _ => "never",
        };
        assert_eq!(op["retry"], expected, "{}", op["operation_id"]);
    }
    for new in [
        "steer",
        "usage",
        "conversations",
        "share",
        "deleteSandboxSnapshots",
        "listOrganizations",
        "setActiveOrganization",
        "createScopedApiKey",
        "rotateApiKey",
        "revokeApiKey",
    ] {
        assert!(
            inventory.iter().any(|op| op["operation_id"] == new),
            "{new}"
        );
    }
}

/// Redacted read-only responses recorded from the live API by
/// `schema/capture.py` must decode into the operation's response type.
#[test]
fn recorded_live_responses_decode() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/recorded");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).expect("recorded fixtures") {
        let path = entry.expect("entry").path();
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        let response = record["response"].clone();
        let id = record["operation_id"].as_str().expect("operation id");
        let decoded = match id {
            "me" => serde_json::from_value::<MeResponse>(response).map(|_| ()),
            "listOrganizations" => serde_json::from_value::<OrgListResponse>(response).map(|_| ()),
            "limits" => serde_json::from_value::<LimitsResponse>(response).map(|_| ()),
            "sandboxes" => serde_json::from_value::<SandboxListResponse>(response).map(|_| ()),
            "environments" => {
                serde_json::from_value::<SandboxEnvironmentListResponse>(response).map(|_| ())
            }
            "listNamedSnapshots" => {
                serde_json::from_value::<NamedSnapshotListResponse>(response).map(|_| ())
            }
            "apiKeys" => serde_json::from_value::<ApiKeysResponse>(response).map(|_| ()),
            other => panic!("no decoder for {other}"),
        };
        decoded.unwrap_or_else(|e| panic!("{id}: {e}"));
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains("requestId"), "{id} keeps a request id");
        count += 1;
    }
    assert_eq!(count, 7);
}
