//! GitHub sign-in through the whole site: the header buttons, `/login`,
//! the trip to a fake GitHub and back, and the session the rest of the app
//! reads. The account service is `oa_auth::local` over real tenancy stores.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use oa_auth::fake::{self, Fake};
use oa_auth::local::LocalService;
use serde_json::json;
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:4300";
const ORIGIN: &str = "http://127.0.0.1:4300";
const REDIRECT: &str = "http://127.0.0.1:4300/auth/github/callback";
const CLIENT: &str = "Ov23liWebTest";
const SECRET: &str = "web-test-secret";

struct World {
    root: tempfile::TempDir,
    site: Router,
    fake: Fake,
    github: reqwest::Client,
}

async fn world(with_github: bool) -> World {
    world_with(with_github, None, None).await
}

/// The site with invite lists on the account service and on the web
/// server's own Cloud config.
async fn world_with(
    with_github: bool,
    service_invite: Option<oa_auth::InviteOnly>,
    web_invite: Option<oa_auth::InviteOnly>,
) -> World {
    let fake = Fake::new(CLIENT, SECRET, REDIRECT, vec![fake::octo(), fake::quiet()]);
    let github_origin = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&github_origin, CLIENT, SECRET, REDIRECT).unwrap();
    let app = credentials.app.clone();
    let root = tempfile::tempdir().unwrap();
    let stores = root.path().join("accounts");
    let service = LocalService::install(
        &stores,
        oa_auth::Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap();
    let service = match service_invite {
        Some(invite) => service.with_invite_only(invite),
        None => service,
    };
    let account_service = service.spawn().await.unwrap();
    let private = root.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let secret = private.join("csrf.key");
    std::fs::write(&secret, [21; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let path = private.join("cloud.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&{
            let mut config = json!({"schema":"openagents.cloud.web-config.v1","public_origin":ORIGIN,"account_service":account_service,"csrf_secret":secret});
            if let Some(invite) = web_invite {
                config["invite_only"] = serde_json::to_value(invite).unwrap();
            }
            config
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(root.path().join("tasks"));
    config.chat_store = Arc::new(crate::chat_store::Store::local(root.path().join("chats")));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    if with_github {
        config.github = Some(Arc::new(app));
    }
    // A key store for the visitor's own Claude key (Settings, Claude).
    let byo = private.join("byo");
    std::fs::create_dir(&byo).unwrap();
    std::fs::set_permissions(&byo, std::fs::Permissions::from_mode(0o700)).unwrap();
    config.cloud_byo = Some(Arc::new(
        crate::cloud::byo::Computers::open(&byo, oa_seal::Keyring::scratch("test").unwrap().0)
            .unwrap()
            .checking(None),
    ));
    World {
        root,
        site: crate::router(config),
        fake,
        github: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}

#[derive(Default)]
struct Browser(BTreeMap<String, String>);

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Answer {
    fn location(&self) -> &str {
        self.headers[header::LOCATION].to_str().unwrap()
    }
    fn set_cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|v| v.starts_with(&format!("{name}=")) && v.contains("Path=/;"))
            .or_else(|| {
                self.headers
                    .get_all(header::SET_COOKIE)
                    .iter()
                    .map(|v| v.to_str().unwrap().to_string())
                    .find(|v| v.starts_with(&format!("{name}=")))
            })
    }
}

impl Browser {
    async fn get(&mut self, world: &World, path: &str) -> Answer {
        self.send(world, path, None).await
    }

    /// POST a form from this site's own page.
    async fn post(&mut self, world: &World, path: &str, form: &[(&str, &str)]) -> Answer {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        self.send(world, path, Some(body)).await
    }

    async fn send(&mut self, world: &World, path: &str, form: Option<String>) -> Answer {
        let mut request = Request::builder()
            .method(if form.is_some() { "POST" } else { "GET" })
            .uri(path)
            .header(header::HOST, HOST)
            .header(header::ACCEPT, "text/html");
        if form.is_some() {
            request = request
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        }
        if !self.0.is_empty() {
            let cookies: Vec<String> = self.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        let response = world
            .site
            .clone()
            .oneshot(
                request
                    .body(form.map(Body::from).unwrap_or_default())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = String::from_utf8(
            to_bytes(response.into_body(), 4 * 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        for value in headers.get_all(header::SET_COOKIE) {
            let value = value.to_str().unwrap();
            let (name, content) = value.split(';').next().unwrap().split_once('=').unwrap();
            if content.is_empty() || value.contains("Max-Age=0") {
                self.0.remove(name);
            } else {
                self.0.insert(name.into(), content.into());
            }
        }
        Answer {
            status,
            headers,
            body,
        }
    }

    /// Start at `/auth/github?return_to=..`, act on the fake GitHub with
    /// `choice` (`login=..` or `deny=1`), and land on the callback.
    async fn through_github(&mut self, world: &World, return_to: &str, choice: &str) -> Answer {
        let start = self
            .get(world, &format!("/auth/github?return_to={return_to}"))
            .await;
        assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
        let flow = start.set_cookie("oa_auth_flow").expect("flow cookie");
        assert!(flow.contains("HttpOnly; SameSite=Lax") && flow.contains("Path=/auth/"));
        let at_github = world
            .github
            .get(format!("{}&{choice}", start.location()))
            .send()
            .await
            .unwrap();
        let back = at_github.headers()["location"]
            .to_str()
            .unwrap()
            .to_string();
        let back = back
            .strip_prefix(ORIGIN)
            .expect("GitHub returns to this site");
        self.get(world, back).await
    }
}

fn accounts(world: &World) -> tenancy::Store {
    tenancy::Accounts::open(&world.root.path().join("accounts"))
        .unwrap()
        .store()
        .unwrap()
}

#[tokio::test]
async fn signed_out_header_offers_log_in_and_sign_up_and_login_offers_github() {
    let world = world(true).await;
    let mut browser = Browser::default();
    let home = browser.get(&world, "/").await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.body.contains(">Log in<") && home.body.contains("href=\"/signup\""));
    assert!(home.body.contains("href=\"/login\""));

    let login = browser.get(&world, "/login?return_to=%2Fcloud%2Fapp").await;
    assert_eq!(login.status, StatusCode::OK);
    assert!(login.body.contains("Log in to OpenAgents"));
    crate::copy_guard::assert_plain("/login", &login.body);
    assert!(login.body.contains("Continue with GitHub"));
    assert!(
        login
            .body
            .contains("href=\"/auth/github?return_to=%2Fcloud%2Fapp\"")
    );
    // No key form, and no Log in button on the log in page itself.
    assert!(!login.body.contains("credential") && !login.body.contains("href=\"/signup\""));
    let signup = browser.get(&world, "/signup").await;
    crate::copy_guard::assert_plain("/signup", &signup.body);
    assert!(
        signup.body.contains("Create your account") && signup.body.contains("Continue with GitHub")
    );

    // The old key form sends people here.
    let old = browser.get(&world, "/sign-in").await;
    assert_eq!(old.location(), "/login");
}

#[tokio::test]
async fn without_github_log_in_is_the_key_form() {
    let world = world(false).await;
    let login = Browser::default().get(&world, "/login").await;
    assert_eq!(login.location(), crate::cloud::SIGN_IN);
    let start = Browser::default().get(&world, "/auth/github").await;
    assert_eq!(start.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn first_github_sign_in_creates_the_account_and_signs_the_browser_in() {
    let world = world(true).await;
    let mut browser = Browser::default();
    let done = browser
        .through_github(&world, "%2Fdocs", "login=octo-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(done.body.contains("You're signed in"));
    crate::copy_guard::assert_plain("/auth/github/callback", &done.body);
    assert!(done.body.contains("content=\"0;url=/docs\""));
    let session = done.set_cookie("oa_cloud_session").expect("session cookie");
    assert!(session.contains("HttpOnly; SameSite=Strict"));
    assert!(
        !browser.0.contains_key("oa_auth_flow"),
        "flow cookie cleared"
    );
    assert!(browser.0["oa_cloud_session"].starts_with("sess_"));

    // Signed in: the account menu shows the GitHub name; no Log in button.
    let home = browser.get(&world, "/").await;
    assert!(
        home.body
            .contains("<span class=\"oa-account-name\">Octo Local</span>")
    );
    assert!(!home.body.contains(">Log in<"));
    // Signed in, /login goes straight on.
    assert_eq!(browser.get(&world, "/login").await.location(), "/");

    let store = accounts(&world);
    assert_eq!(store.accounts.len(), 1);
    let account = store.accounts.values().next().unwrap();
    assert_eq!(account.principals, vec!["github:583231".to_string()]);
    assert_eq!(store.workspaces.len(), 1);
    assert_eq!(
        store.identities.github["583231"].profile.verified_email(),
        Some("octo@example.com")
    );

    // A returning visitor in a new browser reaches the same account.
    let mut again = Browser::default();
    let back = again
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    assert_eq!(back.status, StatusCode::OK);
    assert_eq!(accounts(&world).accounts.len(), 1);
    assert_eq!(world.fake.exchanges(), 2);
}

fn octo_only() -> oa_auth::InviteOnly {
    serde_json::from_value(json!({"github": [{"id": 583231, "admin": true}]})).unwrap()
}

/// Someone not invited lands on the plain invite-only page: no session
/// cookie, nothing kept.
fn assert_turned_away(browser: &Browser, done: &Answer) {
    assert_eq!(done.status, StatusCode::FORBIDDEN, "{}", done.body);
    assert!(done.body.contains("Sign-in is invite-only for now"));
    crate::copy_guard::assert_plain("/auth/github/callback", &done.body);
    assert!(!browser.0.contains_key("oa_cloud_session"));
    assert!(!browser.0.contains_key("oa_auth_flow"));
}

#[tokio::test]
async fn invite_only_turns_away_everyone_but_the_invited_without_an_account() {
    let world = world_with(true, Some(octo_only()), None).await;
    let mut browser = Browser::default();
    let done = browser
        .through_github(&world, "%2F", "login=quiet-local")
        .await;
    assert_turned_away(&browser, &done);
    assert!(accounts(&world).accounts.is_empty(), "no account was made");

    let mut owner = Browser::default();
    let done = owner
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(owner.0["oa_cloud_session"].starts_with("sess_"));
    assert_eq!(accounts(&world).accounts.len(), 1);
}

#[tokio::test]
async fn the_web_servers_own_invite_list_sets_no_cookie_and_ends_the_session() {
    // The account service lets anyone in; this server's list still holds.
    let world = world_with(true, None, Some(octo_only())).await;
    let mut browser = Browser::default();
    let login = browser.get(&world, "/login").await;
    assert!(login.body.contains("Continue with GitHub"));
    assert!(login.body.contains("Sign-in is invite-only for now."));
    assert!(!login.body.contains("creates your account"));
    let done = browser
        .through_github(&world, "%2F", "login=quiet-local")
        .await;
    assert_turned_away(&browser, &done);
    let sessions = tenancy::Sessions::open(&world.root.path().join("accounts"))
        .unwrap()
        .store()
        .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        sessions
            .book
            .sessions
            .values()
            .all(|s| s.standing(now) != tenancy::sessions::SessionState::Active),
        "the issued session was ended"
    );

    let mut owner = Browser::default();
    let done = owner
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert!(owner.0["oa_cloud_session"].starts_with("sess_"));
}

/// The analytics dashboard opens for a signed-in admin, whose account menu
/// links it; an invited person who isn't admin, and anyone signed out, get
/// the plain 404 and no link (#11153).
#[tokio::test]
async fn only_a_signed_in_admin_opens_the_analytics_dashboard() {
    let invite: oa_auth::InviteOnly = serde_json::from_value(json!({"github": [
        {"id": 583231, "admin": true},
        {"login": "quiet-local"}
    ]}))
    .unwrap();
    let world = world_with(true, Some(invite.clone()), Some(invite)).await;
    let dashboard = crate::analytics::DASHBOARD;
    let link = format!("href=\"{dashboard}\"");

    let mut visitor = Browser::default();
    let missing = visitor.get(&world, "/no-such-page").await;
    let answer = visitor.get(&world, dashboard).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.body, missing.body);

    let mut member = Browser::default();
    let done = member
        .through_github(&world, "%2F", "login=quiet-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let answer = member.get(&world, dashboard).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert!(!answer.body.contains("Top pages"));
    member.get(&world, "/settings").await;
    let settings = member.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert!(!settings.body.contains(&link), "no link for a non-admin");

    let mut owner = Browser::default();
    let done = owner
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let answer = owner.get(&world, dashboard).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("Top pages for people"));
    assert_eq!(answer.headers[header::CACHE_CONTROL], "no-store");
    owner.get(&world, "/settings").await;
    let settings = owner.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert!(settings.body.contains(&link) && settings.body.contains(">Analytics<"));
}

#[tokio::test]
async fn an_email_less_github_user_signs_in_under_the_login() {
    let world = world(true).await;
    let mut browser = Browser::default();
    let done = browser
        .through_github(&world, "%2F", "login=quiet-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    let home = browser.get(&world, "/").await;
    assert!(
        home.body
            .contains("<span class=\"oa-account-name\">quiet-local</span>")
    );
}

#[tokio::test]
async fn cancelling_on_github_signs_nobody_in() {
    let world = world(true).await;
    let mut browser = Browser::default();
    let done = browser.through_github(&world, "%2F", "deny=1").await;
    assert_eq!(done.status, StatusCode::OK);
    assert!(done.body.contains("GitHub sign-in was canceled"));
    crate::copy_guard::assert_plain("/auth/github/callback", &done.body);
    assert!(done.set_cookie("oa_cloud_session").is_none());
    assert!(!browser.0.contains_key("oa_auth_flow"));
    assert!(accounts(&world).accounts.is_empty());
}

#[tokio::test]
async fn a_callback_without_this_browsers_state_is_refused() {
    let world = world(true).await;
    let mut victim = Browser::default();
    // An attacker's own completed GitHub trip, replayed into another browser.
    let mut attacker = Browser::default();
    let start = attacker.get(&world, "/auth/github").await;
    let at_github = world
        .github
        .get(format!("{}&login=octo-local", start.location()))
        .send()
        .await
        .unwrap();
    let callback = at_github.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    let path = callback.strip_prefix(ORIGIN).unwrap();
    let refused = victim.get(&world, path).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert!(refused.body.contains("That sign-in expired"));
    crate::copy_guard::assert_plain("/auth/github/callback", &refused.body);
    assert!(refused.set_cookie("oa_cloud_session").is_none());

    // The right cookie with a different state is refused too.
    let mut other = Browser::default();
    other.get(&world, "/auth/github").await;
    let code = url::Url::parse(&callback)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .to_string();
    let wrong = other
        .get(
            &world,
            &format!("/auth/github/callback?code={code}&state=not-the-state"),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert!(accounts(&world).accounts.is_empty());
}

#[tokio::test]
async fn return_to_never_leaves_the_site() {
    let world = world(true).await;
    for hostile in [
        "https%3A%2F%2Fevil.example%2F",
        "%2F%2Fevil.example",
        "%2F%5Cevil.example",
        "javascript%3Aalert(1)",
    ] {
        let login = Browser::default()
            .get(&world, &format!("/login?return_to={hostile}"))
            .await;
        assert!(
            login.body.contains("href=\"/auth/github?return_to=%2F\""),
            "{hostile}"
        );
    }
    let mut browser = Browser::default();
    let done = browser
        .through_github(&world, "%2F%2Fevil.example", "login=octo-local")
        .await;
    assert!(done.body.contains("content=\"0;url=/\""), "{}", done.body);
    assert!(!done.body.contains("evil.example"));
}

/// The value of `name="{name}"` inside the form posting to `action`.
fn form_field(html: &str, action: &str, name: &str) -> String {
    let form = html
        .split("<form ")
        .skip(1)
        .find(|form| form.contains(&format!("action=\"{action}\"")))
        .expect("form")
        .split("</form>")
        .next()
        .unwrap();
    form.split_once(&format!("name=\"{name}\" value=\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .into()
}

#[tokio::test]
async fn a_github_account_adds_sees_and_removes_its_own_claude_key() {
    const KEY: &str = "sk-ant-api03-fake-github-settings-key-for-tests-only";
    let world = world(true).await;
    let mut browser = Browser::default();
    let done = browser
        .through_github(&world, "%2Fsettings", "login=octo-local")
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    // Sign-in opens the account's own workspace, as the key sign-in does.
    assert!(
        browser.0.contains_key("oa_cloud_workspace"),
        "{:?}",
        browser.0.keys()
    );

    let settings = browser.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    crate::copy_guard::assert_plain("/settings", &settings.body);
    assert!(settings.body.contains("Not added"));
    assert!(settings.body.contains("href=\"/settings/claude\""));
    assert!(!settings.body.contains("Unavailable"));

    let page = browser.get(&world, "/settings/claude").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    crate::copy_guard::assert_plain("/settings/claude", &page.body);
    assert!(page.body.contains("Nothing saved."));
    let csrf = form_field(&page.body, "/settings/claude", "csrf");
    let request = form_field(&page.body, "/settings/claude", "request");
    let saved = browser
        .post(
            &world,
            "/settings/claude",
            &[
                ("csrf", csrf.as_str()),
                ("request", request.as_str()),
                ("material", "anthropic_api_key"),
                ("value", KEY),
                ("consent", "custody"),
            ],
        )
        .await;
    assert_eq!(saved.status, StatusCode::SEE_OTHER, "{}", saved.body);
    assert_eq!(saved.location(), "/settings/claude");

    let page = browser.get(&world, "/settings/claude").await;
    assert!(
        page.body.contains("Saved: Anthropic API key"),
        "{}",
        page.body
    );
    assert!(!page.body.contains(KEY) && !page.body.contains("fake-github"));
    let settings = browser.get(&world, "/settings").await;
    assert!(settings.body.contains("Saved: Anthropic API key"));
    assert!(!settings.body.contains(KEY));

    let csrf = form_field(&page.body, "/settings/claude/remove", "csrf");
    let request = form_field(&page.body, "/settings/claude/remove", "request");
    let removed = browser
        .post(
            &world,
            "/settings/claude/remove",
            &[("csrf", csrf.as_str()), ("request", request.as_str())],
        )
        .await;
    assert_eq!(removed.status, StatusCode::SEE_OTHER, "{}", removed.body);
    let page = browser.get(&world, "/settings/claude").await;
    assert!(page.body.contains("Nothing saved."));
}

#[tokio::test]
async fn a_session_without_a_workspace_gets_its_own_one_selected() {
    let world = world(true).await;
    let mut browser = Browser::default();
    browser
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    // A browser signed in before sign-in picked a workspace.
    browser.0.remove("oa_cloud_workspace");
    let healed = browser.get(&world, "/settings/claude").await;
    assert_eq!(healed.status, StatusCode::SEE_OTHER, "{}", healed.body);
    assert_eq!(healed.location(), "/settings/claude");
    assert!(browser.0.contains_key("oa_cloud_workspace"));
    let page = browser.get(&world, "/settings/claude").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
}

/// One request from an app (no cookies): status and JSON body.
async fn app_call(
    world: &World,
    path: &str,
    body: serde_json::Value,
    bearer: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::post(path)
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = world
        .site
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// Start a device sign-in as Coder on `computer`.
async fn device_start(world: &World, computer: &str) -> serde_json::Value {
    let (status, started) = app_call(
        world,
        "/v1/device/code",
        json!({"app": "Coder", "computer": computer}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    started
}

async fn device_poll(
    world: &World,
    started: &serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    app_call(
        world,
        "/device/token",
        json!({"grant_type": "urn:ietf:params:oauth:grant-type:device_code", "device_code": started["device_code"]}),
        None,
    )
    .await
}

/// Open `/device?code=` and press Approve or Deny.
async fn device_decide(world: &World, browser: &mut Browser, code: &str, decision: &str) -> Answer {
    let page = browser.get(world, &format!("/device?code={code}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let csrf = form_field(&page.body, "/device", "csrf");
    browser
        .post(
            world,
            "/device",
            &[
                ("csrf", csrf.as_str()),
                ("code", code),
                ("decision", decision),
            ],
        )
        .await
}

fn session_state(world: &World, token: &str) -> tenancy::sessions::SessionState {
    tenancy::sessions::Sessions::open(&world.root.path().join("accounts"))
        .unwrap()
        .store()
        .unwrap()
        .book
        .session_of_token(token)
        .unwrap()
        .state
}

/// The older `/device/*` paths answer as `/v1/device/*` does, and say
/// which path replaces them (#11158).
#[tokio::test]
async fn the_older_device_paths_answer_and_name_their_successor() {
    let world = world(true).await;
    let request = Request::post("/device/code")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"app": "Coder", "computer": "box"}).to_string(),
        ))
        .unwrap();
    let response = world.site.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["deprecation"],
        crate::older_paths::DEPRECATED_SINCE
    );
    assert_eq!(
        response.headers()[header::LINK],
        "</v1/device/code>; rel=\"successor-version\""
    );
    let started = device_start(&world, "box").await;
    let (status, polled) = device_poll(&world, &started).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{polled}");
    assert_eq!(polled["error"], "authorization_pending");
}

#[tokio::test]
async fn a_computer_signs_in_with_a_code_approved_on_the_website_and_settings_removes_it() {
    let world = world(true).await;
    let started = device_start(&world, "octo-mbp").await;
    let user_code = started["user_code"].as_str().unwrap().to_string();
    assert_eq!(started["verification_uri"], format!("{ORIGIN}/device"));
    assert_eq!(
        started["verification_uri_complete"],
        format!("{ORIGIN}/device?code={user_code}")
    );
    assert_eq!(started["interval"], 5);
    assert_eq!(started["expires_in"], 600);
    let (status, pending) = device_poll(&world, &started).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(pending["error"], "authorization_pending");

    // Signed out, the link goes through sign-in and comes back.
    let mut browser = Browser::default();
    let link = format!("/device?code={user_code}");
    let away = browser.get(&world, &link).await;
    let back = format!("%2Fdevice%3Fcode%3D{user_code}");
    assert_eq!(away.location(), format!("/login?return_to={back}"));
    let done = browser
        .through_github(&world, &back, "login=octo-local")
        .await;
    assert!(
        done.body.contains(&format!("content=\"0;url={link}\"")),
        "{}",
        done.body
    );

    let page = browser.get(&world, &link).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("Sign in to Coder on octo-mbp?"));
    assert!(page.body.contains(&user_code));
    crate::copy_guard::assert_plain("/device", &page.body);

    let approved = device_decide(&world, &mut browser, &user_code, "approve").await;
    assert_eq!(approved.status, StatusCode::OK, "{}", approved.body);
    assert!(approved.body.contains("Coder on octo-mbp is signed in"));

    // The app picks up its own session, once.
    let (status, signed) = device_poll(&world, &started).await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    let token = signed["access_token"].as_str().unwrap().to_string();
    assert!(token.starts_with("sess_"));
    assert_eq!(signed["token_type"], "Bearer");
    assert_eq!(signed["account"]["label"], "Octo Local");
    assert!(signed["expires_in"].as_u64().unwrap() > 29 * 86_400);
    let (_, again) = device_poll(&world, &started).await;
    assert_eq!(again["error"], "invalid_grant");

    // Settings lists it; Remove signs it out.
    let settings = browser.get(&world, "/settings").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert!(
        settings.body.contains("Coder on octo-mbp"),
        "{}",
        settings.body
    );
    crate::copy_guard::assert_plain("/settings", &settings.body);
    let csrf = form_field(&settings.body, "/settings/computers/remove", "csrf");
    let id = form_field(&settings.body, "/settings/computers/remove", "session");
    let removed = browser
        .post(
            &world,
            "/settings/computers/remove",
            &[("csrf", csrf.as_str()), ("session", id.as_str())],
        )
        .await;
    assert_eq!(removed.status, StatusCode::SEE_OTHER, "{}", removed.body);
    assert_eq!(
        session_state(&world, &token),
        tenancy::sessions::SessionState::Revoked
    );
    let settings = browser.get(&world, "/settings").await;
    assert!(settings.body.contains("No computers are signed in"));
}

#[tokio::test]
async fn deny_signs_nothing_in_and_an_app_signs_its_own_token_out() {
    let world = world(true).await;
    let mut browser = Browser::default();
    browser
        .through_github(&world, "%2F", "login=octo-local")
        .await;

    let started = device_start(&world, "box").await;
    let denied = device_decide(
        &world,
        &mut browser,
        started["user_code"].as_str().unwrap(),
        "deny",
    )
    .await;
    assert!(denied.body.contains("Sign-in denied"), "{}", denied.body);
    let (status, polled) = device_poll(&world, &started).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(polled["error"], "access_denied");

    // A wrong code is said plainly; there is nothing to approve.
    let wrong = browser.get(&world, "/device?code=BCDF-GHJK").await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    assert!(wrong.body.contains("That code isn"));

    // Approve another; the app then signs itself out with its token.
    let started = device_start(&world, "box").await;
    device_decide(
        &world,
        &mut browser,
        started["user_code"].as_str().unwrap(),
        "approve",
    )
    .await;
    let (_, signed) = device_poll(&world, &started).await;
    let token = signed["access_token"].as_str().unwrap();
    let (status, out) = app_call(&world, "/device/sign-out", json!({}), Some(token)).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        session_state(&world, token),
        tenancy::sessions::SessionState::Revoked
    );
}
