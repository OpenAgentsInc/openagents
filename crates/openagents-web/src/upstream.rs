//! The upstream fallback: every request for a path this site does not own
//! is reverse-proxied to another server (`--upstream`, or
//! `OPENAGENTS_WEB_UPSTREAM`).
//!
//! On openagents.com the upstream is the previous server, the private
//! `coder` repository's `coder-serve`, which still answers the APIs, sign-in,
//! billing webhooks, MCP, the decision door, release downloads, and
//! everything else this site doesn't draw. It runs beside this server on
//! loopback (`docs/deployment/openagents-web.md`), so the request keeps its
//! original `Host`: that server builds its sign-in callbacks and checkout
//! return addresses from `Host`, and Cloud Run routes a `run.app` address by
//! `Host`, so a separate service could not be reached with it.
//!
//! The proxy streams both ways and passes the method, the headers (less the
//! hop-by-hop ones), the body, and the status through. A WebSocket or any
//! other `Upgrade` is joined end to end. The platform's `X-Forwarded-For`
//! and `X-Forwarded-Proto` pass on unchanged, so the previous server counts
//! its one trusted hop as before; when they are missing (a local run) they
//! are set from the connection. `X-Forwarded-Host` is set to the original
//! `Host`. Nothing is added to a proxied response: the site's security
//! headers are for its own pages.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::uri::{Authority, Scheme};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri, Version, header};
use axum::response::Response;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};

/// The paths this site answers itself, exactly or as a prefix. `/app`, the
/// local task browser, is never proxied; the host guard keeps it local.
const OWNED_EXACT: [&str; 49] = [
    "/",
    "/static/ui.css",
    "/static/ui.js",
    "/static/vendor/alpine-csp.js",
    "/theme",
    "/download",
    "/chat",
    "/pilot",
    "/pilot/install",
    "/install",
    "/desktop",
    "/cli/install.sh",
    "/cli/install.ps1",
    "/docs",
    "/terms",
    "/privacy",
    "/connect",
    "/live",
    "/stats",
    "/efficiency",
    "/everglade",
    "/druid",
    "/grid",
    "/ask",
    "/health",
    "/app",
    "/components",
    "/demo",
    "/cloud",
    "/.well-known/apple-app-site-association",
    "/.well-known/assetlinks.json",
    "/.well-known/agent-card.json",
    "/.well-known/agent-skills/index.json",
    crate::wellknown::SKILL_PATH,
    "/static/site.css",
    "/static/tailwind.css",
    "/static/ask.js",
    "/static/chat.js",
    "/static/htmx.min.js",
    "/static/htmx-sse.js",
    "/static/chat-start.js",
    "/static/chat-html.css",
    "/static/composer.css",
    "/static/demo-html.css",
    "/static/flow.js",
    "/static/everglade.js",
    "/static/verse-grid.jpg",
    "/favicon.svg",
    "/favicon.ico",
];

/// Owned prefixes: everything under them is this site's, found or not.
/// Only the static files above are the site's: the previous server's pages,
/// which are proxied, load their own (`/static/coder.css`,
/// `/static/webtui.css`, `/static/favicon.png`), so the rest of `/static/`
/// goes upstream.
const OWNED_PREFIXES: [&str; 9] = [
    "/docs/",
    "/demo/",
    "/app/",
    "/everglade/",
    "/pilot/",
    "/chat/",
    "/composer/",
    "/components/",
    "/cloud/",
];

/// The sections removed at the owner's direction (2026-09-29). They answer
/// `404` here and are never proxied, with everything under them.
pub(crate) const REMOVED: [&str; 9] = [
    "/forum", "/gym", "/traces", "/trace", "/earn", "/weights", "/qa", "/blog", "/doc",
];

