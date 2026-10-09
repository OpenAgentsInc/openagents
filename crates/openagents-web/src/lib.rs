//! The OpenAgents website.
//!
//! One axum router serves the public pages (the homepage, the download
//! page, the terms and the privacy policy, the pairing link's landing page,
//! and profiles) and the local, read-only task browser at `/app`.
//!
//! Interactive pages include the homepage composer, flow map, Verse demos,
//! Rust component catalog, and separately configured Cloud workspace. Pages that need
//! the production account store read through [`backend::Backend`]; a
//! development server uses [`backend::Development`] and renders every page
//! without records or secrets. The design follows the private Coder
//! service's site, reimplemented here.

pub mod ask;
pub mod backend;
mod chat_html;
pub mod chat_store;
pub mod cloud;
mod components;
mod composer;
mod demo;
mod layout;
mod markdown;
mod pages;
pub mod palette;
pub mod pilot;
mod purchases;
pub mod sales_remote;
mod tasks;
pub mod theme;
pub mod ui_page;
pub mod upstream;
mod wellknown;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::backend::{Backend, Development};

/// The policy every page is served under unless it sets a stricter one:
/// no script from anywhere, styles and images from this site only.
const SITE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
     base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// How the server runs.
#[derive(Clone)]
pub struct Config {
    /// The local task store `/app` reads. It is never created.
    pub store: PathBuf,
    /// The customer store `/app/purchases` reads (`--customer DIRECTORY`).
    /// Read only and never created; without it the page says purchases are
    /// unavailable.
    pub customer: Option<PathBuf>,
    /// The port the server listens on, for the local host check.
    pub port: u16,
    /// Host headers other than `127.0.0.1:PORT` and `localhost:PORT` that
    /// may reach the public pages (for example `openagents.com`). The task
    /// browser answers only the local hosts.
    pub public_hosts: Vec<String>,
    pub backend: Arc<dyn Backend>,
    /// Where the homepage terminal's questions are answered ([`ask`]).
    pub chat: Arc<dyn ask::Chat>,
    /// Durable public conversations, independent of a browser connection.
    pub chat_store: Arc<chat_store::Store>,
    /// Small Rust/Wasm input and scroll adapter.
    pub chat_build: Option<PathBuf>,
    /// The server's secret the visitors' signing keys are derived from.
    /// Random for each process unless set; a deployment with several
    /// instances sets one, so a visitor keeps one key.
    pub ask_salt: [u8; 32],
    /// Whether the visitor cookie is marked `Secure` (served over HTTPS).
    pub secure_cookies: bool,
    /// Where requests for paths this site doesn't own go ([`upstream`]).
    /// With one, a request on a host other than the local and public ones
    /// goes there whole, so the other names the upstream answered keep
    /// answering as before. Without one, unowned paths answer `404` and
    /// other hosts are refused.
    pub upstream: Option<Arc<upstream::Upstream>>,
    /// The pay host for same-origin public flow and stats reads.
    pub pay_upstream: Option<Arc<upstream::Upstream>>,
    /// The Everglade web build and its pack (`--everglade DIR`), served
    /// under `/everglade/`. Without it, `/everglade` says Everglade is
    /// unavailable.
    pub everglade: Option<PathBuf>,
    /// The independently built Rust/Wasm component catalog assets.
    pub components_build: Option<PathBuf>,
    /// Explicit native account adapter; absence leaves the Cloud workspace unavailable.
    pub cloud: Option<Arc<cloud::session::CloudSession>>,
    /// Explicit account/workspace bindings to separately granted resident hosts.
    pub cloud_hosts: Option<Arc<cloud::hosts::Hosts>>,
    /// Rust/Wasm private-view lifecycle assets.
    pub cloud_build: Option<PathBuf>,
    /// Explicit account/workspace delegations to the retail service.
    pub cloud_retail: Option<Arc<cloud::retail::Delegations>>,
    /// Explicit account/workspace delegations to the separate sales-owner
    /// remote adapter; absence leaves Sales unavailable.
    pub cloud_sales: Option<Arc<cloud::sales::Delegations>>,
    /// Private custody of users' own Claude credentials for their own
    /// computers (BYO-04); absence leaves the page unavailable.
    pub cloud_byo: Option<Arc<cloud::byo::Computers>>,
    /// The owner's explicit browser qualification for team controls
    /// (WEB-12); absence leaves the Team page unavailable.
    pub cloud_team: Option<Arc<cloud::team::Qualification>>,
    /// Optional create-only capability into the host-private sales pipeline.
    /// Without owner-accepted terms, the proposed offer has no intake form.
    pub pilot: Option<Arc<pilot::Intake>>,
}

