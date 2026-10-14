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
        "PATCH" => world.http.patch(url).json(&body),
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

    // A project from a repository (the older one-step form).
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
    let held = &project["repositories"][0];
    assert_eq!(held["repository"], "acme/storefront");
    assert_eq!(held["repository_id"], 7003);
    assert_eq!(held["default_branch"], "main");
    assert_eq!(held["private"], true);
    assert_eq!(held["primary"], true);
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
    // Creating never reuses a project: the same repository makes a second.
    let second = again["project"]["id"].as_str().unwrap().to_string();
    assert_ne!(second, id);
    assert_eq!(
        again["project"]["repositories"][0]["repository"],
        "acme/storefront"
    );

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

    // Removing the projects, then disconnecting.
    for id in [&id, &second] {
        let (status, _) = call(
            &world,
            "DELETE",
            &session,
            &format!("/v1/account/projects/{id}"),
            json!({}),
        )
        .await;
        assert_eq!(status, 200);
    }
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
    // The shared list shape beside the older fields (#11156).
    assert_eq!(body["data"], body["repositories"]);
    assert_eq!(body["next"], "2");
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
    assert_eq!(body["next"], Value::Null);
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
    // Two 502s in a row are GitHub's problem, said as such (once the
    // kept page is past its hour).
    forget_pages(&world);
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

/// Step every kept page of this world past the hour it is kept.
fn forget_pages(world: &World) {
    oa_auth::repos::age_cached_lists(world.dir.path(), None, oa_auth::cache::KEEP);
}

/// Make this world's kept pages stale (past five minutes, within the hour).
fn stale_pages(world: &World) {
    oa_auth::repos::age_cached_lists(world.dir.path(), None, oa_auth::cache::FRESH);
}

/// Wait until the fake has answered `count` reads in all.
async fn reads_reach(world: &World, count: usize) {
    for _ in 0..400 {
        if world.fake.api_calls() >= count {
            // Let the refresh put its answer away.
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!(
        "the fake reached {} reads, not {count}",
        world.fake.api_calls()
    );
}

/// Many renders of /projects cost one GitHub read per freshness window;
/// a stale page is served at once while one read refreshes it; past the
/// hour it is read again.
#[tokio::test]
async fn repository_pages_are_read_once_per_five_minutes() {
    let (world, session) = busy(60).await;
    let before = world.fake.api_calls();
    for _ in 0..10 {
        let (status, body) = list(&world, &session, 1).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(names(&body).len(), 29);
    }
    let (status, _) = list(&world, &session, 2).await;
    assert_eq!(status, 200);
    assert_eq!(world.fake.api_calls() - before, 2, "one read per page");

    // Stale: served from the cache, with one refresh behind it.
    world.fake.sso_partial();
    stale_pages(&world);
    let at = world.fake.api_calls();
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["sso_hidden"], false, "the kept page, at once");
    reads_reach(&world, at + 1).await;
    let (_, body) = list(&world, &session, 1).await;
    assert_eq!(body["sso_hidden"], true, "the refreshed page");
    assert_eq!(world.fake.api_calls(), at + 1, "one refresh");

    // GitHub limiting the refresh keeps the stale page.
    stale_pages(&world);
    world.fake.fail("/user/repos", fake::Fault::RateLimited, 1);
    let at = world.fake.api_calls();
    let (status, _) = list(&world, &session, 1).await;
    assert_eq!(status, 200);
    reads_reach(&world, at + 1).await;
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(names(&body).len(), 29);

    // Past the hour it is read before answering, and a limit then shows.
    forget_pages(&world);
    world.fake.fail("/user/repos", fake::Fault::RateLimited, 1);
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (429, json!("github_rate_limited"))
    );
    let (status, _) = list(&world, &session, 1).await;
    assert_eq!(status, 200);
}

