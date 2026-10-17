use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Json;
use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use serde_json::{Value, json};

use super::*;

const TOKEN: &str = "sess_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A website whose `/v1/device/token` answers pending until
/// `approve_after` polls, then the token (or `access_denied` when `deny`).
async fn site(approve_after: usize, deny: bool) -> (String, Arc<AtomicUsize>) {
    site_at("/v1", approve_after, deny).await
}

/// [`site`] with device sign-in under `prefix`: `""` is a site from
/// before #11158, which serves only `/device/*`.
async fn site_at(
    prefix: &'static str,
    approve_after: usize,
    deny: bool,
) -> (String, Arc<AtomicUsize>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let counted = polls.clone();
    let router = Router::new()
        .route(
            &format!("{prefix}/device/code"),
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
            &format!("{prefix}/device/token"),
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
            &format!("{prefix}/device/sign-out"),
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
async fn an_older_site_signs_in_at_its_device_paths() {
    let (origin, polls) = site_at("", 1, false).await;
    let started = start(&origin, "Coder", "box").await.unwrap();
    let saved = wait(&origin, &started).await.unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(saved.token(), TOKEN);
    sign_out(&saved).await.unwrap();
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

const KEY: &str = "oa_sk_abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

#[test]
fn a_key_is_read_from_standard_input_never_an_argument() {
    let mut input = std::io::Cursor::new(format!("\n  {KEY}  \n"));
    assert_eq!(read_key(&mut input).unwrap(), KEY);
    let mut wrong = std::io::Cursor::new("sk-ant-api03-not-ours\n");
    let refused = read_key(&mut wrong).unwrap_err();
    assert!(refused.contains("oa_sk_") && !refused.contains("sk-ant"));
    assert!(read_key(&mut std::io::Cursor::new("")).is_err());
}

#[test]
fn the_token_variable_signs_in_with_no_saved_file() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_string())
        }
    };
    let saved = Saved::from_env(env(&[(TOKEN_ENV, KEY)])).unwrap();
    assert_eq!(saved.token(), KEY);
    assert_eq!(saved.origin, DEFAULT_ORIGIN);
    assert!(!saved.expired(u64::MAX - 1));
    assert!(!format!("{saved:?}").contains(KEY));
    let staged = Saved::from_env(env(&[
        (TOKEN_ENV_OLDER, TOKEN),
        (ORIGIN_ENV, "https://staging.example"),
    ]))
    .unwrap();
    assert_eq!(staged.token(), TOKEN);
    assert_eq!(staged.origin, "https://staging.example");
    assert!(Saved::from_env(env(&[(TOKEN_ENV, "nonsense")])).is_none());
    // A token that isn't a login is still a token the caller must not ignore.
    assert!(crate::token_env_set(env(&[(TOKEN_ENV, "nonsense")])));
    assert!(!crate::token_env_set(env(&[(TOKEN_ENV, "  ")])));
    assert!(!crate::token_env_set(env(&[])));
    assert!(Saved::from_env(env(&[])).is_none());
}

#[tokio::test]
async fn a_key_signs_in_as_its_account_and_a_bad_one_is_refused() {
    use axum::routing::get;
    let router = Router::new().route(
        "/v1/keys",
        get(|headers: HeaderMap| async move {
            if headers["authorization"] != format!("Bearer {KEY}").as_str() {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": {"code": "signed_out"}})),
                );
            }
            (
                StatusCode::OK,
                Json(json!({
                    "kind": "signin",
                    "account": {"id": "acct_1", "label": "Octo"},
                    "keys": [
                        {"id": "x", "name": "other", "expires_at": 5, "current": false},
                        {"id": "y", "name": "agent", "expires_at": null, "current": true}
                    ]
                })),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    let saved = sign_in_with_key(&origin, KEY).await.unwrap();
    assert_eq!(saved.account, "acct_1");
    assert_eq!(saved.label, "Octo");
    assert_eq!(saved.expires_at, u64::MAX);
    assert_eq!(saved.token(), KEY);
    let other = format!("oa_sk_{}", "0".repeat(64));
    let refused = sign_in_with_key(&origin, &other).await.unwrap_err();
    assert!(refused.to_string().contains("doesn't work"));
    // Signing out forgets a key here; it isn't ended on the site.
    sign_out(&saved).await.unwrap();
    let folder = tempfile::tempdir().unwrap();
    saved.store(folder.path()).unwrap();
    assert_eq!(Saved::load_file(folder.path()), Some(saved));
}
