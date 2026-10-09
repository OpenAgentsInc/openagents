//! Connecting GitHub repositories through the whole site: sign in, the
//! Connect GitHub trip (asking for more only then), adding a project, the
//! sidebar's groups and the composer's picker, moving a chat, revocation,
//! and what other people see. The account service is `oa_auth::local`
//! over real tenancy stores; GitHub is the in-process fake.

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

use super::*;
use crate::chat_store::{Conversation, Store};

const HOST: &str = "127.0.0.1:4300";
const ORIGIN: &str = "http://127.0.0.1:4300";
const REDIRECT: &str = "http://127.0.0.1:4300/auth/github/callback";
const CLIENT: &str = "Ov23liWebTest";
const SECRET: &str = "web-test-secret";
const CHAT: &str = "12345678-1234-4234-8234-123456789abc";
const LOOSE: &str = "22345678-1234-4234-8234-123456789abc";

struct World {
    _root: tempfile::TempDir,
    site: Router,
    fake: Fake,
    store: Arc<Store>,
    github: reqwest::Client,
}

async fn world() -> World {
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
    let store = Arc::new(Store::local(root.path().join("chats")));
    config.chat_store = store.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    config.github = Some(Arc::new(app));
    World {
        _root: root,
        site: crate::router(config),
        fake,
        store,
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
}

impl Browser {
    async fn send(
        &mut self,
        world: &World,
        request: axum::http::request::Builder,
        body: Body,
    ) -> Answer {
        let mut request = request
            .header(header::HOST, HOST)
            .header(header::ACCEPT, "text/html");
        if !self.0.is_empty() {
            let cookies: Vec<String> = self.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        let response = world
            .site
            .clone()
            .oneshot(request.body(body).unwrap())
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

    async fn get(&mut self, world: &World, path: &str) -> Answer {
        self.send(world, Request::get(path), Body::empty()).await
    }

    async fn post(&mut self, world: &World, path: &str, form: &[(&str, &str)]) -> Answer {
        let body: String = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        self.send(
            world,
            Request::post(path)
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
            Body::from(body),
        )
        .await
    }

    /// From `start`, act on the fake GitHub as `login`, and land on the
    /// callback.
    async fn through_github(&mut self, world: &World, start: &str, login: &str) -> Answer {
        let start = self.get(world, start).await;
        assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
        let at_github = world
            .github
            .get(format!("{}&login={login}", start.location()))
            .send()
            .await
            .unwrap();
        let back = at_github.headers()["location"]
            .to_str()
            .unwrap()
            .to_string();
        self.get(world, back.strip_prefix(ORIGIN).unwrap()).await
    }

    async fn sign_in(&mut self, world: &World, login: &str) {
        let done = self
            .through_github(world, "/auth/github?return_to=/", login)
            .await;
        assert_eq!(done.status, StatusCode::OK, "{}", done.body);
        assert!(self.0.contains_key("oa_cloud_session"));
    }
}

/// The chat owner for the account that signed in with GitHub `login`.
fn account_owner_of(world: &World, login: &str) -> String {
    let store = tenancy::Accounts::open(&world._root.path().join("accounts"))
        .unwrap()
        .store()
        .unwrap();
    let identity = store
        .identities
        .github
        .values()
        .find(|identity| identity.profile.login == login)
        .unwrap_or_else(|| panic!("no account for {login}"));
    crate::chat_store::account_owner(&identity.account)
}

/// The value of the first hidden `name` input after `marker` in `html`.
fn hidden(html: &str, marker: &str, name: &str) -> String {
    let from = html
        .find(marker)
        .unwrap_or_else(|| panic!("{marker}: {html}"));
    let html = &html[from..];
    let at = html
        .find(&format!(r#"name="{name}" value=""#))
        .unwrap_or_else(|| panic!("{name}: {html}"));
    let rest = &html[at + name.len() + 15..];
    rest[..rest.find('"').unwrap()].to_string()
}

/// The chat composer's CSRF token (the input that joins `chat-form`).
fn chat_csrf(html: &str) -> String {
    html.match_indices(r#"name="csrf" value=""#)
        .map(|(at, _)| &html[at + 19..])
        .find_map(|rest| {
            let end = rest.find('"')?;
            rest[end..]
                .starts_with(r#"" form="chat-form""#)
                .then(|| rest[..end].to_string())
        })
        .unwrap_or_else(|| panic!("no chat csrf: {html}"))
}

fn chat(id: &str, owner: &str, title: &str, project: Option<&str>) -> Conversation {
    Conversation {
        id: id.into(),
        owner: owner.into(),
        revision: 1,
        title: title.into(),
        messages: Vec::new(),
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: project.map(str::to_string),
        terminal: None,
        environment: None,
        tasks: Vec::new(),
        opened_unix: None,
    }
}

#[tokio::test]
async fn connecting_github_adds_a_project_that_groups_chats_for_its_owner_only() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;

    // Before connecting: the page offers Connect GitHub and public only.
    let page = browser.get(&world, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("Connect GitHub"), "{}", page.body);
    assert!(page.body.contains("/auth/github/repos?access=public"));
    crate::copy_guard::assert_plain(PAGE, &page.body);

    // Connect asks GitHub for repo and read:org, only now.
    let start = browser
        .get(&world, "/auth/github/repos?access=private")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let authorize = url::Url::parse(start.location()).unwrap();
    let scope = authorize
        .query_pairs()
        .find(|(k, _)| k == "scope")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(scope, "read:user repo read:org");
    assert!(browser.0["oa_auth_flow"].ends_with(".repos"));
    let at_github = world
        .github
        .get(format!("{}&login=octo-local", start.location()))
        .send()
        .await
        .unwrap();
    let back = at_github.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    // The callback continues with a same-site step and keeps the flow.
    let callback = browser
        .get(&world, back.strip_prefix(ORIGIN).unwrap())
        .await;
    assert_eq!(callback.status, StatusCode::OK, "{}", callback.body);
    assert!(callback.body.contains(FINISH), "{}", callback.body);
    assert!(browser.0.contains_key("oa_auth_flow"));
    let next = callback.body[callback.body.find(FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    let finished = browser.get(&world, &next).await;
    assert_eq!(finished.status, StatusCode::SEE_OTHER, "{}", finished.body);
    assert_eq!(finished.location(), PAGE);
    assert!(!browser.0.contains_key("oa_auth_flow"));

    // The page shows at once; the repositories (private ones included)
    // load after it, one page at a time. Add one.
    let shell = browser.get(&world, PAGE).await;
    assert!(shell.body.contains("Disconnect GitHub"), "{}", shell.body);
    assert!(shell.body.contains("/projects/repositories?page=1"));
    let page = browser.get(&world, "/projects/repositories?page=1").await;
    assert!(page.body.contains("acme/storefront"), "{}", page.body);
    assert!(page.body.contains("octo-local/secret-plans"));
    let csrf = hidden(
        &page.body,
        r#"<form method="post" action="/projects">"#,
        "csrf",
    );
    let added = browser
        .post(
            &world,
            PAGE,
            &[("csrf", &csrf), ("repository", "acme/storefront")],
        )
        .await;
    assert_eq!(added.status, StatusCode::SEE_OTHER, "{}", added.body);
    let page = browser.get(&world, PAGE).await;
    assert!(page.body.contains("Your projects"), "{}", page.body);
    let id = page.body[page.body.find("/?project=prj_").unwrap() + 10..][..20].to_string();
    assert!(oa_auth::repos::project_id(&id), "{id}");

    // The home page offers the project, preselected from a group's New chat.
    let home = browser.get(&world, &format!("/?project={id}")).await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(
        home.body.contains(r#"name="project" form="chat-form""#),
        "{}",
        home.body
    );
    assert!(
        home.body.contains(&format!(
            r#"<option value="{id}" selected>storefront</option>"#
        )),
        "{}",
        home.body
    );

    // Chats in the project group under it; a chat outside stays in Chats.
    // Signed in, chats belong to the account (#11039).
    let owner = account_owner_of(&world, "octo-local");
    world
        .store
        .create(&chat(CHAT, &owner, "Fix the login", Some(&id)))
        .await
        .unwrap();
    world
        .store
        .create(&chat(LOOSE, &owner, "Plan the week", None))
        .await
        .unwrap();
    let home = browser.get(&world, "/").await;
    let at = |text: &str| {
        home.body
            .find(text)
            .unwrap_or_else(|| panic!("{text}: {}", home.body))
    };
    let group = format!(r#"data-oa-project="{id}""#);
    assert!(at(">Projects<") < at(&group));
    assert!(at(&group) < at("Fix the login"));
    assert!(at("Fix the login") < at(">Chats<"));
    assert!(at(">Chats<") < at("Plan the week"));
    assert!(home.body.contains("Manage projects"));
    assert!(home.body.contains("Move to storefront"));
    assert!(home.body.contains("Remove from storefront"));

    // Move the loose chat in, with HTMX: the list comes back regrouped.
    let token = chat_csrf(&home.body);
    let moved = browser
        .send(
            &world,
            Request::post(format!("/chat/{LOOSE}/project"))
                .header(header::ORIGIN, ORIGIN)
                .header("sec-fetch-site", "same-origin")
                .header("hx-request", "true")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
            Body::from(format!("csrf={token}&current=&project={id}")),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.body);
    let at = |text: &str| {
        moved
            .body
            .find(text)
            .unwrap_or_else(|| panic!("{text}: {}", moved.body))
    };
    assert!(at(&group) < at("Plan the week"));
    assert!(!moved.body.contains(">Chats<"));
    assert_eq!(
        world
            .store
            .load(&owner, LOOSE)
            .await
            .unwrap()
            .unwrap()
            .conversation
            .project
            .as_deref(),
        Some(id.as_str())
    );
    // A project that isn't theirs is refused.
    let refused = browser
        .post(
            &world,
            &format!("/chat/{LOOSE}/project"),
            &[("csrf", &token), ("project", "prj_0000000000000000")],
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);

    // Signed out (same browser, no session): the account's chats and
    // project names stay with the account.
    let mut out = Browser::default();
    let home = out.get(&world, "/").await;
    assert!(!home.body.contains("Fix the login"), "{}", home.body);
    assert!(!home.body.contains("storefront"), "{}", home.body);
    assert!(!home.body.contains("acme/"));

    // Another account in that browser sees no names either.
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let home = other.get(&world, "/").await;
    assert!(!home.body.contains("storefront"), "{}", home.body);
    assert!(home.body.contains("Connect a GitHub repository"));

    // Revoked on GitHub: projects stay and say Reconnect GitHub.
    world.fake.revoke("octo-local");
    let list = browser.get(&world, "/projects/repositories?page=1").await;
    assert!(list.body.contains("GitHub access ended"), "{}", list.body);
    let page = browser.get(&world, PAGE).await;
    assert!(page.body.contains("GitHub access ended"), "{}", page.body);
    assert!(page.body.contains("storefront"));
    let home = browser.get(&world, "/").await;
    assert!(home.body.contains("Reconnect GitHub"), "{}", home.body);
    assert!(home.body.contains("Fix the login"));
}

#[tokio::test]
async fn public_only_asks_for_nothing_new_and_signed_out_visitors_go_to_log_in() {
    let world = world().await;
    let mut visitor = Browser::default();
    let page = visitor.get(&world, PAGE).await;
    assert_eq!(page.status, StatusCode::SEE_OTHER);
    assert_eq!(page.location(), "/login?return_to=%2Fprojects");
    let start = visitor
        .get(&world, "/auth/github/repos?access=public")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER);
    assert!(
        start.location().starts_with("/login"),
        "{}",
        start.location()
    );

    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let start = browser
        .get(&world, "/auth/github/repos?access=public")
        .await;
    let authorize = url::Url::parse(start.location()).unwrap();
    assert_eq!(
        authorize
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .unwrap()
            .1,
        "read:user"
    );
    // A finish step without its flow is refused, not guessed at.
    let stale = browser
        .get(&world, &format!("{FINISH}?code=abc&state=nope"))
        .await;
    assert_eq!(stale.status, StatusCode::BAD_REQUEST, "{}", stale.body);
    assert!(stale.body.contains("expired"));
}

#[test]
fn closed_groups_come_from_their_cookie_and_the_page_reads_plainly() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        "oa_theme=dark; oa_project_groups=prj_0123456789abcdef.junk.prj_fedcba9876543210"
            .parse()
            .unwrap(),
    );
    let closed = closed_groups(&headers);
    assert_eq!(
        closed.into_iter().collect::<Vec<_>>(),
        ["prj_0123456789abcdef", "prj_fedcba9876543210"]
    );
    let status = Status {
        access: Access::None,
        projects: Vec::new(),
    };
    let html = view(&status, "t", "", None, false).into_string();
    crate::copy_guard::assert_plain(PAGE, &html);
    let status = Status {
        access: Access::Connected {
            login: "octo".into(),
            private: false,
        },
        projects: Vec::new(),
    };
    // The page shows at once; the list loads after it.
    let html = view(&status, "t", "", None, false).into_string();
    assert!(html.contains("Include private repositories"), "{html}");
    assert!(
        html.contains(r#"hx-get="/projects/repositories?page=1""#),
        "{html}"
    );
    assert!(html.contains("Loading your repositories"));
    crate::copy_guard::assert_plain(PAGE, &html);
    let repo = |id: u64| Repository {
        id,
        full_name: format!("octo/repo-{id}"),
        default_branch: "main".into(),
        private: false,
        archived: id == 2,
        installation: None,
    };
    let listing = |repositories: Vec<Repository>, more: bool| {
        Ok(Listing {
            repositories,
            more,
            sso_hidden: false,
        })
    };
    // One page, newest first, with Show more when GitHub has more.
    let page: Vec<Repository> = (1..=30).map(repo).collect();
    let html = repos_page(&status, listing(page.clone(), true), "t", "", 1).into_string();
    assert!(html.contains("octo/repo-1") && html.contains("octo/repo-30"));
    assert!(
        html.contains(r#"hx-get="/projects/repositories?page=2""#),
        "{html}"
    );
    assert!(html.contains("Show more"));
    assert!(html.contains("Archived"));
    assert!(!html.contains("single sign-on"));
    crate::copy_guard::assert_plain(PAGE, &html);
    let html = repos_page(&status, listing(page.clone(), false), "t", "", 2).into_string();
    assert!(!html.contains("Show more"));
    let html = repos_page(&status, listing(page.clone(), false), "t", "zzz", 1).into_string();
    assert!(html.contains("No repositories match."));
    let html = repos_page(&status, listing(vec![repo(1)], true), "t", "my app", 1).into_string();
    assert!(html.contains("page=2&amp;q=my+app"), "{html}");
    // A repository not on the pages can be added by owner/name; one that
    // is listed is not offered twice.
    let html = repos_page(
        &status,
        listing(page.clone(), true),
        "t",
        "acme/far-away",
        1,
    )
    .into_string();
    assert!(html.contains("Add acme/far-away"), "{html}");
    assert!(!html.contains("No repositories match."));
    crate::copy_guard::assert_plain(PAGE, &html);
    let html =
        repos_page(&status, listing(page.clone(), true), "t", "octo/repo-3", 1).into_string();
    assert!(!html.contains("Add octo/repo-3"));
    // Single sign-on hiding repositories is said once.
    let hidden = Ok(Listing {
        repositories: page,
        more: false,
        sso_hidden: true,
    });
    let html = repos_page(&status, hidden, "t", "", 1).into_string();
    assert!(html.contains("single sign-on"), "{html}");
    crate::copy_guard::assert_plain(PAGE, &html);
    // A rate limit says so, not that GitHub isn't answering.
    let limited = Err(RepoCallError::Repo(RepoError::RateLimited));
    let html = repos_page(&status, limited, "t", "", 1).into_string();
    assert!(
        html.contains("limiting") && !html.contains("answering"),
        "{html}"
    );
}

/// Connect GitHub (public repositories) for a signed-in browser.
async fn connect_public(browser: &mut Browser, world: &World, login: &str) {
    let callback = browser
        .through_github(world, "/auth/github/repos?access=public", login)
        .await;
    assert_eq!(callback.status, StatusCode::OK, "{}", callback.body);
    let next = callback.body[callback.body.find(FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    let finished = browser.get(world, &next).await;
    assert_eq!(finished.status, StatusCode::SEE_OTHER, "{}", finished.body);
}

/// The sealed composer selection after a pick (`#composer-state`).
fn composer_state(html: &str) -> String {
    let from = html
        .find(r#"id="composer-state""#)
        .unwrap_or_else(|| panic!("no composer state: {html}"));
    let rest = &html[from..];
    let at = rest.find(r#"value=""#).unwrap() + 7;
    rest[at..at + rest[at..].find('"').unwrap()].to_string()
}

/// Pick `repository` in the composer: the panel's form, then its post.
async fn pick(browser: &mut Browser, world: &World, repository: &str) -> Answer {
    let panel = browser.get(world, "/composer/repository").await;
    assert_eq!(panel.status, StatusCode::OK, "{}", panel.body);
    let marker = r#"<form action="/composer/repository""#;
    let selection = hidden(&panel.body, marker, "selection");
    let csrf = hidden(&panel.body, marker, "csrf");
    browser
        .post(
            world,
            "/composer/repository",
            &[
                ("selection", &selection),
                ("csrf", &csrf),
                ("value", repository),
            ],
        )
        .await
}

/// The composer reads GitHub as the signed-in person when they connected
/// it, and without a token otherwise. Reads without a token share one
/// small hourly limit for the whole server; past it, those visitors are
/// told so plainly, while connected people are unaffected. Branch lists
/// are kept, so opening the panel again doesn't read GitHub.
#[tokio::test]
async fn the_composer_reads_github_as_the_person_and_keeps_branch_lists() {
    let world = world().await;
    let mut visitor = Browser::default();
    visitor.get(&world, "/").await;
    let picked = pick(&mut visitor, &world, "octo-local/hello-world").await;
    assert!(
        picked.body.contains("Selection updated."),
        "{}",
        picked.body
    );
    assert_eq!(world.fake.anonymous_calls(), 2, "repository and branch");
    let state = composer_state(&picked.body);
    let branches = format!(
        "/composer/branch?selection={}",
        url::form_urlencoded::byte_serialize(state.as_bytes()).collect::<String>()
    );
    for _ in 0..5 {
        let panel = visitor.get(&world, &branches).await;
        assert!(panel.body.contains(">main<"), "{}", panel.body);
    }
    assert_eq!(world.fake.anonymous_calls(), 3, "one branch list read");

    // Spend the shared limit; then visitors without a connection hear why.
    let mut limited = None;
    for _ in 0..40 {
        let picked = pick(&mut visitor, &world, "octo-local/hello-world").await;
        if picked.body.contains(crate::composer::LIMITED_ANONYMOUS) {
            limited = Some(picked.body);
            break;
        }
    }
    let limited = limited.expect("the anonymous limit was reached");
    assert!(!limited.contains("busy"), "{limited}");
    crate::copy_guard::assert_plain("/composer/repository", &limited);
    assert!(world.fake.anonymous_calls() > oa_auth::fake::ANONYMOUS_LIMIT);
    // The kept branch list is still served, stale ones too.
    let panel = visitor.get(&world, &branches).await;
    assert!(panel.body.contains(">main<"), "{}", panel.body);
    crate::composer::age_branch_lists(oa_auth::cache::FRESH);
    let panel = visitor.get(&world, &branches).await;
    assert!(panel.body.contains(">main<"), "{}", panel.body);

    // Signed in without GitHub connected: still without a token.
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let picked = pick(&mut quiet, &world, "octo-local/hello-world").await;
    assert!(
        picked.body.contains(crate::composer::LIMITED_ANONYMOUS),
        "{}",
        picked.body
    );

    // Connected: GitHub is read with the person's own access.
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    connect_public(&mut browser, &world, "octo-local").await;
    let anonymous = world.fake.anonymous_calls();
    let picked = pick(&mut browser, &world, "octo-local/hello-world").await;
    assert!(
        picked.body.contains("Selection updated."),
        "{}",
        picked.body
    );
    let state = composer_state(&picked.body);
    let panel = browser
        .get(
            &world,
            &format!(
                "/composer/branch?selection={}",
                url::form_urlencoded::byte_serialize(state.as_bytes()).collect::<String>()
            ),
        )
        .await;
    assert!(panel.body.contains(">main<"), "{}", panel.body);
    assert_eq!(
        world.fake.anonymous_calls(),
        anonymous,
        "none without a token"
    );
}

mod app;