impl Config {
    /// A development server: loopback on 4300 and the development backend.
    #[must_use]
    pub fn development(store: PathBuf) -> Self {
        Self {
            chat_store: Arc::new(chat_store::Store::local(store.with_file_name("web-chats"))),
            chat_build: None,
            store,
            customer: None,
            port: 4300,
            public_hosts: Vec::new(),
            backend: Arc::new(Development),
            chat: Arc::new(ask::Worker),
            ask_salt: secp256k1::rand::random(),
            secure_cookies: false,
            upstream: None,
            pay_upstream: None,
            everglade: None,
            components_build: None,
            cloud: None,
            cloud_hosts: None,
            cloud_build: None,
            cloud_retail: None,
            cloud_sales: None,
            cloud_byo: None,
            cloud_team: None,
            pilot: None,
        }
    }
}

/// The router's shared state.
#[derive(Clone)]
pub(crate) struct App(Arc<Inner>);

pub(crate) struct Inner {
    pub config: Config,
}

impl std::ops::Deref for App {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

/// The whole site.
pub fn router(config: Config) -> Router {
    let app = App(Arc::new(Inner { config }));
    let hosts = Hosts {
        port: app.config.port,
        public: app.config.public_hosts.clone(),
        upstream: app.config.upstream.clone(),
    };
    Router::new()
        .route("/api/flow/{*path}", get(pay_proxy))
        .route("/api/stats", get(pay_proxy))
        .route("/health", get(|| async { "ok" }))
        .route("/static/legacy-demo.css", get(legacy_stylesheet))
        .route(theme::STYLESHEET_PATH, get(theme::stylesheet))
        .route(theme::SCRIPT_PATH, get(theme::script))
        .route(theme::ALPINE_PATH, get(theme::alpine))
        .route(theme::TOGGLE_PATH, axum::routing::post(theme::toggle))
        .route("/static/verse-grid.jpg", get(verse_grid))
        .route("/static/chat.js", get(chat_script))
        .route("/static/flow.js", get(flow_script))
        .route("/static/everglade.js", get(everglade_script))
        .route("/ui", get(ui_catalog))
        .route("/favicon.svg", get(favicon))
        .route("/favicon.ico", get(favicon))
        .merge(pages::routes())
        .merge(purchases::routes())
        .merge(components::routes())
        .merge(chat_html::routes())
        .merge(composer::routes())
        .merge(demo::routes())
        .merge(cloud::routes())
        .merge(pilot::routes())
        .merge(ask::routes())
        .merge(tasks::routes())
        .merge(wellknown::routes())
        .fallback(not_found)
        .layer(middleware::from_fn(move |request, next| {
            let hosts = hosts.clone();
            async move { guard(hosts, request, next).await }
        }))
        .with_state(app)
}

/// The Host headers the server answers.
#[derive(Clone)]
struct Hosts {
    port: u16,
    public: Vec<String>,
    upstream: Option<Arc<upstream::Upstream>>,
}

impl Hosts {
    fn local(&self, host: &str) -> bool {
        host == format!("127.0.0.1:{}", self.port) || host == format!("localhost:{}", self.port)
    }
}

/// Answers only the configured hosts, keeps the task browser local, sends
/// what the site doesn't own to the upstream, and sets the security headers
/// every response of its own carries.
async fn guard(hosts: Hosts, request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let local = hosts.local(&host);
    let path = request.uri().path();
    let browser = path == "/app" || path.starts_with("/app/");
    // Intake requests can carry contact content. An unconfigured host must
    // refuse them locally rather than forwarding them to another service.
    let intake = path == "/pilot" || path.starts_with("/pilot/");
    // Cloud credentials and private work must stay on this Rust surface,
    // including when an unconfigured Host header would use the legacy proxy.
    let cloud = path == "/cloud" || path.starts_with("/cloud/");
    let chat = path == "/chat"
        || path.starts_with("/chat/")
        || path == "/ask"
        || path.starts_with("/composer/");
    let public = hosts.public.contains(&host);
    let cloud_cookie = request
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .any(|value| {
            value.to_str().map_or(true, |cookies| {
                cookies.split(';').any(|part| {
                    part.trim()
                        .split_once('=')
                        .is_some_and(|(name, _)| name.starts_with("oa_cloud_"))
                })
            })
        });
    let native_session = request
        .headers()
        .get_all(header::AUTHORIZATION)
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|value| {
                value.split_once(' ').is_some_and(|(kind, token)| {
                    kind.eq_ignore_ascii_case("bearer")
                        && token.trim_start_matches(' ').starts_with("sess_")
                })
            })
        });
    // A wrongly routed Cloud credential cannot become a legacy credential.
    if cloud_cookie && (!(local || public) || !upstream::owned(path))
        || native_session && !(local || public)
    {
        return cloud::protect(
            (StatusCode::FORBIDDEN, "Use the configured Cloud address").into_response(),
        );
    }
    if let Some(upstream) = &hosts.upstream
        && !browser
        && !intake
        && !cloud
        && !chat
        && (!(local || public) || !upstream::owned(path))
    {
        return upstream.forward(request).await;
    }
    if !(local || (!browser && public)) {
        return (StatusCode::FORBIDDEN, "Use the local OpenAgents address").into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(SITE_POLICY),
        );
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if !headers.contains_key(header::REFERRER_POLICY) {
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
    }
    response
}

