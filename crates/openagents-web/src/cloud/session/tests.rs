use super::*;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

const ALICE_KEY: &str = "oak_alice.synthetic-private-key";
const BOB_KEY: &str = "oak_bob.synthetic-private-key";
const CANARY: &str = "native-private-response-canary";

struct Native {
    revoked: bool,
    expiry: u64,
    alice_member: bool,
    epoch: u64,
    changes_after_details: bool,
    signouts: u64,
    signins: u64,
    redirect: bool,
    unavailable: bool,
    reads: u64,
}

struct Fixture {
    _root: tempfile::TempDir,
    config: PathBuf,
    secret: PathBuf,
    adapter: CloudSession,
    state: Arc<Mutex<Native>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn token(account: &str) -> String {
    format!(
        "sess_{}",
        if account == "alice" { "a" } else { "b" }.repeat(64)
    )
}

fn account(headers: &HeaderMap, native: &Native) -> Option<&'static str> {
    if native.revoked {
        return None;
    }
    let auth = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    if auth == token("alice") {
        Some("alice")
    } else if auth == token("bob") {
        Some("bob")
    } else {
        None
    }
}

fn refused(status: StatusCode) -> Response {
    (
        status,
        Json(json!({"error":{"code":"unauthenticated","message":CANARY}})),
    )
        .into_response()
}

async fn signin(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut native = state.lock().unwrap();
    native.signins += 1;
    if native.redirect {
        return (
            StatusCode::TEMPORARY_REDIRECT,
            [(header::LOCATION, "/v1/sessions")],
        )
            .into_response();
    }
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let account = if auth == format!("Bearer {ALICE_KEY}") {
        "alice"
    } else if auth == format!("Bearer {BOB_KEY}") {
        "bob"
    } else {
        return refused(StatusCode::UNAUTHORIZED);
    };
    Json(json!({"session":{"id":format!("session-{account}"),"kind":"user","account":account,"created_at":now(),"expires_at":native.expiry},"token":token(account)})).into_response()
}

async fn session(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut native = state.lock().unwrap();
    native.reads += 1;
    let Some(account) = account(&headers, &native) else {
        return refused(StatusCode::UNAUTHORIZED);
    };
    Json(json!({"session":{"id":format!("session-{account}"),"kind":"user","account":account,"created_at":now()-1,"expires_at":native.expiry,"state":"active"}})).into_response()
}

async fn details(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut native = state.lock().unwrap();
    native.reads += 1;
    let Some(account) = account(&headers, &native) else {
        return refused(StatusCode::UNAUTHORIZED);
    };
    let workspaces = if account == "alice" && !native.alice_member {
        vec![]
    } else {
        vec![
            json!({"id":format!("{account}-workspace"),"name":format!("{account} work"),"role":if account == "alice" {"owner"} else {"member"}}),
        ]
    };
    if native.changes_after_details {
        native.revoked = true;
    }
    Json(json!({"account":{"id":account,"label":format!("{account} account"),"principals":[CANARY]},"workspaces":workspaces})).into_response()
}

async fn workspace(
    State(state): State<Arc<Mutex<Native>>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let mut native = state.lock().unwrap();
    native.reads += 1;
    let Some(account) = account(&headers, &native) else {
        return refused(StatusCode::UNAUTHORIZED);
    };
    if id != format!("{account}-workspace") || account == "alice" && !native.alice_member {
        return refused(StatusCode::FORBIDDEN);
    }
    Json(json!({"workspace":{"id":id,"tenant":"synthetic","members_epoch":native.epoch},"role":if account == "alice" {"owner"} else {"member"}})).into_response()
}

async fn signout(State(state): State<Arc<Mutex<Native>>>, headers: HeaderMap) -> Response {
    let mut native = state.lock().unwrap();
    if native.unavailable {
        return refused(StatusCode::SERVICE_UNAVAILABLE);
    }
    if account(&headers, &native).is_none() {
        return refused(StatusCode::UNAUTHORIZED);
    }
    native.signouts += 1;
    native.revoked = true;
    Json(json!({"session":{"state":"revoked"}})).into_response()
}

