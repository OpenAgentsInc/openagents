mod common;

use std::sync::atomic::Ordering;

use archil::{BuildImageOptions, BuildImageRequest, Error, ImageFailureReason, ImageStatus};
use common::*;
use serde_json::{Value, json};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer};

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn image(status: &str, reason: Option<&str>) -> Value {
    let mut v = json!({
        "image_id": ID, "source": "docker.io/library/node:24", "private": false, "status": status,
        "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:01:00Z"
    });
    if status == "ready" {
        v["digest"] = "sha256:abc".into();
        v["canonical_source"] = "docker.io/library/node@sha256:abc".into();
    }
    if let Some(r) = reason {
        v["failure_reason"] = r.into();
    }
    v
}

fn accepted(v: Value) -> wiremock::ResponseTemplate {
    status(202, v)
}

#[tokio::test]
async fn get_image_parses_wire_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/images/{ID}")))
        .respond_with(ok(image("ready", None)))
        .mount(&server)
        .await;
    let img = client(&server).images().get(ID).await.unwrap();
    assert_eq!(img.status, ImageStatus::Ready);
    assert_eq!(img.digest.as_deref(), Some("sha256:abc"));
    assert_eq!(
        img.canonical_source.as_deref(),
        Some("docker.io/library/node@sha256:abc")
    );
    assert!(!img.private);
    assert_eq!(img.updated_at.to_rfc3339(), "2026-01-01T00:01:00+00:00");
}

#[tokio::test]
async fn build_returns_at_once_when_already_ready() {
    let server = MockServer::start().await;
    let post = Seq::new(vec![accepted(image("ready", None))]);
    let hits = post.hits.clone();
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .and(body_json(json!({"source": "node:24"})))
        .respond_with(post)
        .mount(&server)
        .await;
    let img = client(&server)
        .images()
        .build(BuildImageRequest::new("node:24"))
        .await
        .unwrap();
    assert_eq!(img.image_id, ID);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn build_polls_until_ready_and_sends_registry_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .and(body_json(json!({"source": "ghcr.io/acme/agent:v3", "registry_auth": {"username": "bot", "password": "pw"}})))
        .respond_with(accepted(image("building", None)))
        .expect(1)
        .mount(&server)
        .await;
    let get = Seq::new(vec![
        ok(image("building", None)),
        ok(image("building", None)),
        ok(image("ready", None)),
    ]);
    let polls = get.hits.clone();
    Mock::given(method("GET"))
        .and(path(format!("/api/images/{ID}")))
        .respond_with(get)
        .mount(&server)
        .await;
    let img = client(&server)
        .images()
        .build(BuildImageRequest::new("ghcr.io/acme/agent:v3").registry_auth("bot", "pw"))
        .await
        .unwrap();
    assert_eq!(img.status, ImageStatus::Ready);
    assert_eq!(polls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn invalid_source_failure_is_a_typed_error_without_retry() {
    let server = MockServer::start().await;
    let post = Seq::new(vec![accepted(image("failed", Some("invalid_source")))]);
    let hits = post.hits.clone();
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .respond_with(post)
        .mount(&server)
        .await;
    let e = client(&server)
        .images()
        .build(BuildImageRequest::new("nope:1"))
        .await
        .unwrap_err();
    let Error::ImageBuild(ref b) = e else {
        panic!("{e:?}")
    };
    assert_eq!(b.reason(), Some(ImageFailureReason::InvalidSource));
    assert_eq!(b.latest.image_id, ID);
    assert_eq!(
        e.to_string(),
        format!("Image {ID} build failed: invalid_source")
    );
    assert_eq!(
        (e.status(), e.code()),
        (Some(409), Some("IMAGE_BUILD_FAILED"))
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interrupted_builds_are_requested_again_up_to_three_times() {
    let server = MockServer::start().await;
    let post = Seq::new(vec![accepted(image("failed", Some("interrupted")))]);
    let hits = post.hits.clone();
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .respond_with(post)
        .mount(&server)
        .await;
    let e = client(&server)
        .images()
        .build(BuildImageRequest::new("x:1"))
        .await
        .unwrap_err();
    assert!(
        matches!(&e, Error::ImageBuild(b) if b.reason() == Some(ImageFailureReason::Interrupted))
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        4,
        "initial request + 3 re-requests"
    );

    let server = MockServer::start().await;
    let post = Seq::new(vec![
        accepted(image("failed", Some("interrupted"))),
        accepted(image("ready", None)),
    ]);
    let hits = post.hits.clone();
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .respond_with(post)
        .mount(&server)
        .await;
    let img = client(&server)
        .images()
        .build(BuildImageRequest::new("x:1"))
        .await
        .unwrap();
    assert_eq!(img.status, ImageStatus::Ready);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn build_times_out_with_a_coded_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .respond_with(accepted(image("building", None)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/images/{ID}")))
        .respond_with(ok(image("building", None)))
        .mount(&server)
        .await;
    let e = client(&server)
        .images()
        .build_with(
            BuildImageRequest::new("slow:1"),
            BuildImageOptions { timeout_seconds: 0 },
        )
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Archil { .. }));
    assert_eq!(
        (e.status(), e.code()),
        (Some(408), Some("IMAGE_BUILD_TIMEOUT"))
    );
    assert_eq!(
        e.to_string(),
        format!("Image {ID} was not ready after 0 seconds")
    );
}

#[tokio::test]
async fn image_build_polling_interval_backs_off() {
    // The first poll waits 2x the poll interval; pins the documented schedule.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/images"))
        .respond_with(accepted(image("building", None)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/images/{ID}")))
        .respond_with(ok(image("ready", None)))
        .mount(&server)
        .await;
    let start = std::time::Instant::now();
    client(&server)
        .images()
        .build(BuildImageRequest::new("x:1"))
        .await
        .unwrap();
    assert!(
        start.elapsed() >= std::time::Duration::from_millis(20),
        "first poll waits 2x the 10ms interval"
    );
}
