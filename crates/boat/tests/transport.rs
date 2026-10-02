mod support;

use boat::{Client, Error, Nullable, models::*};
use serde_json::json;

#[tokio::test]
async fn create_keeps_null_ttl_idempotency_and_billing_scope() {
    let (client, request) = support::serve(202, &[], br#"{"ok":true,"type":"sandbox.created","status":"provisioning","ttlSeconds":null,"sandbox":{"id":"bx_test","name":"test","state":"provisioning","desktopAvailable":false,"snapshotAvailable":false}}"#).await;
    client
        .create(&CreateParams {
            idempotency_key: Some("stable-request-key".into()),
            org: Some("team 1".into()),
            x_boat_org: Some("team-1".into()),
            body: Some(CreateSandboxRequest {
                no_env: Some(true),
                ttl_seconds: Nullable::Null,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .expect("create");
    let request = request.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes?org=team+1");
    assert_eq!(request.headers["authorization"], "Bearer test-secret");
    assert_eq!(request.headers["idempotency-key"], "stable-request-key");
    assert_eq!(request.headers["x-boat-org"], "team-1");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("JSON"),
        json!({"noEnv":true,"ttlSeconds":null})
    );
}

#[test]
fn nullable_round_trips_omitted_null_and_value() {
    for (wire, expected) in [
        (json!({}), Nullable::Unset),
        (json!({"ttlSeconds":null}), Nullable::Null),
        (json!({"ttlSeconds":120}), Nullable::Value(120)),
    ] {
        let update: UpdateSandboxRequest = serde_json::from_value(wire.clone()).expect("decode");
        assert_eq!(update.ttl_seconds, expected);
        assert_eq!(serde_json::to_value(update).expect("encode"), wire);
    }
}

#[tokio::test]
async fn paths_and_queries_cannot_change_the_operation() {
    let (client, request) = support::serve(404, &[], b"missing").await;
    let result = client
        .read_file(&ReadFileParams {
            sandbox_id: "a/b?#".into(),
            path: "/a & b?x=1".into(),
            ..Default::default()
        })
        .await;
    assert!(matches!(result, Err(Error::Api(_))));
    assert_eq!(
        request.await.expect("request").target,
        "/api/v1/sandboxes/a%2Fb%3F%23/files?path=%2Fa+%26+b%3Fx%3D1"
    );
    assert!(
        client
            .get(&GetParams {
                sandbox_id: "..".into(),
                ..Default::default()
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn structured_errors_keep_status_without_exposing_server_text() {
    let body = br#"{"ok":false,"type":"sandbox.error","status":409,"code":"boat_direct_failed","message":"secret-output","requestId":"request-1","error":{"code":"boat_direct_failed","message":"secret-output","status":409}}"#;
    let (client, request) = support::serve(502, &[("retry-after", "3")], body).await;
    let error = client
        .command(&CommandParams {
            sandbox_id: "bx_test".into(),
            body: CommandRequest {
                command: "secret-command".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .expect_err("refused");
    assert!(!format!("{error:?} {error}").contains("secret"));
    if let Error::Api(error) = error {
        assert_eq!(error.status.as_u16(), 502);
        assert_eq!(error.code(), Some("boat_direct_failed"));
        assert_eq!(error.retry_after.as_deref(), Some("3"));
        assert_eq!(error.request_id.as_deref(), Some("request-1"));
    } else {
        panic!("expected API error");
    }
    assert_eq!(request.await.expect("request").method, "POST");
}

#[tokio::test]
async fn redirects_are_not_followed_with_credentials() {
    let (client, request) =
        support::serve(302, &[("location", "http://127.0.0.1:1/steal")], b"").await;
    let error = client.me().await.expect_err("redirect refused");
    assert!(matches!(error, Error::Api(e) if e.status.as_u16()==302));
    request.await.expect("request");
}

#[tokio::test]
async fn binary_downloads_preserve_bytes_and_enforce_limits() {
    let bytes = b"\x00\xffbinary\r\n";
    let (client, request) =
        support::serve(200, &[("content-type", "application/octet-stream")], bytes).await;
    let download = client
        .artifact(&ArtifactParams {
            sandbox_id: "bx_test".into(),
            path: "file.bin".into(),
            ..Default::default()
        })
        .await
        .expect("download");
    assert_eq!(download.content_type(), Some("application/octet-stream"));
    assert_eq!(download.bytes(100).await.expect("bytes"), bytes);
    request.await.expect("request");
    let (client, request) = support::serve(200, &[], bytes).await;
    let download = client
        .get_snapshot_file(&GetSnapshotFileParams {
            snapshot_id: "snapshot".into(),
            ..Default::default()
        })
        .await
        .expect("download");
    assert!(matches!(
        download.bytes(2).await,
        Err(Error::ResponseTooLarge)
    ));
    request.await.expect("request");
}

#[tokio::test]
async fn command_variants_and_unknown_event_fields_decode() {
    for (wire, detached) in [
        (
            json!({"ok":true,"type":"command.finished","success":false,"exitCode":7,"stdout":"","stderr":"failed","timedOut":false}),
            false,
        ),
        (
            json!({"ok":true,"type":"command.started","success":true,"processId":123,"pid":456,"command":"sleep 1","startedAt":"2026-09-05T00:00:00Z"}),
            true,
        ),
    ] {
        let result: CommandResponseBody = serde_json::from_value(wire).expect("command variant");
        assert_eq!(matches!(result, CommandResponseBody::Started(_)), detached);
    }
    let wire = json!({"type":"future.event","id":"id","newField":7,"data":{"anything":[1,2]}});
    let event: SandboxEvent = serde_json::from_value(wire.clone()).expect("event");
    assert_eq!(serde_json::to_value(event).expect("round trip"), wire);
}

#[test]
fn client_and_models_redact_debug_and_reject_credential_urls() {
    let client = Client::new("secret-credential").expect("client");
    assert!(!format!("{client:?}").contains("secret-credential"));
    let request = PromptRequest {
        prompt: "secret-prompt".into(),
        ..Default::default()
    };
    assert!(!format!("{request:?}").contains("secret-prompt"));
    for url in [
        "https://user:password@example.com/api",
        "https://example.com/api?token=secret",
        "http://example.com/api",
    ] {
        assert!(
            Client::builder(boat::ApiKey::new("secret").expect("key"))
                .base_url(url)
                .build()
                .is_err()
        );
    }
}
