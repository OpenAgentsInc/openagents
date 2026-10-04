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
const PAGES: [&str; 40] = [
    "/",
    "/live",
    "/everglade",
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
        // The homepage terminal (#10106) and the live map (#10197) are
        // the site's scripts.
        let script = uri == "/" || uri == "/live";
        assert_eq!(
            lower.matches("<script").count(),
            usize::from(script),
            "{uri} runs a script"
        );
        assert!(body.contains("href=\"/terms\""), "{uri} links the terms");
        assert!(body.contains("href=\"/privacy\""), "{uri} links the policy");
        assert!(body.contains("class=\"wordmark\""), "{uri} has the header");
        let policy = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(policy.starts_with("default-src 'none'"), "{uri}: {policy}");
        if script {
            assert!(policy.contains("script-src 'self'"), "{uri}: {policy}");
            assert!(policy.contains("connect-src 'self'"), "{uri}: {policy}");
            assert!(!policy.contains("unsafe"), "{uri}: {policy}");
        } else {
            assert!(!policy.contains("script-src"), "{uri}: {policy}");
        }
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
async fn the_homepage_links_one_download_page_and_leads_with_the_terminal() {
    let root = tempfile::tempdir().unwrap();
    let (_, home) = get(router(config(root.path().into())), "/").await;
    assert!(home.contains("<a class=\"button\" href=\"/download\">[ Download OpenAgents ]</a>"));
    assert!(!home.contains("/install"), "every link says /download");
    assert!(
        !home.contains(pages::MAC_DMG),
        "the download lives on /download"
    );
    assert!(!home.contains("curl ") && !home.contains("irm "));
    // The terminal (#10106): its box, its line, and its one script.
    assert!(home.contains("<h2 class=\"box-title\" id=\"term-title\">Ask OpenAgents</h2>"));
    assert!(home.contains("id=\"term-input\""));
    assert!(home.contains("<script src=\"/static/ask.js\" defer></script>"));
    // The Grid's screenshot moved to its own guide.
    assert!(!home.contains("<img"));
    let (status, grid) = get(router(config(root.path().into())), "/docs/the-grid").await;
    assert_eq!(status, StatusCode::OK);
    assert!(grid.contains("<h1>The Grid</h1>"), "{grid}");
    assert!(grid.contains(
        "<img src=\"/static/verse-grid.jpg\" alt=\"The Grid, the OpenAgents Verse world"
    ));
    let (status, headers, image) =
        get_bytes(router(config(root.path().into())), "/static/verse-grid.jpg").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(image.starts_with(&[0xff, 0xd8, 0xff]), "a JPEG");
    let (status, headers, script) =
        get_with(router(config(root.path().into())), "/static/ask.js", LOCAL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(script.contains("fetch(\"/ask\""));
    // `download` is the command; `install` is its alias.
    assert!(script.contains("download: function ()"));
    assert!(script.contains("\"/download\", \"openagents.com/download\""));
    assert!(script.contains("commands.install = commands.download;"));
    assert!(!script.contains("\"/install\""));
    assert!(
        !script.contains("innerHTML = message.text"),
        "only server-drawn HTML"
    );
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
        other_pack.as_str(),
        upper_pack.as_str(),
    ] {
        let (status, _, _) = get_bytes(router(config.clone()), uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
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
async fn the_download_page_links_only_the_release_candidates_and_the_source() {
    let root = tempfile::tempdir().unwrap();
    let (status, body) = get(router(config(root.path().into())), "/download").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        pages::MAC_DMG,
        "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/rc/1.0.0-rc.2/OpenAgents-1.0.0-rc.2.dmg"
    );
    assert!(body.contains(&format!("href=\"{}\"", pages::MAC_DMG)));
    assert!(body.contains("macOS 13 or later"));
    assert!(body.contains("<strong>Applications</strong>"));
    assert!(body.contains(&format!("<pre><code>{}</code></pre>", pages::TERMINAL_SH)));
    assert!(body.contains(&format!("<pre><code>{}</code></pre>", pages::TERMINAL_PS1)));
    assert!(body.contains(&format!(
        "<a href=\"{}\">build from source</a>",
        pages::SOURCE
    )));
    // Nothing else is downloadable here: no TestFlight, no Linux builds.
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
    assert_eq!(links, [pages::MAC_DMG, pages::SOURCE], "{body}");
    for heading in [
        "[1]</span> OpenAgents for Mac",
        "[2]</span> OpenAgents Terminal",
        "[3]</span> Everything else",
    ] {
        assert!(body.contains(heading), "{heading}");
    }
    assert!(body.contains("<title>Download OpenAgents \u{b7} OpenAgents</title>"));
    assert!(body.contains("<h1>Download OpenAgents</h1>"));
    assert!(body.contains("<a href=\"/download\" aria-current=\"page\">Download</a>"));
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

// ---------------------------------------------------------------------
// The homepage terminal's questions (#10106), answered in process.

struct Answering;

struct AnsweringDoor;

impl openagents_chat::basic_coder::Door for AnsweringDoor {
    fn ask(
        &self,
        turns: Vec<openagents_chat::basic_coder::Turn>,
        context: openagents_chat::router::Context,
        reply: Arc<std::sync::Mutex<openagents_chat::basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async move {
            // The website asks as itself, with nothing about a computer.
            let payload = openagents_chat::basic_coder::payload(&turns, &context);
            assert_eq!(payload["context"]["surface"], "web");
            assert_eq!(payload["context"]["computer_ready"], false);
            assert!(payload["context"].get("computer").is_none());
            assert_eq!(payload["client"], "openagents-web");
            assert_eq!(
                payload["instructions"],
                openagents_chat::basic_coder::INSTRUCTIONS_WEB
            );
            let mut reply = openagents_chat::basic_coder::lock(&reply);
            reply.text = format!("You asked **{}** <b>raw</b>", turns.last().unwrap().text);
            reply.done = true;
        })
    }
}

impl ask::Chat for Answering {
    fn door(
        &self,
        _secret: secp256k1::SecretKey,
    ) -> Result<Box<dyn openagents_chat::basic_coder::Door>, String> {
        Ok(Box::new(AnsweringDoor))
    }
}

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
async fn a_question_streams_its_answer_as_the_website_and_names_the_visitor() {
    let root = tempfile::tempdir().unwrap();
    let mut config = config(root.path().into());
    config.chat = Arc::new(Answering);
    let question = json!({"turns": [{"role": "user", "text": "what is OpenAgents?"}]}).to_string();
    let (status, headers, body) = post_ask(router(config.clone()), &question, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "application/x-ndjson; charset=utf-8"
    );
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("oa_visitor="), "{cookie}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"));
    let lines: Vec<serde_json::Value> = body
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let html = lines[0]["html"].as_str().unwrap();
    assert!(
        html.contains("<strong>what is OpenAgents?</strong>"),
        "{html}"
    );
    assert!(
        html.contains("&lt;b&gt;raw&lt;/b&gt;"),
        "raw HTML shows as text: {html}"
    );
    let last = lines.last().unwrap();
    assert_eq!(last["done"], true);
    assert_eq!(last["text"], "You asked **what is OpenAgents?** <b>raw</b>");
    // A visitor the site already named keeps its cookie.
    let named = cookie.split(';').next().unwrap();
    let (status, headers, _) = post_ask(router(config), &question, Some(named)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn a_question_must_end_with_the_visitor() {
    let root = tempfile::tempdir().unwrap();
    let mut config = config(root.path().into());
    config.chat = Arc::new(Answering);
    for body in [
        "not json",
        r#"{"turns":[]}"#,
        r#"{"turns":[{"role":"assistant","text":"hi"}]}"#,
        r#"{"turns":[{"role":"user","text":"   "}]}"#,
        r#"{"turns":[{"role":"system","text":"hi"}]}"#,
    ] {
        let (status, _, _) = post_ask(router(config.clone()), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}

// ---------------------------------------------------------------------
// The upstream fallback: paths the site doesn't own go to the previous
// server, which here is an in-process echo.

/// An upstream that answers every request with `418` and a JSON echo of
/// what reached it, and joins an `Upgrade: echo` on `/ws` to an echo of
/// its bytes. It counts what it was sent.
async fn echo_upstream() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
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
        "/install",
        "/desktop",
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
        "/static/site.css",
        "/static/ask.js",
        "/static/flow.js",
        "/static/everglade.js",
        "/static/verse-grid.jpg",
        "/favicon.svg",
        "/favicon.ico",
        "/app",
        "/app/tasks/x",
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
        "/static/site.css",
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
    assert!(html.contains(
        "<td class=\"stats-id\">alice</td><td>1,200.5 sats</td><td>900 sats</td><td>300.5 sats</td>"
    ));
    // Recent payouts: the author's part only, no treasury-only payout.
    let payouts = &html[html.find("id=\"stats-payouts\"").unwrap()..];
    let payouts = &payouts[..payouts.find("</table>").unwrap()];
    assert_eq!(payouts.matches("<tr>").count(), 2, "{payouts}");
    assert!(payouts.contains(&format!(
        "<td>{}</td><td>explain-error</td><td class=\"stats-id\">alice</td><td>900 sats</td>",
        pages::utc(now + 120_000)
    )));
    // The footing, the series, escaping, and no payer alias anywhere.
    assert!(html.contains("Reconciliation: the ledger matches the wallet."));
    assert!(html.contains(&format!("Last event: {} UTC.", pages::utc(now + 240_000))));
    let day = &html[html.find("id=\"stats-24h\"").unwrap()..];
    assert_eq!(
        day[..day.find("</figure>").unwrap()]
            .matches("class=\"stats-bar\"")
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
