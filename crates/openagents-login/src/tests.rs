use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Json;
use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use serde_json::{Value, json};

use super::*;

const TOKEN: &str = "sess_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A website whose `/device/token` answers pending until `approve_after`
/// polls, then the token (or `access_denied` when `deny`).
async fn site(approve_after: usize, deny: bool) -> (String, Arc<AtomicUsize>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let counted = polls.clone();
    let router = Router::new()
        .route(
            "/device/code",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["app"], "Coder");
                assert_eq!(body["computer"], "box");
                Json(json!({
                    "device_code": "dvc_secret", "user_code": "BCDF-GHJK",
                    "verification_uri": "http://127.0.0.1/device",
                    "verification_uri_complete": "http://127.0.0.1/device?code=BCDF-GHJK",
                    "expires_in": 30, "interval": 1,
                }))
            }),
        )
        .route(
            "/device/token",
            post(move |Json(body): Json<Value>| {
                let polls = counted.clone();
                async move {
                    assert_eq!(body["device_code"], "dvc_secret");
                    let n = polls.fetch_add(1, Ordering::SeqCst) + 1;
                    if n < approve_after {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "authorization_pending"})),
                        );
                    }
                    if deny {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "access_denied"})),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(json!({
                            "access_token": TOKEN, "token_type": "Bearer", "expires_in": 2_592_000,
                            "account": {"id": "acct_1", "label": "Octo Local"},
                        })),
                    )
                }
            }),
        )
        .route(
            "/device/sign-out",
            post(|headers: HeaderMap| async move {
                assert_eq!(
                    headers["authorization"].to_str().unwrap(),
                    format!("Bearer {TOKEN}")
                );
                Json(json!({"signed_out": true}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.ok() });
    (origin, polls)
}

#[tokio::test]
async fn a_sign_in_waits_for_approval_and_keeps_the_token_private() {
    let (origin, polls) = site(2, false).await;
    let started = start(&origin, "Coder", "box").await.unwrap();
    assert_eq!(started.user_code, "BCDF-GHJK");
    assert!(!format!("{started:?}").contains("dvc_secret"));
    let saved = wait(&origin, &started).await.unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert_eq!(saved.label, "Octo Local");
    assert_eq!(saved.token(), TOKEN);
    assert!(!format!("{saved:?}").contains(TOKEN));
    assert!(!saved.expired(unix_now()));

    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("coder-new");
    let path = saved.store(&folder).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&folder).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert_eq!(Saved::load(&folder), Some(saved.clone()));

    sign_out(&saved).await.unwrap();
    Saved::forget(&folder).unwrap();
    assert_eq!(Saved::load(&folder), None);
    Saved::forget(&folder).unwrap();
}

#[tokio::test]
async fn deny_ends_the_wait() {
    let (origin, _) = site(1, true).await;
    let started = start(&origin, "Coder", "box").await.unwrap();
    assert_eq!(wait(&origin, &started).await.unwrap_err(), Error::Denied);
}

#[tokio::test]
async fn an_unreachable_site_says_so() {
    let error = start("http://127.0.0.1:9", "Coder", "box")
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Unreachable(_)));
    assert!(error.to_string().contains("Couldn't reach"));
}

#[test]
fn the_origin_is_openagents_com_unless_pointed_at_a_local_or_https_site() {
    assert_eq!(origin_from(|_| None), DEFAULT_ORIGIN);
    assert_eq!(
        origin_from(|_| Some("http://127.0.0.1:4301/".into())),
        "http://127.0.0.1:4301"
    );
    assert_eq!(
        origin_from(|_| Some("http://evil.example".into())),
        DEFAULT_ORIGIN
    );
}

#[test]
fn the_computer_has_a_name() {
    let name = computer_name();
    assert!(!name.is_empty() && name.chars().count() <= 64);
}