/// Adding or removing a project, disconnecting and connecting drop the
/// account's pages; a GitHub 401 on a refresh drops them too, and the next
/// listing reads GitHub again.
#[tokio::test]
async fn changes_and_a_401_drop_kept_pages() {
    let (world, session) = busy(40).await;
    let at = world.fake.api_calls();
    list(&world, &session, 1).await;
    list(&world, &session, 1).await;
    assert_eq!(world.fake.api_calls() - at, 1);

    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/account/projects",
        json!({"repository": "acme-corp/project-0001"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let id = body["project"]["id"].as_str().unwrap().to_string();
    let at = world.fake.api_calls();
    list(&world, &session, 1).await;
    assert_eq!(world.fake.api_calls() - at, 1, "read again after adding");

    call(
        &world,
        "DELETE",
        &session,
        &format!("/v1/account/projects/{id}"),
        json!({}),
    )
    .await;
    let at = world.fake.api_calls();
    list(&world, &session, 1).await;
    assert_eq!(world.fake.api_calls() - at, 1, "read again after removing");

    // Another account's pages are its own.
    let other = sign_in(&world, "octo-local").await;
    grant(&world, &other, true, "octo-local").await;
    let (_, theirs) = list(&world, &other, 1).await;
    assert_eq!(names(&theirs).len(), 3);
    let at = world.fake.api_calls();
    list(&world, &session, 1).await;
    assert_eq!(world.fake.api_calls(), at, "still kept");

    // Revoked on GitHub: the stale page is served once, its refresh gets
    // the 401, and then the listing says to reconnect.
    world.fake.revoke("busy-local");
    stale_pages(&world);
    let at = world.fake.api_calls();
    let (status, _) = list(&world, &session, 1).await;
    assert_eq!(status, 200);
    reads_reach(&world, at + 1).await;
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_reconnect"))
    );

    // Connecting again reads GitHub again.
    let (status, body) = grant(&world, &session, true, "busy-local").await;
    assert_eq!(status, 200, "{body}");
    let at = world.fake.api_calls();
    let (status, _) = list(&world, &session, 1).await;
    assert_eq!(status, 200);
    assert_eq!(world.fake.api_calls() - at, 1);

    // Disconnecting: nothing kept is served.
    call(
        &world,
        "DELETE",
        &session,
        "/v1/account/github/grant",
        json!({}),
    )
    .await;
    let (status, body) = list(&world, &session, 1).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_not_connected"))
    );
}

/// While the token's hourly budget is nearly spent, kept pages are served
/// without reading GitHub again.
#[tokio::test]
async fn a_nearly_spent_budget_serves_kept_pages() {
    let (world, session) = busy(40).await;
    world.fake.limit(world.fake.api_calls() + 50);
    let (status, _) = list(&world, &session, 1).await;
    assert_eq!(status, 200);
    stale_pages(&world);
    let at = world.fake.api_calls();
    for _ in 0..5 {
        let (status, _) = list(&world, &session, 1).await;
        assert_eq!(status, 200);
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        world.fake.api_calls(),
        at,
        "no refresh while the budget is low"
    );
}