async fn fixture() -> Fixture {
    let state = Arc::new(Mutex::new(Native {
        revoked: false,
        expiry: now() + 3600,
        alice_member: true,
        epoch: 3,
        changes_after_details: false,
        signouts: 0,
        signins: 0,
        redirect: false,
        unavailable: false,
        reads: 0,
    }));
    let router = Router::new()
        .route("/v1/sessions", post(signin))
        .route("/v1/session", get(session).delete(signout))
        .route("/v1/account", get(details))
        .route("/v1/workspaces/{id}", get(workspace))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config = directory.join("cloud.json");
    let secret = directory.join("csrf.key");
    std::fs::write(&secret, [11; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&config, serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":endpoint,"csrf_secret":secret})).unwrap()).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    let adapter = CloudSession::load(&config).unwrap();
    Fixture {
        _root: root,
        config,
        secret,
        adapter,
        state,
        server,
    }
}

fn headers(account: &str, workspace: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4300"));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://127.0.0.1:4300"),
    );
    let mut value = format!("{SESSION_COOKIE}={}", token(account));
    if let Some(id) = workspace {
        value.push_str(&format!("; {WORKSPACE_COOKIE}={id}"));
    }
    let mut cookie = HeaderValue::from_str(&value).unwrap();
    cookie.set_sensitive(true);
    headers.insert(header::COOKIE, cookie);
    headers
}

#[tokio::test]
async fn native_accounts_remain_isolated_and_credentials_stay_private() {
    let fixture = fixture().await;
    let grant = fixture.adapter.sign_in(ALICE_KEY).await.unwrap();
    assert_eq!(grant.viewer.account_id, "alice");
    assert!(grant.viewer.workspace.is_none());
    let cookie = grant.cookies().unwrap();
    assert!(cookie[0].is_sensitive());
    assert!(
        cookie[0]
            .to_str()
            .unwrap()
            .contains("HttpOnly; SameSite=Strict")
    );
    assert!(!format!("{:?}", cookie[0]).contains(&token("alice")));
    let alice = fixture
        .adapter
        .authenticate(&headers("alice", Some("alice-workspace")))
        .await
        .unwrap();
    assert_eq!(alice.workspace.as_ref().unwrap().members_epoch, 3);
    assert_eq!(alice.workspaces.len(), 1);
    let public = serde_json::to_string(&alice.workspaces).unwrap();
    assert!(!public.contains(CANARY));
    assert!(!public.contains(ALICE_KEY));
    assert!(!public.contains(&token("alice")));
    let bob = fixture
        .adapter
        .authenticate(&headers("bob", Some("bob-workspace")))
        .await
        .unwrap();
    assert_eq!(bob.account_id, "bob");
    assert_eq!(bob.workspace.unwrap().role, "member");
    assert!(matches!(
        fixture
            .adapter
            .authenticate(&headers("bob", Some("alice-workspace")))
            .await,
        Err(SessionError::Forbidden)
    ));
    assert!(matches!(
        fixture
            .adapter
            .select_workspace(&headers("alice", None), "bob-workspace")
            .await,
        Err(SessionError::Forbidden)
    ));
    let error = fixture
        .adapter
        .sign_in("oak_other.synthetic-private-key")
        .await
        .err()
        .unwrap();
    assert_eq!(error, SessionError::Unauthenticated);
    assert!(!error.to_string().contains(CANARY));
}

#[tokio::test]
async fn expiry_revocation_recovery_and_removed_membership_fence_reads() {
    let fixture = fixture().await;
    let input = headers("alice", Some("alice-workspace"));
    fixture.adapter.authenticate(&input).await.unwrap();
    fixture.state.lock().unwrap().expiry = now() - 1;
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::Unauthenticated)
    ));
    fixture.state.lock().unwrap().expiry = now() + 3600;
    fixture.state.lock().unwrap().alice_member = false;
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::Forbidden)
    ));
    // An invalid selected workspace never silently switches to another one.
    assert!(matches!(
        fixture
            .adapter
            .select_workspace(&input, "bob-workspace")
            .await,
        Err(SessionError::Forbidden)
    ));
    fixture.state.lock().unwrap().alice_member = true;
    fixture.state.lock().unwrap().revoked = true;
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::Unauthenticated)
    ));
    fixture.state.lock().unwrap().revoked = false;
    fixture.state.lock().unwrap().changes_after_details = true;
    assert!(matches!(
        fixture.adapter.authenticate(&headers("alice", None)).await,
        Err(SessionError::Unauthenticated)
    ));
}

