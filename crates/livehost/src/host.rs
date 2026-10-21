//! The stable host: serves each tenant's current manifest, follows signed
//! pointers over a relay, and swaps manifests in memory.
//!
//! Nothing here is specific to one site. A tenant is a name, the public key
//! that may move its pointer, the channel it follows, and the host names
//! it answers on.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::Stream;
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::handler::{self, Handler, Runtime};
use crate::manifest::{self, Manifest};
use crate::pointer::{self, Pointer};
use crate::store::Store;
use crate::template::{Resolve, Template};
use crate::{ms_since, relay};

const CLIENT_JS: &str = include_str!("client.js");

/// The tenants a host serves, read from a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub tenants: Vec<TenantConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub name: String,
    /// The hex public key whose pointers this tenant follows.
    pub pubkey: String,
    pub channel: String,
    /// Host names (without the port) that serve this tenant. The first
    /// tenant also answers any host name no tenant lists.
    #[serde(default)]
    pub hosts: Vec<String>,
}

/// One manifest, loaded and ready to serve.
pub struct Live {
    pub hash: String,
    pub manifest: Manifest,
    pub pointer: Option<Pointer>,
    templates: HashMap<String, Template>,
    data: HashMap<String, Value>,
    handlers: HashMap<String, Arc<Handler>>,
}

impl Resolve for Live {
    fn partial(&self, name: &str) -> Option<&Template> {
        self.templates.get(name)
    }
    fn asset(&self, path: &str) -> Option<String> {
        let hash = self.manifest.objects.get(path)?;
        Some(manifest::object_url(path, hash))
    }
    fn module(&self, name: &str) -> Option<String> {
        let built = self.manifest.modules.get(name)?;
        Some(manifest::object_url("module.wasm", &built.object))
    }
}

pub struct Tenant {
    pub config: TenantConfig,
    live: RwLock<Option<Arc<Live>>>,
    /// Counts manifest swaps; each update message carries it.
    seq: std::sync::atomic::AtomicU64,
    /// Serializes manifest swaps for this tenant.
    applying: Mutex<()>,
    updates: broadcast::Sender<String>,
}

impl Tenant {
    /// The manifest serving now. A request keeps this `Arc` for its whole
    /// life, so it never mixes two versions.
    pub fn current(&self) -> Option<Arc<Live>> {
        self.live.read().expect("live manifest").clone()
    }
}

pub struct Host {
    tenants: Vec<Arc<Tenant>>,
    by_host: HashMap<String, usize>,
    store: Store,
    /// Every object any loaded manifest uses, shared by all tenants.
    objects: RwLock<HashMap<String, Bytes>>,
    /// Compressed copies by object hash, from every loaded manifest.
    encodings: RwLock<HashMap<String, BTreeMap<String, String>>>,
    runtime: Runtime,
    acks: broadcast::Sender<String>,
}

impl Host {
    pub fn new(config: HostConfig, store: Store) -> Result<Arc<Self>> {
        if config.tenants.is_empty() {
            bail!("the host config lists no tenants");
        }
        let mut by_host = HashMap::new();
        let mut tenants = Vec::new();
        for (index, tenant) in config.tenants.into_iter().enumerate() {
            pointer::check_name("tenant", &tenant.name)?;
            pointer::check_name("channel", &tenant.channel)?;
            for name in &tenant.hosts {
                if by_host.insert(name.to_ascii_lowercase(), index).is_some() {
                    bail!("two tenants list host {name}");
                }
            }
            tenants.push(Arc::new(Tenant {
                config: tenant,
                live: RwLock::new(None),
                seq: std::sync::atomic::AtomicU64::new(0),
                applying: Mutex::new(()),
                updates: broadcast::channel(64).0,
            }));
        }
        Ok(Arc::new(Host {
            tenants,
            by_host,
            store,
            objects: RwLock::new(HashMap::new()),
            encodings: RwLock::new(HashMap::new()),
            runtime: Runtime::new()?,
            acks: broadcast::channel(1024).0,
        }))
    }

    pub fn tenant(&self, name: &str) -> Option<&Arc<Tenant>> {
        self.tenants
            .iter()
            .find(|tenant| tenant.config.name == name)
    }

