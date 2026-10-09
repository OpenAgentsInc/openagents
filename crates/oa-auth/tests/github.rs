//! GitHub sign-in end to end against the fake GitHub and the local account
//! service: the browser steps are driven by hand with a no-redirect client.

use oa_auth::fake::{self, Fake};
use oa_auth::local::LocalService;
use oa_auth::{Flow, Github};
use serde_json::{Value, json};

const CLIENT: &str = "Ov23liFakeClient";
const SECRET: &str = "fake-secret";
const REDIRECT: &str = "http://127.0.0.1:4301/auth/github/callback";

struct World {
    _dir: tempfile::TempDir,
    fake: Fake,
    app: oa_auth::GithubApp,
    service: String,
    http: reqwest::Client,
}

async fn world() -> World {
    world_with(None).await
}

async fn world_with(invite: Option<oa_auth::InviteOnly>) -> World {
    let fake = Fake::new(CLIENT, SECRET, REDIRECT, vec![fake::octo(), fake::quiet()]);
    let origin = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&origin, CLIENT, SECRET, REDIRECT).unwrap();
    let app = credentials.app.clone();
    let dir = tempfile::tempdir().unwrap();
    let local = LocalService::install(
        dir.path(),
        Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap();
    let local = match invite {
        Some(invite) => local.with_invite_only(invite),
        None => local,
    };
    let service = local.spawn().await.unwrap();
    World {
        _dir: dir,
        fake,
        app,
        service,
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}

/// Start a flow and visit the authorize URL with `extra` (`login=..` or
/// `deny=1`): the callback query GitHub sends the browser back with.
async fn authorize(world: &World, extra: &str) -> (Flow, Vec<(String, String)>) {
    let (url, flow) = Flow::start(&world.app, Some("/cloud/app")).unwrap();
    let response = world
        .http
        .get(format!("{url}&{extra}"))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_redirection(), "{}", response.status());
    let location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    assert!(location.as_str().starts_with(REDIRECT));
    (flow, location.query_pairs().into_owned().collect())
}

fn param<'a>(query: &'a [(String, String)], name: &str) -> Option<&'a str> {
    query
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

async fn exchange(world: &World, code: &str, verifier: &str) -> (u16, Value) {
    let response = world
        .http
        .post(format!("{}/v1/sessions/github", world.service))
        .json(&json!({"code": code, "code_verifier": verifier}))
        .send()
        .await
        .unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}

async fn read(world: &World, token: &str, path: &str) -> (u16, Value) {
    let response = world
        .http
        .get(format!("{}{path}", world.service))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}

async fn sign_in(world: &World, login: &str) -> Value {
    let (flow, query) = authorize(world, &format!("login={login}")).await;
    assert!(flow.matches(param(&query, "state").unwrap()));
    let (status, body) = exchange(world, param(&query, "code").unwrap(), flow.verifier()).await;
    assert_eq!(status, 200, "{body}");
    body
}

