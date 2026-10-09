//! Connecting GitHub repositories end to end against the fake GitHub and
//! the local account service: sign in, a second trip for repository
//! access (public only, then private), projects, revocation, and a GitHub
//! account that isn't the one the account signs in with.

use oa_auth::fake::{self, Fake};
use oa_auth::local::LocalService;
use oa_auth::{Flow, Github, Purpose};
use serde_json::{Value, json};

const CLIENT: &str = "Ov23liFakeClient";
const SECRET: &str = "fake-secret";
const REDIRECT: &str = "http://127.0.0.1:4301/auth/github/callback";

struct World {
    dir: tempfile::TempDir,
    fake: Fake,
    app: oa_auth::GithubApp,
    service: String,
    http: reqwest::Client,
}

async fn world() -> World {
    world_of(vec![fake::octo(), fake::quiet()]).await
}

async fn world_of(people: Vec<fake::FakeUser>) -> World {
    let fake = Fake::new(CLIENT, SECRET, REDIRECT, people);
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
    let service = local.spawn().await.unwrap();
    World {
        dir,
        fake,
        app,
        service,
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}

/// A trip to the fake GitHub for `purpose` as `login`: the flow and code.
async fn authorize(world: &World, purpose: Purpose, login: &str) -> (Flow, String) {
    let (url, flow) = Flow::start_for(&world.app, Some("/projects"), purpose).unwrap();
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
    let (flow, code) = authorize(world, Purpose::SignIn, login).await;
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

async fn grant(world: &World, session: &str, private: bool, login: &str) -> (u16, Value) {
    let (flow, code) = authorize(world, Purpose::Repos { private }, login).await;
    call(
        world,
        "POST",
        session,
        "/v1/account/github/grant",
        json!({"code": code, "code_verifier": flow.verifier()}),
    )
    .await
}

fn names(body: &Value) -> Vec<String> {
    body["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["full_name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn public_access_lists_public_repositories_and_private_access_adds_the_rest() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;

    let (status, body) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["github"]["state"], "none");
    assert_eq!(body["projects"], json!([]));
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
        (409, json!("github_not_connected"))
    );

    // Public only: nothing beyond sign-in, so only public repositories.
    let (status, body) = grant(&world, &session, false, "octo-local").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["github"]["state"], "connected");
    assert_eq!(body["github"]["private"], false);
    assert_eq!(body["github"]["login"], "octo-local");
    let (_, list) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(names(&list), ["octo-local/hello-world"]);
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
        (404, json!("repository_not_found"))
    );

    // With private repositories.
    let (status, body) = grant(&world, &session, true, "octo-local").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["github"]["private"], true);
    let (_, list) = call(
        &world,
        "GET",
        &session,
        "/v1/account/github/repositories",
        json!({}),
    )
    .await;
    assert_eq!(
        names(&list),
        [
            "octo-local/hello-world",
            "octo-local/secret-plans",
            "acme/storefront"
        ]
    );

    // The token is on disk only encrypted.
    let store = world.dir.path().join(oa_auth::repos::STORE_DIR);
    for entry in std::fs::read_dir(&store).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(!text.contains("gho_"), "{text}");
    }

    // A project, once per repository.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "acme/storefront"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let project = body["project"].clone();
    assert_eq!(project["name"], "storefront");
    assert_eq!(project["repository"], "acme/storefront");
    assert_eq!(project["repository_id"], 7003);
    assert_eq!(project["default_branch"], "main");
    assert_eq!(project["private"], true);
    let id = project["id"].as_str().unwrap().to_string();
    assert!(oa_auth::repos::project_id(&id));
    let (_, again) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "acme/storefront"}),
    )
    .await;
    assert_eq!(again["project"]["id"], id.as_str());

    // The web server reads the token to read GitHub as the person.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/token",
        json!({}),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body["token"].as_str().unwrap().starts_with("gho_"));
    assert_eq!(body["private"], true);

    // Another account sees none of it.
    let other = sign_in(&world, "quiet-local").await;
    let (_, theirs) = call(&world, "GET", &other, "/v1/account/github", json!({})).await;
    assert_eq!(theirs["projects"], json!([]));
    assert_eq!(theirs["github"]["state"], "none");

    // Revoked on GitHub: the next read says reconnect; projects stay.
    world.fake.revoke("octo-local");
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
        (409, json!("github_reconnect"))
    );
    let (_, body) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
    assert_eq!(body["github"]["state"], "reconnect");
    assert_eq!(body["projects"][0]["id"], id.as_str());
    let (status, _) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/token",
        json!({}),
    )
    .await;
    assert_eq!(status, 409);

    // Connecting again clears it.
    let (_, body) = grant(&world, &session, true, "octo-local").await;
    assert_eq!(body["github"]["state"], "connected");

    // Removing the project, then disconnecting.
    let (status, _) = call(
        &world,
        "DELETE",
        &session,
        &format!("/v1/account/projects/{id}"),
        json!({}),
    )
    .await;
    assert_eq!(status, 200);
    let (_, body) = call(
        &world,
        "DELETE",
        &session,
        "/v1/account/github/grant",
        json!({}),
    )
    .await;
    assert_eq!(body["github"]["state"], "none");
    assert_eq!(body["projects"], json!([]));
}

