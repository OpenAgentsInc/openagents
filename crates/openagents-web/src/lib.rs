//! The OpenAgents website.
//!
//! One axum router serves the public pages (the homepage, the terms and the
//! privacy policy, the docs, the blog, the desktop download, the pairing
//! link's landing page, the release proxy, the Gym's published results, and
//! the read views of traces, the forum, the live boards, and profiles) and
//! the local, read-only task browser at `/app`.
//!
//! No page runs a script. Pages that need the production account store,
//! forum, trace intake, or fleet read through [`backend::Backend`]; a
//! development server uses [`backend::Development`] and renders every page
//! without records or secrets. The design follows the private Coder
//! service's site, reimplemented here.

pub mod backend;
mod layout;
mod markdown;
mod pages;
pub mod palette;
mod tasks;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::backend::{Backend, Development};

/// The public bucket the Coder Terminal release objects live in.
pub const DEFAULT_RELEASES_URL: &str =
    "https://storage.googleapis.com/openagentsgemini-cli-releases";

/// The policy every page is served under unless it sets a stricter one:
/// no script from anywhere, styles and images from this site only.
const SITE_POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'; \
     base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// How the server runs.
#[derive(Clone)]
pub struct Config {
    /// The local task store `/app` reads. It is never created.
    pub store: PathBuf,
    /// The port the server listens on, for the local host check.
    pub port: u16,
    /// Host headers other than `127.0.0.1:PORT` and `localhost:PORT` that
    /// may reach the public pages (for example `openagents.com`). The task
    /// browser answers only the local hosts.
    pub public_hosts: Vec<String>,
    /// The directory of the published Gym results (`index.json`,
    /// `leaderboard.v1.json`, `traces/`).
    pub published: PathBuf,
    /// The base URL `/releases/{name}` proxies.
    pub releases_url: String,
    pub backend: Arc<dyn Backend>,
}

impl Config {
    /// A development server: loopback on 4300, the development backend,
    /// and the results committed in this repository.
    #[must_use]
    pub fn development(store: PathBuf) -> Self {
        Self {
            store,
            port: 4300,
            public_hosts: Vec::new(),
            published: default_published(),
            releases_url: DEFAULT_RELEASES_URL.to_owned(),
            backend: Arc::new(Development),
        }
    }
}

/// `bench/terminal-bench/published` in this repository.
#[must_use]
pub fn default_published() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/published")
}

/// The router's shared state.
#[derive(Clone)]
pub(crate) struct App(Arc<Inner>);

pub(crate) struct Inner {
    pub config: Config,
    /// The client the release proxy and the channel pointers read through.
    pub http: Option<reqwest::Client>,
    /// The two channel pointers and when they were read.
    pub pointers: tokio::sync::Mutex<Option<(Instant, String, String)>>,
}

impl std::ops::Deref for App {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

/// The whole site.
pub fn router(config: Config) -> Router {
    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .no_gzip()
        .build()
        .ok();
    let app = App(Arc::new(Inner {
        config,
        http,
        pointers: tokio::sync::Mutex::new(None),
    }));
    let hosts = Hosts {
        port: app.config.port,
        public: app.config.public_hosts.clone(),
    };
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/static/site.css", get(stylesheet))
        .route("/favicon.svg", get(favicon))
        .route("/favicon.ico", get(favicon))
        .merge(pages::routes())
        .merge(tasks::routes())
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
}

impl Hosts {
    fn local(&self, host: &str) -> bool {
        host == format!("127.0.0.1:{}", self.port) || host == format!("localhost:{}", self.port)
    }
}

/// Answers only the configured hosts, keeps the task browser local, and
/// sets the security headers every response carries.
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
    if !(local || (!browser && hosts.public.contains(&host))) {
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

/// The site stylesheet: the palette's `:root` block, then the rules.
fn css() -> String {
    format!(
        "{}{}",
        palette::root_block(),
        include_str!("../static/site.css")
    )
}

async fn stylesheet() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        css(),
    )
        .into_response()
}

async fn favicon() -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        include_str!("../static/favicon.svg"),
    )
        .into_response()
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