/// Projects without a repository, with several, and the same repository in
/// several projects (2026-10-13): create needs only a name; repositories
/// are added, made primary, and removed; creating never reuses a project.
#[tokio::test]
async fn projects_hold_zero_or_more_repositories() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;

    // A name is enough, before GitHub is connected at all.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/projects",
        json!({"name": "finances", "instructions": "Keep it short."}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let bare = body["project"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["project"]["repositories"], json!([]));
    assert_eq!(body["project"]["instructions"], "Keep it short.");
    // Neither a name nor a repository is refused.
    let (status, _) = call(&world, "POST", &session, "/v1/projects", json!({})).await;
    assert_eq!(status, 400);

    grant(&world, &session, true, "octo-local").await;
    let patch = |change: Value| {
        let (world, session, bare) = (&world, &session, &bare);
        async move {
            call(
                world,
                "PATCH",
                session,
                &format!("/v1/projects/{bare}"),
                change,
            )
            .await
        }
    };
    // The first repository added is the primary.
    let (status, body) = patch(json!({"add_repository": "octo-local/hello-world"})).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["project"]["repositories"][0]["primary"], true);
    // A second one is not, until asked.
    let (_, body) = patch(json!({"add_repository": "octo-local/secret-plans"})).await;
    let repos = &body["project"]["repositories"];
    assert_eq!(repos[1]["repository"], "octo-local/secret-plans");
    assert_eq!(
        (repos[0]["primary"].clone(), repos[1]["primary"].clone()),
        (json!(true), json!(false))
    );
    let (_, body) = patch(json!({"primary_repository": "octo-local/secret-plans"})).await;
    let repos = &body["project"]["repositories"];
    assert_eq!(
        (repos[0]["primary"].clone(), repos[1]["primary"].clone()),
        (json!(false), json!(true))
    );
    // Removing the primary makes the first one left the primary.
    let (_, body) = patch(json!({"remove_repository": "octo-local/secret-plans"})).await;
    let repos = body["project"]["repositories"].as_array().unwrap().clone();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0]["repository"], "octo-local/hello-world");
    assert_eq!(repos[0]["primary"], true);
    // A repository that isn't in it.
    let (status, _) = patch(json!({"remove_repository": "octo-local/secret-plans"})).await;
    assert_eq!(status, 404);
    // Renamed, instructions cleared.
    let (_, body) = patch(json!({"name": "money", "instructions": null})).await;
    assert_eq!(body["project"]["name"], "money");
    assert!(body["project"].get("instructions").is_none());

    // The same repository, primary in a second project with the same name.
    let (status, body) = call(
        &world,
        "POST",
        &session,
        "/v1/projects",
        json!({"name": "money", "repositories": ["octo-local/hello-world"]}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_ne!(body["project"]["id"], bare.as_str());
    let (_, listed) = call(&world, "GET", &session, "/v1/account/github", json!({})).await;
    let projects = listed["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().all(|p| p["name"] == "money"
        && p["repositories"][0]["repository"] == "octo-local/hello-world"
        && p["repositories"][0]["primary"] == true));
}

/// Without the GitHub App, a credential-broker ticket hands one machine the
/// person's own OAuth token (#11572), and only when it has `repo`.
#[tokio::test]
async fn the_broker_hands_out_the_persons_own_token_only_with_private_access() {
    let world = world().await;
    let session = sign_in(&world, "octo-local").await;
    let ticket_for = |repository: &'static str| {
        call(
            &world,
            "POST",
            &session,
            "/v1/account/github/broker",
            json!({"repository": repository, "seconds": 600}),
        )
    };

    // Not connected: no ticket.
    let (status, body) = ticket_for("octo-local/hello-world").await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_not_connected"))
    );

    // Public only: the token can't push for the person.
    assert_eq!(grant(&world, &session, false, "octo-local").await.0, 200);
    let (status, _) = call(
        &world,
        "POST",
        &session,
        "/v1/projects",
        json!({"repositories": ["octo-local/hello-world"]}),
    )
    .await;
    assert_eq!(status, 200);
    let (status, body) = ticket_for("octo-local/hello-world").await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (409, json!("github_needs_repo_access"))
    );

    // Private access: a ticket, redeemed for the person's token for that
    // repository only.
    assert_eq!(grant(&world, &session, true, "octo-local").await.0, 200);
    let (status, body) = ticket_for("octo-local/hello-world").await;
    assert_eq!(status, 200, "{body}");
    let ticket = body["ticket"].as_str().unwrap().to_string();
    let redeem = |path: &'static str, ticket: String| {
        world
            .http
            .post(format!("{}{}", world.service, oa_auth::repos::broker::PATH))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "ticket={ticket}&protocol=https&host=github.com&path={path}"
            ))
            .send()
    };
    let answer = redeem("octo-local/hello-world.git", ticket.clone())
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 200);
    let text = answer.text().await.unwrap();
    assert!(
        text.starts_with("username=x-access-token\npassword="),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.len() > "password=".len() && l.starts_with("password="))
    );
    let answer = redeem("octo-local/secret-plans", ticket.clone())
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 403);
    assert!(!answer.text().await.unwrap().contains("password"));

    // A repository that isn't one of the person's projects: no ticket.
    let (status, body) = ticket_for("octo-local/secret-plans").await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (404, json!("repository_not_found"))
    );
}