/// The stylesheet `/demo` and the full-screen canvas pages keep (UI-13):
/// bundled font faces, Coder Noir tokens, then their rules. Every other page
/// is styled by `openagents-ui` alone.
fn legacy_css() -> String {
    with_fonts(&palette::stylesheet(include_str!(
        "../static/legacy-demo.css"
    )))
}

/// Prefix a web surface's rules with the shared system font stacks.
pub(crate) fn with_fonts(rules: &str) -> String {
    format!("{}{rules}", include_str!("../static/fonts.css"))
}

/// The homepage composer's script.
async fn chat_script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/chat.js"),
    )
        .into_response()
}

/// The `/live` page's map (#10197).
async fn flow_script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/flow.js"),
    )
        .into_response()
}

/// The `/everglade` page's loader for the wasm build (#10525).
async fn everglade_script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/everglade.js"),
    )
        .into_response()
}

async fn legacy_stylesheet() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        legacy_css(),
    )
        .into_response()
}

/// The homepage's Verse screenshot.
async fn verse_grid() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        &include_bytes!("../static/verse-grid.jpg")[..],
    )
        .into_response()
}

async fn favicon() -> Response {
    let svg = include_str!("../static/favicon.svg")
        .replace(
            "{{canvas}}",
            &format!("#{:06x}", coder_ui::coder_noir::CANVAS),
        )
        .replace(
            "{{accent}}",
            &format!("#{:06x}", coder_ui::coder_noir::ACCENT),
        );
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        svg,
    )
        .into_response()
}

/// The policy for `/ui`: the site policy plus scripts from this site, which
/// the catalog's overlays (Alpine CSP build) and copy buttons need.
const UI_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
     script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The `openagents-ui` component catalog (UI-07).
async fn ui_catalog(headers: axum::http::HeaderMap) -> Response {
    let mut response = ui_page::UiPage::new("Components")
        .path("/ui")
        .content(openagents_ui::catalog::render())
        .respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(UI_POLICY),
    );
    response
}

async fn not_found() -> Response {
    layout::problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

#[cfg(test)]
mod tests;

async fn pay_proxy(
    axum::extract::State(app): axum::extract::State<App>,
    mut request: Request,
) -> Response {
    let Some(upstream) = &app.config.pay_upstream else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let path = request
        .uri()
        .path_and_query()
        .unwrap()
        .as_str()
        .strip_prefix("/api")
        .unwrap()
        .to_owned();
    *request.uri_mut() = path.parse().expect("Stripped API path is a valid URI");
    upstream.forward(request).await
}
