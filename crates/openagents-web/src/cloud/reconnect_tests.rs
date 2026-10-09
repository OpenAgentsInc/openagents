//! WEB-17 reconnect acceptance for resident observation streams: a browser
//! `EventSource` that drops (network loss, a suspended tab, a server restart)
//! reconnects with `Last-Event-ID`. The resumed stream must report a change
//! made while detached, stay quiet when nothing changed, and retire instead
//! of failing when the viewer can no longer be admitted, so the page never
//! keeps looking current.

use super::*;

fn watch_url(body: &str) -> String {
    body.split("sse-connect=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("observed page names its watch stream")
        .replace("&amp;", "&")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_watch_resumes_from_last_event_id_and_retires_on_lost_admission() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let list = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/hosts/resident/tasks",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    assert!(list.body.contains("sse-close=\"retire\""));
    let watch = watch_url(&list.body);

    // A first connection names the snapshot it observed.
    let fresh = first_event(&fixture.site, &watch, &cookies, None).await;
    assert_eq!(fresh.status, StatusCode::OK, "{}", fresh.body);
    assert_eq!(fresh.headers[header::CACHE_CONTROL], "no-store, private");
    assert_eq!(fresh.headers["x-accel-buffering"], "no");
    assert!(fresh.body.contains("event: refresh"), "{}", fresh.body);
    let id = event_id(&fresh.body)
        .expect("refresh carries an id")
        .to_owned();
    assert!(id.starts_with("v1:"));

    // Reconnecting to an unchanged snapshot reports nothing new.
    let unchanged = first_event(&fixture.site, &watch, &cookies, Some(&id)).await;
    assert_eq!(unchanged.status, StatusCode::OK);
    assert!(
        unchanged.body.starts_with(": canonical standing checked"),
        "{}",
        unchanged.body
    );

    // A snapshot that changed while detached surfaces as a gap, not silence.
    let (prefix, digest) = id.rsplit_once(':').unwrap();
    let older = format!(
        "{prefix}:{}",
        if digest.starts_with('0') {
            "1".repeat(64)
        } else {
            "0".repeat(64)
        }
    );
    let gap = first_event(&fixture.site, &watch, &cookies, Some(&older)).await;
    assert_eq!(gap.status, StatusCode::OK);
    assert!(gap.body.contains("event: gap"), "{}", gap.body);
    assert!(gap.body.contains("changed while detached"));
    assert_eq!(event_id(&gap.body), Some(id.as_str()));

    // An id from another stream cannot resume this one.
    let foreign = first_event(
        &fixture.site,
        &watch,
        &cookies,
        Some(&format!("v1:{}:{}", "a".repeat(64), "b".repeat(64))),
    )
    .await;
    assert_eq!(foreign.status, StatusCode::OK);
    assert!(foreign.body.contains("event: retire"), "{}", foreign.body);

    // Another account reconnecting with this id retires without content.
    let bob = login(&fixture, "bob").await;
    let other = first_event(&fixture.site, &watch, &bob, Some(&id)).await;
    assert_eq!(other.status, StatusCode::OK);
    assert!(other.body.contains("event: retire"));
    assert!(!other.body.contains("Synthetic resident task"));

    // After native revocation a reconnect retires.
    native.authority.revoke(&native.device, now()).unwrap();
    let revoked = first_event(&fixture.site, &watch, &cookies, Some(&id)).await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert!(revoked.body.contains("event: retire"), "{}", revoked.body);
    assert!(!revoked.body.contains("Synthetic resident task"));
    // A first connection after revocation is refused or retires at once.
    let refused = first_event(&fixture.site, &watch, &cookies, None).await;
    assert!(
        matches!(
            refused.status,
            StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
        ) || refused.body.contains("event: retire"),
        "{} {}",
        refused.status,
        refused.body
    );
    assert!(!refused.body.contains("Synthetic resident task"));

    // A signed-out browser reconnecting gets the same retirement.
    let expired = first_event(&fixture.site, &watch, &Cookies(BTreeMap::new()), Some(&id)).await;
    assert_eq!(expired.status, StatusCode::OK);
    assert!(expired.body.contains("event: retire"));
    native.stop().await;
}
