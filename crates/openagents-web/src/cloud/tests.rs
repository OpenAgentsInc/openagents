//! Isolated HTTP acceptance for the same-origin account adapter and shell.

use super::session::CloudSession;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

#[path = "../../../coder-control/src/tests/relay.rs"]
mod task_relay;

#[path = "agents_tests.rs"]
mod agents;
#[path = "billing_tests.rs"]
mod billing;
#[path = "control_tests.rs"]
mod controls;
#[path = "environment_tests.rs"]
mod environment;
#[path = "environment_e2e_tests.rs"]
mod environment_e2e;

#[path = "../../examples/support/operator_fixture.rs"]
mod operator_fixture;
#[path = "operator_tests.rs"]
mod operator_tests;
#[path = "partners_tests.rs"]
mod partners_web;
#[path = "../../examples/support/project_fixture.rs"]
mod project_fixture;
#[path = "project_tests.rs"]
mod projects;
#[path = "reconnect_tests.rs"]
mod reconnect;
#[path = "retail_tests.rs"]
mod retail_web;
#[path = "sales_floor_tests.rs"]
mod sales_floor_web;
#[path = "sales_tests.rs"]
mod sales_web;
#[path = "team_tests.rs"]
mod team_web;
#[path = "verse_tests.rs"]
mod verse;
#[path = "workbench_tests.rs"]
mod workbench;

const HOST: &str = "127.0.0.1:4300";
const ORIGIN: &str = "http://127.0.0.1:4300";
const CANARY: &str = "synthetic-native-private-canary";

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
    team_removed: bool,
    team_name: Option<String>,
    team_role: Option<String>,
    signins: usize,
    signouts: usize,
    expiry: u64,
    /// Original native billing documents for alice-personal (WEB-11).
    statement: Option<Value>,
    decision: Option<Value>,
    receipt: Option<Value>,
    /// A native team book for alice-team (WEB-12); legacy fields apply without one.
    team: Option<team_web::Book>,
    /// Original referral records for alice (WEB-16), by document name.
    referral: BTreeMap<&'static str, Value>,
}

/// The fake gateway serves billing documents only to alice in alice-personal.
fn billing_read(
    state: &Native,
    headers: &HeaderMap,
    id: &str,
    document: &Option<Value>,
) -> Response {
    if state.offline {
        return native_refusal(StatusCode::SERVICE_UNAVAILABLE);
    }
    let Some(account) = acting(headers, state) else {
        return native_refusal(StatusCode::UNAUTHORIZED);
    };
    match document {
        Some(value) if account == "alice" && id == "alice-personal" => {
            Json(value.clone()).into_response()
        }
        _ => (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"scope_denied","message":CANARY}})),
        )
            .into_response(),
    }
}

async fn native_usage(
    State(state): State<Arc<Mutex<Native>>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: axum::extract::RawQuery,
) -> Response {
    let state = state.lock().unwrap();
    if !request.0.unwrap_or_default().contains("joined=true") {
        return native_refusal(StatusCode::BAD_REQUEST);
    }
    billing_read(&state, &headers, &id, &state.statement)
}

