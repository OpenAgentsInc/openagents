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

/// A development config whose release store is unreachable, so no test
/// touches the network.
fn config(store: PathBuf) -> Config {
    let mut config = Config::development(store);
    config.releases_url = "http://127.0.0.1:1".to_owned();
    config
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

async fn get(router: Router, uri: &str) -> (StatusCode, String) {
    let (status, _, body) = get_with(router, uri, LOCAL).await;
    (status, body)
}

/// Every public HTML page a development server serves.
const PAGES: [&str; 17] = [
    "/",
    "/ask?q=help",
    "/ask?q=download",
    "/ask?q=desktop",
    "/ask?q=what+is+openagents",
    "/terms",
    "/privacy",
    "/docs",
    "/docs/about",
    "/docs/install",
    "/docs/plugins",
    "/docs/changelog",
    "/docs/release-notes-0.5.0",
    "/blog",
    "/blog/introducing-coder",
    "/desktop",
    "/connect",
];

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
async fn the_homepage_offers_the_install_command_the_desktop_app_and_the_ask_box() {
    let root = tempfile::tempdir().unwrap();
    let (_, home) = get(router(config(root.path().into())), "/").await;
    assert!(home.contains("Welcome to OpenAgents."));
    assert!(home.contains("curl -fsSL https://openagents.com/releases/install-terminal.sh | sh"));
    assert!(home.contains("irm https://openagents.com/releases/install-terminal.ps1 | iex"));
    assert!(home.contains(pages::MAC_DMG));
    assert!(home.contains("action=\"/ask\""));
    let (_, answer) = get(router(config(root.path().into())), "/ask?q=%3Cb%3Ehi").await;
    assert!(answer.contains("&lt;b&gt;hi"), "{answer}");
    assert!(!answer.contains("<b>hi"), "{answer}");
    assert!(answer.contains("isn&#39;t connected") || answer.contains("isn't connected"));
    let (status, _) = get(router(config(root.path().into())), "/ask?q=clear").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn the_desktop_page_links_the_published_dmg_and_testflight() {
    let root = tempfile::tempdir().unwrap();
    let (_, body) = get(router(config(root.path().into())), "/desktop").await;
    assert!(body.contains("https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/0.1.0/OpenAgents-0.1.0.dmg"));
    assert!(body.contains(pages::TESTFLIGHT));
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
    for uri in [
        "/nope",
        "/docs/../../etc/passwd",
        "/docs/missing",
        "/blog/missing",
        "/u/-bad-",
    ] {
        let (status, body) = get(router(config(root.path().into())), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(body.contains("href=\"/terms\""), "{uri}");
    }
}

#[tokio::test]
async fn the_removed_sections_are_gone_and_never_linked() {
    let root = tempfile::tempdir().unwrap();
    let removed = [
        "/forum", "/gym", "/traces", "/trace/x", "/earn", "/weights", "/qa",
    ];
    for uri in removed {
        let (status, _) = get(router(config(root.path().into())), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (_, home) = get(router(config(root.path().into())), "/").await;
    for uri in removed {
        assert!(!home.contains(&format!("href=\"{uri}")), "{uri} is linked");
    }
}

#[tokio::test]
async fn the_install_redirects_point_under_releases() {
    let root = tempfile::tempdir().unwrap();
    let (status, headers, _) = get_with(
        router(config(root.path().into())),
        "/install-terminal.sh",
        LOCAL,
    )
    .await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/releases/install-terminal.sh");
    let (status, headers, _) = get_with(
        router(config(root.path().into())),
        "/install-terminal.ps1",
        LOCAL,
    )
    .await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/releases/install-terminal.ps1");
}

/// A stand-in for the public release bucket.
async fn a_bucket() -> String {
    use axum::http::HeaderMap;
    use axum::routing::get as route;
    async fn object(
        axum::extract::Path(name): axum::extract::Path<String>,
        headers: HeaderMap,
    ) -> axum::response::Response {
        match name.as_str() {
            "coder-terminal.stable" => "0.4.0\n".into_response(),
            "coder-terminal.rc" => "not a version".into_response(),
            "coder-terminal-0.4.0-linux-x86_64" => {
                let body: Vec<u8> = (0..256u32).map(|i| b'a' + (i % 26) as u8).collect();
                match headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
                    Some("bytes=0-9") => (
                        StatusCode::PARTIAL_CONTENT,
                        [(header::CONTENT_RANGE, "bytes 0-9/256")],
                        body[..10].to_vec(),
                    )
                        .into_response(),
                    _ => body.into_response(),
                }
            }
            _ => StatusCode::NOT_FOUND.into_response(),
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, Router::new().route("/{name}", route(object))).await;
    });
    format!("http://{address}")
}

#[tokio::test]
async fn releases_stream_from_the_bucket_with_ranges_and_the_install_page_reads_the_channels() {
    let root = tempfile::tempdir().unwrap();
    let mut with_bucket = config(root.path().into());
    with_bucket.releases_url = a_bucket().await;
    let (status, headers, body) = get_with(
        router(with_bucket.clone()),
        "/releases/coder-terminal-0.4.0-linux-x86_64",
        LOCAL,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    assert_eq!(body.len(), 256);
    let response = router(with_bucket.clone())
        .oneshot(
            Request::builder()
                .uri("/releases/coder-terminal-0.4.0-linux-x86_64")
                .header(header::HOST, LOCAL)
                .header(header::RANGE, "bytes=0-9")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 0-9/256");
    let (status, _, _) = get_with(router(with_bucket.clone()), "/releases/missing", LOCAL).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get_with(router(with_bucket.clone()), "/releases/..hidden", LOCAL).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, install) = get_with(router(with_bucket), "/docs/install", LOCAL).await;
    assert!(install.contains("<code>0.4.0</code>"), "{install}");
    assert!(
        install.contains("<code>unknown</code>"),
        "an invalid pointer reads unknown"
    );
    let (status, _, _) = get_with(
        router(config(root.path().into())),
        "/releases/coder-terminal.stable",
        LOCAL,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "an unreachable bucket");
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
    fn answer<'a>(&'a self, question: &'a str) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move { Some(format!("You asked **{question}**.")) })
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
    let (_, answer) = get(router(connected(dir)), "/ask?q=why").await;
    assert!(answer.contains("<strong>why</strong>"), "{answer}");
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