    /// The tenant a request reaches, and the path within it: `/t/NAME/...`
    /// names the tenant, and otherwise the `Host` header does.
    fn route<'a>(&self, headers: &HeaderMap, path: &'a str) -> (&Arc<Tenant>, String, String) {
        if let Some(rest) = path.strip_prefix("/t/") {
            let (name, inner) = rest.split_once('/').unwrap_or((rest, ""));
            if let Some(tenant) = self.tenant(name) {
                return (tenant, format!("/{inner}"), format!("/t/{name}"));
            }
        }
        (self.tenant_for(headers), path.to_owned(), String::new())
    }

    fn tenant_for(&self, headers: &HeaderMap) -> &Arc<Tenant> {
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        let index = self
            .by_host
            .get(&name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0);
        &self.tenants[index]
    }

    /// The relay filters for every tenant's pointer: the newest stored
    /// pointer from each tenant's key, and every new pointer for the
    /// tenants' channels, so pointers from other keys are seen and refused.
    pub fn filters(&self) -> Vec<Value> {
        let mut filters: Vec<Value> = self
            .tenants
            .iter()
            .map(|tenant| {
                json!({
                    "kinds": [pointer::KIND],
                    "authors": [tenant.config.pubkey],
                    "#d": [pointer::d_tag(&tenant.config.name, &tenant.config.channel)],
                    "limit": 1,
                })
            })
            .collect();
        let tags: Vec<String> = self
            .tenants
            .iter()
            .map(|tenant| pointer::d_tag(&tenant.config.name, &tenant.config.channel))
            .collect();
        filters.push(json!({"kinds": [pointer::KIND], "#d": tags, "limit": 0}));
        filters
    }

    /// Checks a pointer event and, when a tenant's key signed it, swaps in
    /// the manifest it names.
    pub async fn receive(self: &Arc<Self>, event: Event) {
        let received = Instant::now();
        let d = event
            .tags
            .iter()
            .find(|tag| tag.name() == Some("d"))
            .and_then(|tag| tag.value())
            .unwrap_or("")
            .to_owned();
        let Some(tenant) = self
            .tenants
            .iter()
            .find(|tenant| pointer::d_tag(&tenant.config.name, &tenant.config.channel) == d)
        else {
            return;
        };
        let pointer = match pointer::verify(
            &event,
            &tenant.config.pubkey,
            &tenant.config.name,
            &tenant.config.channel,
        ) {
            Ok(pointer) => pointer,
            Err(error) => {
                eprintln!("livehost: refused pointer {} for {d}: {error:#}", event.id);
                return;
            }
        };
        match self.apply(tenant, pointer, received).await {
            Ok(Some(line)) => eprintln!("livehost: {line}"),
            Ok(None) => {}
            Err(error) => eprintln!("livehost: {d}: {error:#}"),
        }
    }

    /// Loads the manifest a verified pointer names and makes it current.
    pub async fn apply(
        &self,
        tenant: &Tenant,
        pointer: Pointer,
        received: Instant,
    ) -> Result<Option<String>> {
        let _guard = tenant.applying.lock().await;
        let old = tenant.current();
        if let Some(current) = old.as_ref().and_then(|live| live.pointer.as_ref())
            && (pointer.created_at, &pointer.id) <= (current.created_at, &current.id)
        {
            return Ok(None);
        }
        let bytes = self.store.get(&pointer.manifest).await?;
        let manifest = Manifest::open(&bytes, &pointer.manifest)?;
        if manifest.tenant != tenant.config.name {
            bail!(
                "manifest {} belongs to tenant {}",
                pointer.manifest,
                manifest.tenant
            );
        }
        let fetched = Instant::now();
        let missing: Vec<String> = {
            let objects = self.objects.read().expect("objects");
            manifest
                .object_hashes()
                .into_iter()
                .filter(|hash| !objects.contains_key(hash))
                .collect()
        };
        let fetches = missing.iter().map(|hash| self.store.get(hash));
        let loaded = futures_util::future::try_join_all(fetches).await?;
        let new_objects = loaded.len();
        {
            let mut objects = self.objects.write().expect("objects");
            for (hash, bytes) in missing.into_iter().zip(loaded) {
                objects.insert(hash, Bytes::from(bytes));
            }
        }
        let fetch_ms = ms_since(fetched);
        let compiled = Instant::now();
        let live = self.load(&manifest, &pointer)?;
        let compile_ms = ms_since(compiled);
        let live = Arc::new(live);
        let from = old.as_ref().map(|live| live.hash.clone());
        let css = old
            .as_ref()
            .and_then(|old| manifest.css_patch(&old.manifest));
        {
            let mut encodings = self.encodings.write().expect("encodings");
            for (hash, copies) in &manifest.encodings {
                encodings.insert(hash.clone(), copies.clone());
            }
        }
        *tenant.live.write().expect("live manifest") = Some(live);
        let swap_ms = ms_since(received);
        let seq = tenant.seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let update = json!({
            "tenant": tenant.config.name,
            "channel": tenant.config.channel,
            "manifest": pointer.manifest,
            "seq": seq,
            "from": from,
            "css": css,
            "objects_fetched": new_objects,
            "fetch_ms": fetch_ms,
            "compile_ms": compile_ms,
            "swap_ms": swap_ms,
            "at_ms": unix_ms(),
        });
        let _ = tenant.updates.send(update.to_string());
        Ok(Some(format!(
            "{}/{} now serves {} ({new_objects} new objects, fetch {fetch_ms:.1} ms, \
             load {compile_ms:.1} ms, pointer to live {swap_ms:.1} ms)",
            tenant.config.name,
            tenant.config.channel,
            &pointer.manifest[..12]
        )))
    }

    fn load(&self, manifest: &Manifest, pointer: &Pointer) -> Result<Live> {
        let objects = self.objects.read().expect("objects");
        let object = |hash: &str| -> Result<Bytes> {
            objects
                .get(hash)
                .cloned()
                .context("object missing after fetch")
        };
        let mut templates = HashMap::new();
        let mut data = HashMap::new();
        for (path, hash) in &manifest.objects {
            if let Some(name) = path.strip_prefix("templates/") {
                let source = String::from_utf8(object(hash)?.to_vec())?;
                let template =
                    Template::parse(&source).with_context(|| format!("template {path}"))?;
                templates.insert(name.to_owned(), template);
            } else if path.ends_with(".json") {
                let value: Value = serde_json::from_slice(&object(hash)?)
                    .with_context(|| format!("data {path}"))?;
                data.insert(path.clone(), value);
            }
        }
        let mut handlers = HashMap::new();
        for (name, built) in &manifest.handlers {
            let bytes = object(&built.object)?;
            handlers.insert(name.clone(), self.runtime.load(&built.object, &bytes)?);
        }
        Ok(Live {
            hash: pointer.manifest.clone(),
            manifest: manifest.clone(),
            pointer: Some(pointer.clone()),
            templates,
            data,
            handlers,
        })
    }

    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/_o/{file}", get(object))
            .route("/_livehost/events", get(events))
            .route("/t/{tenant}/_live", get(live_events))
            .route("/t/{tenant}/_manifest/{hash}", get(manifest_document))
            .route("/_livehost/ack", post(ack))
            .route("/_livehost/acks", get(acks))
            .route("/_livehost/state", get(state))
            .fallback(page)
            .with_state(self.clone())
    }
}

