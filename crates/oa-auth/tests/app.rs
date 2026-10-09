//! Repository access through the GitHub App (#11056), end to end against
//! the fake GitHub and the local account service: authorizing the App,
//! installing it on chosen repositories, listing and adding them, and the
//! credential broker's installation tokens (reuse, expiry, one mint for
//! concurrent callers, the 401 retry, an uninstalled repository, a
//! suspended installation, another host).

use std::sync::Arc;

use oa_auth::fake::{self, Fake, FakeApp};
use oa_auth::local::LocalService;
use oa_auth::repos::broker;
use oa_auth::{AppClient, AppInstall, Flow, Github, Purpose, TokenCache};
use serde_json::{Value, json};

const CLIENT: &str = "Ov23liFakeClient";
const SECRET: &str = "fake-secret";
const APP_CLIENT: &str = "Iv23liFakeAppClient";
const APP_SECRET: &str = "fake-app-secret";
const APP_ID: u64 = 424_242;
const SLUG: &str = "openagents-test";
const REDIRECT: &str = "http://127.0.0.1:4301/auth/github/callback";
const SETUP: &str = "http://127.0.0.1:4301/auth/github/setup";

struct World {
    dir: tempfile::TempDir,
    fake: Fake,
    oauth: oa_auth::GithubApp,
    install: AppInstall,
    app: AppClient,
    cache: Arc<TokenCache>,
    service: String,
    http: reqwest::Client,
}

