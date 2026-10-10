//! The OpenAgents website.
//!
//! One axum router serves the public pages (the homepage, the download
//! page, the terms and the privacy policy, the pairing link's landing page,
//! and profiles) and the local, read-only task browser at `/app`.
//!
//! Interactive pages include the homepage composer, flow map, Verse demos,
//! Rust component catalog, and, with an account service configured, sign-in
//! and Settings. Pages that need
//! the production account store read through [`backend::Backend`]; a
//! development server uses [`backend::Development`] and renders every page
//! without records or secrets. The design follows the private Coder
//! service's site, reimplemented here.

pub mod account;
mod account_export;
mod account_memory;
mod agent_ready;
mod agent_work;
pub mod analytics;
mod answer_ui;
mod api_alias;
mod api_keys;
pub mod ask;
mod auth;
pub mod backend;
mod chat_files;
mod chat_html;
mod chat_owner;
pub mod chat_store;
mod chat_vision;
pub mod cloud;
mod coder_sync;
mod components;
mod composer;
mod composer_row;
mod demo;
mod device;
mod docs_mcp;
mod environments;
mod github_tools;
mod layout;
mod markdown;
mod oauth;
mod older_paths;
pub mod own_runs;
mod pages;
pub mod palette;
mod payments;
mod phone_api;
pub mod pilot;
pub mod plan;
mod projects;
mod promises;
mod purchases;
pub mod sales_remote;
mod settings;
pub mod shutdown;
mod suggestions;
mod tasks;
mod terminal_connect;
pub mod theme;
mod traces;
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
    /// The inference gateway (`--inference`): `/api/v1/...` goes there as
    /// `/v1/...`, cookies removed, so `openagents.com/api/v1` is an alias
    /// of `api.openagents.com/v1` (docs/inference/gateway.md, section 3),
    /// and the API docs' models page reads its rate card (`GET /v1/rates`).
    /// Without it, `/api/v1/` answers `404`, and the models page shows the
    /// card the gateway's own adapters publish.
    pub inference: Option<Arc<upstream::Upstream>>,
    /// Staging only (`OPENAGENTS_WEB_API_OPERATOR_SIGNUP=1`): the
    /// `/api/v1` alias also forwards `POST /v1/accounts` with a bearer, for
    /// the smoke suite's operator test account ([`api_alias`]).
    pub api_operator_signup: bool,
    /// The Everglade web build and its pack (`--everglade DIR`), served
    /// under `/everglade/`. Without it, `/everglade` says Everglade is
    /// unavailable.
    pub everglade: Option<PathBuf>,
    /// The Grow Little Bunny web build (`--bunny DIR`), served under
    /// `/games/grow-little-bunny/`. Without it, the game's page says it
    /// can't be played here.
    pub bunny: Option<PathBuf>,
    /// The independently built Rust/Wasm component catalog assets.
    pub components_build: Option<PathBuf>,
    /// Explicit native account adapter; absence leaves sign-in and Settings unavailable.
    pub cloud: Option<Arc<cloud::session::CloudSession>>,
    /// GitHub sign-in (`--github-oauth`): the OAuth App's client id and
    /// callback URL. With `cloud`, the header offers Log in and Sign up and
    /// `/login` continues with GitHub (docs/auth).
    pub github: Option<Arc<oa_auth::GithubApp>>,
    /// Repository access through a GitHub App (`--github-app`): its client
    /// id and slug. With it, `/projects` installs the App on the
    /// repositories a person picks instead of asking the OAuth App for
    /// `repo` (docs/auth/github.md, "GitHub App").
    pub github_install: Option<Arc<oa_auth::AppInstall>>,
    /// Explicit account/workspace bindings to separately granted resident hosts.
    pub cloud_hosts: Option<Arc<cloud::hosts::Hosts>>,
    /// Unused since the Cloud pages left (docs/web/cloud-reset.md); the
    /// `--cloud-build` flag is still accepted so deployments keep starting.
    pub cloud_build: Option<PathBuf>,
    /// Private custody of users' own Claude credentials for their own
    /// computers (BYO-04); absence hides the Claude credential settings.
    pub cloud_byo: Option<Arc<cloud::byo::Computers>>,
    /// Optional create-only capability into the host-private sales pipeline.
    /// Without owner-accepted terms, the proposed offer has no intake form.
    pub pilot: Option<Arc<pilot::Intake>>,
    /// Repository environments set up by an agent, saved, and used by
    /// Claude Code (`--environments PRIVATE_JSON`); absence leaves the
    /// Environments pages unavailable and out of the left panel.
    pub environments: Option<Arc<coder_environment_operator::studio::Studio>>,
    /// The Pro plan and its environment meter (`--plan-meter`,
    /// `--plan-checkout`). Absent, Settings shows the plan and says hours
    /// and subscribing aren't set up on this server.
    pub plan: Option<Arc<plan::Plans>>,
    /// First-party, cookieless counts and the owner's dashboard
    /// ([`analytics`]). In memory only unless a store is configured.
    pub analytics: Arc<analytics::Analytics>,
    /// Accounts that may do agent work on a public host besides site
    /// admins (`OPENAGENTS_WEB_AGENT_ACCOUNTS`, [`agent_work`]): staging's
    /// smoke test account, which has no GitHub identity to invite.
    pub agent_accounts: Vec<String>,
    /// Starts when the server is asked to stop; open event streams end on
    /// it so a rollout can drain ([`shutdown`]).
    pub shutdown: shutdown::Shutdown,
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
            chat: Arc::new(ask::Worker::default()),
            ask_salt: secp256k1::rand::random(),
            secure_cookies: false,
            upstream: None,
            pay_upstream: None,
            inference: None,
            api_operator_signup: false,
            everglade: None,
            bunny: None,
            components_build: None,
            cloud: None,
            github: None,
            github_install: None,
            cloud_hosts: None,
            cloud_build: None,
            cloud_byo: None,
            pilot: None,
            environments: None,
            plan: None,
            analytics: Arc::new(analytics::Analytics::default()),
            agent_accounts: Vec::new(),
            shutdown: shutdown::Shutdown::default(),
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
        api: app.config.inference.is_some(),
    };
    let site_hosts = hosts.clone();
    let site = Router::new()
        .route("/api/v1/{*path}", axum::routing::any(api_proxy))
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
        .merge(chat_files::routes())
        .merge(composer::routes())
        .merge(composer_row::routes())
        .merge(demo::routes())
        .merge(environments::routes(&app))
        .merge(cloud::routes())
        .merge(auth::routes())
        .merge(device::routes())
        .merge(coder_sync::routes())
        .merge(phone_api::routes())
        .merge(own_runs::routes())
        .merge(traces::routes())
        .merge(account_memory::routes())
        .merge(terminal_connect::routes())
        .merge(account::routes())
        .merge(settings::routes())
        .merge(account_export::routes())
        .merge(projects::routes())
        .merge(github_tools::routes())
        .merge(promises::routes())
        .merge(pilot::routes())
        .merge(ask::routes())
        .merge(tasks::routes())
        .merge(wellknown::routes())
        .merge(agent_ready::routes())
        .merge(docs_mcp::routes())
        .merge(oauth::routes())
        .merge(analytics::routes())
        .route_layer(middleware::from_fn(analytics::mark))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            app.clone(),
            agent_work::gate,
        ))
        .layer(middleware::from_fn(projects::scope))
        .layer(middleware::from_fn_with_state(app.clone(), account::scope))
        .layer(middleware::from_fn_with_state(
            app.clone(),
            analytics::observe,
        ))
        .layer(middleware::from_fn(move |request, next| {
            let hosts = hosts.clone();
            async move { guard(hosts, request, next).await }
        }))
        .with_state(app);
    // Before routing, so a request for Markdown reaches the page's twin.
    Router::new()
        .fallback_service(site)
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let host = request
                .headers()
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            let ours = site_hosts.local(host) || site_hosts.public.iter().any(|p| p == host);
            agent_ready::negotiate(ours, request, next)
        }))
}