/// Whether `path` is this site's to answer: one of its pages, or a
/// removed section that stays `404`. Anything else goes upstream.
#[must_use]
pub fn owned(path: &str) -> bool {
    path == "/api/stats"
        || path.starts_with("/api/flow/")
        || OWNED_EXACT.contains(&path)
        || OWNED_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
        || REMOVED.iter().any(|section| {
            path.strip_prefix(section)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
}

/// Headers that describe one connection, never forwarded (RFC 9110 7.6.1).
const HOP_BY_HOP: [HeaderName; 7] = [
    header::CONNECTION,
    HeaderName::from_static("keep-alive"),
    header::PROXY_AUTHENTICATE,
    header::PROXY_AUTHORIZATION,
    header::TE,
    header::TRAILER,
    header::TRANSFER_ENCODING,
];

const X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
const X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
const X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");

/// The server unowned paths are proxied to.
pub struct Upstream {
    authority: Authority,
    client: Client<HttpConnector, Body>,
}

impl std::fmt::Debug for Upstream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Upstream(http://{})", self.authority)
    }
}

impl Upstream {
    /// An upstream at `url`, `http://HOST:PORT` with no path. Plain HTTP
    /// only: the upstream runs beside this server.
    pub fn new(url: &str) -> Result<Self, String> {
        let uri: Uri = url
            .parse()
            .map_err(|_| format!("the upstream {url:?} is not a URL"))?;
        let path = uri.path_and_query().map_or("/", |path| path.as_str());
        match (uri.scheme(), uri.authority()) {
            (Some(scheme), Some(authority)) if *scheme == Scheme::HTTP && path == "/" => {
                let mut connector = HttpConnector::new();
                connector.set_nodelay(true);
                Ok(Self {
                    authority: authority.clone(),
                    client: Client::builder(TokioExecutor::new()).build(connector),
                })
            }
            _ => Err(format!(
                "the upstream {url:?} must be http://HOST:PORT with no path"
            )),
        }
    }

    /// Proxies `request` and streams the answer back.
    pub async fn forward(&self, mut request: Request) -> Response {
        let upgrade = request.headers().contains_key(header::UPGRADE);
        let downstream = upgrade.then(|| hyper::upgrade::on(&mut request));
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(peer)| peer.ip());
        let (mut parts, body) = request.into_parts();
        let path = parts.uri.path_and_query().map_or("/", |path| path.as_str());
        parts.uri = match Uri::builder()
            .scheme(Scheme::HTTP)
            .authority(self.authority.clone())
            .path_and_query(path)
            .build()
        {
            Ok(uri) => uri,
            Err(_) => return status(StatusCode::BAD_REQUEST),
        };
        parts.version = Version::HTTP_11;
        parts.extensions = Default::default();
        strip_hop_by_hop(&mut parts.headers, upgrade);
        let headers = &mut parts.headers;
        if let Some(host) = headers.get(header::HOST).cloned() {
            headers.entry(X_FORWARDED_HOST).or_insert(host);
        }
        headers
            .entry(X_FORWARDED_PROTO)
            .or_insert(HeaderValue::from_static("http"));
        if !headers.contains_key(X_FORWARDED_FOR)
            && let Some(peer) = peer.and_then(|ip| HeaderValue::from_str(&ip.to_string()).ok())
        {
            headers.insert(X_FORWARDED_FOR, peer);
        }
        let mut response = match self.client.request(Request::from_parts(parts, body)).await {
            Ok(response) => response,
            Err(error) => {
                eprintln!("upstream {}: {error}", self.authority);
                return status(StatusCode::BAD_GATEWAY);
            }
        };
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            match downstream {
                Some(downstream) => {
                    let upstream = hyper::upgrade::on(&mut response);
                    tokio::spawn(async move {
                        if let (Ok(downstream), Ok(upstream)) = tokio::join!(downstream, upstream) {
                            let _ = tokio::io::copy_bidirectional(
                                &mut TokioIo::new(downstream),
                                &mut TokioIo::new(upstream),
                            )
                            .await;
                        }
                    });
                }
                None => return status(StatusCode::BAD_GATEWAY),
            }
        } else {
            strip_hop_by_hop(response.headers_mut(), false);
        }
        let (parts, body) = response.into_parts();
        Response::from_parts(parts, Body::new(body))
    }
}

/// Removes the hop-by-hop headers and any the `Connection` header names.
/// An upgrade keeps `Connection: upgrade` and `Upgrade`.
fn strip_hop_by_hop(headers: &mut HeaderMap, upgrade: bool) {
    let named: Vec<HeaderName> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::try_from(name.trim()).ok())
        .collect();
    for name in named.iter().chain(&HOP_BY_HOP) {
        if !(upgrade && name == header::UPGRADE) {
            headers.remove(name);
        }
    }
    if upgrade {
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
    } else {
        headers.remove(header::UPGRADE);
    }
}

fn status(code: StatusCode) -> Response {
    let mut response = Response::new(Body::from(
        code.canonical_reason().unwrap_or_default().to_owned(),
    ));
    *response.status_mut() = code;
    response
}
