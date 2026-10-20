//! WorkOS sign-in through the whole site (#12323): `/login` and `/signup`
//! offer only the methods the environment has on, every method ends in
//! the usual session cookie plus the sealed WorkOS session, the sealed
//! session refreshes, and signing out (or WorkOS ending the session) ends
//! ours. The account service is `oa_auth::local` with the WorkOS sign-in
//! the gateway answers, and WorkOS is the in-process fake.

use axum::Json;
use axum::extract::State as AxumState;
use axum::response::IntoResponse;
use oa_auth::workos::fake::{API_KEY, FakeWorkos};
use oa_auth::workos::{Method, Mode, Workos};

use super::*;

const STAGING: &[Method] = &[
    Method::Github,
    Method::Google,
    Method::Password,
    Method::MagicCode,
    Method::Sso,
];
const PRODUCTION: &[Method] = &[Method::Github, Method::Password, Method::MagicCode];

struct WorkosWorld {
    world: World,
    fake: FakeWorkos,
}

/// The account service's `POST /v1/sessions/workos`, as the gateway runs it.
async fn workos_sign_in(
    AxumState((dir, workos)): AxumState<(std::path::PathBuf, Workos)>,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    let token = body["access_token"].as_str().unwrap_or_default();
    match oa_auth::workos::session::sign_in(&dir, &workos, token, Some("signup"), None).await {
        Ok(signed) => {
            Json(oa_auth::workos::session::signed_in_body(&signed, false)).into_response()
        }
        Err(error) => (
            StatusCode::from_u16(error.status()).unwrap(),
            Json(json!({"error": {"code": error.code(), "message": error.message()}})),
        )
            .into_response(),
    }
}

/// The account-service half of deletion, including the tombstone sync.
async fn workos_delete(
    AxumState((dir, workos)): AxumState<(std::path::PathBuf, Workos)>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    let Some(token) = crate::account_export::app_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(claims) = workos.verify(token).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(account) = tenancy::workos_links::Links::open(&dir)
        .account_of(&claims.sub)
        .unwrap()
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if body["confirm"] != "delete my account" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":{"code":"confirm_required","message":"Confirm deletion."}})),
        )
            .into_response();
    }
    if body["check"] != true {
        tenancy::accounts::Accounts::open(&dir)
            .unwrap()
            .delete_account(&account)
            .unwrap();
        oa_auth::workos::sync::account(&dir, &workos, &account, Default::default())
            .await
            .unwrap();
    }
    Json(json!({"deleted":body["check"] != true,"account":account})).into_response()
}

