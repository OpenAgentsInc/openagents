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
        serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":ORIGIN,"account_service":account_service,"csrf_secret":secret})).unwrap(),
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
        let mut request = Request::get(path)
            .header(header::HOST, HOST)
            .header(header::ACCEPT, "text/html");
        if !self.0.is_empty() {
            let cookies: Vec<String> = self.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        let response = world
            .site
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
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
    let old = browser.get(&world, "/cloud/sign-in").await;
    assert_eq!(old.location(), "/login?return_to=%2Fcloud%2Fapp");
}

#[tokio::test]
async fn without_github_the_header_has_no_sign_in_buttons() {
    let world = world(false).await;
    let home = Browser::default().get(&world, "/").await;
    assert!(!home.body.contains("href=\"/login\"") && !home.body.contains("href=\"/signup\""));
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