/// Set on a request that came to the local address (see [`guard`]); a
/// copy the browser sends is removed first.
pub(crate) const LOCAL_HEADER: &str = "x-openagents-local";

/// Whether the request came to the local address.
pub(crate) fn local_request(headers: &axum::http::HeaderMap) -> bool {
    headers.contains_key(LOCAL_HEADER)
}

/// The Host headers the server answers.
#[derive(Clone)]
struct Hosts {
    port: u16,
    public: Vec<String>,
    upstream: Option<Arc<upstream::Upstream>>,
    /// `/api/v1/` goes to the inference gateway.
    api: bool,
}

impl Hosts {
    fn local(&self, host: &str) -> bool {
        host == format!("127.0.0.1:{}", self.port) || host == format!("localhost:{}", self.port)
    }
}

/// Whether the request's socket peer is this computer (X-SEC-02). The
/// `Host` header is the client's to choose, so local privilege also needs
/// the connection itself to come from a loopback address. A request with
/// no peer (no `ConnectInfo`) is not local: the binary always serves with
/// `into_make_service_with_connect_info`. Unit tests drive the router
/// in-process, with no socket, and are treated as on this computer unless
/// they attach a peer.
fn loopback_peer(request: &Request) -> bool {
    match request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        Some(axum::extract::ConnectInfo(peer)) => peer.ip().to_canonical().is_loopback(),
        None => cfg!(test),
    }
}