async fn world() -> World {
    let fake = Fake::new(
        CLIENT,
        SECRET,
        REDIRECT,
        vec![fake::octo(), fake::quiet(), fake::busy(5)],
    );
    let origin = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&origin, CLIENT, SECRET, REDIRECT).unwrap();
    let app_credentials =
        fake::app_credentials(&origin, APP_ID, SLUG, APP_CLIENT, APP_SECRET, REDIRECT).unwrap();
    fake.with_app(FakeApp::of(&app_credentials, APP_SECRET, SETUP));
    let install = app_credentials.install();
    let cache = Arc::new(TokenCache::new());
    let app = AppClient::with_cache(app_credentials, cache.clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let local = LocalService::install(
        dir.path(),
        Github::new(credentials.clone()).unwrap(),
        "signup",
        3600,
    )
    .unwrap()
    .with_app(app.clone());
    let service = local.spawn().await.unwrap();
    World {
        dir,
        fake,
        oauth: credentials.app.clone(),
        install,
        app,
        cache,
        service,
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}

async fn authorize(
    world: &World,
    app: &oa_auth::GithubApp,
    purpose: Purpose,
    login: &str,
) -> (Flow, String) {
    let (url, flow) = Flow::start_for(app, Some("/projects"), purpose).unwrap();
    let response = world
        .http
        .get(format!("{url}&login={login}"))
        .send()
        .await
        .unwrap();
    let location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let code = location
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    (flow, code)
}

async fn call(world: &World, method: &str, token: &str, path: &str, body: Value) -> (u16, Value) {
    let url = format!("{}{path}", world.service);
    let request = match method {
        "GET" => world.http.get(url),
        "DELETE" => world.http.delete(url),
        _ => world.http.post(url).json(&body),
    };
    let response = request.bearer_auth(token).send().await.unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}

async fn sign_in(world: &World, login: &str) -> String {
    let (flow, code) = authorize(world, &world.oauth, Purpose::SignIn, login).await;
    let response = world
        .http
        .post(format!("{}/v1/sessions/github", world.service))
        .json(&json!({"code": code, "code_verifier": flow.verifier()}))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    body["token"].as_str().unwrap().to_string()
}

/// Authorize the App as `login` and hand the code to the account service.
async fn app_grant(world: &World, session: &str, login: &str) -> (u16, Value) {
    let (flow, code) = authorize(world, &world.install.oauth, Purpose::Install, login).await;
    call(
        world,
        "POST",
        session,
        "/v1/account/github/app/grant",
        json!({"code": code, "code_verifier": flow.verifier()}),
    )
    .await
}

/// GitHub's install page, as `login`, on `repos`; back at the setup URL.
async fn install_on(world: &World, login: &str, repos: &str) -> u64 {
    let url = format!(
        "{}?login={login}&repos={repos}&state=abc",
        world.install.install_url()
    );
    let response = world.http.get(url).send().await.unwrap();
    assert_eq!(response.status().as_u16(), 303);
    let location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    assert!(location.as_str().starts_with(SETUP), "{location}");
    let pair = |name: &str| {
        location
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    assert_eq!(pair("state").as_deref(), Some("abc"));
    assert_eq!(pair("setup_action").as_deref(), Some("install"));
    pair("installation_id").unwrap().parse().unwrap()
}

/// Ask the broker as Git's helper would.
async fn git_credential(world: &World, ticket: &str, host: &str, path: &str) -> (u16, String) {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("ticket", ticket)
        .append_pair("protocol", "https")
        .append_pair("host", host)
        .append_pair("path", path)
        .finish();
    let response = world
        .http
        .post(format!("{}{}", world.service, broker::PATH))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .unwrap();
    (response.status().as_u16(), response.text().await.unwrap())
}

fn password(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("password="))
        .unwrap_or_default()
        .to_string()
}

fn names(body: &Value) -> Vec<String> {
    let mut names: Vec<String> = body["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["full_name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/// Signed in as octo, the App authorized and installed on two of octo's
/// repositories, `octo-local/hello-world` added as a project, and a ticket
/// for it.
async fn ready(world: &World) -> (String, String) {
    let session = sign_in(world, "octo-local").await;
    let (status, body) = app_grant(world, &session, "octo-local").await;
    assert_eq!(status, 200, "{body}");
    install_on(
        world,
        "octo-local",
        "octo-local/hello-world,octo-local/secret-plans",
    )
    .await;
    let (status, body) = call(
        world,
        "POST",
        &session,
        "/v1/account/github/app/refresh",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = call(
        world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "octo-local/hello-world"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = call(
        world,
        "POST",
        &session,
        "/v1/account/github/broker",
        json!({"repository": "octo-local/hello-world"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    (session, body["ticket"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn installing_lists_only_chosen_repositories_and_adds_projects_through_the_app() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;

    // Authorized, not installed yet: nothing to list.
    let (status, body) = app_grant(&world, &session, "octo-local").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["github"]["state"], "installed");
    assert_eq!(body["github"]["login"], "octo-local");
    assert_eq!(body["github"]["installations"], json!([]));
    let (status, body) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(names(&body), Vec::<String>::new());

    // Installed on two repositories, then GitHub sends the person back.
    let installation = install_on(
        &world,
        "octo-local",
        "octo-local/hello-world,octo-local/secret-plans",
    )
    .await;
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/app/refresh",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["github"]["installations"][0]["id"], installation);
    assert_eq!(body["github"]["installations"][0]["account"], "octo-local");
    let (status, list) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{list}");
    assert_eq!(
        names(&list),
        ["octo-local/hello-world", "octo-local/secret-plans"]
    );
    assert_eq!(list["repositories"][0]["installation"], installation);

    // A repository the App isn't installed on can't be added.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "acme/storefront"}),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (404, json!("github_app_not_installed")),
        "{body}"
    );

    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "octo-local/secret-plans"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["project"]["installation_id"], installation);
    assert_eq!(body["project"]["private"], true);

    // The web server's own GitHub reads get the App's user token.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/token",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body["token"].as_str().unwrap().starts_with("ghu_"));

    // Tokens are on disk only sealed.
    for entry in std::fs::read_dir(world.dir.path().join(oa_auth::repos::STORE_DIR)).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!text.contains("ghu_") && !text.contains("ghr_"), "{text}");
    }
}

#[tokio::test]
async fn another_github_account_cant_authorize_the_app_for_this_account() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;
    let (status, body) = app_grant(&world, &session, "quiet-local").await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_other_account")),
        "{body}"
    );
}

#[tokio::test]
async fn the_broker_answers_github_for_the_ticket_repository_only() {
    let world = world().await;
    let (session, ticket) = ready(&world).await;
    assert!(ticket.starts_with(broker::PREFIX));

    let (status, text) =
        git_credential(&world, &ticket, "github.com", "octo-local/hello-world.git").await;
    assert_eq!(status, 200, "{text}");
    assert!(
        text.starts_with("username=x-access-token\npassword=ghs_"),
        "{text}"
    );
    assert!(text.contains("password_expiry_utc="));
    let token = password(&text);
    assert!(world.fake.reaches(&token, "octo-local/hello-world"));
    // Scoped to the one repository, though the installation has two.
    assert!(!world.fake.reaches(&token, "octo-local/secret-plans"));

    // Another host, plain http, another repository, a bad ticket: nothing.
    for (host, path) in [
        ("evil.example", "octo-local/hello-world.git"),
        ("github.com.evil.example", "octo-local/hello-world.git"),
        ("github.com", "octo-local/secret-plans.git"),
        ("github.com", "../x"),
    ] {
        let (status, text) = git_credential(&world, &ticket, host, path).await;
        assert_eq!(status, 403, "{host} {path}: {text}");
        assert!(!text.contains("password"));
    }
    let (status, _) = git_credential(
        &world,
        &format!("{ticket}0"),
        "github.com",
        "octo-local/hello-world",
    )
    .await;
    assert_eq!(status, 401);
    let forged = format!("{}{}", &ticket[..ticket.len() - 4], "beef");
    let (status, _) = git_credential(&world, &forged, "github.com", "octo-local/hello-world").await;
    assert_eq!(status, 401);

    // The ticket is kept only as a digest; disconnecting drops it.
    for entry in std::fs::read_dir(world.dir.path().join(oa_auth::repos::STORE_DIR)).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!text.contains(&ticket) && !text.contains("ghs_"), "{text}");
    }
    let (status, _) = call(
        &world,
        "DELETE",
        &session,
        "/v1/account/github/grant",
        json!({}),
    )
    .await;
    assert_eq!(status, 200);
    let (status, _) = git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn installation_tokens_are_reused_until_fifty_minutes_then_minted_again() {
    let world = world().await;
    let (_, ticket) = ready(&world).await;
    let mints = world.fake.installation_mints();

    let (_, first) = git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    let (_, second) = git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    assert_eq!(password(&first), password(&second));
    assert_eq!(world.fake.installation_mints(), mints + 1);

    // 51 minutes on, the cached token isn't handed out again.
    world
        .cache
        .set_clock(oa_auth::app::github_time("2099-01-01T00:00:00Z").unwrap());
    let (status, third) =
        git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    world.cache.set_clock(0);
    assert_eq!(status, 200, "{third}");
    assert_ne!(password(&third), password(&first));
    assert_eq!(world.fake.installation_mints(), mints + 2);
    assert!(
        world
            .fake
            .reaches(&password(&third), "octo-local/hello-world")
    );
}

#[tokio::test]
async fn a_token_with_under_five_minutes_left_is_never_handed_out() {
    let world = world().await;
    let (_, ticket) = ready(&world).await;
    // GitHub hands out tokens with four minutes left: each fetch mints.
    world.fake.set_token_ttl(4 * 60);
    let mints = world.fake.installation_mints();
    let (_, first) = git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    let (_, second) = git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    assert_ne!(password(&first), password(&second));
    assert_eq!(world.fake.installation_mints(), mints + 2);
}

#[tokio::test]
async fn concurrent_fetches_share_one_mint() {
    let world = Arc::new(world().await);
    let (_, ticket) = ready(&world).await;
    let mints = world.fake.installation_mints();
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let world = world.clone();
        let ticket = ticket.clone();
        tasks.push(tokio::spawn(async move {
            git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await
        }));
    }
    let mut seen = std::collections::BTreeSet::new();
    for task in tasks {
        let (status, text) = task.await.unwrap();
        assert_eq!(status, 200, "{text}");
        seen.insert(password(&text));
    }
    assert_eq!(seen.len(), 1);
    assert_eq!(world.fake.installation_mints(), mints + 1);
}

#[tokio::test]
async fn a_401_mints_again_once() {
    let world = world().await;
    let (_, _) = ready(&world).await;
    let installation = world.fake.installations()[0].id;
    let repo = world
        .app
        .installation_get(installation, None, "/repos/octo-local/hello-world")
        .await
        .unwrap();
    assert_eq!(repo["full_name"], "octo-local/hello-world");
    let mints = world.fake.installation_mints();
    // GitHub revoked the token early: one more mint, and the read works.
    world.fake.revoke_installation_tokens();
    let repo = world
        .app
        .installation_get(installation, None, "/repos/octo-local/hello-world")
        .await
        .unwrap();
    assert_eq!(repo["full_name"], "octo-local/hello-world");
    assert_eq!(world.fake.installation_mints(), mints + 1);
}

#[tokio::test]
async fn an_uninstalled_repository_or_a_suspended_installation_gets_no_token() {
    let world = world().await;
    let (session, ticket) = ready(&world).await;
    let installation = world.fake.installations()[0].id;

    world.fake.set_suspended(installation, true);
    world
        .cache
        .set_clock(oa_auth::app::github_time("2099-01-01T00:00:00Z").unwrap());
    let (status, text) =
        git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    world.cache.set_clock(0);
    assert_eq!(status, 403, "{text}");
    assert!(
        text.contains("github_app_suspended") && !text.contains("password"),
        "{text}"
    );

    world.fake.set_suspended(installation, false);
    world.fake.uninstall(installation);
    world
        .cache
        .set_clock(oa_auth::app::github_time("2099-01-01T00:00:00Z").unwrap());
    let (status, text) =
        git_credential(&world, &ticket, "github.com", "octo-local/hello-world").await;
    world.cache.set_clock(0);
    assert_eq!(status, 404, "{text}");
    assert!(text.contains("github_app_not_installed"), "{text}");

    // And a repository never chosen on GitHub.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/broker",
        json!({"repository": "acme/storefront"}),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (404, json!("repository_not_found")),
        "{body}"
    );
}