#[tokio::test]
async fn first_sign_in_signs_up_and_a_returning_user_finds_the_same_account() {
    let world = world().await;
    let first = sign_in(&world, "octo-local").await;
    assert_eq!(first["created"], true);
    let token = first["token"].as_str().unwrap();
    assert!(token.starts_with("sess_"));

    let (status, session) = read(&world, token, "/v1/session").await;
    assert_eq!(status, 200);
    assert_eq!(session["session"]["state"], "active");
    assert_eq!(session["session"]["account"], first["session"]["account"]);

    let (_, account) = read(&world, token, "/v1/account").await;
    assert_eq!(account["account"]["label"], "Octo Local");
    assert_eq!(account["account"]["principals"], json!(["github:583231"]));
    let workspace = &account["workspaces"][0];
    assert_eq!(workspace["role"], "owner");
    assert_eq!(workspace["kind"], "personal");
    let (status, view) = read(
        &world,
        token,
        &format!("/v1/workspaces/{}", workspace["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(view["role"], "owner");

    // Everything GitHub said is stored, and the token is not.
    let accounts = tenancy::Accounts::open(world._dir.path()).unwrap();
    let identity = accounts.github_identity(583231).unwrap().unwrap();
    assert_eq!(identity.profile.login, "octo-local");
    assert_eq!(identity.profile.company.as_deref(), Some("@openagents"));
    assert_eq!(identity.profile.public_repos, Some(42));
    assert_eq!(
        identity.profile.created_at.as_deref(),
        Some("2011-01-25T18:44:36Z")
    );
    assert_eq!(
        identity.profile.bio.as_deref(),
        Some("Builds agents.\nLikes Rust.")
    );
    assert_eq!(identity.profile.emails.len(), 3);
    assert_eq!(identity.profile.verified_email(), Some("octo@example.com"));
    let raw = std::fs::read_to_string(world._dir.path().join("accounts.json")).unwrap();
    assert!(!raw.contains("gho_"), "no GitHub token at rest");

    // GitHub rename: same account, refreshed login.
    let mut renamed = fake::octo().user;
    renamed["login"] = json!("octo-renamed");
    world.fake.update("octo-local", renamed);
    let again = sign_in(&world, "octo-renamed").await;
    assert_eq!(again["created"], false);
    assert_eq!(again["session"]["account"], first["session"]["account"]);
    assert_eq!(
        accounts
            .github_identity(583231)
            .unwrap()
            .unwrap()
            .profile
            .login,
        "octo-renamed"
    );

    // Sign out ends the session.
    let ended = world
        .http
        .delete(format!("{}/v1/session", world.service))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(ended.status(), 200);
    assert_eq!(read(&world, token, "/v1/session").await.0, 401);
}

#[tokio::test]
async fn invite_only_lets_in_the_invited_admin_and_refuses_everyone_else_without_an_account() {
    let invite: oa_auth::InviteOnly = serde_json::from_value(
        json!({"github": [{"id": 583231, "login": "octo-local", "admin": true}]}),
    )
    .unwrap();
    let world = world_with(Some(invite)).await;

    // Someone not invited: refused, and nothing is written.
    let (flow, query) = authorize(&world, "login=quiet-local").await;
    let (status, body) = exchange(&world, param(&query, "code").unwrap(), flow.verifier()).await;
    assert_eq!(status, 403, "{body}");
    assert_eq!(body["error"]["code"], "invite_only");
    assert!(body.get("token").is_none());
    let raw = std::fs::read_to_string(world._dir.path().join("accounts.json")).unwrap();
    assert!(!raw.contains("acct_"), "no account was created");
    let sessions = std::fs::read_to_string(world._dir.path().join("sessions.json")).unwrap();
    assert!(!sessions.contains("sess_"), "no session was issued");

    // The invited owner signs in and is a site admin.
    let owner = sign_in(&world, "octo-local").await;
    assert_eq!(owner["admin"], true);
    assert_eq!(
        owner["github"],
        json!({"id": 583231, "login": "octo-local"})
    );
    let (_, account) = read(&world, owner["token"].as_str().unwrap(), "/v1/account").await;
    assert_eq!(account["account"]["admin"], true);

    // A rename keeps the owner in: the entry is by id.
    let mut renamed = fake::octo().user;
    renamed["login"] = json!("octo-renamed");
    world.fake.update("octo-local", renamed);
    assert_eq!(sign_in(&world, "octo-renamed").await["admin"], true);

    // The invited owner can't link someone who isn't invited.
    let (flow, query) = authorize(&world, "login=quiet-local").await;
    let response = world
        .http
        .post(format!("{}/v1/account/identities/github", world.service))
        .bearer_auth(owner["token"].as_str().unwrap())
        .json(&json!({"code": param(&query, "code").unwrap(), "code_verifier": flow.verifier()}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn without_an_invite_list_no_one_is_admin() {
    let world = world().await;
    let signed = sign_in(&world, "octo-local").await;
    assert_eq!(signed["admin"], false);
    let (_, account) = read(&world, signed["token"].as_str().unwrap(), "/v1/account").await;
    assert_eq!(account["account"]["admin"], false);
}

#[tokio::test]
async fn an_email_less_github_user_signs_up_under_the_login() {
    let world = world().await;
    let signed = sign_in(&world, "quiet-local").await;
    let (_, account) = read(&world, signed["token"].as_str().unwrap(), "/v1/account").await;
    assert_eq!(account["account"]["label"], "quiet-local");
    let accounts = tenancy::Accounts::open(world._dir.path()).unwrap();
    let identity = accounts.github_identity(9000001).unwrap().unwrap();
    assert!(identity.profile.emails.is_empty() && identity.profile.blog.is_none());
}

#[tokio::test]
async fn a_wrong_verifier_a_reused_code_and_a_made_up_code_are_refused() {
    let world = world().await;
    let (flow, query) = authorize(&world, "login=octo-local").await;
    let code = param(&query, "code").unwrap();
    let (_, other) = Flow::start(&world.app, None).unwrap();
    let (status, body) = exchange(&world, code, other.verifier()).await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(body["error"]["code"], "sign_in_denied");
    // The fake burns a code on any exchange attempt, as GitHub does.
    assert_eq!(exchange(&world, code, flow.verifier()).await.0, 401);
    assert_eq!(exchange(&world, "deadbeef", flow.verifier()).await.0, 401);
    assert_eq!(world.fake.exchanges(), 0);
    let raw = std::fs::read_to_string(world._dir.path().join("accounts.json")).unwrap();
    assert!(!raw.contains("acct_"), "no account was created");
}

#[tokio::test]
async fn cancelling_on_github_returns_access_denied_with_the_state() {
    let world = world().await;
    let (flow, query) = authorize(&world, "deny=1").await;
    assert_eq!(param(&query, "error"), Some("access_denied"));
    assert!(flow.matches(param(&query, "state").unwrap()));
    assert!(param(&query, "code").is_none());
}

#[tokio::test]
async fn linking_a_github_account_that_belongs_to_another_account_is_refused() {
    let world = world().await;
    let octo = sign_in(&world, "octo-local").await;
    let quiet = sign_in(&world, "quiet-local").await;
    // The quiet account tries to link octo's GitHub identity.
    let (flow, query) = authorize(&world, "login=octo-local").await;
    let response = world
        .http
        .post(format!("{}/v1/account/identities/github", world.service))
        .bearer_auth(quiet["token"].as_str().unwrap())
        .json(&json!({"code": param(&query, "code").unwrap(), "code_verifier": flow.verifier()}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "identity_taken");
    let accounts = tenancy::Accounts::open(world._dir.path()).unwrap();
    assert_eq!(
        accounts
            .account_of_principal("github:583231")
            .unwrap()
            .as_deref(),
        octo["session"]["account"].as_str()
    );
}

#[tokio::test]
async fn the_authorize_page_lists_the_fake_people_and_a_cancel_link() {
    let world = world().await;
    let (url, _) = Flow::start(&world.app, None).unwrap();
    let page = world
        .http
        .get(url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("Continue as octo-local") && page.contains("Continue as quiet-local"));
    assert!(page.contains("Cancel") && page.contains("&amp;deny=1"));
}

async fn post(world: &World, token: Option<&str>, path: &str, body: Value) -> (u16, Value) {
    let mut request = world
        .http
        .post(format!("{}{path}", world.service))
        .json(&body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.unwrap();
    (response.status().as_u16(), response.json().await.unwrap())
}

async fn poll(world: &World, device_code: &Value) -> (u16, Value) {
    post(
        world,
        None,
        "/v1/sessions/device/poll",
        json!({"device_code": device_code}),
    )
    .await
}

#[tokio::test]
async fn a_computer_signs_in_with_a_device_code_the_person_approves() {
    let world = world().await;
    let browser = sign_in(&world, "octo-local").await;
    let browser = browser["token"].as_str().unwrap();

    let (status, started) = post(
        &world,
        None,
        "/v1/sessions/device",
        json!({"app": "Coder", "computer": "octo-mbp"}),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let device_code = started["device_code"].as_str().unwrap();
    let user_code = started["user_code"].as_str().unwrap();
    assert_eq!(started["expires_in"], 600);
    assert_eq!(started["interval"], 5);

    let (status, pending) = poll(&world, &started["device_code"]).await;
    assert_eq!(status, 400);
    assert_eq!(pending["error"]["code"], "authorization_pending");
    let (_, fast) = poll(&world, &started["device_code"]).await;
    assert_eq!(fast["error"]["code"], "slow_down");
    assert_eq!(fast["interval"], 10);

    // Looking a code up and deciding need a signed-in browser.
    let lookup = json!({"user_code": user_code});
    assert_eq!(
        post(&world, None, "/v1/sessions/device/lookup", lookup)
            .await
            .0,
        401
    );
    let (status, shown) = post(
        &world,
        Some(browser),
        "/v1/sessions/device/lookup",
        json!({"user_code": user_code.to_lowercase().replace('-', " ")}),
    )
    .await;
    assert_eq!(status, 200, "{shown}");
    assert_eq!(shown["device"]["computer"], "octo-mbp");
    assert_eq!(shown["device"]["app"], "Coder");
    let (status, _) = post(
        &world,
        Some(browser),
        "/v1/sessions/device/decide",
        json!({"user_code": user_code, "approve": true}),
    )
    .await;
    assert_eq!(status, 200);

    let (status, signed) = poll(&world, &started["device_code"]).await;
    assert_eq!(status, 200, "{signed}");
    let app_token = signed["token"].as_str().unwrap();
    assert!(app_token.starts_with("sess_"));
    assert_eq!(signed["account"]["label"], "Octo Local");
    assert_eq!(signed["session"]["computer"], "octo-mbp");
    // The token works like any session, and it was handed out once.
    let (status, account) = read(&world, app_token, "/v1/account").await;
    assert_eq!(status, 200);
    assert_eq!(account["account"]["label"], "Octo Local");
    let (_, again) = poll(&world, &started["device_code"]).await;
    assert_eq!(again["error"]["code"], "invalid_grant");
    let raw = std::fs::read_to_string(world._dir.path().join("sessions.json")).unwrap();
    assert!(!raw.contains(app_token) && !raw.contains(device_code) && !raw.contains(user_code));

    // An app session can't approve another computer.
    let (_, second) = post(
        &world,
        None,
        "/v1/sessions/device",
        json!({"app": "Coder", "computer": "other"}),
    )
    .await;
    let (status, refused) = post(
        &world,
        Some(app_token),
        "/v1/sessions/device/decide",
        json!({"user_code": second["user_code"], "approve": true}),
    )
    .await;
    assert_eq!(status, 403, "{refused}");
    // Deny answers access_denied to the app.
    post(
        &world,
        Some(browser),
        "/v1/sessions/device/decide",
        json!({"user_code": second["user_code"], "approve": false}),
    )
    .await;
    let (_, denied) = poll(&world, &second["device_code"]).await;
    assert_eq!(denied["error"]["code"], "access_denied");

    // Settings lists the computer; Remove signs it out.
    let (status, listed) = read(&world, browser, "/v1/account/sessions").await;
    assert_eq!(status, 200);
    let sessions = listed["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["computer"], "octo-mbp");
    assert_eq!(sessions[0]["current"], false);
    let removed = world
        .http
        .delete(format!(
            "{}/v1/account/sessions/{}",
            world.service,
            sessions[0]["id"].as_str().unwrap()
        ))
        .bearer_auth(browser)
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), 200);
    assert_eq!(read(&world, app_token, "/v1/account").await.0, 401);
    let (_, listed) = read(&world, browser, "/v1/account/sessions").await;
    assert!(listed["sessions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_paired_sign_in_shows_on_the_browser_that_holds_the_pair_code() {
    let world = world().await;
    let browser = sign_in(&world, "octo-local").await;
    let browser = browser["token"].as_str().unwrap();
    let pair = "k7qxm2tz9w4v8r3nqq";
    let (status, started) = post(
        &world,
        None,
        "/v1/sessions/device",
        json!({"app": "Coder", "computer": "octo-mbp", "pair": pair}),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let body = json!({"pair": pair});
    assert_eq!(
        post(&world, None, "/v1/sessions/device/paired", body.clone())
            .await
            .0,
        401
    );
    let (status, shown) = post(
        &world,
        Some(browser),
        "/v1/sessions/device/paired",
        body.clone(),
    )
    .await;
    assert_eq!(status, 200, "{shown}");
    assert_eq!(shown["devices"][0]["user_code"], started["user_code"]);
    assert_eq!(shown["devices"][0]["computer"], "octo-mbp");
    let (_, other) = post(
        &world,
        Some(browser),
        "/v1/sessions/device/paired",
        json!({"pair": "someoneelsespairxx"}),
    )
    .await;
    assert_eq!(other["devices"], json!([]));
    let raw = std::fs::read_to_string(world._dir.path().join("sessions.json")).unwrap();
    assert!(!raw.contains(pair));
    assert!(!raw.contains(started["user_code"].as_str().unwrap()));
}
