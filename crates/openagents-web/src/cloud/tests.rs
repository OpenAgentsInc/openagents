//! Isolated HTTP acceptance for sign-in, sign-out, Settings, and the Claude
//! credential page over a fake account service.

use super::session::CloudSession;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

#[path = "phone_api_tests.rs"]
mod phone_api;

const HOST: &str = "127.0.0.1:4300";
const ORIGIN: &str = "http://127.0.0.1:4300";
const CANARY: &str = "synthetic-native-private-canary";
const FAKE_KEY: &str = "sk-ant-api03-fake-settings-key-for-tests-only";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn credential(account: &str) -> String {
    format!("oak_{account}.synthetic-credential")
}

fn token(account: &str) -> String {
    format!(
        "sess_{}",
        if account == "alice" { "a" } else { "b" }.repeat(64)
    )
}

#[derive(Default)]
struct Native {
    revoked: BTreeSet<String>,
    epoch: u64,
    removed: bool,
    offline: bool,
    signins: usize,
    signouts: usize,
    expiry: u64,
    /// Plan checkouts and billing pages opened: (workspace, what).
    billing: Vec<(String, String)>,
}

fn acting(headers: &HeaderMap, state: &Native) -> Option<&'static str> {
    let bearer = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    ["alice", "bob"]
        .into_iter()
        .find(|account| bearer == token(account) && !state.revoked.contains(*account))
}

fn native_refusal(status: StatusCode) -> Response {
    (
        status,
        Json(json!({"error":{"code":"unauthenticated","message":CANARY}})),
    )
        .into_response()
}

async fn native_sign_in(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut state = state.lock().unwrap();
    state.signins += 1;
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(account) = ["alice", "bob"]
        .into_iter()
        .find(|account| bearer == Some(credential(account).as_str()))
    else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    state.revoked.remove(account);
    Json(json!({"session":{"id":format!("native-session-{account}"),"kind":"user","account":account,"created_at":now(),"expires_at":state.expiry},"token":token(account)})).into_response()
}

