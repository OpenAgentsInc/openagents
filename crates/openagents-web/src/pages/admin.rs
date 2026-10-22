//! `/admin`: the staff entrance (#12440), and Kiln for site admins at
//! `/admin/kiln/`.
//!
//! Who may enter is the account service's `admin` flag, the same one
//! `/admin/analytics` reads (docs/auth/github.md): it belongs to the
//! account, so every sign-in method (GitHub, password, or an emailed code)
//! gives the same answer. Signed out, `/admin` and the Kiln page send the
//! browser to `/login?return_to=...` and sign-in comes back here. Signed in
//! without admin, both answer `403` with a short page for staff; every other
//! Kiln file answers `404`.
//!
//! Kiln (`crates/kiln`, docs/kiln/production.md) is a static build that
//! `scripts/build-kiln-web.sh` writes and the image carries at `/srv/kiln`
//! (`--kiln DIR`). Every response under `/admin/kiln/` carries the
//! cross-origin isolation headers (COOP and COEP), and nothing else on the
//! site does. `config.json` is written here from the server's environment:
//! the runner's public key (`OPENAGENTS_KILN_RUNNER`), the repository
//! address patches go to (`OPENAGENTS_KILN_REPO`), and the relay
//! (`OPENAGENTS_KILN_RELAY`, default `wss://relay.openagents.com`). All
//! three are public; the runner's private key never reaches the site.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// The staff entrance.
pub(crate) const ADMIN: &str = "/admin";
/// Kiln's page. Its files are fetched relative to it, so the address ends
/// in `/`.
pub(crate) const KILN: &str = "/admin/kiln/";
/// The relay Kiln uses when the server names none.
const DEFAULT_RELAY: &str = "wss://relay.openagents.com";

/// The admin pages the hub links, with a line on each.
const PAGES: [(&str, &str, &str); 4] = [
    (
        "Kiln",
        KILN,
        "A coding agent that runs in your browser tab.",
    ),
    (
        "Analytics",
        crate::analytics::DASHBOARD,
        "Visits, chats, and errors over the last 30 days.",
    ),
    (
        "Link attribution",
        crate::attribution::SCOREBOARD,
        "Sign-ups by link, affiliate, and series.",
    ),
    (
        "Drive access",
        crate::connections::access::PAGE,
        "Who can connect Google Drive before Google approves the app.",
    ),
];

/// The Kiln build the site serves, and the public values its
/// `config.json` names.
#[derive(Clone, Debug)]
pub struct Kiln {
    pub dir: PathBuf,
    /// The `kiln-runner`'s public key, hex or npub. Empty: the page asks
    /// for one.
    pub runner: String,
    /// The NIP-34 repository address patches go to, `30617:KEY:ID`.
    pub repo: String,
    pub relay: String,
}

impl Kiln {
    /// The build at `dir`, with the runner, repository, and relay from
    /// the environment.
    #[must_use]
    pub fn from_env(dir: PathBuf) -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        Self::new(
            dir,
            &var("OPENAGENTS_KILN_RUNNER"),
            &var("OPENAGENTS_KILN_REPO"),
            &var("OPENAGENTS_KILN_RELAY"),
        )
    }

    /// The build at `dir`. A value that isn't the shape it should be is
    /// left out, so a mistake in the environment can't put anything else
    /// into the page.
    #[must_use]
    pub fn new(dir: PathBuf, runner: &str, repo: &str, relay: &str) -> Self {
        let runner = runner.trim();
        let repo = repo.trim();
        let relay = relay.trim();
        Self {
            dir,
            runner: if public_key(runner) {
                runner.into()
            } else {
                String::new()
            },
            repo: if repository(repo) {
                repo.into()
            } else {
                String::new()
            },
            relay: if relay.starts_with("wss://") && relay.len() <= 200 && safe_text(relay) {
                relay.into()
            } else {
                DEFAULT_RELAY.into()
            },
        }
    }

    fn config(&self) -> String {
        serde_json::json!({"runner": self.runner, "repo": self.repo, "relay": self.relay})
            .to_string()
    }
}