/// Runs a host: follows the tenants' pointers on `relay` and serves on
/// `listen` until the process ends.
pub async fn serve(
    config: HostConfig,
    store: Store,
    relay: String,
    listen: SocketAddr,
) -> Result<()> {
    let host = Host::new(config, store)?;
    let (sender, mut receiver) = mpsc::channel(256);
    tokio::spawn(relay::follow(relay, host.filters(), sender));
    let follower = host.clone();
    tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            let host = follower.clone();
            // Each tenant swaps in its own order; tenants don't wait on
            // each other.
            tokio::spawn(async move { host.receive(event).await });
        }
    });
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .with_context(|| format!("listening on {listen}"))?;
    eprintln!("livehost: serving on http://{listen}");
    axum::serve(listener, host.router()).await?;
    Ok(())
}

fn unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
}

async fn object(
    State(host): State<Arc<Host>>,
    Path(file): Path<String>,
    headers: HeaderMap,
) -> Response {
    let (hash, ext) = file.split_once('.').unwrap_or((&file, ""));
    let accepts = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let copies = host.encodings.read().expect("encodings").get(hash).cloned();
    let objects = host.objects.read().expect("objects");
    let mut chosen = None;
    if let Some(copies) = &copies {
        for encoding in ["br", "gzip"] {
            let accepted = accepts
                .split(',')
                .any(|item| item.split(';').next().map(str::trim) == Some(encoding));
            if accepted && let Some(bytes) = copies.get(encoding).and_then(|c| objects.get(c)) {
                chosen = Some((encoding, bytes.clone()));
                break;
            }
        }
    }
    let (encoding, bytes) = match chosen {
        Some((encoding, bytes)) => (Some(encoding), bytes),
        None => match objects.get(hash) {
            Some(bytes) => (None, bytes.clone()),
            None => return (StatusCode::NOT_FOUND, "no such object").into_response(),
        },
    };
    drop(objects);
    let mut response = (
        [
            (header::CONTENT_TYPE, manifest::content_type(ext)),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        bytes,
    )
        .into_response();
    if copies.is_some() {
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    }
    if let Some(encoding) = encoding {
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static(encoding));
    }
    response
}

