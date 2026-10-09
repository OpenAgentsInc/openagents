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
const PAGES: [&str; 42] = [
    "/",
    "/live",
    "/everglade",
    "/druid",
    "/grid",
    "/stats",
    "/efficiency",
    "/download",
    "/terms",
    "/privacy",
    "/connect",
    "/docs",
    "/docs/what-is-openagents",
    "/docs/download",
    "/docs/website",
    "/docs/mac",
    "/docs/iphone",
    "/docs/terminal",
    "/docs/cli",
    "/docs/chat",
    "/docs/privacy-and-security",
    "/docs/coder",
    "/docs/coding-agents",
    "/docs/following-coder",
    "/docs/worktrees-and-changes",
    "/docs/github-issues",
    "/docs/ship-from-your-phone",
    "/docs/connect-a-computer",
    "/docs/manage-computers",
    "/docs/plugins",
    "/docs/write-a-plugin",
    "/docs/test-a-plugin",
    "/docs/publish-and-share",
    "/docs/gym-and-xp",
    "/docs/verse",
    "/docs/the-grid",
    "/docs/wallet",
    "/docs/decks",
    "/docs/settings",
    "/docs/troubleshooting",
    "/docs/faq",
    "/docs/glossary",
];

/// The docs list every guide, each guide links its neighbors, and every
/// site link in a guide (its `<main>`, not the shared navigation) answers
/// `200`.
#[tokio::test]
async fn the_docs_list_every_guide_and_their_links_resolve() {
    let root = tempfile::tempdir().unwrap();
    let (_, index) = get(router(config(root.path().into())), "/docs").await;
    for (slug, _) in pages::DOCS {
        assert!(index.contains(&format!("href=\"/docs/{slug}\"")), "{slug}");
        let (status, html) =
            get(router(config(root.path().into())), &format!("/docs/{slug}")).await;
        assert_eq!(status, StatusCode::OK, "{slug}");
        let main = &html[html.find("<main").unwrap()..html.find("</main>").unwrap()];
        for target in main.split("href=\"").skip(1) {
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
        // The composer loads HTMX, its SSE extension, and the Rust adapter,
        // after the design-language shell's component script and Alpine
        // (UI-09: the homepage renders through `UiPage`). The live map loads
        // its own single script.
        let script = uri == "/" || uri == "/live";
        assert_eq!(
            lower.matches("<script").count(),
            if uri == "/" { 5 } else { usize::from(script) },
            "{uri} runs a script"
        );
        if uri == "/" {
            assert!(
                body.contains("src=\"/static/ui.js?v="),
                "the component script"
            );
            assert!(
                body.contains("src=\"/static/vendor/alpine-csp.js\""),
                "Alpine"
            );
            for asset in ["htmx.min.js", "htmx-sse.js", "chat-start.js"] {
                assert!(
                    body.contains(&format!("src=\"/static/{asset}\"")),
                    "{asset}"
                );
            }
            assert!(body.contains("&quot;allowEval&quot;:false"));
            assert!(!body.contains("src=\"/static/chat.js\""));
        }
        assert!(body.contains("href=\"/terms\""), "{uri} links the terms");
        assert!(body.contains("href=\"/privacy\""), "{uri} links the policy");
        // The UiPage shell's wordmark (UI-12; the legacy header is gone, UI-13).
        assert!(
            body.contains("class=\"oa-wordmark\""),
            "{uri} has the header"
        );
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(policy.starts_with("default-src 'none'"), "{uri}: {policy}");
        if script {
            assert!(policy.contains("script-src 'self'"), "{uri}: {policy}");
            assert!(!policy.contains("'unsafe-inline'"), "{uri}: {policy}");
            assert!(!policy.contains("'unsafe-eval'"), "{uri}: {policy}");
            if uri == "/" {
                assert!(policy.contains("'wasm-unsafe-eval'"), "{uri}: {policy}");
            }
        } else {
            assert!(!policy.contains("script-src"), "{uri}: {policy}");
        }
        if uri == "/live" {
            assert!(policy.contains("connect-src 'self'"), "{uri}: {policy}");
        }
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }
}

#[tokio::test]
async fn the_public_cloud_page_redirects_home_and_browser_work_stays_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("unopened-tasks");
    let mut settings = config(store.clone());
    settings.public_hosts.push("openagents.com".into());
    let site = router(settings);
    let (status, headers, html) = get_with(site.clone(), "/cloud", "openagents.com").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/");
    assert!(!headers.contains_key(header::SET_COOKIE));
    assert!(html.is_empty(), "{html}");
    assert!(
        !store.exists(),
        "the Cloud redirect never opens the local task store"
    );
    assert_eq!(
        get_with(site, "/app", "openagents.com").await.0,
        StatusCode::FORBIDDEN,
    );
}

/// `/ui` serves the openagents-ui component catalog in the page shell,
/// with scripts from this site only, and is never proxied.
#[tokio::test]
async fn the_component_catalog_is_served_at_ui() {
    let root = tempfile::tempdir().unwrap();
    let (status, headers, html) = get_with(router(config(root.path().into())), "/ui", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("class=\"oa-catalog\""), "catalog body");
    assert!(html.contains("data-theme=\"light\""), "light pane");
    assert!(html.contains("data-theme=\"dark\""), "dark pane");
    assert!(
        html.contains("data-catalog-component=\"Button\""),
        "specimens"
    );
    assert!(
        html.contains("<title>Components \u{b7} OpenAgents</title>"),
        "title"
    );
    assert!(html.contains(crate::theme::STYLESHEET_PATH), "stylesheet");
    assert!(html.contains(crate::theme::SCRIPT_PATH), "component script");
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("script-src 'self'"), "{policy}");
    assert!(policy.contains("style-src 'self'"), "{policy}");
    assert!(!policy.contains("'unsafe-inline'"), "{policy}");
    assert!(!policy.contains("'unsafe-eval'"), "{policy}");
    assert!(upstream::owned("/ui"));
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
async fn the_homepage_links_one_download_page_and_starts_a_chat() {
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().into()));
    let (_, home) = get(site.clone(), "/").await;
    // Download is a pill link in the header's top-right actions.
    let download = home
        .find("href=\"/download\"")
        .expect("the header links /download");
    assert_eq!(home.matches("href=\"/download\"").count(), 1);
    assert!(home.find("class=\"oa-main-header-actions\"").unwrap() < download);
    assert!(home[home[..download].rfind('<').unwrap()..download].contains("class=\"oa-button\""));
    assert!(download < home.find("data-oa-theme-toggle").unwrap());
    // No Chat or Cloud entries; "New chat" heads the left panel.
    assert!(!home.contains("href=\"/chat\"") && !home.contains("href=\"/cloud/app\""));
    assert!(home.contains(">New chat</span>"));
    assert!(!home.contains("[ Download OpenAgents ]") && !home.contains("<h1>OpenAgents</h1>"));
    assert!(!home.contains("/install"), "every link says /download");
    assert!(!home.contains(".dmg"), "downloads live on /download");
    assert!(!home.contains("curl ") && !home.contains("irm "));
    assert!(home.contains("id=\"chat-input\""));
    assert!(home.contains("action=\"/chat\""));
    assert!(home.contains("<script type=\"module\" src=\"/static/chat-start.js\"></script>"));
    assert!(home.contains("<script src=\"/static/htmx.min.js\" defer></script>"));
    assert!(home.contains("<script src=\"/static/htmx-sse.js\" defer></script>"));
    assert!(!home.contains("term-input") && !home.contains("/static/ask.js"));
    assert!(!home.contains("href=\"/pilot\""));
    assert!(!home.contains("<img"));
    let (status, grid) = get(site.clone(), "/docs/the-grid").await;
    assert_eq!(status, StatusCode::OK);
    assert!(grid.contains("<h1>The Grid</h1>"), "{grid}");
    assert!(grid.contains(
        "<img src=\"/static/verse-grid.jpg\" alt=\"The Grid, the OpenAgents Verse world"
    ));
    let (status, headers, image) = get_bytes(site.clone(), "/static/verse-grid.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(image.starts_with(&[0xff, 0xd8, 0xff]), "a JPEG");
    let (status, headers, script) = get_with(site, "/static/chat-start.js", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(script.contains("import init, { start } from '/chat/assets/coder_chat_web.js'"));
    assert!(script.contains("await init()") && script.contains("start()"));
    assert!(!script.contains("innerHTML") && !script.contains("fetch("));
}

#[tokio::test]
async fn the_homepage_composer_is_the_design_language_component() {
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().into()));
    let (_, home) = get(site.clone(), "/").await;
    // UI-09: the homepage renders through `UiPage`, styled by `/static/ui.css`
    // alone; the legacy site, Tailwind and chat stylesheets are not loaded.
    assert!(home.contains("<link rel=\"stylesheet\" href=\"/static/ui.css?v="));
    for legacy in [
        "site.css",
        "legacy-demo.css",
        "tailwind.css",
        "chat-html.css",
        "composer.css",
        "tw:",
    ] {
        assert!(!home.contains(legacy), "{legacy}");
    }
    assert!(home.contains(
        "<div class=\"oa-home-stage\"><section class=\"oa-composer\" aria-label=\"Start a chat\">"
    ));
    assert!(home.contains(
        "<form id=\"chat-form\" class=\"oa-composer-root\" action=\"/chat\" method=\"post\""
    ));
    // The homepage posts a plain form and follows the redirect to the chat.
    assert!(!home.contains("hx-post="));
    assert!(home.contains("id=\"chat-card\" class=\"oa-composer-body\""));
    assert!(home.contains("placeholder=\"Ask OpenAgents to build, fix bugs, explore\""));
    assert!(home.contains("maxlength=\"4000\""));
    assert!(
        home.contains("<button type=\"submit\" class=\"oa-composer-send\" aria-label=\"Send\"")
    );
    assert!(home.contains("<div id=\"composer-panel\" class=\"oa-composer-panel-host\"></div>"));
    // Controls that do nothing for this visitor are commented out: the
    // context, model and voice buttons only opened placeholder panels, and
    // the source selectors need an admitted Cloud runtime.
    for kind in [
        "repository",
        "branch",
        "environment",
        "context",
        "model",
        "voice",
    ] {
        assert!(
            !home.contains(&format!("hx-get=\"/composer/{kind}\"")),
            "{kind}"
        );
    }
    assert!(!home.contains("Voice input") && !home.contains("Model: Auto"));
    // The legal links are quiet text at the bottom of the left panel.
    assert!(!home.contains("<footer") && !home.contains("class=\"oa-legal\""));
    assert!(home.contains("class=\"oa-sidebar-legal\""));
    let (status, headers, css) = get_with(site, "/static/ui.css", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/css; charset=utf-8");
    for rule in [
        ".oa-home-stage",
        ".oa-composer-body",
        ".oa-composer-panel",
        ".oa-message",
    ] {
        assert!(css.contains(rule), "{rule}");
    }
}

