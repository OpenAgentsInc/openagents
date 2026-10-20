//! WorkOS sign-in over HTTP (#12320 to #12323), against the in-process
//! fake WorkOS: a WorkOS access token becomes our session, WorkOS tokens
//! work as bearers, key sign-ins are dual-written to WorkOS, and `legacy`
//! mode leaves everything as it was.
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nostr::domain::{RelaySigner, Tag};
use oa_auth::workos::fake::{API_KEY, FakeWorkos};
use oa_auth::workos::{Method, Mode};
use sha2::{Digest, Sha256};

use super::*;

const METHODS: &[Method] = &[Method::Github, Method::Password, Method::MagicCode];

fn settings(fake: &FakeWorkos, dir: &std::path::Path, mode: Mode) -> oa_auth::workos::Settings {
    let key = dir.join("workos.key");
    std::fs::write(&key, API_KEY).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut settings = fake.settings(mode, METHODS);
    settings.api_key_file = Some(key);
    settings
}

async fn deploy_with(fake: &FakeWorkos, mode: Mode) -> (Deployment, tempfile::TempDir) {
    let keys = tempfile::tempdir().unwrap();
    let mut accounts = account_config(None);
    accounts.workos = Some(settings(fake, keys.path(), mode));
    (deploy(Some(accounts), true).await, keys)
}

async fn sign_in(d: &Deployment, access: &str) -> (StatusCode, Value) {
    post(
        d,
        "/v1/sessions/workos",
        None,
        &json!({"access_token": access}),
    )
    .await
}

#[tokio::test]
async fn a_workos_sign_in_becomes_one_account_and_its_session() {
    let fake = FakeWorkos::start().await;
    let (d, _keys) = deploy_with(&fake, Mode::Dual).await;
    let user = fake.add_user("pat@example.com", Some("a long password"), true);
    let (access, _) = fake.sign_in(&user);
    let (status, first) = sign_in(&d, &access).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["created"], true);
    assert_eq!(first["workos"]["user_id"], user.as_str());
    assert_eq!(first["workos"]["linked_by"], "new");
    let token = first["token"].as_str().unwrap();
    assert!(token.starts_with("sess_"));
    let account = first["session"]["account"].as_str().unwrap().to_string();
    let (status, view) = get(&d, "/v1/account", token).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["account"]["id"], account.as_str());
    assert_eq!(view["workspaces"].as_array().unwrap().len(), 1);
    // WorkOS now names the account.
    assert_eq!(fake.user(&user).unwrap()["external_id"], account.as_str());

    // The WorkOS access token works as a bearer too.
    let (status, by_token) = get(&d, "/v1/account", &access).await;
    assert_eq!(status, StatusCode::OK, "{by_token}");
    assert_eq!(by_token["account"]["id"], account.as_str());

    // Signing in again finds the same account.
    let (again_access, _) = fake.sign_in(&user);
    let (status, again) = sign_in(&d, &again_access).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["created"], false);
    assert_eq!(again["session"]["account"], account.as_str());

    // A token WorkOS didn't sign, or one that expired, is refused.
    let expired = oa_auth::workos::fake::mint(&user, "session_x", None, -5);
    let (status, refused) = sign_in(&d, &expired).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused}");
    assert_eq!(code(&refused), "sign_in_denied");
    let (status, _) = get(&d, "/v1/account", &expired).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = sign_in(&d, "not a token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

fn signer(seed: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{seed:02x}").repeat(32)).unwrap()
}

async fn key_sign_in(d: &Deployment, key: &RelaySigner) -> Value {
    let (status, challenge) =
        exchange(reqwest::Client::new().get(format!("{}/v1/sessions/nostr/challenge", d.address)))
            .await;
    assert_eq!(status, StatusCode::OK, "{challenge}");
    let url = format!("{}/v1/sessions/nostr", d.address);
    let body = br#"{"app":"OpenAgents"}"#;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let event = key.sign(
        now,
        27_235,
        vec![
            Tag::new(vec!["u".into(), url.clone()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec![
                "challenge".into(),
                challenge["challenge"].as_str().unwrap().into(),
            ]),
            Tag::new(vec!["payload".into(), hex::encode(Sha256::digest(body))]),
        ],
        String::new(),
    );
    let header = format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    );
    let (status, answer) = exchange(
        reqwest::Client::new()
            .post(&url)
            .header("authorization", header)
            .header("content-type", "application/json")
            .body(body.to_vec()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer
}

#[tokio::test]
async fn a_key_sign_in_keeps_our_session_and_gets_a_workos_user() {
    let fake = FakeWorkos::start().await;
    let (d, _keys) = deploy_with(&fake, Mode::Dual).await;
    let answer = key_sign_in(&d, &signer(7)).await;
    // The answer is our own session, as before WorkOS.
    assert!(answer["token"].as_str().unwrap().starts_with("sess_"));
    let account = answer["account"]["id"]
        .as_str()
        .or(answer["session"]["account"].as_str())
        .unwrap()
        .to_string();
    // The dual-write gives the account its User with the synthetic email.
    let email = oa_auth::workos::synthetic_email(&account);
    let mut user = None;
    for _ in 0..50 {
        user = fake.user_by_email(&email);
        if user.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let user = user.expect("the key-only account got its WorkOS user");
    assert_eq!(user["external_id"], account.as_str());
    assert_eq!(user["email_verified"], true);
    assert!(
        user["metadata"]["npub"]
            .as_str()
            .unwrap()
            .starts_with("npub1")
    );
    // That User signing in through WorkOS lands on the same account.
    let (access, _) = fake.sign_in(user["id"].as_str().unwrap());
    let (status, signed) = sign_in(&d, &access).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["session"]["account"], account.as_str());
    assert_eq!(signed["created"], false);
}

#[tokio::test]
async fn an_outage_never_fails_a_key_sign_in() {
    let fake = FakeWorkos::start().await;
    let (d, _keys) = deploy_with(&fake, Mode::Dual).await;
    fake.set_down(true);
    let answer = key_sign_in(&d, &signer(9)).await;
    assert!(answer["token"].as_str().unwrap().starts_with("sess_"));
}

#[tokio::test]
async fn legacy_mode_changes_nothing() {
    let fake = FakeWorkos::start().await;
    let (d, _keys) = deploy_with(&fake, Mode::Legacy).await;
    let user = fake.add_user("lee@example.com", None, true);
    let (access, _) = fake.sign_in(&user);
    let (status, _) = sign_in(&d, &access).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&d, "/v1/account", &access).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    key_sign_in(&d, &signer(8)).await;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(fake.user_count(), 1, "legacy writes nothing to WorkOS");
    assert!(
        fake.calls()
            .iter()
            .all(|call| !call.starts_with("POST /user_management/users"))
    );
}

#[tokio::test]
async fn the_admin_backfill_needs_the_admin_token() {
    let fake = FakeWorkos::start().await;
    let (d, _keys) = deploy_with(&fake, Mode::Dual).await;
    let (status, _) = post(
        &d,
        "/v1/admin/workos/backfill",
        None,
        &json!({"apply": false}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = get(&d, "/v1/admin/workos/reconcile", "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