async fn page(
    State(host): State<Arc<Host>>,
    method: Method,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<Vec<(String, String)>>,
) -> Response {
    let (tenant, path, base) = host.route(&headers, uri.path());
    let path = path.as_str();
    let Some(live) = tenant.current() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Nothing is deployed here yet.",
        )
            .into_response();
    };
    let result = match live.manifest.routes.iter().find(|route| route.path == path) {
        Some(route) => {
            if let Some(name) = &route.template {
                render_template(
                    &live,
                    &tenant.config.name,
                    &base,
                    name,
                    route.data.as_deref(),
                )
            } else if let Some(name) = &route.handler {
                run_handler(&live, name, method.as_str(), path, query).await
            } else {
                Err(anyhow::anyhow!("route {path} has no template or handler"))
            }
        }
        // Without a route, a path names a file in the manifest, and a
        // directory its index.html.
        None => match static_file(&host, &live, path) {
            Some(file) => Ok(file),
            None => return (StatusCode::NOT_FOUND, "Page not found.").into_response(),
        },
    };
    let (status, content_type, body) = match result {
        Ok(rendered) => rendered,
        Err(error) => {
            eprintln!("livehost: {}{path}: {error:#}", tenant.config.name);
            (
                500,
                "text/plain; charset=utf-8".to_owned(),
                b"Something went wrong.".to_vec(),
            )
        }
    };
    let body = if content_type.starts_with("text/html") {
        inject(body, &live.hash, &tenant.config.name)
    } else {
        body
    };
    let mut response = (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        body,
    )
        .into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(value) = HeaderValue::from_str(&live.hash) {
        headers.insert("x-livehost-manifest", value);
    }
    response
}

/// A manifest object served by its path, as a page or a file.
fn static_file(host: &Host, live: &Live, path: &str) -> Option<(u16, String, Vec<u8>)> {
    let path = path.trim_start_matches('/');
    let candidates = if path.is_empty() || path.ends_with('/') {
        vec![format!("{path}index.html")]
    } else {
        vec![path.to_owned(), format!("{path}/index.html")]
    };
    let objects = host.objects.read().expect("objects");
    candidates.into_iter().find_map(|candidate| {
        let hash = live.manifest.objects.get(&candidate)?;
        let bytes = objects.get(hash)?;
        let ext = candidate.rsplit_once('.').map_or("", |(_, ext)| ext);
        Some((200, manifest::content_type(ext).to_owned(), bytes.to_vec()))
    })
}

fn render_template(
    live: &Live,
    tenant: &str,
    base: &str,
    name: &str,
    data: Option<&str>,
) -> Result<(u16, String, Vec<u8>)> {
    let template = live
        .templates
        .get(name)
        .with_context(|| format!("no template named {name}"))?;
    let mut value = data
        .and_then(|path| live.data.get(path))
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Value::Object(map) = &mut value {
        map.insert(
            "livehost".into(),
            json!({"manifest": live.hash, "tenant": tenant, "base": base}),
        );
    }
    let body = template.render(&value, live)?;
    Ok((200, "text/html; charset=utf-8".into(), body.into_bytes()))
}

async fn run_handler(
    live: &Arc<Live>,
    name: &str,
    method: &str,
    path: &str,
    query: Vec<(String, String)>,
) -> Result<(u16, String, Vec<u8>)> {
    let handler = live
        .handlers
        .get(name)
        .with_context(|| format!("no handler named {name}"))?
        .clone();
    let assets = live
        .manifest
        .objects
        .iter()
        .map(|(path, hash)| (path.clone(), manifest::object_url(path, hash)))
        .collect();
    let request = handler::Request {
        method: method.to_owned(),
        path: path.to_owned(),
        query,
        manifest: live.hash.clone(),
        assets,
    };
    // Instantiation and the call are synchronous; keep them off the
    // async workers.
    let response = tokio::task::spawn_blocking(move || handler.handle(&request)).await??;
    Ok((
        response.status,
        response.content_type,
        response.body.into_bytes(),
    ))
}