#[tokio::test]
async fn current_session_logout_is_a_single_native_operation() {
    let fixture = fixture().await;
    fixture
        .adapter
        .sign_out_current(&headers("alice", None))
        .await
        .unwrap();
    assert_eq!(fixture.state.lock().unwrap().signouts, 1);
    assert!(matches!(
        fixture
            .adapter
            .sign_out_current(&headers("alice", None))
            .await,
        Err(SessionError::Unauthenticated)
    ));
    assert_eq!(fixture.state.lock().unwrap().signouts, 1);
    assert_eq!(fixture.state.lock().unwrap().reads, 0);
    assert!(
        fixture
            .adapter
            .clear_cookies()
            .iter()
            .all(|cookie| cookie.to_str().unwrap().contains("Max-Age=0"))
    );
}

#[tokio::test]
async fn reviewed_logout_survives_removed_membership_without_account_reads() {
    let fixture = fixture().await;
    let input = headers("alice", Some("alice-workspace"));
    let viewer = fixture.adapter.authenticate(&input).await.unwrap();
    let ticket = fixture.adapter.logout_csrf(&input, &viewer).unwrap();
    let reads = fixture.state.lock().unwrap().reads;
    {
        let mut native = fixture.state.lock().unwrap();
        native.alice_member = false;
        native.epoch += 1;
    }
    fixture.adapter.verify_logout_csrf(&input, &ticket).unwrap();
    fixture.adapter.sign_out_current(&input).await.unwrap();
    let native = fixture.state.lock().unwrap();
    assert_eq!(native.reads, reads);
    assert_eq!(native.signouts, 1);
    assert!(native.revoked);
}

#[tokio::test]
async fn reviewed_local_logout_survives_unavailable_or_revoked_native_standing() {
    let fixture = fixture().await;
    let input = headers("alice", Some("alice-workspace"));
    let viewer = fixture.adapter.authenticate(&input).await.unwrap();
    let ticket = fixture.adapter.logout_csrf(&input, &viewer).unwrap();
    let reads = fixture.state.lock().unwrap().reads;
    fixture.state.lock().unwrap().unavailable = true;
    fixture.adapter.verify_logout_csrf(&input, &ticket).unwrap();
    assert_eq!(
        fixture.adapter.sign_out_current(&input).await,
        Err(SessionError::Unavailable)
    );
    {
        let mut native = fixture.state.lock().unwrap();
        native.unavailable = false;
        native.revoked = true;
    }
    fixture.adapter.verify_logout_csrf(&input, &ticket).unwrap();
    assert_eq!(
        fixture.adapter.sign_out_current(&input).await,
        Err(SessionError::Unauthenticated)
    );
    // The original loaded signer can authorize local narrowing after a reload
    // fence, but changed configuration cannot dispatch any native operation.
    std::fs::write(&fixture.secret, [12; 32]).unwrap();
    fixture.adapter.verify_logout_csrf(&input, &ticket).unwrap();
    assert!(matches!(
        fixture.adapter.logout_csrf(&input, &viewer),
        Err(SessionError::Unavailable)
    ));
    assert_eq!(
        fixture.adapter.sign_out_current(&input).await,
        Err(SessionError::Unavailable)
    );
    let native = fixture.state.lock().unwrap();
    assert_eq!(native.reads, reads);
    assert_eq!(native.signouts, 0);
    assert!(
        fixture
            .adapter
            .clear_cookies()
            .iter()
            .all(|cookie| cookie.to_str().unwrap().contains("Max-Age=0"))
    );
}

