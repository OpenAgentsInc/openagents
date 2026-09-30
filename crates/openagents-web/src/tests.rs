use std::path::PathBuf;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use futures_util::future::BoxFuture;
use serde_json::json;
use tower::ServiceExt;

use super::*;
use crate::backend::Profile;

const LOCAL: &str = "127.0.0.1:4300";

/// A development config.
fn config(store: PathBuf) -> Config {
    Config::development(store)
}

async fn get_with(
    router: Router,
    uri: &str,
    host: &str,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::HOST, host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

async fn get_bytes(router: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::HOST, LOCAL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, body.to_vec())
}

async fn get(router: Router, uri: &str) -> (StatusCode, String) {
    let (status, _, body) = get_with(router, uri, LOCAL).await;
    (status, body)
}

/// Every public HTML page a development server serves.
const PAGES: [&str; 14] = [
    "/",
    "/install",
    "/terms",
    "/privacy",
    "/connect",
    "/docs",
    "/docs/what-is-openagents",
    "/docs/install",
    "/docs/connect-a-computer",
    "/docs/chat",
    "/docs/coder",
    "/docs/verse",
    "/docs/privacy-and-security",
    "/docs/help",
];

/// The docs list every guide, each guide links its neighbors, and every
/// site link in a guide answers `200`.
#[tokio::test]
async fn the_docs_list_every_guide_and_their_links_resolve() {
    let root = tempfile::tempdir().unwrap();
    let (_, index) = get(router(config(root.path().into())), "/docs").await;
    for (slug, _) in pages::DOCS {
        assert!(index.contains(&format!("href=\"/docs/{slug}\"")), "{slug}");
        let (status, html) =
            get(router(config(root.path().into())), &format!("/docs/{slug}")).await;
        assert_eq!(status, StatusCode::OK, "{slug}");
        for target in html.split("href=\"").skip(1) {
            let target = &target[..target.find('"').unwrap()];
            if target.starts_with('/') && !target.starts_with("/static/") {
                let path = target.split('#').next().unwrap();
                let (status, _) = get(router(config(root.path().join("tasks"))), path).await;
                assert_eq!(status, StatusCode::OK, "{slug} links {target}");
            }
        }
    }
    let (status, _) = get(router(config(root.path().into())), "/docs/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn every_public_page_answers_in_development() {
    let root = tempfile::tempdir().unwrap();
    for uri in PAGES.iter().copied().chain(["/u/AtlantisPleb", "/app"]) {
        let router = router(config(root.path().join("tasks")));
        let (status, headers, body) = get_with(router, uri, LOCAL).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        let lower = body.to_ascii_lowercase();
        assert!(lower.starts_with("<!doctype html>"), "{uri}");
        assert!(!lower.contains("<script"), "{uri} runs a script");
        assert!(body.contains("href=\"/terms\""), "{uri} links the terms");
        assert!(body.contains("href=\"/privacy\""), "{uri} links the policy");
        assert!(body.contains("class=\"wordmark\""), "{uri} has the header");
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(policy.starts_with("default-src 'none'"), "{uri}: {policy}");
        assert!(!policy.contains("script-src"), "{uri}: {policy}");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
}

#[tokio::test]
async fn the_legal_pages_carry_the_published_text() {
    let root = tempfile::tempdir().unwrap();
    let (status, terms) = get(router(config(root.path().into())), "/terms").await;
    assert_eq!(status, StatusCode::OK);
    assert!(terms.contains("<h1>Terms of Service</h1>"), "{terms}");
    assert!(terms.contains("Last updated: 2026-09-03"));
    assert!(terms.contains("OpenAgents, Inc. (“OpenAgents,” “we,” “us,” or “our”)"));
    assert!(terms.contains("governed by the laws of the State of Texas"));
    let (status, privacy) = get(router(config(root.path().into())), "/privacy").await;
    assert_eq!(status, StatusCode::OK);
    assert!(privacy.contains("<h1>Privacy Policy</h1>"), "{privacy}");
    assert!(privacy.contains("You may ask us not to use the content you submit"));
    assert!(privacy.contains("mailto:chris@openagents.com"));
}

#[tokio::test]
async fn the_homepage_links_one_install_page_and_shows_the_verse() {
    let root = tempfile::tempdir().unwrap();
    let (_, home) = get(router(config(root.path().into())), "/").await;
    assert!(home.contains("<a class=\"button\" href=\"/install\">[ Install OpenAgents ]</a>"));
    assert!(
        !home.contains(pages::MAC_DMG),
        "the download lives on /install"
    );
    assert!(!home.contains("curl ") && !home.contains("irm "));
    assert!(!home.contains("class=\"terminal\"") && !home.contains("<form"));
    assert!(home.contains("<img src=\"/static/verse-grid.jpg\""));
    assert!(home.contains("alt=\"The Grid, the OpenAgents Verse world"));
    let (status, headers, image) =
        get_bytes(router(config(root.path().into())), "/static/verse-grid.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(image.starts_with(&[0xff, 0xd8, 0xff]), "a JPEG");
    let (status, _) = get(router(config(root.path().into())), "/ask?q=help").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the ask box is gone");
}

#[tokio::test]
async fn the_install_page_covers_the_mac_the_iphone_and_pairing() {
    let root = tempfile::tempdir().unwrap();
    let (status, body) = get(router(config(root.path().into())), "/install").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg"));
    assert!(body.contains("macOS 13 or later"));
    assert!(body.contains("<strong>Applications</strong>"));
    assert!(body.contains(pages::TESTFLIGHT));
    assert!(body.contains("<strong>Connect a computer</strong>"));
    assert!(body.contains("iPhone Camera"));
    assert!(body.contains("Codex or Claude Code"));
    assert!(body.contains("Android") && body.contains("Linux") && body.contains("Windows"));
    assert!(body.contains("<a href=\"/install\" aria-current=\"page\">Install</a>"));
    let (status, headers, _) =
        get_with(router(config(root.path().into())), "/desktop", LOCAL).await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/install");
}

#[tokio::test]
async fn the_app_association_files_answer_through_the_router() {
    let root = tempfile::tempdir().unwrap();
    for uri in [
        "/.well-known/apple-app-site-association",
        "/.well-known/assetlinks.json",
    ] {
        let (status, headers, body) =
            get_with(router(config(root.path().into())), uri, LOCAL).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json", "{uri}");
        serde_json::from_str::<serde_json::Value>(&body).unwrap();
    }
    let (_, headers, _) = get_with(router(config(root.path().into())), "/connect", LOCAL).await;
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(
        policy.contains("form-action 'none'"),
        "the page keeps its stricter policy"
    );
    assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
}

/// Every color on the site is a gray from the white ladder: the
/// stylesheet and the icon hold every color, and no page styles itself.
#[tokio::test]
async fn no_amber_and_no_hue_anywhere() {
    let root = tempfile::tempdir().unwrap();
    let (status, css) = get(router(config(root.path().into())), "/static/site.css").await;
    assert_eq!(status, StatusCode::OK);
    let (_, favicon) = get(router(config(root.path().into())), "/favicon.svg").await;
    for source in [&css, &favicon] {
        let lower = source.to_ascii_lowercase();
        for word in ["amber", "orange", "gold", "yellow", "rgb(", "hsl("] {
            assert!(!lower.contains(word), "{word}");
        }
        let mut colors = 0;
        for (at, _) in lower.match_indices('#') {
            let hex: String = lower[at + 1..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();
            let rgb = match hex.len() {
                6 => u32::from_str_radix(&hex, 16).ok(),
                3 => u32::from_str_radix(&hex.chars().flat_map(|c| [c, c]).collect::<String>(), 16)
                    .ok(),
                _ => None,
            };
            if let Some(rgb) = rgb {
                colors += 1;
                assert!(palette::is_gray(rgb), "#{hex} is not a gray");
            }
        }
        assert!(colors > 0);
    }
    assert!(css.contains("--w100:#ffffff"));
    assert!(css.contains("--w25:#4a4a4a"));
    for uri in PAGES {
        let (_, page) = get(router(config(root.path().join("tasks"))), uri).await;
        let lower = page.to_ascii_lowercase();
        assert!(
            !lower.contains("<style") && !lower.contains("<link rel=\"stylesheet\" href=\"http"),
            "{uri}"
        );
        for tag in lower.split('<').skip(1) {
            let tag = tag.split('>').next().unwrap_or_default();
            assert!(!tag.contains(" style="), "{uri} styles an element: {tag}");
        }
    }
}

#[tokio::test]
async fn hosts_other_than_the_local_ones_are_refused_and_the_browser_stays_local() {
    let root = tempfile::tempdir().unwrap();
    let (status, _, _) = get_with(
        router(config(root.path().into())),
        "/",
        "attacker.example:4300",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let mut public = config(root.path().join("tasks"));
    public.public_hosts.push("openagents.com".to_owned());
    let (status, _, _) = get_with(router(public.clone()), "/terms", "openagents.com").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = get_with(router(public), "/app", "openagents.com").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the task browser answers only locally"
    );
}

#[tokio::test]
async fn unknown_addresses_and_documents_answer_404_in_the_frame() {
    let root = tempfile::tempdir().unwrap();
    for uri in ["/nope", "/terms/../../etc/passwd", "/u/-bad-"] {
        let (status, body) = get(router(config(root.path().into())), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(body.contains("href=\"/terms\""), "{uri}");
    }
}

#[tokio::test]
async fn the_removed_sections_are_gone_and_never_linked() {
    let root = tempfile::tempdir().unwrap();
    let removed = [
        "/forum",
        "/gym",
        "/traces",
        "/trace/x",
        "/earn",
        "/weights",
        "/qa",
        "/releases/x",
        "/releases/install-terminal.sh",
        "/install-terminal.sh",
        "/install-terminal.ps1",
        "/doc",
        "/doc/install",
        "/blog",
        "/blog/introducing-coder",
    ];
    for uri in removed {
        let (status, _) = get(router(config(root.path().into())), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    for page in PAGES {
        let (_, html) = get(router(config(root.path().join("tasks"))), page).await;
        for uri in removed {
            assert!(
                !html.contains(&format!("href=\"{uri}\""))
                    && !html.contains(&format!("href=\"{uri}/")),
                "{page} links {uri}"
            );
        }
    }
}

/// The old Coder Terminal product is not connected to OpenAgents, so no
/// page names it or its install command. The terms and the privacy policy
/// are the published legal text, unchanged, and name every product they
/// cover, so they may name it; neither links its install command.
#[tokio::test]
async fn no_page_mentions_coder_terminal_or_its_install_command() {
    let root = tempfile::tempdir().unwrap();
    for uri in PAGES.iter().copied().chain(["/u/AtlantisPleb", "/nope"]) {
        let (_, html) = get(router(config(root.path().join("tasks"))), uri).await;
        if !matches!(uri, "/terms" | "/privacy") {
            assert!(!html.contains("Coder Terminal"), "{uri}");
        }
        assert!(!html.contains("install-terminal"), "{uri}");
    }
}

// ---------------------------------------------------------------------
// A connected backend, for the pages' production shapes. Test-only rows.

struct Connected;

impl backend::Backend for Connected {
    fn connected(&self) -> bool {
        true
    }
    fn new_account_credit_cents(&self) -> Option<u64> {
        Some(2500)
    }
    fn profile<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Option<Profile>> {
        Box::pin(async move {
            (login == "tester").then(|| Profile {
                login: "tester".to_owned(),
                name: Some("Test Person".to_owned()),
                joined: "September 2026".to_owned(),
            })
        })
    }
}

fn connected(root: &std::path::Path) -> Config {
    let mut config = config(root.into());
    config.backend = Arc::new(Connected);
    config
}

#[tokio::test]
async fn a_connected_backend_fills_the_pages_and_escapes_what_it_returns() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();
    let (_, home) = get(router(connected(dir)), "/").await;
    assert!(home.contains("Every new account starts with $25 of credit."));
    let (_, profile) = get(router(connected(dir)), "/u/tester").await;
    assert!(profile.contains("Test Person") && profile.contains("https://github.com/tester"));
    for uri in ["/u/nobody"] {
        let (status, _) = get(router(connected(dir)), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

// ---------------------------------------------------------------------
// The local task browser.

#[tokio::test]
async fn missing_store_does_not_create_data() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("uninitialized");
    let (status, body) = get(router(config(store.clone())), "/app").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("No tasks yet"));
    assert!(!store.exists());
    let (status, _) = get(router(config(store.clone())), "/app/tasks/not-found").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!store.exists());
}

#[tokio::test]
async fn task_view_escapes_private_content_and_rejects_bad_cursor() {
    use coder::task::{self, Store};
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("tasks");
    let command = json!({
        "schema": task::COMMAND_SCHEMA,
        "command_id": "example-submit-1",
        "task_id": "example-task-1",
        "expected_revision": null,
        "action": {"type":"submit", "intent": {
            "title":"<script>alert(1)</script>", "prompt":"<img src=x onerror=alert(1)>",
            "workspace":{"path":"/workspace/example", "source_revision":null},
            "configuration":{"adapter":"coder", "model":null}
        }}
    });
    Store::open(&store)
        .unwrap()
        .apply(command.to_string().as_bytes())
        .unwrap();
    let (status, body) = get(router(config(store.clone())), "/app").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!body.contains("<script>"));
    let (status, body) = get(router(config(store.clone())), "/app/tasks/example-task-1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(
        body.contains("not_started") || body.contains("NotStarted"),
        "{body}"
    );
    let (status, _) = get(
        router(config(store)),
        "/app/tasks/example-task-1?cursor=bogus",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
