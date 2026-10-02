mod support;
use boat::{Cancellation, Error, WaitOptions, models::*, webhook};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    future::pending,
    time::{Duration, UNIX_EPOCH},
};
use tokio::time::Instant;

#[tokio::test(start_paused = true)]
async fn deadlines_and_preexisting_cancellation_bound_inflight_work() {
    let cancellation = Cancellation::default();
    assert!(matches!(
        cancellation
            .run(
                Instant::now() + Duration::from_secs(2),
                pending::<boat::Result<()>>()
            )
            .await,
        Err(Error::Deadline)
    ));
    cancellation.cancel();
    assert!(matches!(
        cancellation
            .run(Instant::now() + Duration::from_secs(2), async { Ok(()) })
            .await,
        Err(Error::Cancelled)
    ));
}

#[tokio::test]
async fn readiness_rejects_terminal_states_and_accepts_idle() {
    for state in ["idle", "error", "archived"] {
        let body = format!(
            r#"{{"ok":true,"type":"sandbox.info","sandbox":{{"id":"bx_test","name":"test","state":"{state}","desktopAvailable":false,"snapshotAvailable":false}}}}"#
        );
        let (client, job) = support::serve(200, &[], body.as_bytes()).await;
        let result = client
            .wait_until_ready("bx_test", &WaitOptions::default())
            .await;
        assert_eq!(result.is_ok(), state == "idle");
        job.await.expect("request");
    }
}

#[tokio::test]
async fn events_advance_the_cursor_after_each_delivered_event() {
    let (client, job) = support::serve(200, &[], br#"{"ok":true,"type":"events.list","id":"bx_test","events":[{"id":"a","timestamp":10,"type":"future"},{"id":"b","timestamp":11,"type":"response"}]}"#).await;
    let mut events = client
        .stream_events(
            EventsParams {
                sandbox_id: "bx_test".into(),
                ..Default::default()
            },
            WaitOptions::default(),
        )
        .expect("stream");
    assert_eq!(events.next().await.expect("first").id.as_deref(), Some("a"));
    assert_eq!(events.cursor(), Some("MTA6YQ"));
    assert_eq!(
        events.next().await.expect("second").id.as_deref(),
        Some("b")
    );
    assert_eq!(events.cursor(), Some("MTE6Yg"));
    assert!(job.await.expect("request").target.ends_with("sort=asc"));
}

#[test]
fn webhook_verification_rejects_mutation_expiry_and_wrong_delivery() {
    let delivery = "evt_0123456789abcdef0123456789abcdef";
    let timestamp = "1000";
    let body = format!(
        r#"{{"id":"{delivery}","type":"sandbox.ready","createdAt":"2026-09-05T00:00:00Z","data":{{"sandbox":{{"id":"bx_test","name":"test","state":"ready"}},"previousState":"provisioning","state":"ready"}}}}"#
    );
    let secret = "whsec_test";
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC");
    mac.update(format!("{delivery}.{timestamp}.{body}").as_bytes());
    let signature = format!("v1={:x}", mac.finalize().into_bytes());
    let headers = || webhook::Headers {
        delivery,
        timestamp,
        signature: &signature,
    };
    let now = UNIX_EPOCH + Duration::from_secs(1001);
    assert!(
        webhook::verify(
            secret,
            headers(),
            body.as_bytes(),
            now,
            Duration::from_secs(30)
        )
        .is_ok()
    );
    assert!(webhook::verify(secret, headers(), b"changed", now, Duration::from_secs(30)).is_err());
    assert!(
        webhook::verify(
            "wrong",
            headers(),
            body.as_bytes(),
            now,
            Duration::from_secs(30)
        )
        .is_err()
    );
    assert!(
        webhook::verify(
            secret,
            headers(),
            body.as_bytes(),
            now + Duration::from_secs(60),
            Duration::from_secs(30)
        )
        .is_err()
    );
    assert!(
        webhook::verify(
            secret,
            webhook::Headers {
                delivery: "evt_ffffffffffffffffffffffffffffffff",
                ..headers()
            },
            body.as_bytes(),
            now,
            Duration::from_secs(30)
        )
        .is_err()
    );
}

#[test]
fn operation_inventory_matches_every_pinned_path_operation() {
    let spec = include_str!("../schema/boat-v1.yaml");
    let paths = spec.split_once("\npaths:\n").expect("paths").1;
    let inventory: Vec<serde_json::Value> =
        serde_json::from_str(boat::OPERATIONS).expect("inventory");
    let mut path = "";
    let mut seen = Vec::new();
    for line in paths.lines() {
        if line.starts_with("  /") {
            path = line.trim().trim_end_matches(':');
        }
        for method in ["get", "post", "put", "patch", "delete"] {
            if line == format!("    {method}:") {
                seen.push((method.to_uppercase(), path.to_owned()));
            }
        }
    }
    assert_eq!(seen.len(), 69);
    assert_eq!(inventory.len(), seen.len());
    for (method, path) in seen {
        assert_eq!(
            inventory
                .iter()
                .filter(|v| v["method"] == method && v["path"] == path)
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn blocked_deletion_returns_the_record_without_waiting_for_a_timeout() {
    let (client, job) = support::serve(200, &[], br#"{"ok":true,"type":"deletion.operation","operation":{"id":"bdop_test","kind":"box","targetId":"bx_test","reason":"explicit","status":"blocked","attemptCount":1,"requestedAt":"2026-09-05T00:00:00Z","completedAt":null}}"#).await;
    let result = client
        .wait_for_deletion("bdop_test", &WaitOptions::default())
        .await;
    assert!(
        matches!(result, Err(Error::DeletionBlocked(record)) if record.id=="bdop_test" && record.status=="blocked")
    );
    job.await.expect("request");
}
