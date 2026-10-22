//! Game tenants on livehost, served on the website (#12486): The Haunted
//! House and The Ice Cream Man at `/t/<tenant>/`, so the Grid's hosted
//! arches open them on openagents.com.
//!
//! With a relay and an object store (`OPENAGENTS_LIVE_GAMES_RELAY` and
//! `OPENAGENTS_LIVE_GAMES_STORE`, else the content tier's
//! `OPENAGENTS_LIVE_RELAY` and `OPENAGENTS_LIVE_STORE`,
//! [`crate::live_content`]) and `OPENAGENTS_LIVE_GAMES` naming each game tenant and the public key whose
//! pointers it follows (`haunt=HEX,icecream=HEX`), the server runs a
//! livehost [`Host`] for those tenants on `OPENAGENTS_LIVE_GAMES_CHANNEL`
//! (default `live`): it follows each tenant's signed pointer on the relay,
//! loads its manifest from the store, and serves the tenant's page, live
//! stream, manifests and objects. Only the tenants listed are served, and
//! each follows only its own key; the content tenant isn't among them.
//!
//! The Grid homepage stands an arch for each tenant served
//! ([`tenants`]).

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use livehost::host::{Host, HostConfig, TenantConfig};
use livehost::store::Store;
use tower::ServiceExt;

use crate::App;

/// The page policy for a game tenant's page: its own scripts and module,
/// compiling WebAssembly, and reads from this site.
const GAME_POLICY: &str = "default-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; connect-src 'self'; media-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'self'";

/// The tenant names a game tenant may have: never the content tenant.
const NOT_GAMES: [&str; 2] = [crate::live_content::TENANT, "grid"];

struct Games {
    tenants: Vec<String>,
    router: Router,
}

static GAMES: OnceLock<Games> = OnceLock::new();

/// Reads `name=HEX,name=HEX`.
///
/// # Errors
/// An entry isn't a tenant and a 64-hex key, or names the content tenant.
pub fn parse_games(spec: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for entry in spec.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (name, key) = entry
            .split_once('=')
            .ok_or_else(|| format!("{entry} isn't name=pubkey"))?;
        let (name, key) = (name.trim(), key.trim().to_ascii_lowercase());
        livehost::pointer::check_name("tenant", name).map_err(|e| e.to_string())?;
        if NOT_GAMES.contains(&name) {
            return Err(format!("{name} isn't a game tenant"));
        }
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{name}'s key isn't a 64-hex public key"));
        }
        if out.iter().any(|(n, _): &(String, String)| n == name) {
            return Err(format!("{name} is listed twice"));
        }
        out.push((name.to_owned(), key));
    }
    Ok(out)
}

/// Starts following the game tenants the environment names, on the
/// current Tokio runtime. Returns the tenants served; none without the
/// variables.
///
/// # Errors
/// A variable doesn't read, or the store can't be opened.
pub fn start_from_env() -> Result<Vec<String>, String> {
    let variable = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (Some(relay), Some(store), Some(games)) = (
        variable("OPENAGENTS_LIVE_GAMES_RELAY").or_else(|| variable("OPENAGENTS_LIVE_RELAY")),
        variable("OPENAGENTS_LIVE_GAMES_STORE").or_else(|| variable("OPENAGENTS_LIVE_STORE")),
        variable("OPENAGENTS_LIVE_GAMES"),
    ) else {
        return Ok(Vec::new());
    };
    let channel = variable("OPENAGENTS_LIVE_GAMES_CHANNEL").unwrap_or_else(|| "live".to_owned());
    let games = parse_games(&games)?;
    let store = Store::parse(&store).map_err(|e| format!("live games store: {e:#}"))?;
    let config = HostConfig {
        tenants: games
            .iter()
            .map(|(name, pubkey)| TenantConfig {
                name: name.clone(),
                pubkey: pubkey.clone(),
                channel: channel.clone(),
                hosts: Vec::new(),
            })
            .collect(),
    };
    let host = Host::new(config, store).map_err(|e| format!("live games: {e:#}"))?;
    let (sender, mut receiver) = tokio::sync::mpsc::channel(256);
    tokio::spawn(livehost::relay::follow(relay, host.filters(), sender));
    let follower = host.clone();
    tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            let host = follower.clone();
            tokio::spawn(async move { host.receive(event).await });
        }
    });
    let tenants: Vec<String> = games.into_iter().map(|(name, _)| name).collect();
    install(tenants.clone(), host);
    Ok(tenants)
}

/// Serves `host`'s tenants. The first call wins.
pub fn install(tenants: Vec<String>, host: Arc<Host>) {
    let _ = GAMES.set(Games {
        tenants,
        router: host.router(),
    });
}

/// The game tenants this server serves, in order.
#[must_use]
pub fn tenants() -> Vec<String> {
    GAMES.get().map(|g| g.tenants.clone()).unwrap_or_default()
}