async fn native_session(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let state = state.lock().unwrap();
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(&headers, &state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    Json(json!({"session":{"id":format!("native-session-{account}"),"kind":"user","account":account,"created_at":now()-1,"expires_at":state.expiry,"state":"active"}})).into_response()
}

async fn native_details(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let state = state.lock().unwrap();
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(&headers, &state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    let workspaces = if state.removed {
        vec![]
    } else if account == "alice" {
        vec![
            json!({"id":"alice-team","name":"Alice team","role":"member"}),
            json!({"id":"alice-personal","name":"Alice personal","role":"owner"}),
        ]
    } else {
        vec![json!({"id":"bob-personal","name":"Bob personal","role":"owner"})]
    };
    Json(json!({"account":{"id":account,"label":format!("{account} <account>"),"principals":[CANARY]},"workspaces":workspaces})).into_response()
}

async fn native_workspace(
    State(state): State<Arc<Mutex<Native>>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().unwrap();
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(&headers, &state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    if state.removed
        || !(account == "alice" && matches!(id.as_str(), "alice-personal" | "alice-team")
            || account == "bob" && id == "bob-personal")
    {
        return native_refusal(StatusCode::FORBIDDEN);
    }
    Json(json!({"workspace":{"id":id,"tenant":"synthetic","members_epoch":state.epoch},"role":if id == "alice-team" {"member"} else {"owner"}})).into_response()
}

async fn native_sign_out(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut state = state.lock().unwrap();
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(&headers, &state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    state.revoked.insert(account.into());
    state.signouts += 1;
    Json(json!({"session":{"state":"revoked"}})).into_response()
}

/// The gateway's plan checkout and billing page, answering hosted Stripe
/// addresses for the owner's own workspace.
async fn native_billing(
    State(state): State<Arc<Mutex<Native>>>,
    Path((id, action)): Path<(String, String)>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let mut state = state.lock().unwrap();
    let Some(account) = acting(&headers, &state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    if id != format!("{account}-personal") {
        return native_refusal(StatusCode::FORBIDDEN);
    }
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    match action.as_str() {
        "checkout" => {
            state.billing.push((
                id,
                format!("checkout {}", body["plan"].as_str().unwrap_or("")),
            ));
            Json(json!({"v":"openagents.billing.v1","url":"https://checkout.stripe.com/c/pay/cs_test_web"}))
                .into_response()
        }
        "portal" => {
            state.billing.push((id, "portal".into()));
            Json(json!({"v":"openagents.billing.v1","url":"https://billing.stripe.com/p/session/test_web"}))
                .into_response()
        }
        _ => native_refusal(StatusCode::NOT_FOUND),
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    site: Router,
    state: Arc<Mutex<Native>>,
    server: tokio::task::JoinHandle<()>,
    local_store: PathBuf,
    byo: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
    fixture_with(|_| {}).await
}

async fn fixture_with(configure: impl FnOnce(&mut crate::Config)) -> Fixture {
    let state = Arc::new(Mutex::new(Native {
        epoch: 3,
        expiry: now() + 3600,
        ..Default::default()
    }));
    let native = Router::new()
        .route("/v1/sessions", post(native_sign_in))
        .route("/v1/session", get(native_session).delete(native_sign_out))
        .route("/v1/account", get(native_details))
        .route("/v1/workspaces/{id}", get(native_workspace))
        .route("/v1/workspaces/{id}/billing/{action}", post(native_billing))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, native).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let secret = directory.join("csrf.key");
    std::fs::write(&secret, [13; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let path = directory.join("cloud.json");
    std::fs::write(&path, serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":ORIGIN,"account_service":endpoint,"csrf_secret":secret})).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let byo = directory.join("byo");
    std::fs::create_dir(&byo).unwrap();
    std::fs::set_permissions(&byo, std::fs::Permissions::from_mode(0o700)).unwrap();
    let local_store = directory.join("unopened-local-tasks");
    let mut config = crate::Config::development(local_store.clone());
    config.cloud = Some(Arc::new(CloudSession::load(&path).unwrap()));
    config.cloud_byo = Some(Arc::new(
        super::byo::Computers::open(&byo, oa_seal::Keyring::scratch("test").unwrap().0)
            .unwrap()
            .checking(None),
    ));
    configure(&mut config);
    Fixture {
        _root: root,
        site: crate::router(config),
        state,
        server,
        local_store,
        byo,
    }
}

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

async fn request(
    site: &Router,
    method: Method,
    path: &str,
    cookies: &Cookies,
    form: Option<&str>,
    origin: Option<&str>,
) -> Answer {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::ACCEPT, "text/html");
    if !cookies.0.is_empty() {
        request = request.header(header::COOKIE, cookies.header());
    }
    if let Some(origin) = origin {
        request = request
            .header(header::ORIGIN, origin)
            .header("sec-fetch-site", "same-origin");
    }
    if form.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    }
    let response = site
        .clone()
        .oneshot(
            request
                .body(Body::from(form.unwrap_or("").to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    Answer {
        status,
        headers,
        body,
    }
}

#[derive(Default)]
struct Cookies(BTreeMap<String, String>);

impl Cookies {
    fn apply(&mut self, answer: &Answer) {
        for value in answer.headers.get_all(header::SET_COOKIE) {
            let value = value.to_str().unwrap();
            let (name, content) = value.split(';').next().unwrap().split_once('=').unwrap();
            if content.is_empty() || value.contains("Max-Age=0") {
                self.0.remove(name);
            } else {
                self.0.insert(name.into(), content.into());
            }
        }
    }
    fn header(&self) -> String {
        self.0
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn field(html: &str, name: &str) -> String {
    html.split_once(&format!("name=\"{name}\" value=\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .into()
}

/// The markup of the form posting to `action`.
fn form_at<'a>(html: &'a str, action: &str) -> &'a str {
    html.split("<form ")
        .skip(1)
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .unwrap()
        .split("</form>")
        .next()
        .unwrap()
}

fn form(fields: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish()
}

fn private(answer: &Answer) {
    assert_eq!(answer.headers[header::CACHE_CONTROL], "no-store, private");
    assert_eq!(answer.headers[header::REFERRER_POLICY], "same-origin");
    assert_eq!(answer.headers[header::VARY], "Cookie");
    assert!(!answer.body.contains(CANARY));
    assert!(!answer.body.contains(&token("alice")));
    assert!(!answer.body.contains(&token("bob")));
    assert!(!answer.body.contains(&credential("alice")));
    assert!(!answer.body.contains(FAKE_KEY));
}

/// The page's main area says nothing in machine talk (#11031).
fn plain(answer: &Answer) {
    let body = &answer.body;
    let main = &body[body.find("<main").unwrap()..body.find("</main>").unwrap()];
    let text = oa_copy::visible_text(main);
    assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
}

fn migrated_cookie<'a>(answer: &'a Answer, name: &str) -> &'a str {
    let values: Vec<&str> = answer
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .filter(|value| value.starts_with(&format!("{name}=")))
        .collect();
    assert_eq!(values.len(), 2);
    let legacy = values
        .iter()
        .find(|value| value.contains("; Path=/cloud;"))
        .unwrap();
    assert!(legacy.starts_with(&format!("{name}=;")));
    assert!(legacy.contains("; Max-Age=0"));
    let root = values
        .into_iter()
        .find(|value| value.contains("; Path=/;"))
        .unwrap();
    assert!(root.contains("HttpOnly; SameSite=Strict"));
    root
}

async fn login(fixture: &Fixture, account: &str) -> Cookies {
    let mut cookies = Cookies::default();
    let page = request(&fixture.site, Method::GET, "/sign-in", &cookies, None, None).await;
    assert_eq!(page.status, StatusCode::OK);
    private(&page);
    plain(&page);
    let nonce = migrated_cookie(&page, "oa_cloud_login");
    assert!(nonce.contains("HttpOnly; SameSite=Strict"));
    assert!(nonce.contains("; Path=/;"));
    cookies.apply(&page);
    let csrf = field(&page.body, "csrf");
    let input = form(&[("credential", &credential(account)), ("csrf", &csrf)]);
    let answer = request(
        &fixture.site,
        Method::POST,
        "/sign-in",
        &cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    private(&answer);
    assert_eq!(answer.headers[header::LOCATION], "/");
    for name in ["oa_cloud_session", "oa_cloud_login"] {
        migrated_cookie(&answer, name);
    }
    cookies.apply(&answer);
    assert!(!cookies.0.contains_key("oa_cloud_login"));
    assert_eq!(cookies.0["oa_cloud_session"], token(account));
    // The account's own workspace is selected; there is no picker.
    assert_eq!(
        cookies.0["oa_cloud_workspace"],
        format!("{account}-personal")
    );
    cookies
}

#[tokio::test]
async fn existing_cloud_session_migrates_to_root_without_another_sign_in() {
    let fixture = fixture().await;
    let mut cookies = Cookies::default();
    cookies.0.insert("oa_cloud_session".into(), token("alice"));
    cookies
        .0
        .insert("oa_cloud_workspace".into(), "alice-personal".into());
    let response = request(&fixture.site, Method::GET, "/sign-in", &cookies, None, None).await;
    assert_eq!(response.status, StatusCode::SEE_OTHER);
    assert_eq!(response.headers[header::LOCATION], "/");
    private(&response);
    assert!(
        migrated_cookie(&response, "oa_cloud_session")
            .starts_with(&format!("oa_cloud_session={};", token("alice")))
    );
    assert!(
        migrated_cookie(&response, "oa_cloud_workspace")
            .starts_with("oa_cloud_workspace=alice-personal;")
    );
    assert!(migrated_cookie(&response, "oa_cloud_login").contains("; Max-Age=0"));
    cookies.apply(&response);
    assert_eq!(cookies.0["oa_cloud_session"], token("alice"));
    assert_eq!(cookies.0["oa_cloud_workspace"], "alice-personal");
    assert_eq!(fixture.state.lock().unwrap().signins, 0);
}

#[tokio::test]
async fn legacy_login_nonce_is_reissued_at_root_before_submission() {
    let fixture = fixture().await;
    let mut cookies = Cookies::default();
    cookies.0.insert("oa_cloud_login".into(), "c".repeat(64));
    let page = request(&fixture.site, Method::GET, "/sign-in", &cookies, None, None).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        migrated_cookie(&page, "oa_cloud_login")
            .starts_with(&format!("oa_cloud_login={};", "c".repeat(64)))
    );
    cookies.apply(&page);
    let csrf = field(&page.body, "csrf");
    let input = form(&[("credential", &credential("alice")), ("csrf", &csrf)]);
    let response = request(
        &fixture.site,
        Method::POST,
        "/sign-in",
        &cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(response.status, StatusCode::SEE_OTHER);
    migrated_cookie(&response, "oa_cloud_session");
    cookies.apply(&response);
    assert_eq!(cookies.0["oa_cloud_session"], token("alice"));
    assert!(!cookies.0.contains_key("oa_cloud_login"));
}

#[tokio::test]
async fn cross_origin_malformed_and_unauthenticated_forms_create_no_native_session() {
    let fixture = fixture().await;
    let mut cookies = Cookies::default();
    let page = request(&fixture.site, Method::GET, "/sign-in", &cookies, None, None).await;
    cookies.apply(&page);
    let csrf = field(&page.body, "csrf");
    let input = form(&[("credential", &credential("alice")), ("csrf", &csrf)]);
    for origin in [None, Some("https://other.example.invalid")] {
        let rejected = request(
            &fixture.site,
            Method::POST,
            "/sign-in",
            &cookies,
            Some(&input),
            origin,
        )
        .await;
        assert_eq!(rejected.status, StatusCode::FORBIDDEN);
        private(&rejected);
    }
    let tampered = form(&[("credential", &credential("alice")), ("csrf", "invalid")]);
    let rejected = request(
        &fixture.site,
        Method::POST,
        "/sign-in",
        &cookies,
        Some(&tampered),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(rejected.status, StatusCode::FORBIDDEN);
    private(&rejected);
    let malformed = request(
        &fixture.site,
        Method::POST,
        "/sign-in",
        &cookies,
        Some("credential=synthetic"),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
    private(&malformed);
    let oversize = "credential=".to_owned() + &"x".repeat(9000) + "&csrf=invalid";
    let bounded = request(
        &fixture.site,
        Method::POST,
        "/sign-in",
        &cookies,
        Some(&oversize),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(bounded.status, StatusCode::BAD_REQUEST);
    private(&bounded);
    assert_eq!(fixture.state.lock().unwrap().signins, 0);
    // Settings need a session; sign-out needs the session's own ticket.
    for path in ["/settings", "/settings/claude"] {
        let absent = request(
            &fixture.site,
            Method::GET,
            path,
            &Cookies::default(),
            None,
            None,
        )
        .await;
        assert_eq!(absent.status, StatusCode::SEE_OTHER, "{path}");
        private(&absent);
        assert_eq!(absent.headers[header::LOCATION], "/sign-in");
    }
    let absent = request(
        &fixture.site,
        Method::POST,
        "/sign-out",
        &Cookies::default(),
        Some(&form(&[("csrf", &csrf)])),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(absent.status, StatusCode::FORBIDDEN);
    private(&absent);
}

#[tokio::test]
async fn revoked_sessions_and_removed_memberships_see_no_account() {
    let fixture = fixture().await;
    let cookies = login(&fixture, "alice").await;
    fixture.state.lock().unwrap().removed = true;
    let removed = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(removed.status, StatusCode::FORBIDDEN);
    private(&removed);
    assert!(!removed.body.contains("alice &lt;account&gt;"));
    fixture.state.lock().unwrap().removed = false;
    fixture.state.lock().unwrap().revoked.insert("alice".into());
    let revoked = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::SEE_OTHER);
    assert_eq!(revoked.headers[header::LOCATION], "/sign-in");
    private(&revoked);
    assert!(!revoked.body.contains("native-session-alice"));
}

#[tokio::test]
async fn settings_shows_the_account_theme_and_claude_link() {
    let fixture = fixture().await;
    let cookies = login(&fixture, "alice").await;
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    plain(&page);
    for needle in [
        "<title>Settings",
        "alice &lt;account&gt;",
        ">Profile<",
        ">Theme<",
        ">Claude<",
        "Not added",
        "href=\"/settings/claude\"",
        "action=\"/sign-out\"",
    ] {
        assert!(page.body.contains(needle), "{needle}");
    }
    assert!(!page.body.contains("/cloud/app"));
    assert!(!fixture.local_store.exists());
}

#[tokio::test]
async fn claude_credential_needs_a_fresh_ticket_from_the_same_account() {
    let fixture = fixture().await;
    let alice = login(&fixture, "alice").await;
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings/claude",
        &alice,
        None,
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    plain(&page);
    assert!(page.body.contains("Nothing saved."));
    let add = form_at(&page.body, "/settings/claude");
    let (csrf, ticket) = (field(add, "csrf"), field(add, "request"));
    let submit = |csrf: &str, consent: bool| {
        let mut fields = vec![
            ("csrf", csrf.to_owned()),
            ("request", ticket.clone()),
            ("material", "anthropic_api_key".to_owned()),
            ("value", FAKE_KEY.to_owned()),
        ];
        if consent {
            fields.push(("consent", "custody".to_owned()));
        }
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields.iter().map(|(k, v)| (*k, v.as_str())))
            .finish()
    };
    // Another site, a made-up ticket, or another account's session: refused.
    for (input, origin) in [
        (submit(&csrf, true), "https://other.example.invalid"),
        (submit("invalid", true), ORIGIN),
    ] {
        let refused = request(
            &fixture.site,
            Method::POST,
            "/settings/claude",
            &alice,
            Some(&input),
            Some(origin),
        )
        .await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        private(&refused);
    }
    let bob = login(&fixture, "bob").await;
    let crossed = request(
        &fixture.site,
        Method::POST,
        "/settings/claude",
        &bob,
        Some(&submit(&csrf, true)),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(crossed.status, StatusCode::FORBIDDEN);
    private(&crossed);
    assert_eq!(std::fs::read_dir(&fixture.byo).unwrap().count(), 0);
    // Consent is required.
    let unconsented = request(
        &fixture.site,
        Method::POST,
        "/settings/claude",
        &alice,
        Some(&submit(&csrf, false)),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(unconsented.status, StatusCode::BAD_REQUEST);
    private(&unconsented);
    // Saved for alice only.
    let saved = request(
        &fixture.site,
        Method::POST,
        "/settings/claude",
        &alice,
        Some(&submit(&csrf, true)),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert_eq!(saved.headers[header::LOCATION], "/settings/claude");
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings/claude",
        &alice,
        None,
        None,
    )
    .await;
    private(&page);
    plain(&page);
    assert!(page.body.contains("Saved: Anthropic API key"));
    let bob_page = request(
        &fixture.site,
        Method::GET,
        "/settings/claude",
        &bob,
        None,
        None,
    )
    .await;
    assert!(bob_page.body.contains("Nothing saved."));
    // Bob cannot remove it with alice's ticket; alice can.
    let remove = form_at(&page.body, "/settings/claude/remove");
    let input = form(&[
        ("csrf", &field(remove, "csrf")),
        ("request", &field(remove, "request")),
    ]);
    let crossed = request(
        &fixture.site,
        Method::POST,
        "/settings/claude/remove",
        &bob,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(crossed.status, StatusCode::FORBIDDEN);
    assert_eq!(std::fs::read_dir(&fixture.byo).unwrap().count(), 1);
    let removed = request(
        &fixture.site,
        Method::POST,
        "/settings/claude/remove",
        &alice,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(removed.status, StatusCode::SEE_OTHER);
    assert_eq!(std::fs::read_dir(&fixture.byo).unwrap().count(), 0);
}

#[tokio::test]
async fn old_cloud_addresses_redirect_to_their_new_homes() {
    let fixture = fixture().await;
    for (path, to) in [
        ("/cloud/sign-in", "/sign-in"),
        ("/cloud/app", "/"),
        ("/cloud/app/hosts/resident/tasks", "/"),
        ("/cloud/app/settings", "/settings"),
        ("/cloud/app/settings/claude", "/settings/claude"),
    ] {
        let answer = request(
            &fixture.site,
            Method::GET,
            path,
            &Cookies::default(),
            None,
            None,
        )
        .await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER, "{path}");
        assert_eq!(answer.headers[header::LOCATION], to, "{path}");
    }
}

#[tokio::test]
async fn valid_logout_clears_browser_after_native_or_configuration_standing_changes() {
    for change in ["membership", "offline", "configuration"] {
        let fixture = fixture().await;
        let mut cookies = login(&fixture, "alice").await;
        let settings = request(
            &fixture.site,
            Method::GET,
            "/settings",
            &cookies,
            None,
            None,
        )
        .await;
        assert_eq!(settings.status, StatusCode::OK);
        let csrf = field(form_at(&settings.body, "/sign-out"), "csrf");
        {
            let mut state = fixture.state.lock().unwrap();
            state.offline = change == "offline";
            state.removed = change == "membership";
        }
        if change == "configuration" {
            std::fs::write(
                fixture
                    ._root
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join("private/cloud.json"),
                b"changed synthetic configuration",
            )
            .unwrap();
        }
        for (ticket, origin) in [
            (csrf.as_str(), "https://other.example.invalid"),
            ("invalid", ORIGIN),
        ] {
            let input = form(&[("csrf", ticket)]);
            let refused = request(
                &fixture.site,
                Method::POST,
                "/sign-out",
                &cookies,
                Some(&input),
                Some(origin),
            )
            .await;
            assert_eq!(refused.status, StatusCode::FORBIDDEN);
            private(&refused);
            assert!(!refused.headers.contains_key(header::SET_COOKIE));
            assert_eq!(fixture.state.lock().unwrap().signouts, 0);
        }
        let input = form(&[("csrf", &csrf)]);
        let ended = request(
            &fixture.site,
            Method::POST,
            "/sign-out",
            &cookies,
            Some(&input),
            Some(ORIGIN),
        )
        .await;
        private(&ended);
        cookies.apply(&ended);
        assert!(cookies.0.is_empty());
        if change != "membership" {
            assert_eq!(ended.status, StatusCode::SERVICE_UNAVAILABLE);
            assert!(ended.body.contains("couldn't reach your account"));
            assert_eq!(fixture.state.lock().unwrap().signouts, 0);
        } else {
            assert_eq!(ended.status, StatusCode::SEE_OTHER);
            assert_eq!(ended.headers[header::LOCATION], "/");
            assert_eq!(fixture.state.lock().unwrap().signouts, 1);
        }
        assert!(!fixture.local_store.exists());
    }
}

#[tokio::test]
async fn subscribe_opens_stripe_checkout_and_settings_shows_pro_after_the_event() {
    use retail_cloud::environment;
    let meter = tempfile::tempdir().unwrap();
    let meter_path = meter.path().join("meter.sqlite");
    let journal = retail_cloud::journal::Journal::open(&meter_path).unwrap();
    let fixture = fixture_with(|config| {
        config.plan = Some(Arc::new(crate::plan::Plans::with_meter(
            Some(journal),
            Some("pro".into()),
        )));
    })
    .await;
    let cookies = login(&fixture, "alice").await;
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    plain(&page);
    assert!(page.body.contains("You're not subscribed."));
    let subscribe = form_at(&page.body, "/settings/plan/subscribe");
    let fields = form(&[
        ("csrf", &field(subscribe, "csrf")),
        ("request", &field(subscribe, "request")),
    ]);
    // A forged ticket opens nothing.
    let forged = form(&[
        ("csrf", "forged"),
        ("request", &field(subscribe, "request")),
    ]);
    let refused = request(
        &fixture.site,
        Method::POST,
        "/settings/plan/subscribe",
        &cookies,
        Some(&forged),
        Some(ORIGIN),
    )
    .await;
    assert_ne!(refused.status, StatusCode::SEE_OTHER);
    assert!(fixture.state.lock().unwrap().billing.is_empty());
    let answer = request(
        &fixture.site,
        Method::POST,
        "/settings/plan/subscribe",
        &cookies,
        Some(&fields),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.headers[header::LOCATION],
        "https://checkout.stripe.com/c/pay/cs_test_web"
    );
    assert_eq!(
        fixture.state.lock().unwrap().billing,
        vec![("alice-personal".to_string(), "checkout pro".to_string())]
    );
    // Back from Stripe, before its event: say so.
    let back = request(
        &fixture.site,
        Method::GET,
        "/settings?plan=started",
        &cookies,
        None,
        None,
    )
    .await;
    plain(&back);
    assert!(
        back.body.contains("Stripe is confirming your payment"),
        "{}",
        back.body
    );
    // The gateway writes the paid month into the meter: Pro shows.
    let start = now() as i64 - 60;
    let mut writer = retail_cloud::journal::Journal::open(&meter_path).unwrap();
    environment::record_period(
        &mut writer,
        &environment::Period {
            account: "alice".into(),
            plan: environment::plan().version,
            start,
            end: start + 30 * 86_400,
        },
    )
    .unwrap();
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings?plan=started",
        &cookies,
        None,
        None,
    )
    .await;
    plain(&page);
    assert!(page.body.contains("You're on Pro."), "{}", page.body);
    assert!(page.body.contains("0 of 100 hours used."), "{}", page.body);
    assert!(!page.body.contains("action=\"/settings/plan/subscribe\""));
    let manage = form_at(&page.body, "/settings/plan/manage");
    let fields = form(&[
        ("csrf", &field(manage, "csrf")),
        ("request", &field(manage, "request")),
    ]);
    let answer = request(
        &fixture.site,
        Method::POST,
        "/settings/plan/manage",
        &cookies,
        Some(&fields),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.headers[header::LOCATION],
        "https://billing.stripe.com/p/session/test_web"
    );
    // A dispute and a refund (#11074) are said in plain words.
    environment::set_notice(&mut writer, "alice", Some(environment::Notice::Dispute)).unwrap();
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    plain(&page);
    assert!(
        page.body
            .contains("Your bank has opened a dispute on a Pro payment."),
        "{}",
        page.body
    );
    assert!(page.body.contains("You're on Pro."), "{}", page.body);
    environment::end_period(&mut writer, "alice", now() as i64 - 10).unwrap();
    environment::set_notice(&mut writer, "alice", Some(environment::Notice::Refunded)).unwrap();
    let page = request(
        &fixture.site,
        Method::GET,
        "/settings",
        &cookies,
        None,
        None,
    )
    .await;
    plain(&page);
    assert!(
        page.body
            .contains("Your last Pro payment was refunded, so Pro ended."),
        "{}",
        page.body
    );
    assert!(
        page.body.contains("You're not subscribed."),
        "{}",
        page.body
    );
}