/// Answers only the configured hosts, keeps the task browser local, sends
/// what the site doesn't own to the upstream, and sets the security headers
/// every response of its own carries.
async fn guard(hosts: Hosts, mut request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let local = hosts.local(&host) && loopback_peer(&request);
    let path = request.uri().path();
    // The task browser reads this computer's own task store, so it stays
    // on the local address.
    let browser = path == "/app" || path.starts_with("/app/");
    // Environments, Claude Code runs from a chat (#11037), and continuing a
    // Coder chat on a Cloud computer (#11050) drive machines and models on
    // this server's accounts: never forwarded, and on a public host only
    // for the people `agent_work::gate` allows (#11162).
    let agent = agent_work::path(path);
    // Intake requests can carry contact content. An unconfigured host must
    // refuse them locally rather than forwarding them to another service.
    let intake = path == "/pilot" || path.starts_with("/pilot/");
    // Cloud credentials and private work must stay on this Rust surface,
    // including when an unconfigured Host header would use the legacy proxy.
    let cloud = path == "/cloud"
        || path.starts_with("/cloud/")
        || path == "/login"
        || path == "/signup"
        || path.starts_with("/auth/")
        || path == "/device"
        || path.starts_with("/device/")
        // The OAuth authorization server (#11084).
        || path.starts_with("/oauth/")
        || path.starts_with("/v1/device/")
        || matches!(path, "/sign-in" | "/sign-out" | "/settings" | "/projects")
        || path.starts_with("/settings/")
        || path.starts_with("/projects/")
        // Uploaded traces (#11109), at their API path too (#11158).
        || path == "/api/traces"
        || path.starts_with("/api/traces/")
        || path == "/v1/traces"
        || path.starts_with("/v1/traces/");
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
    let owned = upstream::owned(path) || (hosts.api && path.starts_with("/api/v1/"));
    // A wrongly routed Cloud credential cannot become a legacy credential.
    if cloud_cookie && (!(local || public) || !owned) || native_session && !(local || public) {
        return cloud::protect(
            (StatusCode::FORBIDDEN, "Use the configured Cloud address").into_response(),
        );
    }
    if let Some(upstream) = &hosts.upstream
        && !browser
        && !intake
        && !cloud
        && !chat
        && !agent
        && (!(local || public) || !owned)
    {
        // A 401 from the keyed `/mcp` names its path-form metadata and
        // this site's authorization server (#11084).
        let mcp = public && oauth::keyed_mcp(path);
        let origin = format!("https://{host}");
        let mut response = upstream.forward(request).await;
        if mcp && response.status() == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, oauth::challenge(&origin));
        }
        return response;
    }
    if !(local || (!browser && public)) {
        return (StatusCode::FORBIDDEN, "Use the local OpenAgents address").into_response();
    }
    // Pages that link the local-only pages (a chat's environment and tasks)
    // ask [`LOCAL_HEADER`]; only this guard sets it.
    request.headers_mut().remove(LOCAL_HEADER);
    if local {
        request
            .headers_mut()
            .insert(LOCAL_HEADER, HeaderValue::from_static("1"));
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

pub(crate) async fn not_found() -> Response {
    not_found_page()
}

/// The site's ordinary not-found page.
pub(crate) fn not_found_page() -> Response {
    layout::problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

#[cfg(test)]
mod agent_ready_tests;
#[cfg(test)]
mod copy_guard;
#[cfg(test)]
mod route_owners_tests;
#[cfg(test)]
mod tests;

/// `/api/v1/...` to the API gateway as `/v1/...`. The site's cookies stay
/// behind: an API call carries only its own key.
async fn api_proxy(
    axum::extract::State(app): axum::extract::State<App>,
    mut request: Request,
) -> Response {
    let Some(upstream) = &app.config.inference else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(path) = request
        .uri()
        .path_and_query()
        .and_then(|path| path.as_str().strip_prefix("/api"))
        .and_then(|path| path.parse::<axum::http::Uri>().ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    // A public front: only the PUBLIC routes (#11155).
    let operator =
        app.config.api_operator_signup && request.headers().contains_key(header::AUTHORIZATION);
    if !api_alias::forwards(request.method(), path.path(), operator) {
        return StatusCode::NOT_FOUND.into_response();
    }
    *request.uri_mut() = path;
    request.headers_mut().remove(header::COOKIE);
    upstream.forward(request).await
}

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