async fn native_context(
    State(state): State<Arc<Mutex<Native>>>,
    Path((id, door)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().unwrap();
    let document = state.decision.clone().filter(|d| d["door"] == json!(door));
    billing_read(&state, &headers, &id, &document)
}

async fn native_receipt(
    State(state): State<Arc<Mutex<Native>>>,
    Path((id, digest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let state = state.lock().unwrap();
    let document = state
        .receipt
        .clone()
        .filter(|r| r["receipt"]["digest"] == json!(digest));
    billing_read(&state, &headers, &id, &document)
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
    if let Some(book) = &state.team {
        return team_web::details(book, account, state.removed);
    }
    let workspaces = if state.removed {
        vec![]
    } else if account == "alice" {
        let mut workspaces =
            vec![json!({"id":"alice-personal","name":"Alice personal","role":"owner"})];
        if !state.team_removed {
            workspaces.push(json!({"id":"alice-team","name":state.team_name.as_deref().unwrap_or("Alice team"),"role":state.team_role.as_deref().unwrap_or("member")}));
        }
        workspaces
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
    if let Some(book) = &state.team {
        return team_web::workspace(book, account, &id, &headers);
    }
    if state.removed
        || (state.team_removed && id == "alice-team")
        || !(account == "alice" && matches!(id.as_str(), "alice-personal" | "alice-team")
            || account == "bob" && id == "bob-personal")
    {
        return native_refusal(StatusCode::FORBIDDEN);
    }
    Json(json!({"workspace":{"id":id,"tenant":"synthetic","members_epoch":state.epoch},"role":if id == "alice-team" {state.team_role.as_deref().unwrap_or("member")} else {"owner"}})).into_response()
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

struct Fixture {
    _root: tempfile::TempDir,
    site: Router,
    state: Arc<Mutex<Native>>,
    server: tokio::task::JoinHandle<()>,
    local_store: PathBuf,
    config: crate::Config,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
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
        .route("/v1/workspaces/{id}/usage", get(native_usage))
        .route("/v1/workspaces/{id}/usage/export", get(native_usage))
        .route(
            "/v1/workspaces/{id}/purchase-context/{door}",
            get(native_context),
        )
        .route(
            "/v1/workspaces/{id}/usage/receipts/{digest}",
            get(native_receipt),
        )
        .merge(team_web::native_routes())
        .merge(partners_web::native_routes())
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
    let build = directory.join("build");
    std::fs::create_dir(&build).unwrap();
    // Route acceptance does not execute these assets; the parent verifies the real Wasm build in Chrome.
    std::fs::write(build.join("coder_cloud_web.js"), "synthetic route asset").unwrap();
    std::fs::write(
        build.join("coder_cloud_web_bg.wasm"),
        b"synthetic route asset",
    )
    .unwrap();
    let local_store = directory.join("unopened-local-tasks");
    let mut config = crate::Config::development(local_store.clone());
    config.cloud = Some(Arc::new(CloudSession::load(&path).unwrap()));
    config.cloud_build = Some(build);
    Fixture {
        _root: root,
        site: crate::router(config.clone()),
        state,
        server,
        local_store,
        config,
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
        .header(header::HOST, HOST);
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

/// Opens an observation stream, optionally as a browser reconnect carrying
/// `Last-Event-ID`, and returns its first complete event (or a whole
/// non-stream refusal body) without waiting for the stream to end.
async fn first_event(
    site: &Router,
    path: &str,
    cookies: &Cookies,
    last_event_id: Option<&str>,
) -> Answer {
    use futures_util::StreamExt;
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(header::HOST, HOST);
    if !cookies.0.is_empty() {
        request = request.header(header::COOKIE, cookies.header());
    }
    if let Some(id) = last_event_id {
        request = request.header("last-event-id", id);
    }
    let response = site
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let mut stream = response.into_body().into_data_stream();
    let mut body = String::new();
    while !body.contains("\n\n") {
        match tokio::time::timeout(std::time::Duration::from_secs(20), stream.next()).await {
            Ok(Some(Ok(bytes))) => body.push_str(&String::from_utf8_lossy(&bytes)),
            _ => break,
        }
    }
    Answer {
        status,
        headers,
        body,
    }
}

/// The `id:` field of one server-sent event.
fn event_id(event: &str) -> Option<&str> {
    event
        .lines()
        .find_map(|line| line.strip_prefix("id:"))
        .map(str::trim)
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

fn action_token(html: &str, action: &str, workspace: Option<&str>) -> String {
    html.split("<form ")
        .skip(1)
        .find_map(|form| {
            let form = form.split("</form>").next().unwrap();
            (form.contains(&format!("action=\"{action}\""))
                && workspace
                    .is_none_or(|id| form.contains(&format!("name=\"workspace\" value=\"{id}\""))))
            .then(|| field(form, "csrf"))
        })
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
    let page = request(
        &fixture.site,
        Method::GET,
        "/cloud/sign-in",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    private(&page);
    let nonce = migrated_cookie(&page, "oa_cloud_login");
    assert!(nonce.contains("HttpOnly; SameSite=Strict"));
    assert!(nonce.contains("; Path=/;"));
    cookies.apply(&page);
    let csrf = field(&page.body, "csrf");
    let input = form(&[("credential", &credential(account)), ("csrf", &csrf)]);
    let answer = request(
        &fixture.site,
        Method::POST,
        "/cloud/sign-in",
        &cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    private(&answer);
    assert_eq!(answer.headers[header::LOCATION], "/cloud/app");
    for name in ["oa_cloud_session", "oa_cloud_workspace", "oa_cloud_login"] {
        migrated_cookie(&answer, name);
    }
    cookies.apply(&answer);
    assert!(!cookies.0.contains_key("oa_cloud_login"));
    assert_eq!(cookies.0["oa_cloud_session"], token(account));
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
    let response = request(
        &fixture.site,
        Method::GET,
        "/cloud/sign-in",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::SEE_OTHER);
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
    let page = request(
        &fixture.site,
        Method::GET,
        "/cloud/sign-in",
        &cookies,
        None,
        None,
    )
    .await;
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
        "/cloud/sign-in",
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

struct Resident {
    running: Option<coder_host::Running>,
    relay: tokio::task::JoinHandle<()>,
    authority: coder_access::host::Host,
    device: String,
    task: String,
    config: PathBuf,
    secret: PathBuf,
}

async fn choose_personal(fixture: &Fixture, cookies: &mut Cookies) {
    let page = request(
        &fixture.site,
        Method::GET,
        "/cloud/app",
        cookies,
        None,
        None,
    )
    .await;
    let csrf = action_token(
        &page.body,
        "/cloud/select-workspace",
        Some("alice-personal"),
    );
    let input = form(&[("workspace", "alice-personal"), ("csrf", &csrf)]);
    let answer = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    cookies.apply(&answer);
}

impl Resident {
    async fn stop(mut self) {
        if let Some(running) = self.running.take() {
            running.shutdown().await;
        }
        self.relay.abort();
    }
}

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

async fn resident(fixture: &mut Fixture) -> Resident {
    resident_with_controls(
        fixture,
        coder_access::Rights::new([coder_access::Right::Observe]).unwrap(),
        false,
    )
    .await
}

async fn resident_with_controls(
    fixture: &mut Fixture,
    rights: coder_access::Rights,
    controls: bool,
) -> Resident {
    resident_with_projects(fixture, rights, controls, false).await
}

async fn resident_with_projects(
    fixture: &mut Fixture,
    rights: coder_access::Rights,
    controls: bool,
    projects: bool,
) -> Resident {
    resident_with_services(fixture, rights, controls, projects, false).await
}

async fn resident_with_services(
    fixture: &mut Fixture,
    rights: coder_access::Rights,
    controls: bool,
    projects: bool,
    cloud: bool,
) -> Resident {
    resident_with_inbox(fixture, rights, controls, projects, cloud, |inbox, _| inbox).await
}

/// A resident whose task owner `extend` completes, such as with workshop agents.
async fn resident_with_inbox(
    fixture: &mut Fixture,
    rights: coder_access::Rights,
    controls: bool,
    projects: bool,
    cloud: bool,
    extend: impl FnOnce(coder::task::remote::Inbox, &std::path::Path) -> coder::task::remote::Inbox,
) -> Resident {
    use coder_host::Tasks;
    let private = fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap();
    let root = private.join("resident-checkout");
    std::fs::create_dir(&root).unwrap();
    let workspaces = BTreeMap::from([("checkout".into(), root)]);
    let device = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let device_id = coder_access::protocol::pubkey(&device);
    let mut inbox =
        coder::task::remote::Inbox::new(private.join("resident-tasks"), workspaces.clone())
            .with_settings(private.join("unused-resident-settings"));
    if projects {
        inbox = inbox.with_projects(Arc::new(
            project_fixture::observer(private, &device_id, &workspaces).unwrap(),
        ));
    }
    let task = "c".repeat(64);
    inbox
        .create(
            &task,
            &device_id,
            &coder_access::protocol::TaskCreate {
                title: "Synthetic resident task <script>".into(),
                prompt: "Original private request **retained**".into(),
                workspace: "checkout".into(),
                images: vec![],
                engine: None,
            },
        )
        .unwrap();
    let (relay_url, relay, _) = task_relay::start().await;
    let state = private.join("resident-access");
    let authority = coder_access::host::Host::new(&state, coder_access::RelayPolicy::LoopbackTest);
    let owner = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    authority
        .init(&coder_access::protocol::pubkey(&owner))
        .unwrap();
    let invitation = authority
        .invite(&relay_url, rights, now(), now() + 3600)
        .unwrap();
    let parsed = coder_access::protocol::HostInvitation::parse(
        &invitation.code,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let pending = coder_access::client::prepare_redeem(
        &parsed,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let answer = authority
        .handle_redemption(&pending.event, || Ok(now()))
        .unwrap();
    let access = coder_access::client::finish_redeem(
        &parsed,
        &pending,
        &answer,
        &device,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let mut host = coder_host::config::Config::new(state, vec![relay_url], 7);
    host.policy = coder_access::RelayPolicy::LoopbackTest;
    host.workspaces = workspaces;
    host.telemetry = false;
    if cloud {
        let standing = coder_host::cloud::authority(coder_access::host::Host::new(
            private.join("resident-access"),
            coder_access::RelayPolicy::LoopbackTest,
        ))
        .unwrap();
        inbox = inbox.with_cloud(Arc::new(
            operator_fixture::operator(private, standing, &device_id, &host.workspaces).unwrap(),
        ));
    }
    let inbox = extend(inbox, private);
    let running = coder_host::start(host, Arc::new(inbox)).await.unwrap();
    let secret = private.join("resident-device.key");
    let access_path = private.join("resident-device.access");
    private_file(&secret, &device.secret_bytes());
    private_file(&access_path, &serde_json::to_vec(&access).unwrap());
    let config = private.join("hosts.json");
    let mut document = json!({"schema":"openagents.cloud.host-bindings.v1","bindings":[{"id":"resident","account":"alice","workspace":"alice-personal","members_epoch":3,"host_workspace":"checkout","host_generation":7,"route":format!("tcp://{}",running.local_addr()),"access_file":access_path,"device_secret":secret}]});
    if controls {
        let journal = private.join("browser-controls");
        std::fs::create_dir(&journal).unwrap();
        std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(0o700)).unwrap();
        document["controls"] = json!({"directory":journal,"bindings":["resident"]});
    }
    private_file(&config, &serde_json::to_vec(&document).unwrap());
    fixture.config.cloud_hosts = Some(Arc::new(super::hosts::Hosts::load(&config).unwrap()));
    fixture.site = crate::router(fixture.config.clone());
    Resident {
        running: Some(running),
        relay,
        authority,
        device: device_id,
        task,
        config,
        secret,
    }
}

fn opening_tag<'a>(body: &'a str, id: &str) -> &'a str {
    let position = body
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("The admitted page is missing {id}."));
    let start = body[..position].rfind('<').unwrap();
    let end = position + body[position..].find('>').unwrap() + 1;
    &body[start..end]
}

fn private_mount(body: &str) {
    let tag = opening_tag(body, "cloud-private");
    assert!(tag.starts_with("<div "));
    assert!(tag.contains(" hidden>") || tag.contains(" hidden "));
    assert!(tag.contains(" hx-history=\"false\""));
}

fn resource_descriptor(body: &str) -> Value {
    let tag = opening_tag(body, "cloud-resource-standing");
    assert!(tag.starts_with("<pre "));
    assert!(tag.contains(" hidden>") || tag.contains(" hidden "));
    let start = body
        .split_once(tag)
        .unwrap()
        .1
        .split("</pre>")
        .next()
        .unwrap();
    let decoded = start
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    serde_json::from_str(&decoded).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_reads_are_native_scoped_and_clear_after_native_revocation() {
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
    private(&list);
    assert!(list.body.contains("Synthetic resident task &lt;script&gt;"));
    private_mount(&list.body);
    assert!(!fixture.local_store.exists());
    let route = format!("/cloud/app/hosts/resident/tasks/{}", native.task);
    let task = request(&fixture.site, Method::GET, &route, &cookies, None, None).await;
    assert_eq!(task.status, StatusCode::OK, "{}", task.body);
    private(&task);
    assert!(task.body.contains("Original private request"));
    assert!(task.body.contains("Cost: Unknown"));
    assert!(task.body.contains("Integration: unknown"));
    let descriptor = resource_descriptor(&task.body);
    let endpoint = descriptor["endpoint"].as_str().unwrap();
    let standing = request(&fixture.site, Method::GET, endpoint, &cookies, None, None).await;
    assert_eq!(standing.status, StatusCode::OK, "{}", standing.body);
    assert_eq!(
        serde_json::from_str::<Value>(&standing.body).unwrap()["identity"],
        descriptor["identity"]
    );
    fixture.state.lock().unwrap().team_name = Some("Changed unselected membership".into());
    let changed = request(&fixture.site, Method::GET, endpoint, &cookies, None, None).await;
    assert_eq!(changed.status, StatusCode::OK);
    assert_ne!(
        serde_json::from_str::<Value>(&changed.body).unwrap()["identity"],
        descriptor["identity"]
    );
    fixture.state.lock().unwrap().team_name = None;
    let artifact = task
        .body
        .split("href=\"")
        .filter_map(|s| s.split('"').next())
        .find(|s| s.contains("/original?cursor="))
        .unwrap()
        .replace("&amp;", "&");
    let chunk = request(&fixture.site, Method::GET, &artifact, &cookies, None, None).await;
    assert_eq!(chunk.status, StatusCode::OK, "{}", chunk.body);
    assert!(chunk.body.contains("Original chunk bytes (base64)"));
    let bob = login(&fixture, "bob").await;
    for route in [
        &route,
        "/cloud/app/hosts/resident/tasks",
        endpoint,
        &artifact,
    ] {
        let refused = request(&fixture.site, Method::GET, route, &bob, None, None).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        assert!(!refused.body.contains("Original private request"));
    }
    let mutation = request(
        &fixture.site,
        Method::POST,
        &route,
        &cookies,
        Some(""),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(mutation.status, StatusCode::METHOD_NOT_ALLOWED);
    native.authority.revoke(&native.device, now()).unwrap();
    let still_account = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(still_account.status, StatusCode::OK);
    let lost = request(&fixture.site, Method::GET, endpoint, &cookies, None, None).await;
    assert!(matches!(
        lost.status,
        StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
    ));
    assert!(!lost.body.contains("Original private request"));
    assert!(!fixture.local_store.exists());
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resident_projection_rechecks_membership_and_pinned_configuration() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    fixture.state.lock().unwrap().epoch = 4;
    let lost = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/hosts/resident/tasks",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(lost.status, StatusCode::FORBIDDEN);
    fixture.state.lock().unwrap().epoch = 3;
    private_file(&native.secret, &[1; 32]);
    let changed = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/hosts/resident/tasks",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(changed.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!changed.body.contains("Synthetic resident task"));
    assert!(native.config.exists());
    assert!(!fixture.local_store.exists());
    native.stop().await;
}

#[tokio::test]
async fn login_shell_standing_switch_and_logout_use_the_native_session() {
    let fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    let shell = request(
        &fixture.site,
        Method::GET,
        "/cloud/app",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(shell.status, StatusCode::OK);
    private(&shell);
    private_mount(&shell.body);
    assert!(shell.body.contains("<pre id=\"cloud-standing\" hidden>"));
    assert!(shell.body.contains("alice &lt;account&gt;"));
    assert!(shell.body.contains("No connected work to report"));
    let standing = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    private(&standing);
    let value: Value = serde_json::from_str(&standing.body).unwrap();
    assert_eq!(value["account"], "alice");
    assert!(value["workspace"].is_null());
    let csrf = action_token(
        &shell.body,
        "/cloud/select-workspace",
        Some("alice-personal"),
    );
    let input = form(&[("workspace", "alice-personal"), ("csrf", &csrf)]);
    let switched = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        &cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(switched.status, StatusCode::SEE_OTHER);
    private(&switched);
    assert!(
        migrated_cookie(&switched, "oa_cloud_workspace")
            .starts_with("oa_cloud_workspace=alice-personal;")
    );
    cookies.apply(&switched);
    assert_eq!(cookies.0["oa_cloud_workspace"], "alice-personal");
    let shell = request(
        &fixture.site,
        Method::GET,
        "/cloud/app",
        &cookies,
        None,
        None,
    )
    .await;
    let standing = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    let value: Value = serde_json::from_str(&standing.body).unwrap();
    assert_eq!(value["workspace"], "alice-personal");
    assert_eq!(value["members_epoch"], 3);
    let csrf = action_token(&shell.body, "/cloud/sign-out", None);
    let payload = URL_SAFE_NO_PAD
        .decode(csrf.split('.').next().unwrap())
        .unwrap();
    let payload = String::from_utf8(payload).unwrap();
    assert!(!payload.contains("alice"));
    assert!(!payload.contains("native-session-alice"));
    let ticket: Value = serde_json::from_str(&payload).unwrap();
    assert!(ticket["viewer"].is_null());
    assert!(ticket["scope"].as_str() == Some("sign-out"));
    let resume = shell
        .body
        .split("id=\"cloud-resume\"")
        .nth(1)
        .unwrap()
        .split("</section>")
        .next()
        .unwrap();
    assert!(resume.contains("action=\"/cloud/sign-out\""));
    assert!(!resume.contains("alice &lt;account&gt;"));
    let input = form(&[("csrf", &csrf)]);
    let ended = request(
        &fixture.site,
        Method::POST,
        "/cloud/sign-out",
        &cookies,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(ended.status, StatusCode::SEE_OTHER);
    private(&ended);
    for name in ["oa_cloud_session", "oa_cloud_workspace", "oa_cloud_login"] {
        assert!(migrated_cookie(&ended, name).contains("; Max-Age=0"));
    }
    cookies.apply(&ended);
    assert!(cookies.0.is_empty());
    assert_eq!(fixture.state.lock().unwrap().signouts, 1);
    let refused = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    private(&refused);
    assert!(!fixture.local_store.exists());
}

#[tokio::test]
async fn switch_tickets_bind_the_exact_workspace_session_and_displayed_membership() {
    let fixture = fixture().await;
    let mut alice = login(&fixture, "alice").await;
    let bob = login(&fixture, "bob").await;
    let shell = request(&fixture.site, Method::GET, "/cloud/app", &alice, None, None).await;
    let csrf = action_token(
        &shell.body,
        "/cloud/select-workspace",
        Some("alice-personal"),
    );
    for (cookies, target) in [(&alice, "alice-team"), (&bob, "alice-personal")] {
        let input = form(&[("workspace", target), ("csrf", &csrf)]);
        let refused = request(
            &fixture.site,
            Method::POST,
            "/cloud/select-workspace",
            cookies,
            Some(&input),
            Some(ORIGIN),
        )
        .await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        private(&refused);
        assert!(!refused.headers.contains_key(header::SET_COOKIE));
    }
    let input = form(&[("workspace", "alice-personal"), ("csrf", &csrf)]);
    let switched = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        &alice,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    alice.apply(&switched);
    let shell = request(&fixture.site, Method::GET, "/cloud/app", &alice, None, None).await;
    let csrf = action_token(&shell.body, "/cloud/select-workspace", Some("alice-team"));
    fixture.state.lock().unwrap().epoch = 4;
    let input = form(&[("workspace", "alice-team"), ("csrf", &csrf)]);
    let stale = request(
        &fixture.site,
        Method::POST,
        "/cloud/select-workspace",
        &alice,
        Some(&input),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(stale.status, StatusCode::FORBIDDEN);
    private(&stale);
    assert!(!stale.headers.contains_key(header::SET_COOKIE));
    let bob_shell = request(&fixture.site, Method::GET, "/cloud/app", &bob, None, None).await;
    assert!(!bob_shell.body.contains("Alice personal"));
    assert!(!bob_shell.body.contains("Alice team"));
    alice
        .0
        .insert("oa_cloud_workspace".into(), "bob-personal".into());
    let denied = request(&fixture.site, Method::GET, "/cloud/app", &alice, None, None).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    private(&denied);
    assert!(!denied.body.contains("Bob personal"));
}

#[tokio::test]
async fn cross_origin_malformed_and_unauthenticated_forms_create_no_native_session() {
    let fixture = fixture().await;
    let mut cookies = Cookies::default();
    let page = request(
        &fixture.site,
        Method::GET,
        "/cloud/sign-in",
        &cookies,
        None,
        None,
    )
    .await;
    cookies.apply(&page);
    let csrf = field(&page.body, "csrf");
    let input = form(&[("credential", &credential("alice")), ("csrf", &csrf)]);
    for origin in [None, Some("https://other.example.invalid")] {
        let rejected = request(
            &fixture.site,
            Method::POST,
            "/cloud/sign-in",
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
        "/cloud/sign-in",
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
        "/cloud/sign-in",
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
        "/cloud/sign-in",
        &cookies,
        Some(&oversize),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(bounded.status, StatusCode::BAD_REQUEST);
    private(&bounded);
    assert_eq!(fixture.state.lock().unwrap().signins, 0);
    let absent = request(
        &fixture.site,
        Method::GET,
        "/cloud/app",
        &Cookies::default(),
        None,
        None,
    )
    .await;
    assert_eq!(absent.status, StatusCode::SEE_OTHER);
    private(&absent);
    assert_eq!(absent.headers[header::LOCATION], "/cloud/sign-in");
    for path in ["/cloud/select-workspace", "/cloud/sign-out"] {
        let input = if path == "/cloud/select-workspace" {
            form(&[("workspace", "alice-personal"), ("csrf", &csrf)])
        } else {
            form(&[("csrf", &csrf)])
        };
        let absent = request(
            &fixture.site,
            Method::POST,
            path,
            &Cookies::default(),
            Some(&input),
            Some(ORIGIN),
        )
        .await;
        assert_eq!(
            absent.status,
            if path == "/cloud/sign-out" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
        private(&absent);
    }
}

#[tokio::test]
async fn revocation_and_removed_selected_membership_clear_the_private_projection() {
    let fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    cookies
        .0
        .insert("oa_cloud_workspace".into(), "alice-personal".into());
    fixture.state.lock().unwrap().removed = true;
    let removed = request(
        &fixture.site,
        Method::GET,
        "/cloud/app",
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
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::UNAUTHORIZED);
    private(&revoked);
    assert!(!revoked.body.contains("native-session-alice"));
}

#[tokio::test]
async fn standing_digest_fences_changed_unselected_workspace_projection() {
    let fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    cookies
        .0
        .insert("oa_cloud_workspace".into(), "alice-personal".into());
    let initial = request(
        &fixture.site,
        Method::GET,
        "/cloud/app/session",
        &cookies,
        None,
        None,
    )
    .await;
    assert_eq!(initial.status, StatusCode::OK);
    private(&initial);
    let initial: Value = serde_json::from_str(&initial.body).unwrap();
    assert!(
        initial["projection_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    for change in ["name", "role", "removal"] {
        {
            let mut state = fixture.state.lock().unwrap();
            state.team_name = (change == "name").then(|| "Renamed team".into());
            state.team_role = (change == "role").then(|| "admin".into());
            state.team_removed = change == "removal";
        }
        let current = request(
            &fixture.site,
            Method::GET,
            "/cloud/app/session",
            &cookies,
            None,
            None,
        )
        .await;
        assert_eq!(current.status, StatusCode::OK);
        private(&current);
        let current: Value = serde_json::from_str(&current.body).unwrap();
        assert_eq!(current["workspace"], "alice-personal");
        assert_eq!(current["members_epoch"], 3);
        assert_ne!(
            current["projection_digest"], initial["projection_digest"],
            "{change}"
        );
    }
}

#[tokio::test]
async fn valid_logout_clears_browser_after_native_or_configuration_standing_changes() {
    for change in ["membership", "offline", "configuration"] {
        let fixture = fixture().await;
        let mut cookies = login(&fixture, "alice").await;
        cookies
            .0
            .insert("oa_cloud_workspace".into(), "alice-personal".into());
        let shell = request(
            &fixture.site,
            Method::GET,
            "/cloud/app",
            &cookies,
            None,
            None,
        )
        .await;
        assert_eq!(shell.status, StatusCode::OK);
        let csrf = action_token(&shell.body, "/cloud/sign-out", None);
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
                "/cloud/sign-out",
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
            "/cloud/sign-out",
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
            assert!(ended.body.contains("native logout outcome is unknown"));
            assert_eq!(fixture.state.lock().unwrap().signouts, 0);
        } else {
            assert_eq!(ended.status, StatusCode::SEE_OTHER);
            assert_eq!(ended.headers[header::LOCATION], "/cloud");
            assert_eq!(fixture.state.lock().unwrap().signouts, 1);
        }
        assert!(!fixture.local_store.exists());
    }
}

/// WEB-17 packaging: every generated Rust/Wasm file the Cloud routes serve
/// from `--cloud-build` is built and checked by both site images, so a
/// deployed workbench never loads a missing terminal module.
#[test]
fn site_images_package_every_served_cloud_build_asset() {
    for (name, dockerfile) in [
        ("Dockerfile", include_str!("../../Dockerfile")),
        (
            "Dockerfile.components",
            include_str!("../../Dockerfile.components"),
        ),
    ] {
        for asset in super::BUILD_ASSETS {
            assert!(
                dockerfile.contains(&format!("test -s /build/cloud/{asset}")),
                "{name} does not check {asset}"
            );
        }
        for package in ["-p coder-cloud-web", "-p coder-browser-web"] {
            assert!(
                dockerfile.contains(package),
                "{name} does not build {package}"
            );
        }
    }
    assert!(include_str!("../../Dockerfile").contains("\"--cloud-build\", \"/srv/cloud\""));
}

/// WEB-17 accessibility basics for the authenticated shell: one skip link
/// to `<main>`, a labelled workspace navigation that marks the current
/// section, a live region for standing changes, and named buttons.
#[tokio::test]
async fn workspace_shell_keeps_keyboard_and_screen_reader_basics() {
    let fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    for (path, current) in [
        (
            "/cloud/app",
            "<a aria-current=\"page\" href=\"/cloud/app\">Overview</a>",
        ),
        (
            "/cloud/app/settings",
            "<a aria-current=\"page\" href=\"/cloud/app/settings\">Settings</a>",
        ),
    ] {
        let page = request(&fixture.site, Method::GET, path, &cookies, None, None).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let body = &page.body;
        assert!(body.contains("<html lang=\"en\""));
        assert!(body.contains("<a class=\"oa-skip-link\" href=\"#content\">"));
        assert!(body.contains("<main id=\"content\""));
        assert!(body.contains("<nav aria-label=\"Workspace\">"));
        // One current section in the nav; the breadcrumb also marks the page.
        let breadcrumb = body
            .matches("oa-breadcrumb-current\" aria-current=\"page\"")
            .count();
        assert_eq!(
            body.matches("aria-current=\"page\"").count() - breadcrumb,
            1,
            "{path}"
        );
        assert!(body.contains(current), "{path}");
        assert!(body.contains("id=\"cloud-resume\" aria-live=\"polite\""));
        assert!(body.contains("name=\"viewport\""));
        for button in body.split("<button").skip(1) {
            let text = button
                .split_once('>')
                .and_then(|(_, rest)| rest.split_once("</button>"))
                .map(|(text, _)| text.trim())
                .unwrap_or_default();
            assert!(!text.is_empty(), "{path}: unnamed button");
        }
    }
}