#[tokio::test]
async fn local_logout_requires_original_cookie_and_exact_unexpired_review() {
    let fixture = fixture().await;
    fixture.state.lock().unwrap().expiry = now() + 60;
    let input = headers("alice", Some("alice-workspace"));
    let viewer = fixture.adapter.authenticate(&input).await.unwrap();
    let ticket = fixture.adapter.logout_csrf(&input, &viewer).unwrap();
    let public = URL_SAFE_NO_PAD
        .decode(ticket.split_once('.').unwrap().0)
        .unwrap();
    let decoded: Ticket = serde_json::from_slice(&public).unwrap();
    assert!(decoded.viewer.is_none());
    assert!(decoded.expires_at <= viewer.expires_at);
    let public = String::from_utf8(public).unwrap();
    for private in [
        viewer.account_id.as_str(),
        viewer.account_label.as_str(),
        viewer.session_id.as_str(),
        "alice-workspace",
        ALICE_KEY,
        &token("alice"),
    ] {
        assert!(!public.contains(private));
    }
    assert_eq!(
        fixture
            .adapter
            .verify_csrf(&input, Some(&viewer), "sign-out", "", &ticket),
        Err(SessionError::Csrf)
    );
    let changed_selection = headers("alice", Some("different-workspace"));
    fixture
        .adapter
        .verify_logout_csrf(&changed_selection, &ticket)
        .unwrap();
    assert_eq!(
        fixture
            .adapter
            .verify_logout_csrf(&headers("bob", None), &ticket),
        Err(SessionError::Csrf)
    );
    let wrong_action = fixture
        .adapter
        .csrf(&input, &viewer, "select-workspace", "alice-workspace")
        .unwrap();
    assert_eq!(
        fixture.adapter.verify_logout_csrf(&input, &wrong_action),
        Err(SessionError::Csrf)
    );
    let private_identity = fixture
        .adapter
        .csrf(&input, &viewer, "sign-out", "")
        .unwrap();
    assert_eq!(
        fixture
            .adapter
            .verify_logout_csrf(&input, &private_identity),
        Err(SessionError::Csrf)
    );
    let mut cross_origin = input.clone();
    cross_origin.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://attacker.example"),
    );
    assert_eq!(
        fixture.adapter.verify_logout_csrf(&cross_origin, &ticket),
        Err(SessionError::Csrf)
    );
    assert_eq!(
        fixture.adapter.sign_out_current(&cross_origin).await,
        Err(SessionError::Csrf)
    );
    let mut expired: Ticket = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(ticket.split_once('.').unwrap().0)
            .unwrap(),
    )
    .unwrap();
    expired.issued_at = now() - TICKET_SECONDS - 10;
    expired.expires_at = now() - 10;
    let payload = serde_json::to_vec(&expired).unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(&fixture.adapter.csrf_key).unwrap();
    mac.update(b"openagents.cloud.csrf.v1\0");
    mac.update(&payload);
    let expired = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    );
    assert_eq!(
        fixture.adapter.verify_logout_csrf(&input, &expired),
        Err(SessionError::Csrf)
    );
    assert_eq!(fixture.state.lock().unwrap().signouts, 0);
}

#[tokio::test]
async fn csrf_binds_native_session_action_target_origin_and_membership_epoch() {
    let fixture = fixture().await;
    let input = headers("alice", Some("alice-workspace"));
    let viewer = fixture.adapter.authenticate(&input).await.unwrap();
    let ticket = fixture
        .adapter
        .csrf(&input, &viewer, "select-workspace", "alice-workspace")
        .unwrap();
    fixture
        .adapter
        .verify_csrf(
            &input,
            Some(&viewer),
            "select-workspace",
            "alice-workspace",
            &ticket,
        )
        .unwrap();
    assert_eq!(
        fixture
            .adapter
            .verify_csrf(&input, Some(&viewer), "sign-out", "", &ticket),
        Err(SessionError::Csrf)
    );
    assert_eq!(
        fixture.adapter.verify_csrf(
            &input,
            Some(&viewer),
            "select-workspace",
            "bob-workspace",
            &ticket
        ),
        Err(SessionError::Csrf)
    );
    assert_eq!(
        fixture.adapter.verify_csrf(
            &headers("bob", Some("bob-workspace")),
            Some(&viewer),
            "select-workspace",
            "alice-workspace",
            &ticket
        ),
        Err(SessionError::Csrf)
    );
    let mut cross_origin = input.clone();
    cross_origin.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://attacker.example"),
    );
    assert_eq!(
        fixture.adapter.verify_csrf(
            &cross_origin,
            Some(&viewer),
            "select-workspace",
            "alice-workspace",
            &ticket
        ),
        Err(SessionError::Csrf)
    );
    cross_origin.remove(header::ORIGIN);
    assert_eq!(
        fixture.adapter.verify_csrf(
            &cross_origin,
            Some(&viewer),
            "select-workspace",
            "alice-workspace",
            &ticket
        ),
        Err(SessionError::Csrf)
    );
    fixture.state.lock().unwrap().epoch = 4;
    let current = fixture.adapter.authenticate(&input).await.unwrap();
    assert_eq!(
        fixture.adapter.verify_csrf(
            &input,
            Some(&current),
            "select-workspace",
            "alice-workspace",
            &ticket
        ),
        Err(SessionError::Csrf)
    );
    let public = URL_SAFE_NO_PAD
        .decode(ticket.split_once('.').unwrap().0)
        .unwrap();
    let public = String::from_utf8(public).unwrap();
    assert!(!public.contains(&token("alice")) && !public.contains(ALICE_KEY));
}