/// Adds the live-reload client to an HTML page.
fn inject(body: Vec<u8>, manifest: &str, tenant: &str) -> Vec<u8> {
    let Ok(mut body) = String::from_utf8(body) else {
        return Vec::new();
    };
    let script = format!(
        "<script data-manifest=\"{manifest}\" data-tenant=\"{tenant}\">{CLIENT_JS}</script>"
    );
    match body.rfind("</body>") {
        Some(at) => body.insert_str(at, &script),
        None => body.push_str(&script),
    }
    body.into_bytes()
}

type EventStream = Response;

fn stream(first: Option<String>, receiver: broadcast::Receiver<String>) -> EventStream {
    let stream = futures_util::stream::unfold((first, receiver), |(first, mut receiver)| async {
        if let Some(line) = first {
            return Some((Ok(SseEvent::default().data(line)), (None, receiver)));
        }
        loop {
            match receiver.recv().await {
                Ok(line) => return Some((Ok(SseEvent::default().data(line)), (None, receiver))),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    let stream: std::pin::Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>> =
        Box::pin(stream);
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn tenant_stream(tenant: &Tenant) -> EventStream {
    let receiver = tenant.updates.subscribe();
    let first = tenant.current().map(|live| {
        json!({
            "tenant": tenant.config.name,
            "channel": tenant.config.channel,
            "manifest": live.hash,
            "seq": tenant.seq.load(std::sync::atomic::Ordering::SeqCst),
        })
        .to_string()
    });
    stream(first, receiver)
}

/// The manifest stream for the requesting host name's tenant. The first
/// message is the current manifest, so a page that missed a swap catches
/// up.
async fn events(State(host): State<Arc<Host>>, headers: HeaderMap) -> EventStream {
    tenant_stream(host.tenant_for(&headers))
}

/// `/t/TENANT/_live`: one JSON message per pointer change, readable from
/// any origin.
async fn live_events(State(host): State<Arc<Host>>, Path(name): Path<String>) -> Response {
    let Some(tenant) = host.tenant(&name) else {
        return (StatusCode::NOT_FOUND, "No such tenant.").into_response();
    };
    let mut response = tenant_stream(tenant);
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

/// `/t/TENANT/_manifest/HASH`: a manifest document of that tenant.
async fn manifest_document(
    State(host): State<Arc<Host>>,
    Path((name, hash)): Path<(String, String)>,
) -> Response {
    let Some(tenant) = host.tenant(&name) else {
        return (StatusCode::NOT_FOUND, "No such tenant.").into_response();
    };
    let found = match host.store.get(&hash).await {
        Ok(bytes) => Manifest::open(&bytes, &hash)
            .ok()
            .filter(|manifest| manifest.tenant == tenant.config.name)
            .map(|_| bytes),
        Err(_) => None,
    };
    match found {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            ],
            bytes,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "No such manifest.").into_response(),
    }
}

/// A browser reports that it shows a manifest.
async fn ack(State(host): State<Arc<Host>>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let Ok(Value::Object(mut report)) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let named = report
        .get("tenant")
        .and_then(Value::as_str)
        .and_then(|name| host.tenant(name));
    let tenant = named.unwrap_or_else(|| host.tenant_for(&headers));
    report.insert("tenant".into(), json!(tenant.config.name));
    report.insert("at_ms".into(), json!(unix_ms()));
    let _ = host.acks.send(Value::Object(report).to_string());
    StatusCode::NO_CONTENT
}

/// Every browser report, for benchmarks.
async fn acks(State(host): State<Arc<Host>>) -> EventStream {
    stream(None, host.acks.subscribe())
}

async fn state(State(host): State<Arc<Host>>) -> axum::Json<Value> {
    let tenants: BTreeMap<String, Value> = host
        .tenants
        .iter()
        .map(|tenant| {
            let live = tenant.current();
            (
                tenant.config.name.clone(),
                json!({
                    "channel": tenant.config.channel,
                    "manifest": live.as_ref().map(|live| live.hash.clone()),
                    "pointer": live.as_ref().and_then(|live| live.pointer.as_ref()).map(|p| &p.id),
                }),
            )
        })
        .collect();
    axum::Json(json!({ "tenants": tenants }))
}
