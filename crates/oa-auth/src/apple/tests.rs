//! Sign in with Apple against a fixture key set. The two keys here are
//! test-only RSA keys made for these tests; they sign nothing real.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use serde_json::{Value, json};

use super::*;

const KEY_A: &[u8] = include_bytes!("test-key-a.pk8");
const KEY_B: &[u8] = include_bytes!("test-key-b.pk8");
const AUD: &str = "com.openagents.app";
const NOW: u64 = 1_800_000_000;
const RAW_NONCE: &str = "6b1f0c2e-raw-nonce-from-the-app";

fn jwk(kid: &str, der: &[u8]) -> Value {
    let pair = RsaKeyPair::from_pkcs8(der).unwrap();
    let public = ring::signature::RsaPublicKeyComponents::<Vec<u8>>::from(pair.public());
    json!({
        "kty": "RSA",
        "kid": kid,
        "use": "sig",
        "alg": "RS256",
        "n": URL_SAFE_NO_PAD.encode(&public.n),
        "e": URL_SAFE_NO_PAD.encode(&public.e),
    })
}

fn keys() -> Keys {
    serde_json::from_value(json!({"keys": [jwk("key-a", KEY_A)]})).unwrap()
}

fn claims() -> Value {
    json!({
        "iss": ISSUER,
        "aud": AUD,
        "exp": NOW + 600,
        "iat": NOW - 5,
        "sub": "001234.0a1b2c3d4e5f60718293a4b5c6d7e8f9.1234",
        "nonce": nonce_digest(RAW_NONCE),
        "nonce_supported": true,
        "email": "pat@example.com",
        "email_verified": "true",
        "is_private_email": "false",
        "auth_time": NOW - 5,
    })
}

fn token(kid: &str, der: &[u8], claims: &Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(json!({"alg": "RS256", "kid": kid}).to_string());
    let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
    let signed = format!("{header}.{payload}");
    let pair = RsaKeyPair::from_pkcs8(der).unwrap();
    let mut signature = vec![0; pair.public().modulus_len()];
    pair.sign(
        &RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        signed.as_bytes(),
        &mut signature,
    )
    .unwrap();
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature))
}

fn audiences() -> Vec<String> {
    vec![AUD.to_string()]
}

fn check(token: &str) -> Result<AppleProfile, Rejected> {
    verify_with(&keys(), &audiences(), token, RAW_NONCE, NOW)
}

#[test]
fn a_valid_token_verifies() {
    let profile = check(&token("key-a", KEY_A, &claims())).unwrap();
    assert_eq!(profile.sub, "001234.0a1b2c3d4e5f60718293a4b5c6d7e8f9.1234");
    assert_eq!(profile.email.as_deref(), Some("pat@example.com"));
    assert!(profile.email_verified);
    assert!(!profile.private_email);
}

#[test]
fn wrong_audience_issuer_expiry_and_nonce_are_refused() {
    let mut wrong = claims();
    wrong["aud"] = json!("com.example.other");
    assert_eq!(
        check(&token("key-a", KEY_A, &wrong)),
        Err(Rejected::Audience)
    );

    let mut wrong = claims();
    wrong["iss"] = json!("https://appleid.example.com");
    assert_eq!(check(&token("key-a", KEY_A, &wrong)), Err(Rejected::Issuer));

    let mut wrong = claims();
    wrong["exp"] = json!(NOW - 120);
    assert_eq!(
        check(&token("key-a", KEY_A, &wrong)),
        Err(Rejected::Expired)
    );

    let mut wrong = claims();
    wrong["nonce"] = json!(nonce_digest("some other nonce"));
    assert_eq!(check(&token("key-a", KEY_A, &wrong)), Err(Rejected::Nonce));

    // The raw nonce itself in the claim is not its digest.
    let mut wrong = claims();
    wrong["nonce"] = json!(RAW_NONCE);
    assert_eq!(check(&token("key-a", KEY_A, &wrong)), Err(Rejected::Nonce));

    let mut wrong = claims();
    wrong.as_object_mut().unwrap().remove("nonce");
    assert_eq!(check(&token("key-a", KEY_A, &wrong)), Err(Rejected::Nonce));
}