/// 64 hex digits, or an `npub1` key.
fn public_key(value: &str) -> bool {
    (value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        || (value.starts_with("npub1")
            && value.len() == 63
            && value.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// `30617:<64 hex>:<id>`.
fn repository(value: &str) -> bool {
    let mut parts = value.splitn(3, ':');
    parts.next() == Some("30617")
        && parts.next().is_some_and(public_key)
        && parts.next().is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 100
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}

fn safe_text(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\\' | '<' | '>'))
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(ADMIN, get(hub))
        .route("/admin/", get(|| async { Redirect::permanent(ADMIN) }))
        .route("/admin/kiln", get(kiln_bare))
        .route(KILN, get(kiln_page))
        .route("/admin/kiln/{*file}", get(kiln_file))
}

/// Whether `path` is one of these pages, for [`crate::upstream::owned`].
pub(crate) fn owns(path: &str) -> bool {
    path == ADMIN || path == "/admin/" || path == "/admin/kiln" || path.starts_with(KILN)
}

/// Who is asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visitor {
    SignedOut,
    Member,
    Admin,
}

async fn visitor(app: &App, headers: &HeaderMap) -> Visitor {
    let Some(service) = app.config.cloud.as_deref() else {
        return Visitor::SignedOut;
    };
    match service.authenticate(headers).await {
        Ok(viewer) if viewer.admin => Visitor::Admin,
        Ok(_) => Visitor::Member,
        Err(_) => Visitor::SignedOut,
    }
}

/// Sign in, then come back to `path`.
fn sign_in(path: &str) -> Response {
    guarded(Redirect::to(&crate::auth::login_href(path, false)).into_response())
}

/// Never cached by anyone else, indexed, or sent on as a referrer.
fn guarded(response: Response) -> Response {
    let mut response = crate::analytics::dashboard::guarded(response);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

/// The page a signed-in person without admin gets.
pub(crate) fn staff_only(headers: &HeaderMap, path: &str) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="staff-title" {
            (MarkdownRoot::new(html! {
                h1 id="staff-title" { "Staff only" }
                p.oa-page-lead { "This page is for OpenAgents staff." }
            }))
            div.oa-page-actions {
                (action_link("Go to the home page", "/"))
            }
        }
    });
    guarded(
        UiPage::new("Staff only")
            .path(path)
            .scriptless()
            .status(StatusCode::FORBIDDEN)
            .content(content)
            .respond(headers),
    )
}

/// The hub's content: one link a page.
pub(crate) fn hub_content(kiln: bool) -> maud::Markup {
    html! {
        section aria-labelledby="admin-title" {
            (MarkdownRoot::new(html! {
                h1 id="admin-title" { "Admin" }
                p.oa-page-lead { "Pages for OpenAgents staff." }
                ul {
                    @for (name, href, line) in PAGES {
                        li {
                            a href=(href) { (name) }
                            ": " (line)
                            @if href == KILN && !kiln {
                                " It isn't installed on this server."
                            }
                        }
                    }
                }
                p { "On the home page, Kiln also opens from the terminal." }
            }))
        }
    }
}

async fn hub(State(app): State<App>, headers: HeaderMap) -> Response {
    match visitor(&app, &headers).await {
        Visitor::SignedOut => sign_in(ADMIN),
        Visitor::Member => staff_only(&headers, ADMIN),
        Visitor::Admin => guarded(
            UiPage::new("Admin")
                .path(ADMIN)
                .scriptless()
                .content(PageColumn::new(hub_content(build(&app).is_some())))
                .respond(&headers),
        ),
    }
}

async fn kiln_bare(State(app): State<App>, headers: HeaderMap) -> Response {
    match visitor(&app, &headers).await {
        Visitor::SignedOut => sign_in(KILN),
        Visitor::Member => staff_only(&headers, KILN),
        Visitor::Admin => guarded(Redirect::permanent(KILN).into_response()),
    }
}

/// The build, when it holds the page.
fn build(app: &App) -> Option<&Kiln> {
    app.config
        .kiln
        .as_deref()
        .filter(|kiln| kiln.dir.join("index.html").is_file())
}

