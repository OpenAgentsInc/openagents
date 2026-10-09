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

/// The scripts a page runs: every `<script` but the JSON-LD data block,
/// which a browser never runs.
fn scripts(lower: &str) -> usize {
    lower.matches("<script").count()
        - lower
            .matches("<script type=\"application/ld+json\">")
            .count()
}

async fn get(router: Router, uri: &str) -> (StatusCode, String) {
    let (status, _, body) = get_with(router, uri, LOCAL).await;
    (status, body)
}

/// Every public HTML page a development server serves in the OpenAgents
/// shell. `/studios/blue-rush` is left out on purpose: it is the Blue Rush
/// Studios brand, with its own stylesheet, script, nav and footer instead
/// of `UiPage`, so the shell's checks (one `ui.css` link, the wordmark, no
/// script) don't apply. Its own test below holds it to the same policy
/// rules: no inline script or style, scripts from this site only.
const PAGES: [&str; 56] = [
    "/",
    "/live",
    "/promises",
    "/roadmap",
    "/everglade",
    "/druid",
    "/grid",
    "/games/grow-little-bunny",
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
    "/docs/api",
    "/docs/api/quickstart",
    "/docs/api/models",
    "/docs/api/decisions",
    "/docs/api/responses",
    "/docs/api/chat-completions",
    "/docs/api/routing",
    "/docs/api/bring-your-own-key",
    "/docs/api/errors",
    "/docs/api/limits",
    "/docs/api/privacy",
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

/// Every docs page opens with its trail: Docs, its section when it has
/// one (a link to the index anchor, which exists, or to /docs/api), and
/// itself as the current page. The index shows just "Docs".
#[tokio::test]
async fn every_docs_page_shows_its_breadcrumb_trail() {
    let root = tempfile::tempdir().unwrap();
    let trail = |html: &str| -> String {
        let start = html.find(r#"<nav id="oa-breadcrumb""#).expect("breadcrumb");
        html[start..start + html[start..].find("</nav>").unwrap()].to_owned()
    };
    let (_, index) = get(router(config(root.path().into())), "/docs").await;
    let top = trail(&index);
    assert!(top.contains(r#"aria-current="page" title="Docs""#), "{top}");
    assert!(!top.contains("<a "), "{top}");
    for (slug, _) in pages::DOCS {
        let (_, html) = get(router(config(root.path().into())), &format!("/docs/{slug}")).await;
        let t = trail(&html);
        let section = pages::section_of(slug).expect("every guide is in a section");
        let anchor = pages::section_anchor(section);
        assert!(t.contains(r#"href="/docs">Docs</a>"#), "{slug}: {t}");
        assert!(
            t.contains(&format!(r#"href="/docs#{anchor}">"#)),
            "{slug}: {t}"
        );
        assert!(index.contains(&format!(r#"<h2 id="{anchor}">"#)), "{slug}");
        assert!(t.contains(r#"aria-current="page""#), "{slug}: {t}");
    }
    for uri in ["/docs/api", "/docs/api/quickstart"] {
        let (_, html) = get(router(config(root.path().into())), uri).await;
        let t = trail(&html);
        assert!(t.contains(r#"href="/docs">Docs</a>"#), "{uri}: {t}");
        assert_eq!(
            t.contains(r#"href="/docs/api">API</a>"#),
            uri != "/docs/api",
            "{uri}: {t}"
        );
    }
    assert!(index.contains(r#"<h2 id="api">"#));
    // The Markdown twin is untouched.
    let (_, md) = get(
        router(config(root.path().into())),
        "/docs/api/quickstart.md",
    )
    .await;
    assert!(md.starts_with("# ") && !md.contains("oa-breadcrumb"));
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
        // its own single script. The home and download pages also load the
        // site's own counting script (`/static/a.js`, #11153).
        let script = uri == "/" || uri == "/live" || uri == "/download";
        assert_eq!(
            scripts(&lower),
            if uri == "/" { 7 } else { usize::from(script) },
            "{uri} runs a script"
        );
        if uri == "/" || uri == "/download" {
            assert!(
                body.contains("src=\"/static/a.js\""),
                "{uri}: the counting script"
            );
        }
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
        // Only the home page shows the legal links (along its bottom).
        assert_eq!(
            body.contains("class=\"oa-home-legal\""),
            uri == "/",
            "{uri}: legal links only on home"
        );
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

/// `/studios/blue-rush`: the studio's own page, its stylesheet, script and
/// pictures, under a strict policy and never proxied.
#[tokio::test]
async fn the_blue_rush_studio_page_serves_its_own_brand() {
    let root = tempfile::tempdir().unwrap();
    let site = router(config(root.path().join("tasks")));
    let (status, headers, html) = get_with(site.clone(), pages::BLUE_RUSH, LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.to_ascii_lowercase().starts_with("<!doctype html>"));
    assert!(html.contains("<title>Blue Rush Studios</title>"));
    assert!(html.contains("href=\"/games/grow-little-bunny\""));
    assert!(
        html.contains("id=\"br-water\""),
        "the water behind the cards"
    );
    assert!(html.contains("id=\"br-sand\""), "the sand garden");
    assert!(!html.contains("oa-wordmark"), "not the OpenAgents shell");
    let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.starts_with("default-src 'none'"), "{policy}");
    assert!(policy.contains("script-src 'self'"), "{policy}");
    assert!(policy.contains("style-src 'self'"), "{policy}");
    assert!(!policy.contains("'unsafe-inline'"), "{policy}");
    assert!(!policy.contains("'unsafe-eval'"), "{policy}");
    let lower = html.to_ascii_lowercase();
    assert!(!lower.contains("<style"));
    assert_eq!(scripts(&lower), 1);
    for tag in lower.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        assert!(!tag.contains(" style="), "styles an element: {tag}");
        assert!(
            !tag.split_whitespace()
                .any(|word| word.starts_with("on") && word.contains('=')),
            "inline handler: {tag}"
        );
        if tag.starts_with("script") {
            assert!(
                tag.contains(&format!("src=\"{}bluerush.js?v=", pages::BLUE_RUSH_ASSETS)),
                "{tag}"
            );
        }
    }
    for target in html.split("href=\"").skip(1) {
        let target = &target[..target.find('"').unwrap()];
        if target.starts_with('/') && !target.starts_with("/games/") {
            let path = target.split('?').next().unwrap();
            let (status, _) = get(site.clone(), path).await;
            assert_eq!(status, StatusCode::OK, "links {target}");
        }
    }
    for (file, kind) in [
        ("bluerush.css", "text/css"),
        ("bluerush.js", "text/javascript"),
        ("hero-bay.jpg", "image/jpeg"),
        ("lighthouse.jpg", "image/jpeg"),
        ("harbor.jpg", "image/jpeg"),
        ("sea-arch.jpg", "image/jpeg"),
    ] {
        let path = format!("{}{file}", pages::BLUE_RUSH_ASSETS);
        let (status, headers, body) = get_bytes(site.clone(), &path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!body.is_empty(), "{path}");
        let content_type = headers[header::CONTENT_TYPE].to_str().unwrap();
        assert!(content_type.starts_with(kind), "{path}: {content_type}");
        assert!(headers.contains_key(header::CACHE_CONTROL), "{path}");
        assert!(upstream::owned(&path), "{path}");
    }
    let missing = format!("{}nope.js", pages::BLUE_RUSH_ASSETS);
    assert_eq!(get(site, &missing).await.0, StatusCode::NOT_FOUND);
    assert!(upstream::owned(pages::BLUE_RUSH));
}

#[tokio::test]
async fn the_legal_pages_carry_the_published_text() {
    let root = tempfile::tempdir().unwrap();
    let (status, terms) = get(router(config(root.path().into())), "/terms").await;
    assert_eq!(status, StatusCode::OK);
    assert!(terms.contains("<h1>Terms of Service</h1>"), "{terms}");
    assert!(terms.contains("Last updated: 2026-10-09"));
    assert!(terms.contains("OpenAgents, Inc. (“OpenAgents,” “we,” “us,” or “our”)"));
    assert!(terms.contains("governed by the laws of the State of Texas"));
    let (status, privacy) = get(router(config(root.path().into())), "/privacy").await;
    assert_eq!(status, StatusCode::OK);
    assert!(privacy.contains("<h1>Privacy Policy</h1>"), "{privacy}");
    assert!(privacy.contains("On a paid plan, you may ask us not to use the content you submit"));
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
    // The theme toggle sits in the sidebar's bottom-right corner.
    let toggle = home.find("data-oa-theme-toggle").unwrap();
    assert!(home.find("class=\"oa-sidebar-corner\"").unwrap() < toggle);
    assert!(toggle < home.find("</aside>").unwrap());
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
    // Every new chat looks like a chat page: an app-mode page whose
    // composer docks at the bottom, the starter questions over it, and the
    // "learn about" cards in the middle, each one whole-card link.
    assert!(home.contains("class=\"oa-layout\" data-mode=\"app\""));
    let dock = home
        .find("<div class=\"oa-main-composer\">")
        .expect("docked composer");
    let chips = home.find("id=\"chat-suggestions\"").unwrap();
    let composer = home
        .find("<section class=\"oa-composer\" aria-label=\"Start a chat\">")
        .unwrap();
    assert!(dock < chips && chips < composer);
    let stage = home
        .find("<div class=\"oa-thread-column oa-home-stage\"><div class=\"oa-link-cards-frame\"><ul class=\"oa-link-cards\" aria-label=\"Learn about OpenAgents\">")
        .expect("link cards");
    assert!(stage < home.find("</main>").unwrap() && stage < dock);
    assert_eq!(home.matches("<a class=\"oa-link-card\" href=").count(), 4);
    for card in crate::pages::home::learn() {
        let href = maud::html! { (card.href) }.into_string();
        assert!(home.contains(&format!("href=\"{href}\"")), "{href}");
        assert!(
            home.contains(&maud::html! { (card.title) }.into_string()),
            "{}",
            card.title
        );
        assert!(
            home.contains(&maud::html! { (card.line) }.into_string()),
            "{}",
            card.line
        );
        // Every card goes somewhere real: a site page this server answers.
        if card.href.starts_with('/') {
            let (status, _) = get(site.clone(), card.href).await;
            assert_eq!(status, StatusCode::OK, "{}", card.href);
        } else {
            assert!(
                openagents_ui::content::safe_href(card.href).is_some(),
                "{}",
                card.href
            );
        }
    }
    assert!(home.contains(
        "<form id=\"chat-form\" class=\"oa-composer-root\" action=\"/chat\" method=\"post\""
    ));
    // The homepage posts a plain form and follows the redirect to the chat.
    assert!(!home.contains("hx-post="));
    // The four starter questions sit under the composer, each
    // a plain form posting its words to `/chat`.
    assert!(home.contains(
        "<div class=\"oa-suggestions\" role=\"group\" aria-label=\"Suggestions\" id=\"chat-suggestions\">"
    ));
    for suggestion in openagents_chat::suggestions::SUGGESTIONS.iter().take(4) {
        assert!(
            home.contains(&format!(
                "<button type=\"submit\" class=\"oa-suggestion-chip\">{}</button>",
                maud::html! { (suggestion.label) }.into_string()
            )),
            "{}",
            suggestion.id
        );
    }
    assert_eq!(home.matches("class=\"oa-suggestion-chip\"").count(), 4);
    assert!(home.contains("id=\"chat-card\" class=\"oa-composer-body\""));
    assert!(home.contains("placeholder=\"Ask OpenAgents anything\""));
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
    // The legal links are quiet text centered under the docked composer,
    // and only here.
    assert!(!home.contains("<footer") && !home.contains("class=\"oa-legal\""));
    let legal = home
        .find("<div class=\"oa-home-legal\">")
        .expect("legal links");
    assert!(home.find("id=\"chat-form\"").unwrap() < legal);
    assert!(home.find("</main>").unwrap() < legal && home.find("</aside>").unwrap() < legal);
    // Home has no breadcrumb; New chat carries its Ctrl+N shortcut.
    assert!(!home.contains("class=\"oa-breadcrumb\""));
    assert!(home.contains("aria-keyshortcuts=\"Control+N\""));
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
    assert!(
        !html.contains("class=\"oa-sidebar-legal\""),
        "legal links only on home"
    );
    // The chat's title is the header row's breadcrumb, left of the actions;
    // the thread has no title row and no Beginning/Latest words.
    let crumb = html
        .find("<nav id=\"oa-breadcrumb\" class=\"oa-breadcrumb\" aria-label=\"Breadcrumb\">")
        .expect("breadcrumb");
    assert!(crumb < html.find("class=\"oa-main-header-actions\"").unwrap());
    assert!(html[crumb..].contains("aria-current=\"page\" title=\"Set up OpenAgents"));
    assert!(!html.contains("oa-thread-header") && !html.contains("oa-thread-title"));
    assert!(!html.contains(">Beginning<") && !html.contains(">Latest<"));
    // The scroll-to-bottom button floats above the docked composer.
    let button = html
        .find("<button type=\"button\" class=\"oa-scroll-bottom\" data-oa-scroll-bottom=\"#chat-thread\" aria-label=\"Scroll to bottom\"")
        .expect("scroll-to-bottom button");
    assert!(html.find("id=\"chat-thread\"").unwrap() < button);
    assert!(button < html.find("class=\"oa-main-composer\"").unwrap());
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
    assert!(home.contains("class=\"oa-home-legal\""));
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
    assert_eq!(scripts(&lower), 1, "one script");
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
        assert!(scripts(&html.to_ascii_lowercase()) == 0);
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(!policy.contains("script-src"), "{policy}");
        let (status, _, _) =
            get_bytes(router(config), &format!("/everglade/{}", pages::WASM)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// The `<main>` element of a page, where its own words are.
fn main_of(html: &str) -> &str {
    &html[html.find("<main").unwrap()..html.find("</main>").unwrap()]
}

/// A server started with `--bunny DIR` holding a stand-in Grow Little
/// Bunny build.
fn with_bunny(root: &std::path::Path) -> Config {
    let build = root.join("bunny");
    std::fs::create_dir_all(&build).unwrap();
    std::fs::write(
        build.join(pages::BUNNY_GLUE),
        "export default async function init() {}",
    )
    .unwrap();
    std::fs::write(build.join(pages::BUNNY_WASM), b"\0asm\x01\0\0\0").unwrap();
    std::fs::write(
        build.join(pages::BUNNY_START),
        "import init from \"./bunny_web.js\";",
    )
    .unwrap();
    std::fs::write(
        build.join(format!("{}.gz", pages::BUNNY_WASM)),
        b"gzipped wasm",
    )
    .unwrap();
    std::fs::write(build.join("index.html"), "not served").unwrap();
    std::fs::write(root.join("secret.js"), "outside the build").unwrap();
    let mut config = config(root.join("tasks"));
    config.bunny = Some(build);
    config
}

/// `/games/grow-little-bunny`: a full-window canvas, the build's one start
/// script, the game's policy, and the build's files with their types.
#[tokio::test]
async fn the_bunny_page_serves_the_game_build() {
    let root = tempfile::tempdir().unwrap();
    let config = with_bunny(root.path());
    let (status, headers, html) =
        get_with(router(config.clone()), "/games/grow-little-bunny", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("<title>Grow Little Bunny"), "{html}");
    assert!(html.contains(&format!("<canvas id=\"{}\"", pages::BUNNY_CANVAS)));
    assert!(html.contains("<html lang=\"en\" class=\"stage\">"));
    assert!(html.contains("user-scalable=no"));
    assert!(html.contains("id=\"bunny-status\""));
    let lower = html.to_ascii_lowercase();
    assert_eq!(scripts(&lower), 1, "one script");
    assert!(html.contains(&format!(
        "<script type=\"module\" src=\"/games/grow-little-bunny/{}\"></script>",
        pages::BUNNY_START
    )));
    assert!(!lower.contains(" style=") && !lower.contains("<style"));
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        pages::BUNNY_POLICY
    );
    let text = oa_copy::visible_text(main_of(&html));
    assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");

    for (file, content_type) in [
        (pages::BUNNY_GLUE, "text/javascript; charset=utf-8"),
        (pages::BUNNY_START, "text/javascript; charset=utf-8"),
        (pages::BUNNY_WASM, "application/wasm"),
    ] {
        let uri = format!("/games/grow-little-bunny/{file}");
        let (status, headers, _) = get_bytes(router(config.clone()), &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], content_type, "{uri}");
        assert_eq!(
            headers[header::CACHE_CONTROL],
            "public, max-age=300",
            "{uri}"
        );
    }
    let (_, _, wasm) = get_bytes(
        router(config.clone()),
        &format!("/games/grow-little-bunny/{}", pages::BUNNY_WASM),
    )
    .await;
    assert!(wasm.starts_with(b"\0asm"));
    let response = router(config.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/games/grow-little-bunny/{}", pages::BUNNY_WASM))
                .header(header::HOST, LOCAL)
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"gzipped wasm");

    for uri in [
        "/games/grow-little-bunny/index.html",
        "/games/grow-little-bunny/missing.js",
        "/games/grow-little-bunny/..%2Fsecret.js",
        "/games/grow-little-bunny/%2E%2E%2Fsecret.js",
        "/games/grow-little-bunny/../secret.js",
        "/games/grow-little-bunny/a/b.js",
    ] {
        let (status, _, _) = get_bytes(router(config.clone()), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

/// Without the build directory, or without any of its three files, the
/// page says the game can't be played here, runs no script, keeps the
/// site's policy, and serves no build file.
#[tokio::test]
async fn the_bunny_page_says_it_cannot_be_played_without_the_build() {
    let root = tempfile::tempdir().unwrap();
    let mut elsewhere = config(root.path().join("tasks"));
    elsewhere.bunny = Some(root.path().join("nowhere"));
    let no_start = with_bunny(root.path());
    std::fs::remove_file(root.path().join("bunny").join(pages::BUNNY_START)).unwrap();
    for config in [config(root.path().join("tasks")), elsewhere, no_start] {
        let (status, headers, html) =
            get_with(router(config.clone()), "/games/grow-little-bunny", LOCAL).await;
        assert_eq!(status, StatusCode::OK);
        let text = oa_copy::visible_text(main_of(&html));
        assert!(text.contains("be played here right now"), "{text}");
        assert!(scripts(&html.to_ascii_lowercase()) == 0);
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(!policy.contains("script-src"), "{policy}");
        assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
        let (status, _, _) = get_bytes(
            router(config),
            &format!("/games/grow-little-bunny/{}", pages::BUNNY_WASM),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn the_download_page_offers_every_platform_from_its_table() {
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
        "openagentsgemini-cli-releases/openagents/install.",
    ] {
        assert!(!body.contains(legacy), "legacy download remained: {legacy}");
    }
    // Desktop files show only once the release is out (`DESKTOP_RELEASED`).
    assert_eq!(body.contains(".dmg"), pages::DESKTOP_RELEASED);
    assert_eq!(body.contains("AppImage"), pages::DESKTOP_RELEASED);
    let main = &body[body.find("<main").unwrap()..body.find("</main>").unwrap()];
    let links: Vec<&str> = main
        .split("href=\"")
        .skip(1)
        .map(|rest| &rest[..rest.find('"').unwrap()])
        .filter(|href| href.starts_with("http"))
        .collect();
    // One Coder download per platform, never the engine or the CLI as a
    // download of its own. A release published as separate executables
    // (up to 1.0.0-rc.5) is installed only by the one-line installers.
    let mut expected = Vec::new();
    if pages::published_as_archives(pages::CODER_VERSION) {
        for (_, platform) in pages::CODER_PLATFORMS {
            expected.push(format!(
                "{}/{}",
                pages::CODER_BASE,
                pages::coder_archive(pages::CODER_VERSION, platform)
            ));
        }
        expected.push(format!(
            "{}/SHA256SUMS-coder-{}",
            pages::CODER_BASE,
            pages::CODER_VERSION
        ));
    }
    // Then every other platform, in the table's order.
    for part in [pages::Part::Desktop, pages::Part::Phone, pages::Part::Web] {
        for (download, url) in pages::shown(part, pages::DESKTOP_RELEASED) {
            if download.platform == "iPhone" {
                expected.push(pages::TESTFLIGHT_APP.to_owned());
            }
            expected.push(url.to_owned());
        }
    }
    assert_eq!(links, expected, "{body}");
    // The Phone section shows only while a phone download is listed.
    let phone = pages::shown(pages::Part::Phone, pages::DESKTOP_RELEASED)
        .next()
        .is_some();
    assert_eq!(body.contains(pages::TESTFLIGHT), phone);
    assert!(!main.to_lowercase().contains("microcoder"), "{main}");
    assert!(body.contains("<h2 id=\"coder-title\">Coder</h2>"));
    assert_eq!(body.contains("<h2 id=\"phone-title\">Phone</h2>"), phone);
    assert!(body.contains("<h2 id=\"web-title\">Web</h2>"));
    assert!(body.contains("<title>Download OpenAgents \u{b7} OpenAgents</title>"));
    assert!(body.contains("<h1>Download OpenAgents</h1>"));
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
    assert!(!guide.to_lowercase().contains("microcoder"));
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

/// #11031: no page a visitor can open without signing in, nor any demo
/// chat, shows machine talk.
#[tokio::test]
async fn public_and_demo_pages_have_no_machine_talk() {
    let root = tempfile::tempdir().unwrap();
    let routes = ui_pages().into_iter().chain([
        "/u/AtlantisPleb",
        "/app",
        "/demo/environment",
        "/demo/lease-fix",
        "/demo/benchmark",
    ]);
    for uri in routes {
        let (_, html) = get(router(config(root.path().join("tasks"))), uri).await;
        crate::copy_guard::assert_plain(uri, &html);
    }
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
    // The Cloud pages' stylesheet left with them (docs/web/cloud-reset.md).
    let (status, _) = get(site, "/cloud/assets/cloud.css").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
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
        assert!(body.contains("class=\"oa-wordmark\""), "{uri}");
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
        "/login",
        "/signup",
        "/auth/github",
        "/auth/github/callback",
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
        "/games/grow-little-bunny",
        "/games/grow-little-bunny/bunny_web.js",
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
        "/sign-in",
        "/sign-out",
        "/settings",
        "/settings/claude",
        "/environments",
        "/environments/new",
        "/environments/env_1/runs/run_1/events",
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
        "/logout",
        "/stripe/webhook",
        "/mcp",
        "/releases/coder-latest.tar.gz",
        "/install-terminal.sh",
        "/install-terminal.ps1",
        "/u/someone",
        "/u/someone/avatar",
        "/ws",
        "/.well-known/oauth-protected-resource",
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
    assert_eq!(hits.load(Ordering::SeqCst), 10);
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
            body.contains("class=\"oa-wordmark\""),
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
    assert!(scripts(&html.to_ascii_lowercase()) == 0);
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
    crate::copy_guard::assert_plain("/stats", &html);
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
    assert!(html.contains("Balance check: the books match the wallet."));
    assert!(html.contains(&format!("Last payment: {} UTC.", pages::utc(now + 240_000))));
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
    assert!(html.contains("Balance check: not checked yet. Last payment: none yet."));
    assert!(!html.contains("<table") && !html.contains("sats</dd>"));

    // No pay host configured, and one that doesn't answer.
    for config in [
        config(root.path().join("tasks")),
        with_pay_host(root.path(), "http://127.0.0.1:9"),
    ] {
        let (status, _, html) = get_with(router(config), "/stats", LOCAL).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("id=\"stats-unreachable\""), "{html}");
        crate::copy_guard::assert_plain("/stats", &html);
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
    assert!(body.contains("This server has no purchases to show."));
    let mut with = config(store);
    with.customer = Some(customer.clone());
    let (status, body) = get(router(with.clone()), "/app/purchases").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("No purchases") && body.contains("Plugins you buy in the app show up here.")
    );
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
        "up to 6010 msat",
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

/// The API docs: the index lists every guide, each guide answers as a
/// page and as Markdown, `llms.txt` lists the Markdown, and every site
/// link in a guide answers `200`.
#[tokio::test]
async fn the_api_docs_list_every_guide_and_serve_markdown() {
    let root = tempfile::tempdir().unwrap();
    let site = || router(config(root.path().join("tasks")));
    let (_, docs) = get(site(), "/docs").await;
    assert!(docs.contains("href=\"/docs/api\""));
    let (_, index) = get(site(), "/docs/api").await;
    let (status, headers, llms) = get_with(site(), "/docs/api/llms.txt", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    for (slug, _) in pages::API_DOCS {
        assert!(
            index.contains(&format!("href=\"/docs/api/{slug}\"")),
            "{slug}"
        );
        assert!(
            llms.contains(&format!("https://openagents.com/docs/api/{slug}.md")),
            "{slug}"
        );
        let (status, html) = get(site(), &format!("/docs/api/{slug}")).await;
        assert_eq!(status, StatusCode::OK, "{slug}");
        assert!(html.contains("Beta"), "{slug} says it's a beta");
        let main = &html[html.find("<main").unwrap()..html.find("</main>").unwrap()];
        assert!(!main.contains("{{"), "{slug} has an undrawn table");
        for target in main.split("href=\"").skip(1) {
            let target = &target[..target.find('"').unwrap()];
            if target.starts_with('/') && !target.starts_with("/static/") {
                let path = target.split('#').next().unwrap();
                let (status, _) = get(site(), path).await;
                assert_eq!(status, StatusCode::OK, "{slug} links {target}");
            }
        }
        let (status, headers, markdown) =
            get_with(site(), &format!("/docs/api/{slug}.md"), LOCAL).await;
        assert_eq!(status, StatusCode::OK, "{slug}.md");
        assert!(
            headers[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/markdown")
        );
        assert!(markdown.starts_with("# "), "{slug}.md");
        assert!(!markdown.contains("{{"), "{slug}.md has an undrawn table");
    }
    assert_eq!(get(site(), "/docs/api/nope").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        get(site(), "/docs/api/nope.md").await.0,
        StatusCode::NOT_FOUND
    );
}

/// The models page draws the rate card the gateway serves at
/// `GET /v1/rates`, every row with its list price, margin, price, and
/// sats; without a gateway it draws the published card, the card a
/// gateway with no rate overrides serves (`crates/gateway/tests/
/// inference_rates.rs` holds the gateway to that). A page and an API that
/// disagree fail here.
#[tokio::test]
async fn the_models_page_shows_the_gateways_rate_card() {
    use inference::rates::{Card, Kind, SatsRate};
    fn shows(main: &str, card: &Card) {
        let text = main.replace("<strong>", "").replace("</strong>", "");
        for row in &card.rows {
            for amount in [&row.input, &row.cached_input, &row.output] {
                let cell = match row.kind {
                    Kind::List => format!(
                        "${} + ${} = ${}",
                        amount.list_usd, amount.margin_usd, amount.price_usd
                    ),
                    Kind::Promotion => format!("${}", amount.price_usd),
                };
                let cell = match amount.price_sats {
                    Some(sats) => format!("{cell} ({sats} sats)"),
                    None => cell,
                };
                assert!(text.contains(&cell), "{} misses {cell}", row.model);
            }
        }
        let drawn = text.matches("<tr>").count();
        // Two tables: the card (a header and its rows) and the models.
        assert!(drawn > card.rows.len(), "{drawn} rows");
    }
    let root = tempfile::tempdir().unwrap();

    // No gateway: the published card, dollars only.
    let (status, html) = get(
        router(config(root.path().join("tasks"))),
        "/docs/api/models",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let published = Card::published(None);
    shows(&html, &published);
    assert!(!html.contains(" sats)"));

    // A gateway: its card, with sats and a promotion row.
    let mut rates = inference::rates::published();
    let mut glm = rates.get("zai", "zai/glm-5.3-flash").cloned().unwrap();
    glm.promotion = Some(inference::meter::Promotion {
        label: "Free this week".into(),
        input: 0,
        cached_input: None,
        output: 0,
    });
    rates.set(glm);
    let served = Card::from_rates(
        &rates,
        Some(&SatsRate {
            usd_per_btc: 100_000,
            as_of: "2026-10-09".into(),
        }),
    );
    let body = serde_json::to_string(&served).unwrap();
    let app = Router::new().route(
        "/v1/rates",
        axum::routing::get(move || {
            let body = body.clone();
            async move { ([(header::CONTENT_TYPE, "application/json")], body) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut settings = config(root.path().join("tasks"));
    settings.inference = Some(Arc::new(
        upstream::Upstream::new(&format!("http://{addr}")).unwrap(),
    ));
    let (_, html) = get(router(settings.clone()), "/docs/api/models").await;
    shows(&html, &served);
    assert!(html.contains("$100,000 per bitcoin"));
    assert!(html.contains("promotion: Free this week"));
    crate::copy_guard::assert_plain("/docs/api/models", &html);
    let (_, markdown) = get(router(settings), "/docs/api/models.md").await;
    assert!(markdown.contains("(158 sats)"), "{markdown}");
    server.abort();
}

/// `openagents.com/api/v1/...` is the API gateway's `/v1/...` (#11065):
/// the key rides along, the site's cookies stay behind, and nothing reaches
/// the legacy upstream.
#[tokio::test]
async fn the_api_alias_goes_to_the_gateway_without_cookies() {
    let api = Router::new().fallback(|request: Request<Body>| async move {
        let headers = request.headers();
        let value = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        axum::Json(json!({
            "method": request.method().as_str(),
            "uri": request.uri().to_string(),
            "authorization": value("authorization"),
            "cookie": value("cookie"),
        }))
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let root = tempfile::tempdir().unwrap();
    let (legacy, hits) = echo_upstream().await;
    let mut config = proxying(root.path(), &legacy);
    config.inference = Some(Arc::new(upstream::Upstream::new(&address).unwrap()));
    let response = router(config)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/chat/completions?x=1")
                .header(header::HOST, "openagents.com")
                .header(header::AUTHORIZATION, "Bearer oak_1.secret")
                .header(header::COOKIE, "oa_cloud_session=abc; theme=dark")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let echoed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(echoed["method"], "POST");
    assert_eq!(echoed["uri"], "/v1/chat/completions?x=1");
    assert_eq!(echoed["authorization"], "Bearer oak_1.secret");
    assert_eq!(echoed["cookie"], "");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}

/// Every route path the gateway source mounts (string literals that look
/// like one), with `{param}` segments filled in.
fn gateway_route_paths() -> Vec<String> {
    let mut files = vec![];
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../gateway/src")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let prefixes = [
        "/v1/",
        "/admin",
        "/dashboard",
        "/playground",
        "/healthz",
        "/join",
    ];
    let mut paths = std::collections::BTreeSet::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for literal in text.split('"').skip(1).step_by(2) {
            if !prefixes.iter().any(|p| literal.starts_with(p))
                || literal.contains(' ')
                || literal.len() > 120
            {
                continue;
            }
            let path = literal.split('?').next().unwrap();
            let filled: Vec<String> = path
                .split('/')
                .map(|segment| {
                    if segment.starts_with('{') {
                        "x1".to_string()
                    } else {
                        segment.to_string()
                    }
                })
                .collect();
            paths.insert(filled.join("/"));
        }
    }
    paths.into_iter().collect()
}

/// The `/api/v1` alias forwards only the PUBLIC gateway routes (#11155):
/// every route the gateway mounts is tried with every method, and only the
/// allowlist reaches the gateway; the rest answer 404.
#[tokio::test]
async fn the_api_alias_forwards_only_the_public_gateway_routes() {
    let paths = gateway_route_paths();
    for internal in [
        "/v1/admin/inference/status",
        "/admin/inference",
        "/v1/accounts",
        "/v1/account/github/token",
        "/v1/sessions/device/lookup",
        "/v1/sessions/device/decide",
        "/v1/sessions/device/paired",
        "/dashboard",
        "/playground",
    ] {
        assert!(
            paths.iter().any(|p| p == internal),
            "the scan finds {internal}"
        );
    }
    let root = tempfile::tempdir().unwrap();
    let (legacy, legacy_hits) = echo_upstream().await;
    let (gateway, gateway_hits) = echo_upstream().await;
    let mut config = proxying(root.path(), &legacy);
    config.inference = Some(Arc::new(upstream::Upstream::new(&gateway).unwrap()));
    let site = router(config);
    let mut forwarded = 0;
    let mut tried: Vec<(String, String)> = paths
        .iter()
        .flat_map(|path| {
            ["GET", "POST", "PUT", "PATCH", "DELETE"]
                .into_iter()
                .map(move |m| (m.to_string(), path.clone()))
        })
        .collect();
    tried.extend(
        crate::api_alias::tests::REFUSED
            .iter()
            .chain(crate::api_alias::tests::PUBLIC)
            .map(|(m, p)| (m.to_string(), p.to_string())),
    );
    // Only `/v1/...` is under the alias (`/api/v1/...`); the gateway's
    // other paths (`/admin`, `/dashboard`, ...) have no way through it.
    tried.retain(|(_, path)| path.starts_with("/v1/"));
    for (method, path) in tried {
        let before = gateway_hits.load(std::sync::atomic::Ordering::SeqCst);
        let response = site
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.as_str())
                    .uri(format!("/api{path}"))
                    .header(header::HOST, "openagents.com")
                    .header(header::AUTHORIZATION, "Bearer oak_1.secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let reached = gateway_hits.load(std::sync::atomic::Ordering::SeqCst) > before;
        let public = crate::api_alias::forwards(&method.parse().unwrap(), &path, false);
        assert_eq!(reached, public, "{method} {path}");
        if public {
            forwarded += 1;
        } else {
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        }
    }
    assert!(
        forwarded > 20,
        "public routes still go through ({forwarded})"
    );
    assert_eq!(legacy_hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}

/// `security.txt` names the security contact and an expiry at least a
/// month away (RFC 9116), at both the well-known and the legacy path.
#[tokio::test]
async fn security_txt_names_the_contact_and_has_not_lapsed() {
    let root = tempfile::tempdir().unwrap();
    for path in ["/.well-known/security.txt", "/security.txt"] {
        let (status, body) = get(router(config(root.path().into())), path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            body.contains("Contact: mailto:chris+security@openagents.com"),
            "{body}"
        );
        assert!(body.contains("Expires: "), "{body}");
    }
    // Production proxies every path the site doesn't own to the previous
    // server, which has no security.txt: both paths must be owned so they
    // never go upstream (a production-only 404, 2026-10-09).
    let (url, hits) = echo_upstream().await;
    let site = router(proxying(root.path(), &url));
    for path in ["/.well-known/security.txt", "/security.txt"] {
        assert!(upstream::owned(path), "{path}");
        let (status, _, body) = get_with(site.clone(), path, "openagents.com").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(body.contains("Contact: "), "{path}: {body}");
    }
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    let expires = std::time::UNIX_EPOCH
        + std::time::Duration::from_secs(
            // 2027-10-01T00:00:00Z
            1_822_348_800,
        );
    assert_eq!(crate::wellknown::SECURITY_EXPIRES, "2027-10-01T00:00:00Z");
    let month = std::time::Duration::from_secs(30 * 24 * 3600);
    assert!(
        std::time::SystemTime::now() + month < expires,
        "renew SECURITY_EXPIRES in wellknown.rs"
    );
}