async fn workos_world(mode: Mode, methods: &[Method]) -> WorkosWorld {
    let fake = FakeWorkos::start().await;
    let workos = fake.client(mode, methods);
    let root = tempfile::tempdir().unwrap();
    let stores = root.path().join("accounts");
    let github = Fake::new(CLIENT, SECRET, REDIRECT, vec![fake::octo()]);
    let github_origin = github.spawn().await.unwrap();
    let credentials = fake::credentials(&github_origin, CLIENT, SECRET, REDIRECT).unwrap();
    let service = LocalService::install(
        &stores,
        oa_auth::Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap()
    .with_workos(workos.clone());
    let routes = Router::new()
        .route("/v1/sessions/workos", axum::routing::post(workos_sign_in))
        .route("/v1/account", axum::routing::delete(workos_delete))
        .route(
            "/v1/admin/workos/{operation}",
            axum::routing::post(
                |headers: axum::http::HeaderMap, Json(body): Json<serde_json::Value>| async move {
                    if headers.get("authorization").and_then(|h| h.to_str().ok())
                        != Some("Bearer fixture-admin-secret")
                    {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error":{"code":"unauthenticated","message":"Admin token required."}})),
                        )
                            .into_response();
                    }
                    Json(body).into_response()
                },
            ),
        )
        .with_state((stores.clone(), workos.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let account_service = format!("http://{}", listener.local_addr().unwrap());
    let nostr = oa_auth::nostr::NostrAuth::open(&stores, &[ORIGIN.to_string()]).unwrap();
    let router = service
        .router()
        .merge(routes)
        .merge(oa_auth::nostr::router_workos(
            nostr,
            "signup",
            workos.clone(),
        ));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let private = root.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let secret = private.join("csrf.key");
    std::fs::write(&secret, [21; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let key = private.join("workos.key");
    std::fs::write(&key, API_KEY).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut settings = serde_json::to_value(json!({
        "mode": mode_name(mode),
        "client_id": oa_auth::workos::fake::CLIENT_ID,
        "api_key_file": key,
        "base_url": fake.url,
        "methods": methods.iter().map(|m| method_name(*m)).collect::<Vec<_>>(),
    }))
    .unwrap();
    if methods.is_empty() {
        settings["methods"] = json!([]);
    }
    let path = private.join("cloud.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema": "openagents.cloud.web-config.v1",
            "public_origin": ORIGIN,
            "account_service": account_service,
            "csrf_secret": secret,
            "workos": settings,
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(root.path().join("tasks"));
    config.chat_store = Arc::new(crate::chat_store::Store::local(root.path().join("chats")));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    config.agent_key_seed = Some([7; 32]);
    WorkosWorld {
        world: World {
            root,
            site: crate::router(config),
            fake: github,
            github: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        },
        fake,
    }
}

fn mode_name(mode: Mode) -> &'static str {
    mode.as_str()
}

fn method_name(method: Method) -> &'static str {
    match method {
        Method::Github => "github",
        Method::Google => "google",
        Method::Password => "password",
        Method::MagicCode => "magic_code",
        Method::Sso => "sso",
    }
}

/// The CSRF ticket a page's forms carry.
fn csrf(body: &str) -> String {
    let at = body
        .find("name=\"csrf\" value=\"")
        .expect("a form with a ticket")
        + 19;
    body[at..].split('"').next().unwrap().to_string()
}

/// The sign-out form's ticket on a signed-in page.
fn sign_out_csrf(body: &str) -> String {
    let at = body.find("id=\"oa-sign-out\"").expect("a sign-out form");
    csrf(&body[at..])
}

async fn signed_in(browser: &mut Browser, world: &World) -> bool {
    let login = browser.get(world, "/login?return_to=%2Fsettings").await;
    login.status == StatusCode::SEE_OTHER && login.location() == "/settings"
}

#[tokio::test]
async fn login_offers_only_the_methods_the_environment_has_on() {
    let production = workos_world(Mode::Dual, PRODUCTION).await;
    let mut browser = Browser::default();
    let login = browser.get(&production.world, "/login").await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    for shown in [
        "Continue with GitHub",
        "Email me a code",
        "Forgot your password?",
        "Log in with your browser extension",
        "Create an account",
    ] {
        assert!(login.body.contains(shown), "{shown}");
    }
    for hidden in [
        "Continue with Google",
        "single sign-on",
        "Signed up with GitHub before",
    ] {
        assert!(!login.body.contains(hidden), "{hidden}");
    }
    let signup = browser.get(&production.world, "/signup").await;
    assert!(
        signup.body.contains("Create your account")
            && signup.body.contains("At least 10 characters.")
    );
    assert!(signup.body.contains("autocomplete=\"new-password\""));
    crate::copy_guard::assert_plain("/login", &login.body);
    crate::copy_guard::assert_plain("/signup", &signup.body);

    let staging = workos_world(Mode::Dual, STAGING).await;
    let login = Browser::default().get(&staging.world, "/login").await;
    assert!(login.body.contains("Continue with Google"));
    assert!(login.body.contains("Continue with single sign-on"));

    // The old GitHub address goes through WorkOS now.
    let old = Browser::default()
        .get(&production.world, "/auth/github?return_to=%2Fprojects")
        .await;
    assert_eq!(old.status, StatusCode::SEE_OTHER);
    assert_eq!(old.location(), "/auth/workos/github?return_to=%2Fprojects");
    // Google is off in production: its address answers, with nothing to do.
    let google = Browser::default()
        .get(&production.world, "/auth/workos/google")
        .await;
    assert_eq!(google.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sign_up_with_a_password_confirms_the_email_then_signs_in() {
    let w = workos_world(Mode::Dual, PRODUCTION).await;
    let mut browser = Browser::default();
    let page = browser.get(&w.world, "/signup?return_to=%2Fsettings").await;
    let ticket = csrf(&page.body);
    // A short password is refused before WorkOS sees it.
    let weak = browser
        .post(
            &w.world,
            "/signup",
            &[
                ("csrf", &ticket),
                ("return_to", "/settings"),
                ("email", "new@example.com"),
                ("password", "short"),
            ],
        )
        .await;
    assert_eq!(weak.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(weak.body.contains("at least 10 characters"));
    let ticket = csrf(&weak.body);
    let made = browser
        .post(
            &w.world,
            "/signup",
            &[
                ("csrf", &ticket),
                ("return_to", "/settings"),
                ("name", "Nia Ng"),
                ("email", "new@example.com"),
                ("password", "a long enough password"),
            ],
        )
        .await;
    assert_eq!(made.status, StatusCode::SEE_OTHER, "{}", made.body);
    assert_eq!(made.location(), "/login/verify");
    let verify = browser.get(&w.world, "/login/verify").await;
    assert!(verify.body.contains("We sent a code to new@example.com"));
    crate::copy_guard::assert_plain("/login/verify", &verify.body);
    let ticket = csrf(&verify.body);
    let wrong = browser
        .post(
            &w.world,
            "/login/verify",
            &[("csrf", &ticket), ("code", "000000")],
        )
        .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert!(wrong.body.contains("That code isn't right"));
    let code = w
        .fake
        .last_email("new@example.com", "email_verification")
        .unwrap();
    let ticket = csrf(&wrong.body);
    let done = browser
        .post(
            &w.world,
            "/login/verify",
            &[("csrf", &ticket), ("code", &code)],
        )
        .await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    assert_eq!(done.location(), "/settings");
    assert!(browser.0.contains_key("oa_cloud_session") && browser.0.contains_key("oa_workos"));
    assert!(!browser.0.contains_key("oa_workos_pending"));
    assert!(signed_in(&mut browser, &w.world).await);
    let store = accounts(&w.world);
    assert_eq!(store.accounts.len(), 1);
    let account = store.accounts.values().next().unwrap();
    assert_eq!(account.label, "Nia Ng");
    let user = w.fake.user_by_email("new@example.com").unwrap();
    assert_eq!(user["external_id"], account.id.as_str());

    // Signing up again with the same email says to log in instead.
    let mut other = Browser::default();
    let page = other.get(&w.world, "/signup").await;
    let again = other
        .post(
            &w.world,
            "/signup",
            &[
                ("csrf", &csrf(&page.body)),
                ("email", "new@example.com"),
                ("password", "a long enough password"),
            ],
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert!(again.body.contains("Log in instead"));

    // And logging in with the password works, and a wrong one doesn't.
    let page = other.get(&w.world, "/login").await;
    let bad = other
        .post(
            &w.world,
            "/login",
            &[
                ("csrf", &csrf(&page.body)),
                ("email", "new@example.com"),
                ("password", "not the password"),
            ],
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.body.contains("don&#39;t match") || bad.body.contains("don't match"));
    let good = other
        .post(
            &w.world,
            "/login",
            &[
                ("csrf", &csrf(&bad.body)),
                ("email", "new@example.com"),
                ("password", "a long enough password"),
                ("return_to", "/settings"),
            ],
        )
        .await;
    assert_eq!(good.status, StatusCode::SEE_OTHER, "{}", good.body);
    assert_eq!(good.location(), "/settings");
    assert_eq!(accounts(&w.world).accounts.len(), 1, "the same account");
}

#[tokio::test]
async fn an_emailed_code_signs_in_and_sign_out_ends_the_workos_session() {
    let w = workos_world(Mode::Dual, PRODUCTION).await;
    let mut browser = Browser::default();
    let page = browser.get(&w.world, "/login").await;
    let sent = browser
        .post(
            &w.world,
            "/login/code",
            &[
                ("csrf", &csrf(&page.body)),
                ("email", "code@example.com"),
                ("return_to", "/device"),
            ],
        )
        .await;
    assert_eq!(sent.status, StatusCode::SEE_OTHER, "{}", sent.body);
    let verify = browser.get(&w.world, "/login/verify").await;
    assert!(verify.body.contains("Enter it to log in"));
    let code = w.fake.last_email("code@example.com", "magic_auth").unwrap();
    let done = browser
        .post(
            &w.world,
            "/login/verify",
            &[("csrf", &csrf(&verify.body)), ("code", &code)],
        )
        .await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    assert_eq!(done.location(), "/device");
    // The device page (where Coder 1.0's code is approved) is signed in.
    let device = browser.get(&w.world, "/device").await;
    assert_eq!(device.status, StatusCode::OK, "{}", device.body);
    // Coder 1.0's device code, approved by this WorkOS-made session.
    let started = device_start(&w.world, "workos-mbp").await;
    let code = started["user_code"].as_str().unwrap().to_string();
    let approved = device_decide(&w.world, &mut browser, &code, "approve").await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    let (status, signed) = device_poll(&w.world, &started).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert!(
        signed["access_token"]
            .as_str()
            .unwrap()
            .starts_with("sess_")
    );
    let settings = browser.get(&w.world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    let ticket = sign_out_csrf(&settings.body);
    let sealed = crate::cloud::session::CloudSession::sealed_value(
        &{
            let mut headers = HeaderMap::new();
            headers.insert(
                header::COOKIE,
                format!("oa_workos={}", browser.0["oa_workos"])
                    .parse()
                    .unwrap(),
            );
            headers
        },
        "oa_workos",
    )
    .unwrap();
    let workos = w.fake.client(Mode::Dual, PRODUCTION);
    let password = cookie_password();
    let sid = workos
        .sdk()
        .session(sealed, password)
        .authenticate()
        .await
        .session_id;
    assert!(!sid.is_empty());
    let out = browser
        .post(&w.world, "/sign-out", &[("csrf", &ticket)])
        .await;
    assert_eq!(out.status, StatusCode::SEE_OTHER, "{}", out.body);
    assert!(w.fake.revoked(&sid), "signing out ends the WorkOS session");
    assert!(!browser.0.contains_key("oa_cloud_session") && !browser.0.contains_key("oa_workos"));
}

/// The password the site seals WorkOS cookies with, from the test's CSRF
/// secret.
fn cookie_password() -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"openagents-web workos cookie\0");
    digest.update([21u8; 32]);
    digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn github_through_workos_returns_to_the_page() {
    let w = workos_world(Mode::Dual, PRODUCTION).await;
    let mut browser = Browser::default();
    w.fake
        .queue_sign_in("GitHubOAuth", "octo@example.com", "583231", Some("Octo"));
    let start = browser
        .get(&w.world, "/auth/workos/github?return_to=%2Fsettings")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER);
    let flow = start.set_cookie("oa_workos_flow").expect("flow cookie");
    assert!(flow.contains("SameSite=Lax") && flow.contains("HttpOnly"));
    let at_workos = w.world.github.get(start.location()).send().await.unwrap();
    let back = at_workos.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    let back = back
        .strip_prefix(ORIGIN)
        .expect("WorkOS returns to this site")
        .to_string();
    assert!(back.starts_with("/auth/callback?"));
    let done = browser.get(&w.world, &back).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(done.body.contains("You're logged in") || done.body.contains("You&#39;re logged in"));
    assert!(done.body.contains("url=/settings"));
    assert!(browser.0.contains_key("oa_cloud_session") && browser.0.contains_key("oa_workos"));
    assert!(!browser.0.contains_key("oa_workos_flow"));
    // A replayed callback has no flow left.
    let replay = browser.get(&w.world, &back).await;
    assert_eq!(replay.status, StatusCode::SEE_OTHER);
    assert!(replay.location().starts_with("/login?error=expired"));
    // Canceling at the provider says so.
    let mut other = Browser::default();
    let start = other.get(&w.world, "/auth/workos/github").await;
    let state = url::Url::parse(start.location())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let canceled = other
        .get(
            &w.world,
            &format!("/auth/callback?error=access_denied&state={state}"),
        )
        .await;
    assert!(canceled.location().starts_with("/login?error=canceled"));
}

#[tokio::test]
async fn provider_callback_failures_are_distinct_from_cancellations() {
    let w = workos_world(Mode::Dual, PRODUCTION).await;
    for (error, notice, copy) in [
        (
            "access_denied",
            "canceled",
            "Login was canceled. Nothing changed.",
        ),
        (
            "canceled",
            "canceled",
            "Login was canceled. Nothing changed.",
        ),
        (
            "oauth_failed",
            "provider_failed",
            "Sign-in didn't finish. Try again.",
        ),
        (
            "server_error",
            "provider_failed",
            "Sign-in didn't finish. Try again.",
        ),
    ] {
        let mut browser = Browser::default();
        let start = browser
            .get(&w.world, "/auth/workos/github?return_to=%2Fsettings")
            .await;
        let state = url::Url::parse(start.location())
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let mut callback = url::Url::parse(&format!("{ORIGIN}/auth/callback")).unwrap();
        callback.query_pairs_mut().extend_pairs([
            ("error", error),
            ("state", state.as_str()),
            (
                "error_description",
                "Unable to exchange authorization code with Github.",
            ),
        ]);
        let done = browser
            .get(
                &w.world,
                &format!("{}?{}", callback.path(), callback.query().unwrap()),
            )
            .await;
        assert_eq!(done.status, StatusCode::SEE_OTHER);
        let location = done.location();
        assert!(
            location.starts_with(&format!("/login?error={notice}")),
            "{location}"
        );
        assert!(location.contains("return_to=%2Fsettings"));
        assert!(!location.contains("exchange"));
        assert!(!browser.0.contains_key("oa_workos_flow"));
        assert!(!browser.0.contains_key("oa_workos"));
        assert!(!browser.0.contains_key("oa_cloud_session"));
        let page = browser.get(&w.world, location).await;
        assert!(page.body.contains(copy), "{}", page.body);
        assert!(!page.body.contains("Unable to exchange"));
        if notice == "provider_failed" {
            assert!(!page.body.contains("Login was canceled"));
        }
    }
}

#[tokio::test]
async fn sso_starts_for_the_organization_with_that_domain() {
    let w = workos_world(Mode::Dual, STAGING).await;
    let org = w.fake.add_sso_organization("Acme", "acme.test");
    let mut browser = Browser::default();
    let page = browser.get(&w.world, "/login").await;
    let unknown = browser
        .post(
            &w.world,
            "/login/sso",
            &[("csrf", &csrf(&page.body)), ("email", "pat@nowhere.test")],
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert!(
        unknown.body.contains("Single sign-on isn&#39;t set up")
            || unknown.body.contains("Single sign-on isn't set up")
    );
    let started = browser
        .post(
            &w.world,
            "/login/sso",
            &[("csrf", &csrf(&unknown.body)), ("email", "wile@acme.test")],
        )
        .await;
    assert_eq!(started.status, StatusCode::SEE_OTHER);
    assert!(
        started
            .location()
            .contains(&format!("organization_id={org}"))
    );
}

#[tokio::test]
async fn a_forgotten_password_is_reset_by_the_emailed_link() {
    let w = workos_world(Mode::Dual, PRODUCTION).await;
    w.fake
        .add_user("kay@example.com", Some("the first password"), true);
    let mut browser = Browser::default();
    let page = browser.get(&w.world, "/login/forgot").await;
    assert_eq!(page.status, StatusCode::OK);
    crate::copy_guard::assert_plain("/login/forgot", &page.body);
    let sent = browser
        .post(
            &w.world,
            "/login/forgot",
            &[("csrf", &csrf(&page.body)), ("email", "kay@example.com")],
        )
        .await;
    assert!(sent.body.contains("we sent it a link"));
    // An unknown email reads the same.
    let page = browser.get(&w.world, "/login/forgot").await;
    let nobody = browser
        .post(
            &w.world,
            "/login/forgot",
            &[("csrf", &csrf(&page.body)), ("email", "nobody@example.com")],
        )
        .await;
    assert!(nobody.body.contains("we sent it a link"));
    let token = w
        .fake
        .last_email("kay@example.com", "password_reset")
        .unwrap();
    let reset = browser
        .get(&w.world, &format!("/login/reset?token={token}"))
        .await;
    assert_eq!(reset.status, StatusCode::OK);
    let done = browser
        .post(
            &w.world,
            "/login/reset",
            &[
                ("csrf", &csrf(&reset.body)),
                ("token", &token),
                ("password", "the second password"),
            ],
        )
        .await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    assert!(signed_in(&mut browser, &w.world).await);
}

#[tokio::test]
async fn an_expired_workos_session_refreshes_and_a_revoked_one_signs_out() {
    let w = workos_world(Mode::Workos, PRODUCTION).await;
    let mut browser = Browser::default();
    let page = browser.get(&w.world, "/login").await;
    browser
        .post(
            &w.world,
            "/login/code",
            &[("csrf", &csrf(&page.body)), ("email", "ref@example.com")],
        )
        .await;
    let verify = browser.get(&w.world, "/login/verify").await;
    let code = w.fake.last_email("ref@example.com", "magic_auth").unwrap();
    browser
        .post(
            &w.world,
            "/login/verify",
            &[("csrf", &csrf(&verify.body)), ("code", &code)],
        )
        .await;
    assert!(signed_in(&mut browser, &w.world).await);
    assert!(browser.0.contains_key("oa_workos"));
    assert!(!browser.0.contains_key("oa_cloud_session"));
    assert!(
        tenancy::Sessions::open(&w.world.root.path().join("accounts"))
            .unwrap()
            .store()
            .unwrap()
            .book
            .sessions
            .is_empty()
    );

    // Replace the sealed session with one whose access token has expired.
    let workos = w.fake.client(Mode::Workos, PRODUCTION);
    let user = w.fake.user_by_email("ref@example.com").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (fresh, refresh) = w.fake.sign_in(&user);
    let sid = workos.verify(&fresh).await.unwrap().sid;
    let expired = oa_auth::workos::fake::mint(&user, &sid, None, -5);
    let seal = |access: &str, refresh: &str| {
        let sealed = oa_auth::workos::sdk::seal_session(
            &oa_auth::workos::sdk::SessionData {
                access_token: access.into(),
                refresh_token: refresh.into(),
                user: None,
                impersonator: None,
            },
            &cookie_password(),
        )
        .unwrap();
        sealed.replace('+', "-").replace('/', "_").replace('=', ".")
    };
    browser
        .0
        .insert("oa_workos".into(), seal(&expired, &refresh));
    let before = browser.0["oa_workos"].clone();
    assert!(
        signed_in(&mut browser, &w.world).await,
        "a refresh keeps the session"
    );
    assert_ne!(
        browser.0["oa_workos"], before,
        "the refreshed session is sealed again"
    );

    // WorkOS ends the session: the next refresh is refused, and in
    // `workos` mode that signs this browser out.
    let (fresh, refresh) = w.fake.sign_in(&user);
    let sid = workos.verify(&fresh).await.unwrap().sid;
    workos.revoke_session(&sid).await.unwrap();
    let expired = oa_auth::workos::fake::mint(&user, &sid, None, -5);
    browser
        .0
        .insert("oa_workos".into(), seal(&expired, &refresh));
    assert!(!signed_in(&mut browser, &w.world).await);
    assert!(!browser.0.contains_key("oa_cloud_session"));
}

#[tokio::test]
async fn legacy_mode_keeps_the_old_login() {
    let w = workos_world(Mode::Legacy, PRODUCTION).await;
    let login = Browser::default().get(&w.world, "/login").await;
    // No GitHub app here, so the old page sends people to the key form.
    assert!(!login.body.contains("Email me a code"));
    let start = Browser::default()
        .get(&w.world, "/auth/workos/github")
        .await;
    assert_eq!(start.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn primary_password_and_provider_logins_use_only_the_sealed_workos_cookie() {
    for provider in [false, true] {
        let w = workos_world(Mode::Workos, PRODUCTION).await;
        let mut browser = Browser::default();
        if provider {
            w.fake.queue_sign_in(
                "GitHubOAuth",
                "primary@example.com",
                "583232",
                Some("Person"),
            );
            let start = browser.get(&w.world, "/auth/workos/github").await;
            let returned = w.world.github.get(start.location()).send().await.unwrap();
            let back = returned.headers()["location"]
                .to_str()
                .unwrap()
                .strip_prefix(ORIGIN)
                .unwrap();
            let done = browser.get(&w.world, back).await;
            assert_eq!(done.status, StatusCode::OK, "{}", done.body);
        } else {
            w.fake
                .add_user("primary@example.com", Some("a long enough password"), true);
            let login = browser.get(&w.world, "/login").await;
            let done = browser
                .post(
                    &w.world,
                    "/login",
                    &[
                        ("csrf", &csrf(&login.body)),
                        ("email", "primary@example.com"),
                        ("password", "a long enough password"),
                    ],
                )
                .await;
            assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
        }
        assert!(browser.0.contains_key("oa_workos"));
        assert!(!browser.0.contains_key("oa_cloud_session"));
        assert!(signed_in(&mut browser, &w.world).await);
        let rows = tenancy::Sessions::open(&w.world.root.path().join("accounts"))
            .unwrap()
            .store()
            .unwrap();
        assert!(rows.book.sessions.is_empty());
        let token = browser.0["oa_workos"]
            .clone()
            .replace('-', "+")
            .replace('_', "/")
            .replace('.', "=");
        let data = oa_auth::workos::sdk::unseal_session(&token, &cookie_password()).unwrap();
        let sid = w
            .fake
            .client(Mode::Workos, PRODUCTION)
            .verify(data.access_token.expose())
            .await
            .unwrap()
            .sid;
        let settings = browser.get(&w.world, "/settings").await;
        assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
        let done = browser
            .post(
                &w.world,
                "/sign-out",
                &[("csrf", &sign_out_csrf(&settings.body))],
            )
            .await;
        assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
        assert!(w.fake.revoked(&sid));
        assert!(!browser.0.contains_key("oa_workos"));
        assert!(!signed_in(&mut browser, &w.world).await);
    }
}

#[tokio::test]
async fn primary_extension_and_phone_approval_create_no_legacy_cookie_or_session() {
    use super::web_sign_in::{call, call_capability, key, nip98, scanned};
    for phone in [false, true] {
        let w = workos_world(Mode::Workos, PRODUCTION).await;
        let mut browser = Browser::default();
        if phone {
            let (status, started) = call(
                &w.world,
                &mut browser,
                "POST",
                "/login/key",
                Some(ORIGIN),
                None,
                Some(br#"{"return_to":"/settings"}"#),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{started}");
            let (request, challenge) = scanned(started["link"].as_str().unwrap());
            let body = json!({"request":request}).to_string();
            let signed = nip98(
                &key(43),
                &format!("{ORIGIN}/v1/web-sign-in/approve"),
                &challenge,
                body.as_bytes(),
            );
            let (status, approved) = call_capability(
                &w.world,
                &mut Browser::default(),
                "POST",
                "/v1/web-sign-in/approve",
                None,
                Some(&signed),
                Some(body.as_bytes()),
                false,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{approved}");
            let (status, done) = call(
                &w.world,
                &mut browser,
                "GET",
                &format!("/login/key/{request}/status"),
                None,
                None,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{done}");
        } else {
            let (_, challenge) = call(
                &w.world,
                &mut browser,
                "GET",
                "/v1/device/nostr/challenge",
                None,
                None,
                None,
            )
            .await;
            let body = br#"{"return_to":"/settings"}"#;
            let signed = nip98(
                &key(44),
                &format!("{ORIGIN}/login/key/extension"),
                challenge["challenge"].as_str().unwrap(),
                body,
            );
            let (status, done) = call_capability(
                &w.world,
                &mut browser,
                "POST",
                "/login/key/extension",
                Some(ORIGIN),
                Some(&signed),
                Some(body),
                false,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{done}");
        }
        assert!(browser.0.contains_key("oa_workos"));
        assert!(!browser.0.contains_key("oa_cloud_session"));
        assert!(signed_in(&mut browser, &w.world).await);
        assert!(
            tenancy::Sessions::open(&w.world.root.path().join("accounts"))
                .unwrap()
                .store()
                .unwrap()
                .book
                .sessions
                .is_empty()
        );
    }
}

#[tokio::test]
async fn primary_account_deletion_accepts_workos_bearers_and_sealed_browser_credentials() {
    async fn remove(
        world: &World,
        browser: &Browser,
        bearer: Option<&str>,
        site: &str,
        ticket: Option<&str>,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::delete("/v1/account")
            .header(header::HOST, HOST)
            .header(header::CONTENT_TYPE, "application/json")
            .header("sec-fetch-site", site);
        if let Some(token) = bearer {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(ticket) = ticket {
            request = request.header(crate::cookie_api::CSRF_HEADER, ticket);
        }
        let cookie = browser
            .0
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        let response = world
            .site
            .clone()
            .oneshot(
                request
                    .header(header::COOKIE, cookie)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or_default())
    }

    for cookie in [false, true] {
        let w = workos_world(Mode::Workos, PRODUCTION).await;
        w.fake
            .add_user("delete@example.com", Some("a long enough password"), true);
        let mut browser = Browser::default();
        let login = browser.get(&w.world, "/login").await;
        let done = browser
            .post(
                &w.world,
                "/login",
                &[
                    ("csrf", &csrf(&login.body)),
                    ("email", "delete@example.com"),
                    ("password", "a long enough password"),
                ],
            )
            .await;
        assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
        assert!(!browser.0.contains_key("oa_cloud_session"));
        let sealed = browser.0["oa_workos"]
            .replace('-', "+")
            .replace('_', "/")
            .replace('.', "=");
        let data = oa_auth::workos::sdk::unseal_session(&sealed, &cookie_password()).unwrap();
        let token = data.access_token.expose();
        let before = accounts(&w.world);
        let id = before.accounts.keys().next().unwrap().clone();
        let confirmed = json!({"confirm":"delete my account"});
        let empty = Browser::default();
        let (status, _) = remove(
            &w.world,
            &empty,
            Some("eyJhbGciOiJSUzI1NiJ9.invalid.signature"),
            "none",
            None,
            confirmed.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, missing) =
            remove(&w.world, &empty, Some(token), "none", None, json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");
        assert_eq!(missing["error"]["code"], "confirm_required");
        assert!(missing["error"]["export_url"].is_string());
        let bootstrap = browser.get(&w.world, "/lander8/session").await;
        assert_eq!(bootstrap.status, StatusCode::OK, "{}", bootstrap.body);
        let bootstrap: serde_json::Value = serde_json::from_str(&bootstrap.body).unwrap();
        let ticket = bootstrap["csrf"].as_str().unwrap();
        for (site, csrf) in [("cross-site", Some(ticket)), ("same-origin", None)] {
            let (status, _) = remove(&w.world, &browser, None, site, csrf, confirmed.clone()).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
        }
        assert_eq!(
            accounts(&w.world).accounts[&id].label,
            before.accounts[&id].label
        );
        assert!(
            tenancy::workos_links::Links::open(&w.world.root.path().join("accounts"))
                .user_of(&id)
                .unwrap()
                .is_some()
        );
        let (status, deleted) = if cookie {
            remove(
                &w.world,
                &browser,
                None,
                "same-origin",
                Some(ticket),
                confirmed,
            )
            .await
        } else {
            remove(&w.world, &empty, Some(token), "none", None, confirmed).await
        };
        assert_eq!(status, StatusCode::OK, "{deleted}");
        assert_eq!(deleted["deleted"], true);
        let after = accounts(&w.world);
        assert_eq!(after.accounts[&id].label, "Deleted account");
        assert!(after.accounts[&id].principals.is_empty());
        assert!(
            tenancy::workos_links::Links::open(&w.world.root.path().join("accounts"))
                .user_of(&id)
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn retirement_forwarding_requires_the_operators_explicit_bearer() {
    use super::web_sign_in::call;
    let w = workos_world(Mode::Workos, PRODUCTION).await;
    for operation in ["retire-sessions", "restore-sessions"] {
        let path = format!("/v1/admin/workos/{operation}");
        let mut browser = Browser::default();
        browser
            .0
            .insert("oa_cloud_session".into(), "sess_fake".into());
        let (status, _) = call(
            &w.world,
            &mut browser,
            "POST",
            &path,
            Some(ORIGIN),
            None,
            Some(br#"{"apply":false}"#),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = call(
            &w.world,
            &mut browser,
            "POST",
            &path,
            Some(ORIGIN),
            Some("Bearer ordinary-user-token"),
            Some(br#"{"apply":false}"#),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, answer) = call(
            &w.world,
            &mut browser,
            "POST",
            &path,
            Some(ORIGIN),
            Some("Bearer fixture-admin-secret"),
            Some(br#"{"apply":false}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["apply"], false);
    }
}

#[tokio::test]
async fn app_nostr_returns_workos_tokens_in_primary_and_legacy_tokens_in_dual() {
    use super::web_sign_in::{call, call_capability, key, nip98};
    for (mode, capable) in [
        (Mode::Workos, true),
        (Mode::Workos, false),
        (Mode::Dual, true),
        (Mode::Dual, false),
    ] {
        let w = workos_world(mode, PRODUCTION).await;
        let mut app = Browser::default();
        let (_, challenge) = call(
            &w.world,
            &mut app,
            "GET",
            "/v1/device/nostr/challenge",
            None,
            None,
            None,
        )
        .await;
        let body = br#"{"app":"OpenAgents","computer":"test phone"}"#;
        let signed = nip98(
            &key(45),
            &format!("{ORIGIN}/v1/device/nostr"),
            challenge["challenge"].as_str().unwrap(),
            body,
        );
        let (status, answer) = call_capability(
            &w.world,
            &mut app,
            "POST",
            "/v1/device/nostr",
            None,
            Some(&signed),
            Some(body),
            capable,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        let token = answer["access_token"].as_str().unwrap();
        assert!(answer["expires_in"].as_u64().unwrap() > 0);
        if mode == Mode::Workos && capable {
            assert!(oa_auth::workos::token::looks_like_access_token(token));
            assert!(!answer["refresh_token"].as_str().unwrap().is_empty());
        } else {
            assert!(token.starts_with("sess_"));
        }
        let rows = tenancy::Sessions::open(&w.world.root.path().join("accounts"))
            .unwrap()
            .store()
            .unwrap();
        assert_eq!(
            rows.book.sessions.len(),
            usize::from(mode == Mode::Dual || !capable)
        );
    }
}
