//! `POST /v1/sessions/apple` (#11980) over HTTP, with a fixture Apple key
//! set served on the loopback. The key is oa-auth's test-only key.
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};

use super::*;

const KEY: &[u8] = include_bytes!("../../../oa-auth/src/apple/test-key-a.pk8");
const NONCE: &str = "raw-nonce-kept-by-the-phone";

async fn apple_keys() -> String {
    let pair = RsaKeyPair::from_pkcs8(KEY).unwrap();
    let public = ring::signature::RsaPublicKeyComponents::<Vec<u8>>::from(pair.public());
    let jwks = json!({"keys": [{
        "kty": "RSA", "kid": "fixture", "use": "sig", "alg": "RS256",
        "n": URL_SAFE_NO_PAD.encode(&public.n),
        "e": URL_SAFE_NO_PAD.encode(&public.e),
    }]});
    let app = axum::Router::new().route(
        "/auth/keys",
        axum::routing::get(move || {
            let jwks = jwks.clone();
            async move { axum::Json(jwks) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, app).into_future());
    format!("http://{address}/auth/keys")
}

fn identity_token(aud: &str, sub: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let header = URL_SAFE_NO_PAD.encode(json!({"alg": "RS256", "kid": "fixture"}).to_string());
    let claims = json!({
        "iss": "https://appleid.apple.com", "aud": aud, "sub": sub,
        "exp": now + 600, "iat": now,
        "nonce": oa_auth::apple::nonce_digest(NONCE),
        "email": "pat@example.com", "email_verified": true,
    });
    let signed = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
    let pair = RsaKeyPair::from_pkcs8(KEY).unwrap();
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

#[tokio::test]
async fn apple_sign_in_is_off_until_configured() {
    let d = deploy(Some(account_config(None)), true).await;
    let (status, body) = post(
        &d,
        "/v1/sessions/apple",
        None,
        &json!({"identity_token": "x", "nonce": "y"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(code(&body), "apple_disabled");
}

#[tokio::test]
async fn apple_sign_in_makes_one_account_and_app_sessions_that_work() {
    let mut accounts = account_config(None);
    accounts.apple = Some(config::AppleSignIn {
        audiences: vec!["com.openagents.app".into()],
        keys_url: Some(apple_keys().await),
    });
    let d = deploy(Some(accounts), true).await;
    let sub = "001234.0a1b2c3d.1234";
    let body = json!({
        "identity_token": identity_token("com.openagents.app", sub),
        "nonce": NONCE,
        "device": {"app": "OpenAgents", "computer": "Pat's iPhone"},
        "name": "Pat Doe",
    });
    let (status, first) = post(&d, "/v1/sessions/apple", None, &body).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["created"], json!(true));
    assert_eq!(first["account"]["label"], json!("Pat Doe"));
    assert_eq!(first["session"]["computer"], json!("Pat's iPhone"));
    let token = first["token"].as_str().unwrap();
    assert!(token.starts_with("sess_"));
    let (status, view) = get(&d, "/v1/account", token).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["account"]["id"], first["account"]["id"]);
    let (_, apps) = get(&d, "/v1/account/sessions", token).await;
    assert_eq!(apps["sessions"].as_array().unwrap().len(), 1);

    let (status, second) = post(&d, "/v1/sessions/apple", None, &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["created"], json!(false));
    assert_eq!(second["account"]["id"], first["account"]["id"]);

    // Wrong audience, wrong nonce, or a malformed body never sign in.
    let mut wrong = body.clone();
    wrong["identity_token"] = json!(identity_token("com.example.other", sub));
    let (status, refused) = post(&d, "/v1/sessions/apple", None, &wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&refused), "sign_in_denied");
    let mut wrong = body.clone();
    wrong["nonce"] = json!("another");
    let (status, _) = post(&d, "/v1/sessions/apple", None, &wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post(&d, "/v1/sessions/apple", None, &json!({"nonce": NONCE})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