#[tokio::test]
async fn posting_the_homepage_composer_opens_a_chat_page() {
    struct OfflineChat;
    impl ask::Chat for OfflineChat {
        fn door(
            &self,
            _: secp256k1::SecretKey,
        ) -> Result<Box<dyn openagents_chat::basic_coder::Door>, String> {
            Err("The synthetic test chat has no network connection.".into())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut configured = config(root.path().join("tasks"));
    configured.chat = Arc::new(OfflineChat);
    let site = router(configured);
    let (_, home_headers, home) = get_with(site.clone(), "/", LOCAL).await;
    let visitor = home_headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let field = |name: &str| {
        home.split_once(&format!("name=\"{name}\""))
            .unwrap()
            .1
            .split('>')
            .next()
            .unwrap()
            .split_once("value=\"")
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap()
    };
    let input = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("q", "Set up OpenAgents"),
            ("request_id", field("request_id")),
            ("csrf", field("csrf")),
            ("selection", field("selection")),
        ])
        .finish();
    let response = site
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/chat")
                .header(header::HOST, LOCAL)
                .header(header::COOKIE, visitor)
                .header(header::ORIGIN, format!("http://{LOCAL}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(input))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert!(
        location.starts_with("/chat/") && location.len() == 42,
        "{location}"
    );
    let chat = site
        .clone()
        .oneshot(
            Request::builder()
                .uri(location)
                .header(header::HOST, LOCAL)
                .header(header::COOKIE, visitor)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chat.status(), StatusCode::OK);
    let html =
        String::from_utf8(to_bytes(chat.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(html.contains("Set up OpenAgents"));
    assert!(html.contains(&format!("action=\"{location}\"")));
    // UI-09: an app-mode `UiPage`: the shell header, no legal footer, and the
    // composer docked under the scrolling thread.
    assert!(html.contains("<header class=\"oa-main-header\">"));
    assert!(html.contains("class=\"oa-layout\" data-mode=\"app\""));
    assert!(!html.contains("<footer"), "the chat page has no footer");
    assert!(html.contains("class=\"oa-sidebar-legal\""));
    assert_eq!(html.matches("href=\"/terms\"").count(), 1);
    assert!(html.contains("<main id=\"content\" class=\"oa-workspace\""));
    // The open chat is the current row of the recent-chat list, and it
    // loads into the content area with HTMX.
    let row = html
        .find(&format!("<a class=\"oa-nav-item\" href=\"{location}\""))
        .expect("the chat lists itself");
    let row = &html[row..row + html[row..].find('>').unwrap()];
    assert!(
        row.contains(&format!("hx-get=\"{location}/workspace\"")),
        "{row}"
    );
    assert!(row.contains("aria-current=\"page\""), "{row}");
    assert!(!html.contains("Showing up to 256") && !html.contains("Onboarding demo"));
    let thread = html.find("id=\"chat-thread\"").unwrap();
    let dock = html.find("class=\"oa-main-composer\"").unwrap();
    let card = html.find("id=\"chat-card\"").unwrap();
    assert!(
        thread < dock && dock < card,
        "the composer docks under the thread"
    );
    assert!(
        html.contains("hx-post=\"/chat/"),
        "the chat posts with HTMX"
    );
    assert!(
        html.contains("id=\"chat-sidebar\""),
        "the conversation list"
    );
    assert!(html.contains("id=\"chat-feedback\""));
    let (_, home) = get(site.clone(), "/").await;
    assert!(home.contains("class=\"oa-sidebar-legal\""));
    assert!(home.contains("href=\"/terms\"") && home.contains("href=\"/privacy\""));
    // The homepage lists the same visitor's chats as plain links.
    let home = site
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, LOCAL)
                .header(header::COOKIE, visitor)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home =
        String::from_utf8(to_bytes(home.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(home.contains("id=\"chat-sidebar\""));
    assert!(home.contains(&format!("<a class=\"oa-nav-item\" href=\"{location}\">")));
    assert!(
        !home.contains("/workspace\""),
        "no content area to load into"
    );
    let (status, missing) = get(site, "/chat/00000000-0000-4000-8000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(missing.contains("<h1>Not found</h1>"));
}

#[tokio::test]
async fn the_pilot_pages_answer_not_found() {
    let root = tempfile::tempdir().unwrap();
    for uri in ["/pilot", "/pilot/install"] {
        let (status, html) = get(router(config(root.path().into())), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(html.contains("<h1>Not found</h1>"), "{uri}: {html}");
    }
}

/// `/live` (#10197): the map's canvas, the totals, its one script reading
/// the flow endpoints on this origin, and the event mapping's own tests
/// (`static/flow.test.js`, run with node when it is installed).
#[tokio::test]
async fn the_live_page_draws_the_flow_stream_on_the_route_map() {
    let root = tempfile::tempdir().unwrap();
    let (status, headers, live) =
        get_with(router(config(root.path().into())), "/live", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(live.contains("<canvas id=\"flow-map\""));
    assert!(live.contains("data-snapshot=\"/api/flow/snapshot\""));
    assert!(live.contains("data-stream=\"/api/flow/stream\""));
    assert!(live.contains("id=\"flow-totals\""));
    assert!(live.contains("<script src=\"/static/flow.js\" defer></script>"));
    assert!(live.contains("Nothing here is simulated."));
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("script-src 'self'") && policy.contains("connect-src 'self'"));
    let (status, headers, script) =
        get_with(router(config(root.path().into())), "/static/flow.js", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    // Same-origin reads, the browser's own reconnect, and no made-up
    // traffic: no timer or random source feeds the schedule.
    assert!(script.contains("new EventSource("));
    assert!(script.contains("getAttribute(\"data-stream\")"));
    assert!(!script.contains("Math.random"));
    assert!(!script.contains("http://") && !script.contains("https://"));
    assert!(
        !script.contains("innerHTML"),
        "event text goes in as text only"
    );
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    match std::process::Command::new("node")
        .arg("--test")
        .arg(crate_dir.join("static/flow.test.js"))
        .output()
    {
        Ok(out) => assert!(
            out.status.success(),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(_) => eprintln!("node is not installed; static/flow.test.js did not run"),
    }
}

/// A server started with `--everglade DIR` holding a stand-in build and
/// pack, and the pack's file name.
fn with_everglade(root: &std::path::Path) -> (Config, String) {
    let build = root.join("everglade");
    std::fs::create_dir_all(build.join("pack")).unwrap();
    std::fs::write(
        build.join(pages::GLUE),
        "export default async function init() {}",
    )
    .unwrap();
    std::fs::write(build.join(pages::WASM), b"\0asm\x01\0\0\0").unwrap();
    std::fs::write(build.join("snippets.js"), "export {};").unwrap();
    std::fs::write(build.join("notes.txt"), "not served").unwrap();
    let pack = format!("{}.vtp", "ab".repeat(32));
    std::fs::write(build.join("pack").join(&pack), b"VTP pack bytes").unwrap();
    std::fs::create_dir_all(build.join("kit")).unwrap();
    std::fs::write(build.join("kit").join(&pack), b"VTP kit bytes").unwrap();
    std::fs::write(root.join("secret.js"), "outside the build").unwrap();
    let mut config = config(root.join("tasks"));
    config.everglade = Some(build);
    (config, pack)
}

/// `/everglade` (#10525): the canvas, the one same-origin loader, the glue
/// and the wasm from the build directory with their types, the
/// digest-named pack with an immutable cache, and nothing else from disk.
#[tokio::test]
async fn the_everglade_page_serves_the_web_build_and_its_pack() {
    let root = tempfile::tempdir().unwrap();
    let (config, pack) = with_everglade(root.path());
    let (status, headers, html) = get_with(router(config.clone()), "/everglade", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(&format!("<canvas id=\"{}\"", pages::CANVAS_ID)));
    assert!(html.contains(&format!("data-module=\"/everglade/{}\"", pages::GLUE)));
    assert!(html.contains(&format!("data-wasm=\"/everglade/{}\"", pages::WASM)));
    assert!(html.contains("data-pack=\"/everglade/pack/\""));
    // The canvas fills the window: no site header or footer, no page zoom.
    assert!(html.contains("<html lang=\"en\" class=\"stage\">"));
    assert!(html.contains("user-scalable=no"));
    assert!(!html.contains("site-header"));
    assert!(!html.contains("site-footer"));
    assert!(html.contains("id=\"everglade-status\""));
    let lower = html.to_ascii_lowercase();
    assert_eq!(lower.matches("<script").count(), 1, "one script");
    assert!(html.contains("<script type=\"module\" src=\"/static/everglade.js\"></script>"));
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        pages::EVERGLADE_POLICY
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");

    let (status, headers, script) =
        get_with(router(config.clone()), "/static/everglade.js", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(script.contains("import(glue)"));
    // The loader downloads the module itself, with progress, and hands the
    // bytes to the glue's init.
    assert!(script.contains("fetch(wasm"));
    assert!(script.contains("default({ module_or_path: loaded[1] })"));
    assert!(script.contains("data-wasm-bytes"));
    assert!(!script.contains("http://") && !script.contains("https://"));
    assert!(!script.contains("innerHTML"));

    for (uri, content_type, cache) in [
        (
            format!("/everglade/{}", pages::GLUE),
            "text/javascript; charset=utf-8",
            "public, max-age=300",
        ),
        (
            "/everglade/snippets.js".to_owned(),
            "text/javascript; charset=utf-8",
            "public, max-age=300",
        ),
        (
            format!("/everglade/{}", pages::WASM),
            "application/wasm",
            "public, max-age=300",
        ),
        (
            format!("/everglade/pack/{pack}"),
            "application/octet-stream",
            "public, max-age=31536000, immutable",
        ),
    ] {
        let (status, headers, _) = get_bytes(router(config.clone()), &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], content_type, "{uri}");
        assert_eq!(headers[header::CACHE_CONTROL], cache, "{uri}");
    }
    let (_, _, wasm) = get_bytes(
        router(config.clone()),
        &format!("/everglade/{}", pages::WASM),
    )
    .await;
    assert!(wasm.starts_with(b"\0asm"));
    let (_, _, bytes) = get_bytes(router(config.clone()), &format!("/everglade/pack/{pack}")).await;
    assert_eq!(bytes, b"VTP pack bytes");
    let (_, _, bytes) = get_bytes(router(config.clone()), &format!("/everglade/kit/{pack}")).await;
    assert_eq!(bytes, b"VTP kit bytes");

    // Other files, other names, and every way out of the directory are 404.
    let other_pack = format!("/everglade/pack/{}.vtp", "cd".repeat(32));
    let upper_pack = format!("/everglade/pack/{}", pack.to_uppercase());
    for uri in [
        "/everglade/notes.txt",
        "/everglade/missing.js",
        "/everglade/..%2Fsecret.js",
        "/everglade/%2E%2E%2Fsecret.js",
        "/everglade/../secret.js",
        "/everglade/pack/notes.txt",
        "/everglade/pack/..%2F..%2Fsecret.js",
        "/everglade/pack/a/b.vtp",
        "/everglade/kit/notes.txt",
        "/everglade/kit/..%2F..%2Fsecret.js",
        other_pack.as_str(),
        upper_pack.as_str(),
    ] {
        let (status, _, _) = get_bytes(router(config.clone()), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn everglade_bake_layers_are_digest_named_immutable_downloads() {
    let root = tempfile::tempdir().unwrap();
    let (config, _) = with_everglade(root.path());
    let directory = config.everglade.as_ref().unwrap().join("kit/bake");
    std::fs::create_dir_all(&directory).unwrap();
    let digest = "ab".repeat(32);
    let name = format!("{digest}.vlay");
    std::fs::write(directory.join(&name), b"synthetic light layer fixture").unwrap();
    std::fs::write(directory.join("notes.txt"), b"not a layer").unwrap();
    let (status, headers, bytes) = get_bytes(
        router(config.clone()),
        &format!("/everglade/kit/bake/{name}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"synthetic light layer fixture");
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    for file in [
        "notes.txt".to_owned(),
        format!("{digest}.vtp"),
        format!("{}.vlay", digest.to_uppercase()),
        format!("{}.vlay", &digest[1..]),
        format!("{}.vlay", "cd".repeat(32)),
        "..%2Fnotes.txt".to_owned(),
        "%2E%2E%2F..%2Fsecret.js".to_owned(),
    ] {
        let (status, _, _) = get_bytes(
            router(config.clone()),
            &format!("/everglade/kit/bake/{file}"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{file}");
    }
}

#[tokio::test]
async fn everglade_bake_layers_stream_beyond_the_cloud_run_buffer_limit() {
    use futures_util::StreamExt;
    use hyper::body::Body as _;

    let root = tempfile::tempdir().unwrap();
    let (config, _) = with_everglade(root.path());
    let directory = config.everglade.as_ref().unwrap().join("kit/bake");
    std::fs::create_dir_all(&directory).unwrap();
    let name = format!("{}.vlay", "ab".repeat(32));
    let size = 33 * 1024 * 1024;
    std::fs::File::create(directory.join(&name))
        .unwrap()
        .set_len(size)
        .unwrap();
    let response = router(config)
        .oneshot(
            Request::builder()
                .uri(format!("/everglade/kit/bake/{name}"))
                .header(header::HOST, LOCAL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
    assert_eq!(response.body().size_hint().exact(), None);
    let mut stream = response.into_body().into_data_stream();
    let mut received = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        assert!(chunk.len() <= 64 * 1024);
        assert!(chunk.iter().all(|byte| *byte == 0));
        received += chunk.len() as u64;
    }
    assert_eq!(received, size);
}

#[tokio::test]
async fn everglade_wasm_streams_beyond_the_cloud_run_buffer_limit() {
    use futures_util::StreamExt;
    use hyper::body::Body as _;

    let root = tempfile::tempdir().unwrap();
    let (config, _) = with_everglade(root.path());
    let directory = config.everglade.as_ref().unwrap().clone();
    std::fs::create_dir_all(&directory).unwrap();
    let name = "everglade_web_bg.wasm";
    let size = 33 * 1024 * 1024;
    std::fs::File::create(directory.join(&name))
        .unwrap()
        .set_len(size)
        .unwrap();
    let response = router(config)
        .oneshot(
            Request::builder()
                .uri(format!("/everglade/{name}"))
                .header(header::HOST, LOCAL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/wasm");
    assert_eq!(response.headers()[header::VARY], "accept-encoding");
    assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
    assert_eq!(response.body().size_hint().exact(), None);
    let mut stream = response.into_body().into_data_stream();
    let mut received = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        assert!(chunk.len() <= 64 * 1024);
        assert!(chunk.iter().all(|byte| *byte == 0));
        received += chunk.len() as u64;
    }
    assert_eq!(received, size);
}

/// `/druid` (#10611): the same full-screen page and build, which starts in
/// the Grove on this path, under the same policy.
#[tokio::test]
async fn the_druid_page_serves_the_same_build_for_the_grove() {
    let root = tempfile::tempdir().unwrap();
    let (config, _) = with_everglade(root.path());
    let (status, headers, html) = get_with(router(config.clone()), "/druid", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<title>Druid"), "{html}");
    assert!(html.contains("Loading the Grove"));
    assert!(html.contains(&format!("<canvas id=\"{}\"", pages::CANVAS_ID)));
    assert!(html.contains(&format!("data-wasm=\"/everglade/{}\"", pages::WASM)));
    assert!(html.contains("<html lang=\"en\" class=\"stage\">"));
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        pages::EVERGLADE_POLICY
    );
    let mut absent = config;
    absent.everglade = None;
    let (status, _, html) = get_with(router(absent), "/druid", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Everglade is unavailable on this server"));
}

/// `/grid` (#10587): the same build, which opens the shared Grid on this
/// path, under the build's policy plus the public relay's WebSocket.
#[tokio::test]
async fn the_grid_page_serves_the_same_build_and_admits_the_relay() {
    let root = tempfile::tempdir().unwrap();
    let (config, _) = with_everglade(root.path());
    let (status, headers, html) = get_with(router(config.clone()), "/grid", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<title>Grid"), "{html}");
    assert!(html.contains("Loading the Grid"));
    assert!(html.contains(&format!("data-wasm=\"/everglade/{}\"", pages::WASM)));
    assert_eq!(headers[header::CONTENT_SECURITY_POLICY], pages::GRID_POLICY);
    let mut absent = config;
    absent.everglade = None;
    let (_, _, html) = get_with(router(absent), "/grid", LOCAL).await;
    assert!(html.contains("Everglade is unavailable on this server"));
}

/// Without the build directory, or without the glue in it, the page says
/// Everglade is unavailable, runs no script, keeps the site's policy, and
/// serves no build file.
#[tokio::test]
async fn the_everglade_page_says_it_is_unavailable_without_the_build() {
    let root = tempfile::tempdir().unwrap();
    let mut absent = config(root.path().join("tasks"));
    absent.everglade = Some(root.path().join("nowhere"));
    let (no_glue, _) = with_everglade(root.path());
    std::fs::remove_file(root.path().join("everglade").join(pages::GLUE)).unwrap();
    for config in [config(root.path().join("tasks")), absent, no_glue] {
        let (status, headers, html) = get_with(router(config.clone()), "/everglade", LOCAL).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            html.contains("Everglade is unavailable on this server"),
            "{html}"
        );
        assert!(!html.to_ascii_lowercase().contains("<script"));
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(!policy.contains("script-src"), "{policy}");
        let (status, _, _) =
            get_bytes(router(config), &format!("/everglade/{}", pages::WASM)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn the_download_page_links_only_the_coder_release_bundle() {
    let root = tempfile::tempdir().unwrap();
    let (status, body) = get(router(config(root.path().into())), "/download").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(&format!("<pre><code>{}</code></pre>", pages::CODER_SH)));
    assert!(body.contains(&format!("<pre><code>{}</code></pre>", pages::CODER_PS1)));
    assert!(body.contains("Run <code>coder</code> to open the new terminal"));
    for legacy in [
        "1.0.0-rc.2",
        "OpenAgents for Mac",
        "OpenAgents Terminal",
        "Everything else",
        ".dmg",
        "openagentsgemini-cli-releases/openagents/install.",
    ] {
        assert!(!body.contains(legacy), "legacy download remained: {legacy}");
    }
    assert!(!body.contains(pages::TESTFLIGHT));
    assert!(!body.contains("testflight") && !body.contains("TestFlight"));
    assert!(!body.contains("AppImage") && !body.contains("amd64.deb"));
    let main = &body[body.find("<main").unwrap()..body.find("</main>").unwrap()];
    let links: Vec<&str> = main
        .split("href=\"")
        .skip(1)
        .map(|rest| &rest[..rest.find('"').unwrap()])
        .filter(|href| href.starts_with("http"))
        .collect();
    let mut expected = Vec::new();
    for (_, platform) in pages::CODER_PLATFORMS {
        let extension = if platform.starts_with("windows-") {
            ".exe"
        } else {
            ""
        };
        for command in ["coder", "openagents", "microcoder"] {
            expected.push(format!(
                "{}/{command}-{}-{platform}{extension}",
                pages::CODER_BASE,
                pages::CODER_VERSION
            ));
        }
        if platform.starts_with("windows-") {
            expected.push(format!(
                "{}/coder-boundary-{}-{platform}.exe",
                pages::CODER_BASE,
                pages::CODER_VERSION
            ));
        }
    }
    expected.push(format!(
        "{}/SHA256SUMS-coder-{}",
        pages::CODER_BASE,
        pages::CODER_VERSION
    ));
    assert_eq!(links, expected, "{body}");
    assert!(body.contains("<h2 id=\"coder-title\">Coder + OpenAgents CLI</h2>"));
    assert!(body.contains("<title>Download Coder \u{b7} OpenAgents</title>"));
    assert!(body.contains("<h1>Download Coder</h1>"));
    // Download is the header pill now; on its own page it is marked current.
    let pill = body.find("href=\"/download\"").expect("download pill");
    let tag_end = pill + body[pill..].find('>').unwrap();
    let tag_start = body[..pill].rfind('<').unwrap();
    assert!(
        body[tag_start..tag_end].contains("aria-current=\"page\""),
        "{}",
        &body[tag_start..tag_end]
    );
    assert!(!body.contains("href=\"/docs\" aria-current"));
    let (status, guide) = get(router(config(root.path().into())), "/docs/download").await;
    assert_eq!(status, StatusCode::OK);
    assert!(guide.contains(pages::CODER_SH));
    assert!(guide.contains(pages::CODER_PS1));
    assert!(!guide.contains("1.0.0-rc.2"));
    assert!(!guide.contains(".dmg"));
    assert!(!guide.contains("OpenAgents Terminal"));
    // Its older addresses, and the guide's old name, redirect for good.
    for (old, new) in [
        ("/install", "/download"),
        ("/desktop", "/download"),
        ("/docs/install", "/docs/download"),
        ("/docs/help", "/docs/troubleshooting"),
    ] {
        let (status, headers, _) = get_with(router(config(root.path().into())), old, LOCAL).await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{old}");
        assert_eq!(headers[header::LOCATION], new, "{old}");
    }
}

#[tokio::test]
async fn hosted_cli_installers_serve_the_bundled_scripts_without_proxying() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for (uri, expected) in [
        (
            "/cli/install.sh",
            include_str!("../../../scripts/install/coder.sh"),
        ),
        (
            "/cli/install.ps1",
            include_str!("../../../scripts/install/coder.ps1"),
        ),
    ] {
        let (status, headers, body) = get_with(site.clone(), uri, "openagents.com").await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], "text/plain; charset=utf-8");
        assert_eq!(headers[header::CACHE_CONTROL], "public, max-age=300");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(body, expected, "{uri}");
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
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

/// Application styles use the same semantic tokens as native Coder Noir.
#[tokio::test]
async fn application_styles_share_coder_noir_roles() {
    let root = tempfile::tempdir().unwrap();
    let variables = coder_ui::coder_noir::css_variables();
    for path in [
        "/static/legacy-demo.css",
        "/components/assets/components.css",
    ] {
        let (status, css) = get(router(config(root.path().into())), path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(css.matches(&variables).count(), 1, "{path}");
        assert!(css.contains("--noir-accent:#ededed"), "{path}");
        assert!(css.contains("--noir-terminal-cursor:#ededed"), "{path}");
        assert!(
            css.contains("--noir-control-hover:rgb(237 237 237 / 0.047)"),
            "{path}"
        );
    }
    let (_, favicon) = get(router(config(root.path().into())), "/favicon.svg").await;
    assert!(favicon.contains(&format!("fill=\"#{:06x}\"", coder_ui::coder_noir::CANVAS)));
    assert!(favicon.contains(&format!("stroke=\"#{:06x}\"", coder_ui::coder_noir::ACCENT)));
    assert!(!favicon.contains("{{"));
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

/// The stylesheet links of a page, in order, as served paths.
fn stylesheet_links(page: &str) -> Vec<String> {
    page.split("<link rel=\"stylesheet\" href=\"")
        .skip(1)
        .map(|rest| rest[..rest.find('"').unwrap_or(rest.len())].replace("&amp;", "&"))
        .collect()
}

/// The pages in the Coder Light / Coder Noir design language that need no
/// sign-in: every public page, the component catalog, and the problem page.
fn ui_pages() -> Vec<&'static str> {
    PAGES
        .iter()
        .copied()
        .chain([
            "/ui",
            "/nope",
            "/demo",
            "/demo/lease-fix",
            "/demo/benchmark",
        ])
        .collect()
}

/// UI-13: a `UiPage` page links one stylesheet, `/static/ui.css`, within
/// the `openagents-ui` byte budget; no legacy stylesheet is shipped.
#[tokio::test]
async fn ui_pages_ship_only_the_design_language_css_within_budget() {
    use openagents_ui::css_classes::STYLESHEET_BUDGET_BYTES;
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().join("tasks")));
    for uri in ui_pages() {
        let (_, page) = get(site.clone(), uri).await;
        let links = stylesheet_links(&page);
        assert_eq!(links.len(), 1, "{uri} links {links:?}");
        assert!(
            links[0].starts_with(&format!("{}?v=", theme::STYLESHEET_PATH)),
            "{uri} links {links:?}"
        );
        let (status, _, css) = get_bytes(site.clone(), &links[0]).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(
            css.len() <= STYLESHEET_BUDGET_BYTES,
            "{uri} ships {} bytes of CSS, over the {STYLESHEET_BUDGET_BYTES}-byte budget",
            css.len()
        );
    }
}

/// UI-13: every class a design-language page renders has a rule in the CSS
/// that page links (script hooks named in `openagents_ui::css_classes`
/// excepted). A class with no rule is a missing rule or a legacy leftover.
#[tokio::test]
async fn ui_pages_reference_no_class_without_a_rule() {
    use openagents_ui::css_classes::{markup_classes, needs_rule, selector_classes};
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().join("tasks")));
    for uri in ui_pages() {
        let (_, page) = get(site.clone(), uri).await;
        let mut defined = std::collections::BTreeSet::new();
        for link in stylesheet_links(&page) {
            let (status, css) = get(site.clone(), &link).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {link}");
            defined.extend(selector_classes(&css));
        }
        let missing: Vec<_> = markup_classes(&page)
            .into_iter()
            .filter(|class| needs_rule(class) && !defined.contains(class))
            .collect();
        assert!(
            missing.is_empty(),
            "{uri} uses classes with no rule: {missing:?}"
        );
    }
}

/// UI-13: no design-language page carries an inline `style` attribute or a
/// `<style>` element (`style-src 'self'` would drop them).
#[tokio::test]
async fn ui_pages_carry_no_inline_style() {
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().join("tasks")));
    for uri in ui_pages() {
        let (_, page) = get(site.clone(), uri).await;
        let lower = page.to_ascii_lowercase();
        assert!(!lower.contains("<style"), "{uri}");
        for tag in lower.split('<').skip(1) {
            let tag = tag.split('>').next().unwrap_or_default();
            assert!(!tag.contains(" style="), "{uri} styles an element: {tag}");
        }
    }
}

/// UI-13: the legacy site stylesheet, Tailwind, and the retired terminal
/// script are gone; only the full-screen canvas pages load the
/// small Coder Noir stylesheet they keep.
#[tokio::test]
async fn the_legacy_styles_are_removed() {
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().into()));
    for path in ["/static/site.css", "/static/tailwind.css", "/static/ask.js"] {
        let (status, _) = get(site.clone(), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, css) = get(site.clone(), "/static/legacy-demo.css").await;
    assert_eq!(status, StatusCode::OK);
    for gone in [
        ".site-header",
        ".term",
        ".md{",
        ".list{",
        ".dim{",
        ".error{",
        ".button",
    ] {
        assert!(!css.contains(gone), "{gone}");
    }
    let (_, cloud) = get(site, "/cloud/assets/cloud.css").await;
    assert!(
        !cloud.contains("--noir-"),
        "the Cloud area styles from openagents-ui tokens"
    );
    assert!(!cloud.contains(":not([class])"), "no bare-control rules");
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
    assert!(!home.contains("of credit"));
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

// ---------------------------------------------------------------------
// The retired homepage terminal route.

async fn post_ask(
    router: Router,
    body: &str,
    cookie: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::builder()
        .method("POST")
        .uri("/ask")
        .header(header::HOST, LOCAL)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = router
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_retired_question_route_links_to_the_homepage_without_naming_a_visitor() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path().into());
    let question = json!({"turns": [{"role": "user", "text": "what is OpenAgents?"}]}).to_string();
    let (status, headers, body) = post_ask(router(config.clone()), &question, None).await;
    assert_eq!(status, StatusCode::GONE, "{body}");
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store, private");
    assert_eq!(headers[header::LINK], "</>; rel=\"alternate\"");
    assert!(headers.get(header::SET_COOKIE).is_none());
    assert!(
        body.contains("Start a chat") && body.contains("href=\"/\""),
        "{body}"
    );
    assert!(!body.contains("what is OpenAgents?"));
    let named = format!("{}={}", ask::COOKIE, ask::new_visitor());
    let (status, headers, _) = post_ask(router(config), &question, Some(&named)).await;
    assert_eq!(status, StatusCode::GONE);
    assert!(headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn retired_questions_do_not_parse_the_old_request_body() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path().into());
    for body in [
        "not json",
        r#"{"turns":[]}"#,
        r#"{"turns":[{"role":"assistant","text":"hi"}]}"#,
        r#"{"turns":[{"role":"user","text":"   "}]}"#,
        r#"{"turns":[{"role":"system","text":"hi"}]}"#,
    ] {
        let (status, _, _) = post_ask(router(config.clone()), body, None).await;
        assert_eq!(status, StatusCode::GONE, "{body}");
    }
}

// ---------------------------------------------------------------------
// The upstream fallback: paths the site doesn't own go to the previous
// server, which here is an in-process echo.

/// An upstream that answers every request with `418` and a JSON echo of
/// what reached it, and joins an `Upgrade: echo` on `/ws` to an echo of
/// its bytes. It counts what it was sent.
pub(crate) async fn echo_upstream() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let app = Router::new().fallback(move |mut request: Request<Body>| {
        let counted = counted.clone();
        async move {
            counted.fetch_add(1, Ordering::SeqCst);
            let headers = request.headers().clone();
            let header = |name: &str| {
                headers
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned()
            };
            if request.uri().path() == "/ws" && header("upgrade") == "echo" {
                let upgraded = hyper::upgrade::on(&mut request);
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut io = hyper_util::rt::TokioIo::new(upgraded.await.unwrap());
                    let mut buffer = [0u8; 64];
                    let read = io.read(&mut buffer).await.unwrap();
                    io.write_all(&buffer[..read]).await.unwrap();
                });
                return axum::response::Response::builder()
                    .status(StatusCode::SWITCHING_PROTOCOLS)
                    .header(header::CONNECTION, "upgrade")
                    .header(header::UPGRADE, "echo")
                    .body(Body::empty())
                    .unwrap();
            }
            let mut echo = json!({
                "method": request.method().as_str(),
                "uri": request.uri().to_string(),
                "host": header("host"),
                "forwarded_host": header("x-forwarded-host"),
                "forwarded_for": header("x-forwarded-for"),
                "forwarded_proto": header("x-forwarded-proto"),
                "keep_alive": header("keep-alive"),
            });
            let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
            echo["body"] = json!(String::from_utf8_lossy(&body));
            axum::response::Response::builder()
                .status(StatusCode::IM_A_TEAPOT)
                .header("set-cookie", "upstream=1; Path=/; HttpOnly")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(echo.to_string()))
                .unwrap()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), hits)
}

/// A public openagents.com server whose unowned paths go to `upstream`.
fn proxying(root: &std::path::Path, upstream: &str) -> Config {
    let mut config = config(root.join("tasks"));
    config.public_hosts.push("openagents.com".to_owned());
    config.upstream = Some(Arc::new(upstream::Upstream::new(upstream).unwrap()));
    config
}

#[test]
fn the_site_owns_its_pages_and_the_removed_sections() {
    for path in [
        "/",
        "/download",
        "/chat",
        "/chat/x",
        "/install",
        "/desktop",
        "/cli/install.sh",
        "/cli/install.ps1",
        "/docs",
        "/docs/download",
        "/docs/install",
        "/docs/nope",
        "/terms",
        "/privacy",
        "/connect",
        "/live",
        "/stats",
        "/efficiency",
        "/everglade",
        "/everglade/everglade_web.js",
        "/everglade/pack/x.vtp",
        "/ask",
        "/health",
        "/.well-known/apple-app-site-association",
        "/.well-known/assetlinks.json",
        "/static/legacy-demo.css",
        "/static/chat.js",
        "/static/flow.js",
        "/static/everglade.js",
        "/static/verse-grid.jpg",
        "/favicon.svg",
        "/favicon.ico",
        "/app",
        "/app/tasks/x",
        "/cloud",
        "/cloud/app",
        "/cloud/app/tasks/x",
        "/forum",
        "/forum/x",
        "/gym",
        "/traces",
        "/trace/x",
        "/earn",
        "/weights",
        "/qa",
        "/blog",
        "/blog/introducing-coder",
        "/doc",
        "/doc/install",
    ] {
        assert!(upstream::owned(path), "{path}");
    }
    for path in [
        "/v1/token",
        "/api/v1/chat",
        "/login",
        "/logout",
        "/auth/github/callback",
        "/stripe/webhook",
        "/mcp",
        "/releases/coder-latest.tar.gz",
        "/install-terminal.sh",
        "/install-terminal.ps1",
        "/u/someone",
        "/u/someone/avatar",
        "/ws",
        "/settings",
        "/.well-known/oauth-protected-resource",
        "/robots.txt",
        "/static/coder.css",
        "/static/webtui.css",
        "/static/favicon.png",
        "/forums",
        "/documents",
        "/installer",
        "/earnings",
    ] {
        assert!(!upstream::owned(path), "{path}");
    }
}

#[tokio::test]
async fn unowned_paths_are_proxied_with_their_method_host_body_and_status() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    let response = site
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/token?scope=a%20b")
                .header(header::HOST, "openagents.com")
                .header("x-forwarded-for", "203.0.113.7")
                .header("x-forwarded-proto", "https")
                .header("keep-alive", "timeout=5")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"ask\":1}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
    let headers = response.headers().clone();
    assert_eq!(headers["set-cookie"], "upstream=1; Path=/; HttpOnly");
    assert!(
        !headers.contains_key(header::CONTENT_SECURITY_POLICY),
        "the site's headers are for its own pages"
    );
    let echo: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(echo["method"], "POST");
    assert_eq!(echo["uri"], "/v1/token?scope=a%20b");
    assert_eq!(echo["host"], "openagents.com", "the original Host");
    assert_eq!(echo["forwarded_host"], "openagents.com");
    assert_eq!(echo["forwarded_for"], "203.0.113.7", "passed on unchanged");
    assert_eq!(echo["forwarded_proto"], "https");
    assert_eq!(echo["keep_alive"], "", "hop-by-hop headers stop here");
    assert_eq!(echo["body"], "{\"ask\":1}");
    for uri in [
        "/api/v1/chat",
        "/login",
        "/auth/github/callback",
        "/releases/install-terminal.sh",
        "/install-terminal.sh",
        "/u/someone",
        "/u/someone/avatar",
        "/.well-known/oauth-protected-resource",
        "/static/coder.css",
        "/forums",
    ] {
        let (status, _, body) = get_with(site.clone(), uri, "openagents.com").await;
        assert_eq!(status, StatusCode::IM_A_TEAPOT, "{uri}");
        assert!(
            body.contains(&format!("\"uri\":\"{uri}\"")),
            "{uri}: {body}"
        );
    }
    // Another name the previous server answered goes there whole.
    let (status, _, body) = get_with(site.clone(), "/", "new.openagents.com").await;
    assert_eq!(status, StatusCode::IM_A_TEAPOT);
    assert!(body.contains("\"host\":\"new.openagents.com\""), "{body}");
    assert_eq!(hits.load(Ordering::SeqCst), 12);
}

#[tokio::test]
async fn owned_pages_removed_sections_and_the_task_browser_never_go_upstream() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for uri in PAGES.iter().copied().chain([
        "/health",
        "/.well-known/apple-app-site-association",
        "/.well-known/assetlinks.json",
        "/static/legacy-demo.css",
        "/favicon.svg",
    ]) {
        let (status, headers, _) = get_with(site.clone(), uri, "openagents.com").await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(
            headers.contains_key(header::CONTENT_SECURITY_POLICY),
            "{uri}"
        );
    }
    for uri in ["/install", "/desktop", "/docs/install", "/docs/help"] {
        let (status, _, _) = get_with(site.clone(), uri, "openagents.com").await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{uri}");
    }
    for uri in [
        "/forum",
        "/gym",
        "/traces",
        "/trace/x",
        "/earn",
        "/weights",
        "/qa",
        "/blog",
        "/blog/introducing-coder",
        "/doc",
        "/doc/install",
        "/docs/nope",
    ] {
        let (status, _, body) = get_with(site.clone(), uri, "openagents.com").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(
            body.contains("href=\"/terms\""),
            "{uri}: in the site's frame"
        );
    }
    for host in ["openagents.com", "new.openagents.com"] {
        for uri in ["/app", "/app/tasks/x"] {
            let (status, _, _) = get_with(site.clone(), uri, host).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{host}{uri}");
        }
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cloud_credentials_and_proposed_paths_never_reach_the_legacy_proxy() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for host in ["openagents.com", "unknown.openagents.com"] {
        for path in ["/cloud", "/cloud/app", "/cloud/app/tasks/private"] {
            let response = site
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .header(header::HOST, host)
                        .header(header::COOKIE, "oa_cloud=sess_synthetic_private")
                        .header(header::AUTHORIZATION, "Bearer synthetic_private")
                        .body(Body::from("synthetic private request"))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            assert_eq!(
                status,
                if host == "openagents.com" {
                    StatusCode::METHOD_NOT_ALLOWED
                } else {
                    StatusCode::FORBIDDEN
                },
                "{host}{path}",
            );
            let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("synthetic_private"));
        }
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn misplaced_cloud_credentials_never_proxy_on_unowned_paths() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for host in ["openagents.com", "unknown.openagents.com"] {
        let response = site
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/old-private-api")
                    .header(header::HOST, host)
                    .header(header::COOKIE, "oa_cloud_session=sess_synthetic_private")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store, private"
        );
    }
    for authorization in [
        "Bearer sess_synthetic_private",
        "bEaReR  sess_synthetic_private",
    ] {
        for path in [
            "/old-private-api",
            "/",
            "/chat/00000000-0000-4000-8000-000000000000",
        ] {
            let response = site
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::HOST, "unknown.openagents.com")
                        .header(header::AUTHORIZATION, authorization)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "no-store, private"
            );
        }
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cloud_cookies_reach_owned_pages_only_on_a_configured_host() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for cookie in [
        "oa_cloud_session=sess_synthetic_private",
        "oa_cloud_workspace=synthetic",
        "oa_cloud_login=synthetic",
        "oa_cloud_future=synthetic",
        "oa_cloud_session=sess_synthetic_private; oa_cloud_workspace=synthetic; oa_cloud_login=synthetic",
    ] {
        for host in ["openagents.com", LOCAL, "localhost:4300"] {
            for path in ["/", "/demo", "/static/chat-start.js", "/cloud"] {
                let response = site
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .header(header::HOST, host)
                            .header(header::COOKIE, cookie)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                // `/cloud` answers locally with its redirect home.
                let expected = if path == "/cloud" {
                    StatusCode::SEE_OTHER
                } else {
                    StatusCode::OK
                };
                assert_eq!(response.status(), expected, "{host}{path} {cookie}");
            }
        }
        for (host, path) in [
            ("openagents.com", "/old-private-api"),
            (LOCAL, "/old-private-api"),
            ("localhost:4300", "/old-private-api"),
            ("unknown.openagents.com", "/old-private-api"),
            ("unknown.openagents.com", "/"),
            ("unknown.openagents.com", "/demo"),
            ("unknown.openagents.com", "/cloud"),
            (
                "unknown.openagents.com",
                "/chat/00000000-0000-4000-8000-000000000000",
            ),
        ] {
            let response = site
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::HOST, host)
                        .header(header::COOKIE, cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{host}{path} {cookie}"
            );
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "no-store, private"
            );
        }
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_unreachable_upstream_answers_502() {
    let root = tempfile::tempdir().unwrap();
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let (status, _, _) = get_with(
        router(proxying(root.path(), &url)),
        "/v1/token",
        "openagents.com",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[test]
fn the_upstream_is_a_plain_http_origin() {
    assert!(upstream::Upstream::new("http://127.0.0.1:8081").is_ok());
    for bad in [
        "https://example.com",
        "http://127.0.0.1:8081/base",
        "127.0.0.1:8081",
        "nope",
    ] {
        assert!(upstream::Upstream::new(bad).is_err(), "{bad}");
    }
}

#[tokio::test]
async fn an_upgrade_is_joined_end_to_end() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = tempfile::tempdir().unwrap();
    let (url, _) = echo_upstream().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let site = router(proxying(root.path(), &url));
    tokio::spawn(async move {
        axum::serve(
            listener,
            site.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            b"GET /ws HTTP/1.1\r\nHost: openagents.com\r\nConnection: Upgrade\r\nUpgrade: echo\r\n\r\n",
        )
        .await
        .unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await.unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    assert!(
        head.to_ascii_lowercase().contains("upgrade: echo"),
        "{head}"
    );
    stream.write_all(b"ping").await.unwrap();
    let mut echoed = [0u8; 4];
    stream.read_exact(&mut echoed).await.unwrap();
    assert_eq!(&echoed, b"ping");
}

#[tokio::test]
async fn payment_routes_use_dedicated_upstream_and_strip_api_prefix() {
    let root = tempfile::tempdir().unwrap();
    let (pay, hits) = echo_upstream().await;
    let mut config = proxying(root.path(), &pay);
    config.pay_upstream = Some(Arc::new(upstream::Upstream::new(&pay).unwrap()));
    for (path, target) in [
        ("/api/flow/snapshot", "/flow/snapshot"),
        ("/api/stats", "/stats"),
        ("/api/flow/stream?x=1", "/flow/stream?x=1"),
    ] {
        let (status, headers, body) =
            get_with(router(config.clone()), path, "openagents.com").await;
        assert_eq!(status, StatusCode::IM_A_TEAPOT);
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["uri"], target);
    }
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 3);
}

#[tokio::test]
async fn payment_proxy_preserves_resume_and_streams_before_upstream_finishes() {
    use futures_util::StreamExt;
    let app = Router::new().route(
        "/flow/stream",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["last-event-id"], "41");
            let body = futures_util::stream::once(async {
                Ok::<_, std::convert::Infallible>("id: 42\ndata: {\"v\":1,\"seq\":42}\n\n")
            })
            .chain(futures_util::stream::pending());
            axum::response::Response::builder()
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(Body::from_stream(body))
                .unwrap()
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let root = tempfile::tempdir().unwrap();
    let mut config = config(root.path().into());
    config.public_hosts.push("openagents.com".into());
    config.pay_upstream = Some(Arc::new(
        upstream::Upstream::new(&format!("http://{addr}")).unwrap(),
    ));
    let response = router(config)
        .oneshot(
            Request::builder()
                .uri("/api/flow/stream")
                .header("host", "openagents.com")
                .header("last-event-id", "41")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let mut body = response.into_body().into_data_stream();
    let chunk = tokio::time::timeout(std::time::Duration::from_secs(2), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8(chunk.to_vec())
            .unwrap()
            .contains("id: 42")
    );
    server.abort();
}

// ---------------------------------------------------------------------
// `/stats` (#10196): drawn on the server from the pay host's public
// `/stats` and `/flow/snapshot`, here a stub serving fixtures.

/// A pay host answering `/stats` and `/flow/snapshot` with `stats` and
/// `snapshot`.
async fn stub_pay_host(stats: serde_json::Value, snapshot: serde_json::Value) -> String {
    let app = Router::new()
        .route(
            "/stats",
            axum::routing::get(move || {
                let stats = stats.clone();
                async move { axum::Json(stats) }
            }),
        )
        .route(
            "/flow/snapshot",
            axum::routing::get(move || {
                let snapshot = snapshot.clone();
                async move { axum::Json(snapshot) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn with_pay_host(root: &std::path::Path, pay: &str) -> Config {
    let mut config = config(root.join("tasks"));
    config.public_hosts.push("openagents.com".to_owned());
    config.pay_upstream = Some(Arc::new(upstream::Upstream::new(pay).unwrap()));
    config
}

fn totals(received: f64, paid: f64, pending: f64, calls: u64, earned: f64) -> serde_json::Value {
    json!({
        "received_sats": received,
        "paid_out_sats": paid,
        "pending_accruals_sats": pending,
        "calls": calls,
        "earnings_sats": earned,
    })
}

#[tokio::test]
async fn the_stats_page_renders_the_pay_hosts_numbers() {
    let hour = 3_600_000_i64;
    let now = 1_790_000_000_000_i64 / hour * hour;
    let mut series_24h: Vec<_> = (0..24)
        .map(|i| json!({"at": now - (23 - i) * hour, "width_ms": hour, "totals": totals(0.0, 0.0, 0.0, 0, 0.0)}))
        .collect();
    series_24h[23]["totals"] = totals(1500.0, 0.0, 0.0, 3, 0.0);
    let stats = json!({
        "totals": totals(1500.0, 900.0, 600.0, 3, 1200.5),
        "per_plugin": {
            "explain-error": totals(0.0, 900.0, 300.5, 2, 1200.5),
            "summarize": totals(0.0, 0.0, 0.0, 1, 0.0),
            "<i>x</i>": totals(0.0, 0.0, 0.0, 0, 0.0),
        },
        "per_author": {
            "alice": totals(0.0, 900.0, 300.5, 0, 1200.5),
        },
        "series_24h": series_24h,
        "series_30d": [],
        "reconciliation": "ok",
    });
    let snapshot = json!({
        "events": [
            {"v": 1, "seq": 1, "at": now + 60_000, "type": "call", "resource": "plugin",
             "plugin": "explain-error", "node": "plugin:explain-error", "payer": "fox-17"},
            {"v": 1, "seq": 2, "at": now + 120_000, "type": "payout", "resource": "plugin",
             "plugin": "explain-error", "node": "plugin:explain-error", "amount_sats": 905,
             "split": {"author": 900, "lsp_fee": 5}, "author": "alice"},
            {"v": 1, "seq": 3, "at": now + 180_000, "type": "payout", "resource": "route",
             "node": "router", "amount_sats": 40, "split": {"openagents": 40}},
            {"v": 1, "seq": 4, "at": now + 240_000, "type": "payment", "resource": "plugin",
             "plugin": "<b>x</b>", "node": "plugin:x", "amount_sats": 10, "payer": "owl-3"},
        ],
        "totals": totals(1500.0, 900.0, 600.0, 3, 1200.5),
        "topology": [],
    });
    let pay = stub_pay_host(stats, snapshot).await;
    let root = tempfile::tempdir().unwrap();
    let (status, headers, html) = get_with(
        router(with_pay_host(root.path(), &pay)),
        "/stats",
        "openagents.com",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(!policy.contains("script-src"), "{policy}");
    assert!(!html.to_ascii_lowercase().contains("<script"));
    assert!(html.contains("href=\"/live\""));
    // Totals, exact and grouped.
    assert!(
        html.contains("<dt>Received</dt><dd>1,500 sats</dd>"),
        "{html}"
    );
    assert!(html.contains("<dt>Paid out</dt><dd>900 sats</dd>"));
    assert!(html.contains("<dt>Pending</dt><dd>600 sats</dd>"));
    assert!(html.contains("<dt>Calls</dt><dd>3</dd>"));
    assert!(html.contains("<dt>Author earnings</dt><dd>1,200.5 sats</dd>"));
    // Plugins by earnings, then authors.
    let plugins = &html[html.find("id=\"stats-plugins\"").unwrap()..];
    assert!(plugins.find("explain-error").unwrap() < plugins.find("summarize").unwrap());
    assert!(
        plugins.contains("<td>explain-error</td><td>2</td><td>1,200.5 sats</td><td>900 sats</td>")
    );
    assert!(
        html.contains("<td>alice</td><td>1,200.5 sats</td><td>900 sats</td><td>300.5 sats</td>")
    );
    // Recent payouts: the author's part only, no treasury-only payout.
    let payouts = &html[html.find("id=\"stats-payouts\"").unwrap()..];
    let payouts = &payouts[..payouts.find("</table>").unwrap()];
    assert_eq!(payouts.matches("<tr>").count(), 2, "{payouts}");
    assert!(payouts.contains(&format!(
        "<td>{}</td><td>explain-error</td><td>alice</td><td>900 sats</td>",
        pages::utc(now + 120_000)
    )));
    // The footing, the series, escaping, and no payer alias anywhere.
    assert!(html.contains("Reconciliation: the ledger matches the wallet."));
    assert!(html.contains(&format!("Last event: {} UTC.", pages::utc(now + 240_000))));
    let day = &html[html.find("id=\"stats-24h\"").unwrap()..];
    assert_eq!(
        day[..day.find("</figure>").unwrap()]
            .matches("class=\"oa-chart-bar\"")
            .count(),
        24
    );
    assert!(day.contains("1,500 sats received over 3 calls"));
    assert!(html.contains("The last 30 days, by day: nothing received."));
    assert!(!html.contains("<b>x</b>") && !html.contains("<i>x</i>"));
    assert!(html.contains("<td>&lt;i&gt;x&lt;/i&gt;</td>"));
    assert!(!html.contains("fox-17") && !html.contains("owl-3"));
}

#[tokio::test]
async fn the_stats_page_says_when_there_are_no_payments_or_no_pay_host() {
    let empty = json!({
        "totals": totals(0.0, 0.0, 0.0, 0, 0.0),
        "per_plugin": {}, "per_author": {}, "series_24h": [], "series_30d": [],
        "reconciliation": "unknown",
    });
    let pay = stub_pay_host(empty, json!({"events": [], "topology": []})).await;
    let root = tempfile::tempdir().unwrap();
    let (status, _, html) = get_with(
        router(with_pay_host(root.path(), &pay)),
        "/stats",
        "openagents.com",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("id=\"stats-empty\""), "{html}");
    assert!(html.contains("Reconciliation: not checked yet. Last event: none yet."));
    assert!(!html.contains("<table") && !html.contains("sats</dd>"));

    // No pay host configured, and one that doesn't answer.
    for config in [
        config(root.path().join("tasks")),
        with_pay_host(root.path(), "http://127.0.0.1:9"),
    ] {
        let (status, _, html) = get_with(router(config), "/stats", LOCAL).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("id=\"stats-unreachable\""), "{html}");
        assert!(!html.contains("<table") && !html.contains(" sats"));
        assert!(html.contains("href=\"/live\""));
    }
}

#[tokio::test]
async fn the_site_serves_the_agent_discovery_documents_for_its_public_origin() {
    let store = tempfile::tempdir().unwrap();
    let mut config = Config::development(store.path().to_path_buf());
    config.public_hosts.push("openagents.com".to_owned());
    let app = router(config);
    for (path, needle) in [
        (
            "/.well-known/agent-card.json",
            "https://api.typesafe.ai/v1/systemone",
        ),
        (
            "/.well-known/agent-skills/index.json",
            "https://openagents.com/.well-known/agent-skills/openagents-decision-api/SKILL.md",
        ),
        (
            "/.well-known/agent-skills/openagents-decision-api/SKILL.md",
            "openagents-decision-api",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::get(path)
                    .header("host", "openagents.com")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "{path}");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains(needle), "{path}: {body}");
    }
}

// ---------------------------------------------------------------------
// The local plugin purchase browser (REV-44).

fn purchase_summary() -> coder::customer::plugins::Summary {
    use coder::customer::plugins::{Charge, Phase, Summary};
    Summary {
        id: "one".into(),
        phase: Phase::Unknown,
        account: "buyer".into(),
        workspace: "<b>ws</b>".into(),
        plugin: Some("meeting-action-items".into()),
        release: Some("sha256:ab".into()),
        url: "https://api.example.com/v1/plugins/x/invoke".into(),
        quote_digest: "sha256:q".into(),
        approval_digest: "sha256:approval".into(),
        price_msat: 6000,
        max_fee_msat: 10,
        payer_node: "02node".into(),
        payer_network: "bitcoin".into(),
        created_at_ms: 1,
        expires_at_ms: 2,
        charge: Some(Charge {
            payment_hash: "hash".into(),
            amount_msat: 6000,
            fee_msat: 3,
        }),
        settled: None,
        transaction: None,
        delivery_status: None,
        result_present: false,
        unresolved_maximum_msat: Some(6010),
        recovery_present: false,
    }
}

#[tokio::test]
async fn purchase_browser_reads_without_creating_or_writing() {
    let root = tempfile::tempdir().unwrap();
    let store = root.path().join("tasks");
    let customer = root.path().join("customer");
    let (status, body) = get(router(config(store.clone())), "/app/purchases").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("--customer"));
    let mut with = config(store);
    with.customer = Some(customer.clone());
    let (status, body) = get(router(with.clone()), "/app/purchases").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("No purchases") && body.contains("Unavailable in the browser"));
    assert!(!customer.exists());
    let (status, _) = get(router(with), "/app/purchases/one").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!customer.exists());
}

#[test]
fn purchase_pages_show_identical_terms_and_name_the_installed_client_step() {
    use coder::customer::plugins::Phase;
    let mut item = purchase_summary();
    let list = crate::purchases::render_list(std::slice::from_ref(&item));
    assert!(list.contains("meeting-action-items") && list.contains("Unknown / 6000 msat"));
    let one = crate::purchases::render_one(&item);
    assert!(one.contains("&lt;b&gt;ws&lt;/b&gt;") && !one.contains("<b>ws</b>"));
    for term in [
        "sha256:q",
        "sha256:approval",
        "02node on bitcoin",
        "at most 6010 msat",
    ] {
        assert!(one.contains(term), "{term}");
    }
    assert!(one.contains("purchase recover --root ROOT --purchase one"));
    assert!(!one.contains("<form") && !one.contains("<script"));
    item.phase = Phase::Quoted;
    let quoted = crate::purchases::render_one(&item);
    assert!(
        quoted.contains("purchase approve --root ROOT --purchase one --digest sha256:approval")
    );
    item.phase = Phase::Approved;
    assert!(crate::purchases::render_one(&item).contains("purchase invoke --root ROOT"));
}