#[tokio::test]
async fn the_app_user_token_is_renewed_and_a_refused_refresh_asks_to_reconnect() {
    let world = world().await;
    // User tokens that expire within the minute: every use renews.
    world.fake.set_user_token_ttl(30);
    let (session, _) = ready(&world).await;
    let refreshes = world.fake.refreshes();
    let (status, body) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(world.fake.refreshes() > refreshes);

    // GitHub stops accepting the user token but the refresh token works:
    // renewed once on the 401.
    world.fake.set_user_token_ttl(8 * 3600);
    let (status, _) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(status, 200);
    world.fake.revoke_user_tokens();
    let (status, body) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(status, 200, "{body}");

    // The whole authorization revoked on GitHub: connect again.
    world.fake.revoke_app_authorization();
    let (status, body) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_reconnect")),
        "{body}"
    );
    let (_, body) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
    assert_eq!(body["github"]["state"], "reconnect");
}

#[tokio::test]
async fn a_jwt_for_another_app_is_refused() {
    let world = world().await;
    let (_, _) = ready(&world).await;
    let installation = world.fake.installations()[0].id;
    // The App's key with another App's id (a file from the wrong
    // deployment): GitHub refuses the JWT, and nothing is minted.
    let origin = world.install.oauth.endpoints.api_url.clone();
    let mut wrong =
        fake::app_credentials(&origin, APP_ID, SLUG, APP_CLIENT, APP_SECRET, REDIRECT).unwrap();
    wrong.app_id = APP_ID + 1;
    let wrong = AppClient::with_cache(wrong, Arc::new(TokenCache::new())).unwrap();
    let mints = world.fake.installation_mints();
    let refused = wrong
        .installation_get(installation, None, "/repos/octo-local/hello-world")
        .await
        .unwrap_err();
    assert_eq!(refused, oa_auth::repos::RepoError::NotConfigured);
    assert_eq!(world.fake.installation_mints(), mints);
}