#[tokio::test]
async fn access_from_another_github_account_or_without_a_session_is_refused() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;
    let _quiet = sign_in(&world, "quiet-local").await;
    let (status, body) = grant(&world, &session, true, "quiet-local").await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_other_account"))
    );
    let (_, body) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
    assert_eq!(body["github"]["state"], "none");

    let (status, _) = call(&world, "GET", "sess_nope", "/v1/account/github", json!({})).await;
    assert_eq!(status, 401);
    // A reused code is refused.
    let (flow, code) = authorize(&world, Purpose::Repos { private: true }, "octo-local").await;
    let body = json!({"code": code, "code_verifier": flow.verifier()});
    let (status, _) = call(
        &world,
        "POST",
        &session,
        "/v1/account/github/grant",
        body.clone(),
    )
    .await;
    assert_eq!(status, 200);
    let (status, again) = call(&world, "POST", &session, "/v1/account/github/grant", body).await;
    assert_eq!(
        (status, again["error"]["code"].clone()),
        (401, json!("github_denied"))
    );
}

/// A connected busy person and their session.
async fn busy(count: usize) -> (World, String) {
    let world = world_of(vec![fake::busy(count), fake::octo()]).await;
    let session = sign_in(&world, "busy-local").await;
    let (status, body) = grant(&world, &session, true, "busy-local").await;
    assert_eq!(status, 200, "{body}");
    (world, session)
}

async fn list(world: &World, session: &str, page: u32) -> (u16, Value) {
    call(
        world,
        "GET",
        session,
        &format!("/v1/account/github/repositories?page={page}"),
        json!({}),
    )
    .await
}

/// A real account: 250 repositories, one real-sized GitHub page per page
/// shown, "more" taken from GitHub's `Link` header. Disabled ones are left
/// out, archived ones marked, and a missing default branch reads as `main`.
#[tokio::test]
async fn a_busy_account_pages_by_githubs_links() {
    let (world, session) = busy(250).await;
    let before = world.fake.api_calls();
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(status, 200, "{}", body["error"]);
    assert_eq!(world.fake.api_calls() - before, 1);
    let listed = names(&body);
    assert_eq!(listed.len(), 29, "thirty, less the disabled one");
    assert!(!listed.contains(&"example-labs/project-0005".to_string()));
    assert_eq!(body["more"], true);
    assert_eq!(body["sso_hidden"], false);
    let rows = body["repositories"].as_array().unwrap();
    let find = |name: &str| rows.iter().find(|r| r["full_name"] == name).unwrap();
    assert_eq!(find("example-labs/project-0011")["archived"], true);
    assert!(find("acme-corp/project-0010").get("archived").is_none());
    assert_eq!(find("acme-corp/project-0013")["default_branch"], "main");
    assert_eq!(find("busy-local/project-0021")["private"], true);

    let (_, body) = list(&world, &session, 8).await;
    assert_eq!(
        (names(&body).len(), body["more"].clone()),
        (30, json!(true))
    );
    let (_, body) = list(&world, &session, 9).await;
    assert_eq!(
        (names(&body).len(), body["more"].clone()),
        (10, json!(false))
    );
    assert_eq!(names(&body).last().unwrap(), "acme-corp/project-0250");
}

/// Each way GitHub refuses says what happened, never "isn't answering",
/// and none of them marks the access ended.
#[tokio::test]
async fn github_refusals_say_what_went_wrong() {
    let (world, session) = busy(30).await;
    for (fault, status, code) in [
        (fake::Fault::RateLimited, 429, "github_rate_limited"),
        (fake::Fault::SecondaryRateLimit, 429, "github_rate_limited"),
        (fake::Fault::TooManyRequests, 429, "github_rate_limited"),
        (fake::Fault::ServerError(500), 502, "github_error"),
        (fake::Fault::NotJson, 502, "github_bad_answer"),
        (fake::Fault::OrgRestricted, 403, "github_forbidden"),
    ] {
        world.fake.fail("/user/repos", fault, 1);
        let (got, body) = list(&world, &session, 1).await;
        assert_eq!(
            (got, body["error"]["code"].as_str().unwrap()),
            (status, code),
            "{fault:?}: {body}"
        );
        let message = body["error"]["message"].as_str().unwrap();
        assert!(!message.contains("isn't answering"), "{fault:?}: {message}");
        let (_, state) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
        assert_eq!(state["github"]["state"], "connected", "{fault:?}");
    }
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(status, 200, "{body}");

    // An organization repository behind single sign-on.
    world
        .fake
        .fail("/repos/acme-corp/", fake::Fault::SsoRequired, 1);
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "acme-corp/project-0001"}),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (403, json!("github_sso_required")),
        "{body}"
    );
    // A repository GitHub disabled can't become a project.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "example-labs/project-0005"}),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (403, json!("github_forbidden")),
        "{body}"
    );
}

/// GitHub's 502 for a slow listing is tried once more, and single sign-on
/// hiding repositories is reported.
#[tokio::test]
async fn a_502_is_retried_and_hidden_organizations_are_reported() {
    let (world, session) = busy(60).await;
    world
        .fake
        .fail("/user/repos", fake::Fault::ServerError(502), 1);
    let before = world.fake.api_calls();
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(names(&body).len(), 29);
    assert_eq!(world.fake.api_calls() - before, 2);
    // Two 502s in a row are GitHub's problem, said as such.
    world
        .fake
        .fail("/user/repos", fake::Fault::ServerError(502), 2);
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (502, json!("github_error"))
    );

    world.fake.sso_partial();
    let (_, body) = list(&world, &session, 1).await;
    assert_eq!(body["sso_hidden"], true);
}