async fn kiln_page(State(app): State<App>, headers: HeaderMap) -> Response {
    match visitor(&app, &headers).await {
        Visitor::SignedOut => sign_in(KILN),
        Visitor::Member => staff_only(&headers, KILN),
        Visitor::Admin => match build(&app) {
            Some(kiln) => html_file(&kiln.dir, "index.html").await,
            None => unavailable(&headers),
        },
    }
}

async fn kiln_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    if visitor(&app, &headers).await != Visitor::Admin {
        return guarded(crate::not_found().await);
    }
    let Some(kiln) = build(&app) else {
        return guarded(crate::not_found().await);
    };
    let response = match kind(&file) {
        Some(Kind::Config) => {
            ([(header::CONTENT_TYPE, "application/json")], kiln.config()).into_response()
        }
        Some(Kind::Html) => return html_file(&kiln.dir, &file).await,
        Some(Kind::Asset(content_type, cache)) => {
            super::everglade::serve_build_cached(&kiln.dir, &file, content_type, &headers, cache)
                .await
        }
        None => crate::not_found().await,
    };
    isolated(response, &policy(&[]))
}

/// What a file under the page is.
#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Config,
    Html,
    Asset(&'static str, &'static str),
}

/// Content-addressed tools never change; everything else is checked on
/// each load.
const FRESH: &str = "private, no-cache";
const IMMUTABLE: &str = "private, max-age=31536000, immutable";

fn kind(file: &str) -> Option<Kind> {
    const JS: &str = "text/javascript; charset=utf-8";
    match file {
        "config.json" => Some(Kind::Config),
        "index.html" | "review.html" => Some(Kind::Html),
        "worker.js" | "pkg/kiln.js" | "transcript/openagents_web_kiln_transcript.js" => {
            Some(Kind::Asset(JS, FRESH))
        }
        // The transcript: the desktop's chat components drawing Kiln's run
        // (apps/openagents/web/kiln-transcript).
        "pkg/kiln_bg.wasm" | "transcript/openagents_web_kiln_transcript_bg.wasm" => {
            Some(Kind::Asset("application/wasm", FRESH))
        }
        "tools/manifest.json" => Some(Kind::Asset("application/json", FRESH)),
        _ => {
            let hash = file.strip_prefix("tools/")?.strip_suffix(".wasm")?;
            (hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            .then_some(Kind::Asset("application/wasm", IMMUTABLE))
        }
    }
}

/// An HTML file of the build, under a policy that admits its inline
/// scripts by hash.
async fn html_file(dir: &Path, file: &str) -> Response {
    let Ok(text) = tokio::fs::read_to_string(dir.join(file)).await else {
        return guarded(crate::not_found().await);
    };
    let hashes: Vec<String> = inline_scripts(&text)
        .into_iter()
        .map(super::lander5::source_hash)
        .collect();
    let response = (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, FRESH),
        ],
        text,
    )
        .into_response();
    isolated(response, &policy(&hashes))
}

/// The policy's source for an inline script, for tests.
#[cfg(test)]
pub(crate) fn script_hash(text: &str) -> String {
    super::lander5::source_hash(text)
}

/// The bodies of the inline `<script>` elements in `html`.
fn inline_scripts(html: &str) -> Vec<&str> {
    let mut scripts = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let open = &after[..open_end];
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</script>") else {
            break;
        };
        if !open.contains(" src=") {
            scripts.push(&body[..close]);
        }
        rest = &body[close..];
    }
    scripts
}

/// Kiln's policy: its own scripts and WebAssembly, the inline scripts
/// named by hash, `eval` for the worker's one-line WASI shims, the relay
/// and GitHub's public reads, and framing by this site only (the Grid's
/// Kiln panel).
fn policy(hashes: &[String]) -> String {
    let mut scripts = String::from("'self' 'wasm-unsafe-eval' 'unsafe-eval'");
    for hash in hashes {
        scripts.push(' ');
        scripts.push_str(hash);
    }
    format!(
        "default-src 'none'; script-src {scripts}; style-src 'unsafe-inline'; \
         connect-src 'self' wss: https://api.github.com https://raw.githubusercontent.com; \
         worker-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; \
         frame-ancestors 'self'"
    )
}