#[test]
fn a_bad_signature_or_unknown_key_is_refused() {
    // Signed by key B but naming key A.
    assert_eq!(
        check(&token("key-a", KEY_B, &claims())),
        Err(Rejected::Signature)
    );
    // A key we don't hold.
    assert_eq!(
        check(&token("key-b", KEY_B, &claims())),
        Err(Rejected::UnknownKey)
    );
    // A payload changed after signing.
    let good = token("key-a", KEY_A, &claims());
    let mut parts: Vec<&str> = good.split('.').collect();
    let mut changed = claims();
    changed["sub"] = json!("009999.attacker.9999");
    let payload = URL_SAFE_NO_PAD.encode(changed.to_string());
    parts[1] = &payload;
    assert_eq!(check(&parts.join(".")), Err(Rejected::Signature));
    // Not RS256.
    let header = URL_SAFE_NO_PAD.encode(json!({"alg": "none", "kid": "key-a"}).to_string());
    let forged = format!("{header}.{}.", URL_SAFE_NO_PAD.encode(claims().to_string()));
    assert_eq!(check(&forged), Err(Rejected::Malformed));
    assert_eq!(check("not-a-token"), Err(Rejected::Malformed));
}

#[test]
fn signing_in_creates_the_account_once_with_an_app_session() {
    let dir = tempfile::tempdir().unwrap();
    tenancy::Accounts::install(dir.path()).unwrap();
    tenancy::sessions::Sessions::install(
        dir.path(),
        tenancy::sessions::SessionBook::new(3600, 600),
    )
    .unwrap();
    let profile = check(&token("key-a", KEY_A, &claims())).unwrap();
    let device = Device {
        app: "OpenAgents".into(),
        computer: "Pat's iPhone".into(),
    };
    let first = sign_in(
        dir.path(),
        "signup",
        &profile,
        Some("Pat Doe"),
        Some(&device),
    )
    .unwrap();
    assert!(first.created);
    assert_eq!(first.account.label, "Pat Doe");
    assert!(first.session.once.starts_with("sess_"));
    let label = first.session.session.app.as_ref().unwrap();
    assert_eq!(label.computer, "Pat's iPhone");
    assert_eq!(
        first.session.session.expires_at - first.session.session.created_at,
        tenancy::sessions::APP_SESSION_TTL
    );
    let body = signed_in_body(&first);
    assert_eq!(body["account"]["id"], json!(first.account.id));
    assert_eq!(body["token"], json!(first.session.once));
    assert_eq!(body["created"], json!(true));

    let second = sign_in(dir.path(), "signup", &profile, None, None).unwrap();
    assert!(!second.created);
    assert_eq!(second.account.id, first.account.id);
    assert_ne!(second.session.once, first.session.once);
    assert_eq!(
        second.session.session.app.as_ref().unwrap().app,
        "OpenAgents"
    );

    // The session is listed among the account's signed-in apps.
    let store = tenancy::sessions::Sessions::open(dir.path())
        .unwrap()
        .store()
        .unwrap();
    let user = tenancy::workspaces::UserId::from(first.account.id.as_str());
    assert_eq!(
        store
            .book
            .app_sessions(&user, first.session.session.created_at)
            .len(),
        2
    );
}

#[tokio::test]
async fn keys_are_fetched_from_the_published_address_and_cached() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let fetches = Arc::new(AtomicUsize::new(0));
    let counted = fetches.clone();
    let app = axum::Router::new().route(
        "/auth/keys",
        axum::routing::get(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            async { axum::Json(json!({"keys": [jwk("key-a", KEY_A)]})) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let apple = Apple::new(audiences(), Some(format!("http://{address}/auth/keys"))).unwrap();

    let mut live = claims();
    let now = now();
    live["exp"] = json!(now + 600);
    live["iat"] = json!(now);
    let good = token("key-a", KEY_A, &live);
    assert!(apple.verify(&good, RAW_NONCE).await.is_ok());
    assert!(apple.verify(&good, RAW_NONCE).await.is_ok());
    assert_eq!(fetches.load(Ordering::SeqCst), 1);
    assert_eq!(
        apple.verify(&good, "wrong nonce").await.unwrap_err(),
        AuthError::AppleDenied
    );
    // An unknown key right after a fetch doesn't fetch again.
    let other = token("key-b", KEY_B, &live);
    assert_eq!(
        apple.verify(&other, RAW_NONCE).await.unwrap_err(),
        AuthError::AppleDenied
    );
    assert_eq!(fetches.load(Ordering::SeqCst), 1);
}

#[test]
fn a_verifier_needs_an_audience() {
    assert!(Apple::new(Vec::new(), None).is_err());
}