#[tokio::test]
async fn login_ticket_and_cookie_require_the_same_reviewed_operation() {
    let fixture = fixture().await;
    let mut input = HeaderMap::new();
    input.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4300"));
    input.insert(
        header::ORIGIN,
        HeaderValue::from_static("http://127.0.0.1:4300"),
    );
    let form = fixture.adapter.login_csrf(&input, "sign-in", "").unwrap();
    let pair = form
        .cookie
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let public = URL_SAFE_NO_PAD
        .decode(form.token.split_once('.').unwrap().0)
        .unwrap();
    assert!(
        !String::from_utf8(public)
            .unwrap()
            .contains(pair.split_once('=').unwrap().1)
    );
    input.insert(header::COOKIE, HeaderValue::from_str(&pair).unwrap());
    fixture
        .adapter
        .verify_csrf(&input, None, "sign-in", "", &form.token)
        .unwrap();
    assert_eq!(
        fixture
            .adapter
            .verify_csrf(&input, None, "recover", "", &form.token),
        Err(SessionError::Csrf)
    );
    input.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    assert_eq!(
        fixture
            .adapter
            .verify_csrf(&input, None, "sign-in", "", &form.token),
        Err(SessionError::Csrf)
    );
}

#[tokio::test]
async fn duplicate_cookies_and_unconfigured_hosts_refuse() {
    let fixture = fixture().await;
    let mut input = headers("alice", None);
    input.insert(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION_COOKIE}={}; {SESSION_COOKIE}={}",
            token("alice"),
            token("bob")
        ))
        .unwrap(),
    );
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::InvalidRequest)
    ));
    let mut input = headers("alice", None);
    input.insert(header::HOST, HeaderValue::from_static("attacker.example"));
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::Forbidden)
    ));
    let mut input = headers("alice", None);
    input.append(header::COOKIE, HeaderValue::from_static("other=value"));
    assert!(matches!(
        fixture.adapter.authenticate(&input).await,
        Err(SessionError::InvalidRequest)
    ));
}

#[tokio::test]
async fn replaced_configuration_and_changed_secret_fence_running_adapter() {
    let fixture = fixture().await;
    fixture.adapter.health().unwrap();
    std::fs::write(&fixture.secret, [12; 32]).unwrap();
    assert!(fixture.adapter.health().is_err());
    assert!(matches!(
        fixture.adapter.authenticate(&headers("alice", None)).await,
        Err(SessionError::Unavailable)
    ));
    std::fs::write(&fixture.secret, [11; 32]).unwrap();
    fixture.adapter.health().unwrap();
    let replacement = fixture.config.with_file_name("replacement.json");
    std::fs::copy(&fixture.config, &replacement).unwrap();
    std::fs::rename(&replacement, &fixture.config).unwrap();
    assert!(fixture.adapter.health().is_err());
}

#[tokio::test]
async fn sdk_redirects_and_implicit_credentials_are_not_used() {
    let fixture = fixture().await;
    fixture.state.lock().unwrap().redirect = true;
    assert!(matches!(
        fixture.adapter.sign_in(ALICE_KEY).await,
        Err(SessionError::Unavailable)
    ));
    assert_eq!(fixture.state.lock().unwrap().signins, 1);
    assert!(matches!(
        fixture.adapter.sign_in("").await,
        Err(SessionError::InvalidRequest)
    ));
    assert!(matches!(
        fixture.adapter.sign_in(&token("alice")).await,
        Err(SessionError::InvalidRequest)
    ));
}

#[test]
fn endpoints_require_tls_or_explicit_literal_loopback() {
    for endpoint_url in [
        "http://accounts.example",
        "http://localhost:4300",
        "https://user:secret@example.com",
        "https://example.com?secret=value",
        "https://example.com/path",
        "https://example.com#token",
    ] {
        assert!(endpoint(endpoint_url, true).is_err());
    }
    assert!(endpoint("https://openagents.com", true).is_ok());
    assert!(endpoint("http://127.0.0.1:4300", true).is_ok());
    assert!(endpoint("http://[::1]:4300", true).is_ok());
    let cookie = cookie(SESSION_COOKIE, &token("alice"), 30, true).unwrap();
    assert!(cookie.to_str().unwrap().ends_with("; Secure"));
    assert!(!cookie.to_str().unwrap().contains("Domain="));
}