/// `response` with Kiln's policy and the cross-origin isolation headers.
fn isolated(response: Response, policy: &str) -> Response {
    let cache = response.headers().get(header::CACHE_CONTROL).cloned();
    let mut response = guarded(response);
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(policy) {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "cross-origin-embedder-policy",
        HeaderValue::from_static("require-corp"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    // [`guarded`] forbids storing; a build file keeps its own rule, made
    // private.
    if let Some(cache) = cache
        && let Ok(text) = cache.to_str()
        && text.starts_with("private")
    {
        headers.insert(header::CACHE_CONTROL, cache);
    }
    response
}

/// Kiln's page on a server without its build.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="kiln-title" {
            (MarkdownRoot::new(html! {
                h1 id="kiln-title" { "Kiln" }
                p.oa-page-lead { "Kiln isn't installed on this server." }
            }))
            div.oa-page-actions {
                (action_link("Back to Admin", ADMIN))
            }
        }
    });
    guarded(
        UiPage::new("Kiln")
            .path(KILN)
            .scriptless()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .content(content)
            .respond(headers),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_builds_files_are_served() {
        assert_eq!(kind("config.json"), Some(Kind::Config));
        assert_eq!(kind("index.html"), Some(Kind::Html));
        assert!(matches!(
            kind("pkg/kiln_bg.wasm"),
            Some(Kind::Asset(_, FRESH))
        ));
        let tool = format!("tools/{}.wasm", "ab".repeat(32));
        assert!(matches!(kind(&tool), Some(Kind::Asset(_, IMMUTABLE))));
        for refused in [
            "../index.html",
            "tools/../index.html",
            "tools/x.wasm",
            "pkg/other.js",
            "",
            "tools/ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB.wasm",
        ] {
            assert_eq!(kind(refused), None, "{refused}");
        }
    }

    #[test]
    fn the_inline_scripts_are_admitted_by_hash() {
        let html = "<script type=\"module\">\n  import init from './pkg/kiln.js';\n</script>\
                    <script src=\"x.js\"></script><script>b()</script>";
        assert_eq!(
            inline_scripts(html),
            ["\n  import init from './pkg/kiln.js';\n", "b()"]
        );
        let policy = policy(&[super::super::lander5::source_hash("b()")]);
        assert!(policy.starts_with("default-src 'none'; script-src 'self' 'wasm-unsafe-eval'"));
        assert!(policy.contains(&super::super::lander5::source_hash("b()")));
        assert!(policy.contains("frame-ancestors 'self'"));
        assert!(!policy.contains("script-src 'unsafe-inline'"));
    }

    #[test]
    fn the_config_holds_only_well_formed_public_values() {
        let key = "ab".repeat(32);
        let kiln = Kiln::new(
            PathBuf::new(),
            &key,
            &format!("30617:{key}:openagents-kiln-demo"),
            "",
        );
        let config: serde_json::Value = serde_json::from_str(&kiln.config()).unwrap();
        assert_eq!(config["runner"], key);
        assert_eq!(config["repo"], format!("30617:{key}:openagents-kiln-demo"));
        assert_eq!(config["relay"], DEFAULT_RELAY);
        // A value of the wrong shape is dropped.
        let wrong = Kiln::new(PathBuf::new(), "nsec1abc", "30617:x:y", "http://relay");
        assert_eq!(wrong.runner, "");
        assert_eq!(wrong.repo, "");
        assert_eq!(wrong.relay, DEFAULT_RELAY);
    }

    #[test]
    fn the_admin_pages_read_plainly() {
        let html = hub_content(true).into_string();
        for (_, href, _) in PAGES {
            assert!(html.contains(&format!("href=\"{href}\"")), "{href}");
        }
        crate::copy_guard::assert_plain(ADMIN, &html);
        crate::copy_guard::assert_plain(ADMIN, &hub_content(false).into_string());
        let staff = staff_only(&HeaderMap::new(), ADMIN);
        assert_eq!(staff.status(), StatusCode::FORBIDDEN);
    }
}
