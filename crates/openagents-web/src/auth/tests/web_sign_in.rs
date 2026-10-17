//! Signing a browser in with a Nostr key (#12001): a phone scans the code
//! `/login` shows and approves, or a browser extension signs. Both end in
//! the usual session cookie, and a key-only account then approves a Coder
//! 1.0 device code on `/device`. Keys are throwaway test keys.

use std::sync::{Arc, OnceLock};

use axum::routing::any;
use base64::Engine as _;
use nostr::domain::{RelaySigner, Tag};
use sha2::{Digest, Sha256};

use super::*;

fn key(seed: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{seed:02x}").repeat(32)).unwrap()
}

/// `Nostr <base64 event>`: NIP-98 over `POST url` with `challenge` and the
/// body's hash.
fn nip98(key: &RelaySigner, url: &str, challenge: &str, body: &[u8]) -> String {
    let hash: String = Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let event = key.sign(
        crate::cloud::session::now(),
        27_235,
        vec![
            Tag::new(vec!["u".into(), url.into()]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec!["challenge".into(), challenge.into()]),
            Tag::new(vec!["payload".into(), hash]),
        ],
        String::new(),
    );
    format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&event).unwrap())
    )
}

/// The site over an account service that signs people in with a key
/// (`oa_auth::nostr`), accepting events for this site's origin.
async fn key_world() -> World {
    let cell: Arc<OnceLock<Router>> = Arc::default();
    let inner = cell.clone();
    let forward = move |request: Request<Body>| {
        let inner = inner.clone();
        async move {
            inner
                .get()
                .expect("set")
                .clone()
                .oneshot(request)
                .await
                .unwrap()
        }
    };
    let extra = Router::new()
        .route("/v1/sessions/nostr/challenge", any(forward.clone()))
        .route("/v1/sessions/nostr", any(forward));
    let world = world_with_extra_routes(true, None, None, Some(extra)).await;
    let auth =
        oa_auth::nostr::NostrAuth::open(&world.root.path().join("accounts"), &[ORIGIN.to_string()])
            .unwrap();
    assert!(cell.set(oa_auth::nostr::router(auth, "signup")).is_ok());
    world
}

