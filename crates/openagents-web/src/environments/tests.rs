use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use coder_environment_operator::activity::{CheckLine, Entry, Record, Stage};
use coder_environment_operator::agent::Phase;
use coder_environment_operator::studio::claude::{Run, RunState};
use coder_environment_operator::studio::{Candidate, Status, Summary, Version, View};
use serde_json::json;
use tower::ServiceExt;

use super::{same_site, view};

async fn send(config: crate::Config, request: Request<Body>) -> (StatusCode, String) {
    let response = crate::router(config).oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

fn get(path: &str, host: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header(header::HOST, host)
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .unwrap()
}

/// The text a person reads: tags removed.
fn text(html: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => {
                tag = false;
                out.push(' ');
            }
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn summary(status: Status) -> Summary {
    Summary {
        id: "env-1".into(),
        repository: "example/repo".into(),
        branch: "main".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        status,
        saved: None,
        updated_ms: 1,
        account: None,
    }
}

fn record(entry: Entry) -> Record {
    Record { at_ms: 1, entry }
}

fn conversation() -> Vec<Record> {
    vec![
        record(Entry::User {
            text: "Set up example/repo on main so agents can build and test it.".into(),
        }),
        record(Entry::Starting),
        record(Entry::Source {
            ok: true,
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
            output: String::new(),
        }),
        record(Entry::Agent {
            text: "I'll look at how this builds.".into(),
        }),
        record(Entry::Explored {
            command: "cat README.md".into(),
            exit: Some(0),
            output: "# repo\n".into(),
        }),
        record(Entry::Explored {
            command: "ls".into(),
            exit: Some(0),
            output: "Makefile\n".into(),
        }),
        record(Entry::Recipe {
            revision: 2,
            script: "make deps".into(),
            previous: None,
        }),
        record(Entry::Install {
            revision: 2,
            exit: Some(1),
            output: "libfoo is missing\n".into(),
        }),
        record(Entry::Question {
            text: "May I add libfoo?".into(),
        }),
        record(Entry::User {
            text: "Yes.".into(),
        }),
        record(Entry::Recipe {
            revision: 3,
            script: "apt-get install libfoo\nmake deps".into(),
            previous: Some("make deps".into()),
        }),
        record(Entry::Install {
            revision: 3,
            exit: Some(0),
            output: "done\n".into(),
        }),
        record(Entry::Checks {
            checks: vec![CheckLine {
                name: "build".into(),
                command: "make".into(),
            }],
        }),
        record(Entry::Build {
            stage: Stage::Started,
            detail: "Recipe revision 3 on a fresh machine".into(),
        }),
        record(Entry::Build {
            stage: Stage::Passed,
            detail: "Image saved".into(),
        }),
        record(Entry::Verify {
            stage: Stage::Started,
            detail: "1 check on a fresh machine from the image".into(),
        }),
        record(Entry::Verify {
            stage: Stage::Passed,
            detail: "1 check passed".into(),
        }),
        record(Entry::Ready {
            summary: "Installs and builds with make.".into(),
        }),
    ]
}

fn review() -> View {
    View {
        summary: summary(Status::ReadyToSave),
        records: conversation(),
        phase: Phase::Review {
            verification: "ver-1".into(),
        },
        question: None,
        candidate: Some(Candidate {
            digest: "c".repeat(64),
            recipe_revision: 3,
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            image: "oaenv-build-1".into(),
            checks: vec![CheckLine {
                name: "build".into(),
                command: "make".into(),
            }],
            summary: "Installs and builds with make.".into(),
        }),
        versions: vec![],
        recipe: Some("apt-get install libfoo\nmake deps".into()),
        runs: vec![],
    }
}

#[tokio::test]
async fn without_a_studio_the_pages_say_so_and_stay_local() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.public_hosts = vec!["openagents.com".into()];
    let (status, body) = send(config.clone(), get("/environments", "127.0.0.1:4300")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body.contains("Environments aren&#39;t set up on this server.")
            || body.contains("Environments aren't set up on this server.")
    );
    assert!(!body.contains("href=\"/environments\""));
    // A public host without environments: the ordinary not-found page.
    let (status, body) = send(config, get("/environments", "openagents.com")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body.contains("Nothing on this site has that address."),
        "{body}"
    );
}

#[test]
fn a_ready_setup_reads_as_a_conversation_with_a_save_card() {
    let html = view::transcript(&review(), view::Claude::Unavailable, None).into_string();
    for needle in [
        "Explored the repository",
        "2 commands",
        "Wrote the install recipe",
        "Edited the install recipe",
        "+apt-get install libfoo",
        "Ran the install",
        "Exit 1",
        "May I add libfoo?",
        "Build a clean image from the recipe",
        "Check it on a fresh computer",
        "Ready to save",
        "1 of 1 passed",
        "action=\"/environments/env-1/save\"",
        "name=\"candidate\"",
        "Save environment",
    ] {
        assert!(html.contains(needle), "{needle} missing");
    }
    assert!(
        oa_copy::violations(&text(&html), &[]).is_empty(),
        "{:?}",
        oa_copy::violations(&text(&html), &[])
    );
}

#[test]
fn a_stopped_setup_offers_a_retry_and_a_saved_one_offers_claude_code() {
    let mut v = review();
    v.records.push(record(Entry::Failed {
        reason: "The fresh-machine check failed: make exited 2.".into(),
    }));
    v.phase = Phase::Failed { reason: "x".into() };
    v.candidate = None;
    let html = view::transcript(&v, view::Claude::Unavailable, None).into_string();
    assert!(html.contains("Setup stopped") && html.contains("/environments/env-1/retry"));
    assert!(html.contains("Try again"));

    let mut v = review();
    v.records.push(record(Entry::Saved { number: 1 }));
    v.phase = Phase::Saved { number: 1 };
    v.candidate = None;
    v.versions = vec![Version {
        number: 1,
        selected: true,
        image: "oaenv-build-1".into(),
        created_ms: 1,
    }];
    let without = view::transcript(&v, view::Claude::Unavailable, None).into_string();
    assert!(without.contains("add your Anthropic API key"));
    assert!(!without.contains("/environments/env-1/claude"));
    let with = view::transcript(&v, view::Claude::Ready, None).into_string();
    assert!(with.contains("Run Claude Code on version 1"));
    assert!(with.contains("action=\"/environments/env-1/claude\""));
    assert!(!with.contains("name=\"candidate\""));
    let settings = view::transcript(&v, view::Claude::AddKey, None).into_string();
    assert!(settings.contains("href=\"/settings/claude\""), "{settings}");
    assert!(!settings.contains("/environments/env-1/claude"));
    // Progress lines read as plain lines, without a visible "Status".
    let mut working = review();
    working.phase = Phase::Working;
    working.candidate = None;
    let working = view::transcript(&working, view::Claude::Ready, None).into_string();
    assert!(working.contains("Working…"));
    assert!(
        !working.contains(r#"<h2 class="oa-message-author">"#),
        "{working}"
    );
    // The setup chat keeps its newest line in view.
    let thread = view::thread("/environments/env-1/events", maud::html! {}).into_string();
    assert!(thread.contains("data-oa-scroll-follow"), "{thread}");
    for html in [&without, &with, &settings] {
        assert!(oa_copy::violations(&text(html), &[]).is_empty());
    }
}

#[test]
fn a_claude_code_run_reads_as_a_conversation() {
    let run = Run {
        id: "claude-env-1-1".into(),
        environment: "env-1".into(),
        prompt: "Fix the failing test".into(),
        version: Some(1),
        state: RunState::Running,
        events: vec![
            json!({"event":"delta","text":"Looking at the test."}),
            json!({"event":"entry","entry":{"tool":"Bash","command":"make test"}}),
        ],
        reply: None,
        error: None,
        created_ms: 1,
        cost_usd: None,
        paused_until: None,
    };
    let html = view::run_transcript(&run).into_string();
    for needle in [
        "Fix the failing test",
        "Started from environment version 1",
        "Looking at the test.",
        "make test",
        "Claude Code is working",
        "/environments/env-1/runs/claude-env-1-1/stop",
    ] {
        assert!(html.contains(needle), "{needle} missing");
    }
    assert!(oa_copy::violations(&text(&html), &[]).is_empty());
}

#[test]
fn the_list_and_picker_have_their_empty_and_error_states() {
    let empty = view::index(&[]).into_string();
    assert!(empty.contains("No environments yet") && empty.contains("/environments/new"));
    let mut row = summary(Status::NeedsInput);
    row.saved = Some(2);
    let list = view::index(&[row]).into_string();
    assert!(list.contains("Needs your answer") && list.contains("Version 2"));
    let pick = view::pick(&view::Pick {
        list: false,
        repo: "nope",
        error: Some("Enter a GitHub repository as owner/name or its github.com address."),
    })
    .into_string();
    assert!(pick.contains("Enter a GitHub repository"));
    let branch =
        view::branch("example/repo", &["main".into(), "dev".into()], "main", None).into_string();
    assert!(branch.contains("name=\"branch\"") && branch.contains("Set up environment"));
    for html in [&empty, &list, &pick, &branch] {
        assert!(oa_copy::violations(&text(html), &[]).is_empty());
    }
}

#[test]
fn posts_must_come_from_this_site() {
    let mut h = HeaderMap::new();
    assert!(same_site(&h), "a client without browser headers");
    h.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    assert!(!same_site(&h));
    h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
    assert!(same_site(&h));
    let mut h = HeaderMap::new();
    h.insert("origin", HeaderValue::from_static("http://127.0.0.1:4300"));
    h.insert("host", HeaderValue::from_static("127.0.0.1:4300"));
    assert!(same_site(&h));
    h.insert("origin", HeaderValue::from_static("https://evil.example"));
    assert!(!same_site(&h));
}

#[tokio::test]
async fn on_a_server_that_signs_people_in_the_pages_need_a_signed_in_person() {
    use crate::projects::tests::{Browser, world};
    let world = world().await;
    let mut browser = Browser::default();
    // Signed out: log in first, then come back to the same page.
    for (path, back) in [
        ("/environments", "%2Fenvironments"),
        ("/environments/new", "%2Fenvironments%2Fnew"),
        (
            "/environments/new?repo=octo%2Frepo",
            "%2Fenvironments%2Fnew%3Frepo%3Docto%252Frepo",
        ),
    ] {
        let page = browser.get(&world, path).await;
        assert_eq!(page.status, StatusCode::SEE_OTHER, "{path}: {}", page.body);
        assert_eq!(page.location(), format!("/login?return_to={back}"));
    }
    // The repository list (an HTMX fragment) sends HTMX to log in.
    let fragment = browser
        .send(
            &world,
            Request::get(super::REPOS).header("hx-request", "true"),
            Body::empty(),
        )
        .await;
    assert_eq!(fragment.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        fragment.headers["hx-redirect"],
        "/login?return_to=%2Fenvironments"
    );
    // A post from a signed-out browser goes to log in.
    let post = browser
        .send(
            &world,
            Request::post("/environments")
                .header("sec-fetch-site", "same-origin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
            Body::from("repo=octo%2Frepo"),
        )
        .await;
    assert_eq!(post.status, StatusCode::SEE_OTHER);
    assert!(post.location().starts_with("/login"));
    // Signed in, the page and the header agree: no Log in in the header
    // (this server has no studio, so the page says so).
    browser.sign_in(&world, "octo-local").await;
    let page = browser.get(&world, "/environments").await;
    assert_eq!(page.status, StatusCode::NOT_FOUND, "{}", page.body);
    assert!(!page.body.contains("href=\"/login"), "{}", page.body);
}

#[test]
fn the_empty_state_and_log_in_card_center_in_the_main_area() {
    let empty = view::index(&[]).into_string();
    assert!(
        empty.contains(r#"<div class="oa-page" data-align="center"><div class="oa-empty-message""#),
        "{empty}"
    );
    let css = openagents_ui::stylesheet();
    assert!(css.contains(r#".oa-page[data-align="center"]"#));
}

#[test]
fn the_repository_list_loads_after_the_page_a_page_at_a_time() {
    let pick = view::pick(&view::Pick {
        list: true,
        repo: "",
        error: None,
    })
    .into_string();
    assert!(
        pick.contains(r#"hx-get="/environments/repositories?page=1""#),
        "{pick}"
    );
    assert!(pick.contains(r#"hx-trigger="load""#) && pick.contains("Loading your repositories"));
    assert!(pick.contains("Or paste a GitHub address"));
    let rows = [
        view::RepoRow {
            full_name: "octo/newest",
            private: true,
        },
        view::RepoRow {
            full_name: "octo/older",
            private: false,
        },
    ];
    let page = view::repos(&rows, 1, true).into_string();
    assert!(page.find("octo/newest") < page.find("octo/older"));
    assert!(
        page.contains(r#"href="/environments/new?repo=octo%2Fnewest""#),
        "{page}"
    );
    assert!(page.contains("Private") && page.contains("Show more"));
    assert!(page.contains(r#"hx-get="/environments/repositories?page=2""#));
    let last = view::repos(&rows, 2, false).into_string();
    assert!(!last.contains("Show more"));
    let none = view::repos(&[], 1, false).into_string();
    assert!(none.contains("No repositories found."));
    let unconnected = view::repos_unconnected(false).into_string();
    assert!(unconnected.contains(r#"href="/projects""#));
    let failed = view::repos_failed("GitHub didn't answer.", 1).into_string();
    assert!(failed.contains("Try again") && failed.contains(r#"id="env-repos-1""#));
    for html in [&pick, &page, &none, &unconnected, &failed] {
        assert!(oa_copy::violations(&text(html), &[]).is_empty(), "{html}");
    }
}

/// A studio on fake machines whose setup agent never answers: enough for
/// the pages, which only read its records.
fn fake_studio(
    state: &std::path::Path,
) -> std::sync::Arc<coder_environment_operator::studio::Studio> {
    use coder_environment_operator::studio::{Config, SCHEMA, Studio, Transports};
    use coder_working_computer::provider::fake::FakeProvider;
    use std::collections::{BTreeMap, BTreeSet};
    let provider = std::sync::Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let config = Config {
        schema: SCHEMA.into(),
        state: state.into(),
        machines: coder_environment_operator::Config {
            schema: coder_environment_operator::SCHEMA.into(),
            provider: coder_environment_operator::ProviderKind::Boat,
            gce: None,
            workdir: "/home/user/repo".into(),
            template: Some("oa-coder-runtime-20261008".into()),
            credential_names: BTreeSet::new(),
            tick_seconds: 15,
        },
        owner: coder_working_computer::Principal {
            workspace: "local".into(),
            principal: "owner".into(),
        },
        codex_home: None,
        model: None,
        size: None,
        deadline_seconds: None,
        github_token: None,
        claude_key: None,
        model_api: None,
    };
    let transports: Transports<codex_transport::fake::FakeTransport> =
        std::sync::Arc::new(|_env: &str| Ok(codex_transport::fake::FakeTransport::default()));
    Studio::start(
        config,
        "oa-coder-runtime-20261008",
        coder_environment_operator::Providers {
            setup: provider.clone(),
            build: provider.clone(),
            verify: provider,
        },
        std::sync::Arc::new(
            |_: &BTreeSet<String>| Ok(coder_environment::evidence::Redactor::new()),
        ),
        transports,
        std::time::Duration::from_millis(5),
    )
    .unwrap()
}

fn resolved(repository: &str) -> coder_environment_operator::studio::github::Resolved {
    coder_environment_operator::studio::github::Resolved {
        repository: coder_environment_operator::studio::github::RepoName::parse(repository)
            .unwrap(),
        branch: "main".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        private: false,
    }
}

/// On a public host (#11162): a signed-out visitor logs in first; a
/// signed-in person who isn't a site admin finds nothing (no link, the
/// not-found page); the site admin gets Environments, and reaches only
/// their own environments, never someone else's or one made before
/// environments had owners. Posts still have to come from this site.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_a_public_host_environments_are_for_the_site_admin_and_their_own() {
    use crate::projects::tests::{Browser, Options, world_with};
    let state = tempfile::tempdir().unwrap();
    let studio = fake_studio(state.path());
    let world = world_with(Options {
        public: true,
        invite: Some(json!({"github": [
            {"id": 583231, "login": "octo-local", "admin": true},
            {"login": "quiet-local"}
        ]})),
        environments: Some(studio.clone()),
        ..Options::default()
    })
    .await;

    // Signed out: log in first, then come back.
    let mut visitor = Browser::default();
    for (path, back) in [
        ("/environments", "%2Fenvironments"),
        ("/environments/new", "%2Fenvironments%2Fnew"),
    ] {
        let page = visitor.get(&world, path).await;
        assert_eq!(page.status, StatusCode::SEE_OTHER, "{path}: {}", page.body);
        assert_eq!(page.location(), format!("/login?return_to={back}"));
    }
    let home = visitor.get(&world, "/").await;
    assert!(!home.body.contains("href=\"/environments\""));

    // Invited but not an admin: no link, and nothing at the address.
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let home = quiet.get(&world, "/").await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(
        !home.body.contains("href=\"/environments\""),
        "{}",
        home.body
    );
    for path in ["/environments", "/environments/new", "/chat/abc/claude"] {
        let page = quiet.get(&world, path).await;
        assert_eq!(page.status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.body.contains("Nothing on this site has that address."));
        assert!(!page.body.contains("local OpenAgents address"));
    }

    // The site admin: the link, the page, and only their own.
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    let theirs = studio
        .create_for(
            &resolved("quiet-local/theirs"),
            Some(&world.account_of("quiet-local")),
        )
        .unwrap();
    let older = studio.create(&resolved("old/unowned")).unwrap();
    let mine = studio
        .create_for(
            &resolved("octo-local/mine"),
            Some(&world.account_of("octo-local")),
        )
        .unwrap();
    let home = octo.get(&world, "/").await;
    assert!(
        home.body.contains("href=\"/environments\""),
        "{}",
        home.body
    );
    let list = octo.get(&world, "/environments").await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    assert!(list.body.contains("octo-local/mine"), "{}", list.body);
    assert!(!list.body.contains("quiet-local/theirs"));
    assert!(!list.body.contains("old/unowned"));
    assert!(!list.body.contains("local OpenAgents address"));
    let page = octo.get(&world, &format!("/environments/{mine}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(!page.body.contains("quiet-local/theirs"));
    for other in [&theirs, &older] {
        let page = octo.get(&world, &format!("/environments/{other}")).await;
        assert_eq!(page.status, StatusCode::NOT_FOUND, "{other}");
        let post = octo
            .send(
                &world,
                axum::http::Request::post(format!("/environments/{other}/message"))
                    .header("sec-fetch-site", "same-origin")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
                Body::from("q=hello"),
            )
            .await;
        assert_eq!(post.status, StatusCode::NOT_FOUND, "{other}");
        let stream = octo
            .get(&world, &format!("/environments/{other}/events"))
            .await;
        assert_eq!(stream.status, StatusCode::NOT_FOUND, "{other}");
    }
    // A post from another site is refused.
    let forged = octo
        .send(
            &world,
            axum::http::Request::post(format!("/environments/{mine}/message"))
                .header("sec-fetch-site", "cross-site")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded"),
            Body::from("q=hello"),
        )
        .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    crate::copy_guard::assert_plain("/environments", &list.body);
}