/// Where the Grid asks which game tenants this server serves, to stand an
/// arch for each.
pub const GAMES_PATH: &str = "/live/games";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(
            GAMES_PATH,
            get(|| async {
                (
                    [(header::CACHE_CONTROL, "public, max-age=60")],
                    axum::Json(serde_json::json!({"tenants": tenants()})),
                )
            }),
        )
        .route("/t/{tenant}", get(tenant_page))
        .route("/t/{tenant}/", any(tenant_page))
        .route("/t/{tenant}/{*rest}", any(tenant_page))
        .route("/_o/{file}", get(forward_any))
        .route("/_livehost/ack", post(forward_any))
}

async fn forward(games: &Games, request: Request<Body>) -> Response {
    // The site's router left its own path parameters on the request; the
    // host's router reads its own.
    let (mut parts, body) = request.into_parts();
    parts.extensions = axum::http::Extensions::new();
    let request = Request::from_parts(parts, body);
    match games.router.clone().oneshot(request).await {
        Ok(mut response) => {
            let headers = response.headers_mut();
            let html = headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("text/html"));
            if html {
                headers.insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static(GAME_POLICY),
                );
                headers.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
            }
            response
        }
        Err(never) => match never {},
    }
}

async fn tenant_page(request: Request<Body>) -> Response {
    let Some(games) = GAMES.get() else {
        return crate::not_found().await;
    };
    let path = request.uri().path().to_owned();
    let name = path
        .strip_prefix("/t/")
        .map(|rest| rest.split('/').next().unwrap_or_default())
        .unwrap_or_default();
    if !games.tenants.iter().any(|t| t == name) {
        return crate::not_found().await;
    }
    if path == format!("/t/{name}") {
        return (
            StatusCode::PERMANENT_REDIRECT,
            [(header::LOCATION, format!("/t/{name}/"))],
        )
            .into_response();
    }
    forward(games, request).await
}

async fn forward_any(request: Request<Body>) -> Response {
    match GAMES.get() {
        Some(games) => forward(games, request).await,
        None => crate::not_found().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use livehost::manifest::{HOST_VERSION, Manifest, Route, SCHEMA};
    use nostr::domain::RelaySigner;

    async fn get(app: &App, uri: &str) -> (StatusCode, Option<String>, String) {
        let response = routes()
            .with_state(app.clone())
            .oneshot(axum::http::Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let policy = response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, policy, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn a_game_tenant_is_served_at_its_address_and_nothing_else_is() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::parse(dir.path().to_str().unwrap()).unwrap();
        let key = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let page = store
            .put(b"<!doctype html><canvas id=\"haunt-canvas\"></canvas></body>")
            .await
            .unwrap()
            .0;
        let mut objects = BTreeMap::new();
        objects.insert("templates/index.html".to_owned(), page);
        let manifest = Manifest {
            schema: SCHEMA.into(),
            tenant: "haunt".into(),
            host_min: HOST_VERSION,
            parent: None,
            objects,
            routes: vec![Route {
                path: "/".into(),
                template: Some("index.html".into()),
                data: None,
                handler: None,
            }],
            modules: BTreeMap::new(),
            handlers: BTreeMap::new(),
            encodings: BTreeMap::new(),
            meta: None,
        };
        let hash = store.put(&manifest.to_bytes()).await.unwrap().0;
        let host = Host::new(
            HostConfig {
                tenants: vec![TenantConfig {
                    name: "haunt".into(),
                    pubkey: key.pubkey().into(),
                    channel: "live".into(),
                    hosts: Vec::new(),
                }],
            },
            Store::parse(dir.path().to_str().unwrap()).unwrap(),
        )
        .unwrap();
        host.receive(livehost::pointer::sign(&key, "haunt", "live", &hash, None, 10).unwrap())
            .await;
        install(vec!["haunt".into()], host);
        let app = App(Arc::new(crate::Inner {
            config: crate::Config::development(dir.path().join("tasks")),
        }));
        let (status, policy, body) = get(&app, "/t/haunt/").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.contains("haunt-canvas"), "{body}");
        assert_eq!(policy.as_deref(), Some(GAME_POLICY));
        assert_eq!(
            get(&app, "/t/haunt").await.0,
            StatusCode::PERMANENT_REDIRECT
        );
        assert_eq!(
            get(&app, &format!("/t/haunt/_manifest/{hash}")).await.0,
            StatusCode::OK
        );
        assert_eq!(get(&app, "/t/openagents/").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get(&app, "/t/grid/").await.0, StatusCode::NOT_FOUND);
        assert_eq!(tenants(), ["haunt"]);
        let (_, _, listed) = get(&app, GAMES_PATH).await;
        assert_eq!(listed, r#"{"tenants":["haunt"]}"#);
    }

    #[test]
    fn game_tenants_read_and_the_content_tenant_never_does() {
        let key = "ab".repeat(32);
        let games = parse_games(&format!("haunt={key}, icecream={}", key.to_uppercase())).unwrap();
        assert_eq!(games[0], ("haunt".to_owned(), key.clone()));
        assert_eq!(games[1].1, key);
        assert!(parse_games(&format!("openagents={key}")).is_err());
        assert!(parse_games(&format!("grid={key}")).is_err());
        assert!(parse_games("haunt=xyz").is_err());
        assert!(parse_games(&format!("haunt={key},haunt={key}")).is_err());
        assert!(parse_games("").unwrap().is_empty());
    }
}