/// One JSON call from `browser`, keeping its cookies; `origin` is the page
/// it comes from.
async fn call(
    world: &World,
    browser: &mut Browser,
    method: &str,
    path: &str,
    origin: Option<&str>,
    authorization: Option<&str>,
    body: Option<&[u8]>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::USER_AGENT, BROWSER)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    if !browser.0.is_empty() {
        let cookies: Vec<String> = browser.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
        request = request.header(header::COOKIE, cookies.join("; "));
    }
    let response = world
        .site
        .clone()
        .oneshot(
            request
                .body(body.map(|b| Body::from(b.to_vec())).unwrap_or_default())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    for value in response.headers().get_all(header::SET_COOKIE) {
        let value = value.to_str().unwrap();
        let (name, content) = value.split(';').next().unwrap().split_once('=').unwrap();
        if content.is_empty() || value.contains("Max-Age=0") {
            browser.0.remove(name);
        } else {
            browser.0.insert(name.into(), content.into());
        }
    }
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

/// The request and challenge in a scanned link.
fn scanned(link: &str) -> (String, String) {
    let fragment = link
        .strip_prefix(&format!("{ORIGIN}/login/scan#"))
        .expect("the site's own scan link");
    let fields: BTreeMap<String, String> = url::form_urlencoded::parse(fragment.as_bytes())
        .into_owned()
        .collect();
    (fields["request"].clone(), fields["challenge"].clone())
}

#[tokio::test]
async fn the_login_page_offers_the_phone_code_and_the_extension() {
    let world = key_world().await;
    let mut browser = Browser::default();
    let login = browser.get(&world, "/login").await;
    assert_eq!(login.status, StatusCode::OK);
    assert!(login.body.contains("data-oa-web-sign-in"), "{}", login.body);
    assert!(login.body.contains("src=\"/login/sign-in.js\""));
    assert!(login.body.contains("Log in with your browser extension"));
    crate::copy_guard::assert_plain("/login", &login.body);
    let script = browser.get(&world, "/login/sign-in.js").await;
    assert_eq!(script.status, StatusCode::OK);
    assert!(script.body.contains("/login/key/extension"));
    let scan = browser.get(&world, "/login/scan").await;
    assert_eq!(scan.status, StatusCode::OK);
    crate::copy_guard::assert_plain("/login/scan", &scan.body);
}

#[tokio::test]
async fn a_phone_scan_signs_the_browser_in_and_a_key_only_account_approves_coder() {
    let world = key_world().await;
    let mut browser = Browser::default();
    let back = json!({"return_to": "/settings"}).to_string();
    let (status, started) = call(
        &world,
        &mut browser,
        "POST",
        "/login/key",
        Some(ORIGIN),
        None,
        Some(back.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert!(browser.0.contains_key("oa_web_sign_in"));
    let request = started["request"].as_str().unwrap().to_string();
    let (scanned_request, challenge) = scanned(started["link"].as_str().unwrap());
    assert_eq!(scanned_request, request);

    // The code, and what the phone shows before it signs.
    let qr = browser
        .get(&world, &format!("/login/key/{request}/qr.svg"))
        .await;
    assert_eq!(qr.status, StatusCode::OK);
    assert!(qr.body.starts_with("<svg"));
    let mut phone = Browser::default();
    let (status, about) = call(
        &world,
        &mut phone,
        "GET",
        &format!("/v1/web-sign-in/{request}"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{about}");
    assert_eq!(about["browser"], "Chrome on macOS");
    assert_eq!(about["origin"], ORIGIN);

    // Nothing yet; another browser can't wait on it.
    let status_path = format!("/login/key/{request}/status");
    let (_, waiting) = call(&world, &mut browser, "GET", &status_path, None, None, None).await;
    assert_eq!(waiting["status"], "waiting");
    let mut other = Browser::default();
    let (status, _) = call(&world, &mut other, "GET", &status_path, None, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A key signing another challenge is refused.
    let phone_key = key(9);
    let approve_url = format!("{ORIGIN}/v1/web-sign-in/approve");
    let body = json!({"request": request}).to_string();
    let wrong = nip98(&phone_key, &approve_url, "v1.1.00.00", body.as_bytes());
    let (status, refused) = call(
        &world,
        &mut phone,
        "POST",
        "/v1/web-sign-in/approve",
        None,
        Some(&wrong),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused}");

    // The phone approves.
    let signed = nip98(&phone_key, &approve_url, &challenge, body.as_bytes());
    let (status, approved) = call(
        &world,
        &mut phone,
        "POST",
        "/v1/web-sign-in/approve",
        None,
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["approved"], true);
    assert_eq!(approved["created"], true);
    assert!(!phone.0.contains_key("oa_cloud_session"));

    // The same approval again finds nothing waiting.
    let (status, _) = call(
        &world,
        &mut phone,
        "POST",
        "/v1/web-sign-in/approve",
        None,
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The browser's next poll signs it in, once.
    let (status, done) = call(&world, &mut browser, "GET", &status_path, None, None, None).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(
        done,
        json!({"status": "signed_in", "return_to": "/settings"})
    );
    assert!(browser.0["oa_cloud_session"].starts_with("sess_"));
    assert!(!browser.0.contains_key("oa_web_sign_in"));
    let (status, _) = call(&world, &mut browser, "GET", &status_path, None, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let settings = browser.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    crate::copy_guard::assert_plain("/settings", &settings.body);
    // A browser session, not a computer: Settings lists no computers.
    assert!(settings.body.contains("No computers are logged in"));

    // Coder 1.0's device code, approved by the key-only account.
    let started = device_start(&world, "key-mbp").await;
    let code = started["user_code"].as_str().unwrap().to_string();
    let approved = device_decide(&world, &mut browser, &code, "approve").await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    let (status, signed) = device_poll(&world, &started).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert!(
        signed["access_token"]
            .as_str()
            .unwrap()
            .starts_with("sess_")
    );
}

#[tokio::test]
async fn a_browser_extension_signs_its_browser_in() {
    let world = key_world().await;
    let mut browser = Browser::default();
    let (status, challenge) = call(
        &world,
        &mut browser,
        "GET",
        "/v1/device/nostr/challenge",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{challenge}");
    let challenge = challenge["challenge"].as_str().unwrap().to_string();
    let url = format!("{ORIGIN}/login/key/extension");
    let body = json!({"return_to": "/settings"}).to_string();
    let signed = nip98(&key(11), &url, &challenge, body.as_bytes());

    // Not from another site.
    let (status, _) = call(
        &world,
        &mut browser,
        "POST",
        "/login/key/extension",
        Some("https://elsewhere.example"),
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!browser.0.contains_key("oa_cloud_session"));

    let (status, done) = call(
        &world,
        &mut browser,
        "POST",
        "/login/key/extension",
        Some(ORIGIN),
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["return_to"], "/settings");
    assert!(browser.0["oa_cloud_session"].starts_with("sess_"));
    let settings = browser.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);

    // Each signed login works once.
    let mut again = Browser::default();
    let (status, _) = call(
        &world,
        &mut again,
        "POST",
        "/login/key/extension",
        Some(ORIGIN),
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    assert!(!again.0.contains_key("oa_cloud_session"));
}

#[tokio::test]
async fn a_scan_sign_in_starts_only_from_this_site() {
    let world = key_world().await;
    let mut browser = Browser::default();
    let (status, _) = call(
        &world,
        &mut browser,
        "POST",
        "/login/key",
        Some("https://elsewhere.example"),
        None,
        Some(b"{}"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(browser.0.is_empty());
}
