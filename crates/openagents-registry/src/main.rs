use anyhow::{Context, Result};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use clap::Parser;
use nostr::encode_npub;
use nostr::Event;
use nostr::nip_sa::profile::{AgentProfileContent, AutonomyLevel, KIND_AGENT_PROFILE};
use nostr::nsec_to_private_key;
use nostr_client::{RelayConnection, RelayMessage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

static INDEX_HTML: &str = include_str!("../assets/index.html");

#[derive(Parser, Debug)]
#[command(name = "openagents-registry")]
#[command(about = "OpenAgents public agent directory (Nostr NIP-SA profiles)")]
struct Args {
    /// Bind address (e.g. 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,

    /// Comma-separated relay URLs to index
    #[arg(long, default_value = "wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol")]
    relays: String,

    /// Max events to request per relay
    #[arg(long, default_value = "500")]
    limit: usize,

    /// Per-relay fetch timeout in seconds
    #[arg(long, default_value = "4")]
    relay_timeout_secs: u64,

    /// Refresh interval in seconds
    #[arg(long, default_value = "60")]
    refresh_secs: u64,

    /// NIP-42 auth key for authenticated relays (hex32 or nsec).
    ///
    /// Tip: prefer setting via env over CLI to avoid leaking in shell history.
    #[arg(long)]
    auth_key: Option<String>,

    /// Read the NIP-42 auth key from a file (hex32 or nsec). Overrides env.
    #[arg(long)]
    auth_key_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct RegistryCache {
    agents: Vec<AgentEntry>,
    last_updated_at: Option<u64>,
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct AppState {
    cache: Arc<RwLock<RegistryCache>>,
    relays: Vec<String>,
    limit: usize,
    relay_timeout: Duration,
    refresh_interval: Duration,
    auth_key: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize)]
struct AgentEntry {
    pubkey: String,
    npub: Option<String>,
    name: String,
    about: String,
    picture: Option<String>,
    capabilities: Vec<String>,
    autonomy_level: AutonomyLevel,
    version: String,
    lud16: Option<String>,
    operator: Option<String>,
    created_at: u64,
    event_id: String,
}

#[derive(Debug, Serialize)]
struct AgentsResponse {
    agents: Vec<AgentEntry>,
    last_updated_at: Option<u64>,
    last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentsQuery {
    q: Option<String>,
    autonomy: Option<String>,
    capability: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let bind_addr: SocketAddr = args
        .bind
        .parse()
        .with_context(|| format!("invalid --bind value: {}", args.bind))?;

    let relays = args
        .relays
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    if relays.is_empty() {
        anyhow::bail!("no relays configured (use --relays)");
    }

    let auth_key = load_auth_key(&args)?;

    let app_state = Arc::new(AppState {
        cache: Arc::new(RwLock::new(RegistryCache::default())),
        relays,
        limit: args.limit,
        relay_timeout: Duration::from_secs(args.relay_timeout_secs),
        refresh_interval: Duration::from_secs(args.refresh_secs),
        auth_key,
    });

    // Prime cache and then refresh periodically.
    {
        let state = Arc::clone(&app_state);
        tokio::spawn(async move {
            refresh_loop(state).await;
        });
    }

    let app = axum::Router::new()
        .route("/", get(root_redirect))
        .route("/registry", get(registry_page))
        .route("/registry/api/agents", get(api_agents))
        .route("/registry/api/health", get(api_health))
        .with_state(app_state);

    tracing::info!(bind = %bind_addr, "openagents-registry listening");
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn refresh_loop(state: Arc<AppState>) {
    // Do an immediate refresh, then tick.
    if let Err(e) = refresh_once(Arc::clone(&state)).await {
        tracing::warn!(error = %e, "registry refresh failed");
    }

    let mut tick = tokio::time::interval(state.refresh_interval);
    loop {
        tick.tick().await;
        if let Err(e) = refresh_once(Arc::clone(&state)).await {
            tracing::warn!(error = %e, "registry refresh failed");
        }
    }
}

async fn refresh_once(state: Arc<AppState>) -> Result<()> {
    let fetched = fetch_profiles_from_relays(
        &state.relays,
        state.limit,
        state.relay_timeout,
        state.auth_key,
    )
    .await;

    let mut cache = state.cache.write().await;
    match fetched {
        Ok(agents) => {
            cache.agents = agents;
            cache.last_updated_at = Some(now_secs());
            cache.last_error = None;
        }
        Err(e) => {
            cache.last_error = Some(e.to_string());
        }
    }
    Ok(())
}

async fn fetch_profiles_from_relays(
    relays: &[String],
    limit: usize,
    per_relay_timeout: Duration,
    auth_key: Option<[u8; 32]>,
) -> Result<Vec<AgentEntry>> {
    let mut handles = Vec::new();
    for url in relays.iter().cloned() {
        let key = auth_key;
        let handle = tokio::spawn(async move {
            fetch_profile_events(&url, limit, per_relay_timeout, key).await
        });
        handles.push(handle);
    }

    let mut all_events: Vec<Event> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for handle in handles {
        match handle.await {
            Ok(Ok(events)) => all_events.extend(events),
            Ok(Err(e)) => errors.push(e.to_string()),
            Err(e) => errors.push(e.to_string()),
        }
    }

    // Deduplicate to the latest profile per pubkey.
    let mut latest_by_pubkey: HashMap<String, Event> = HashMap::new();
    for ev in all_events {
        if ev.kind != KIND_AGENT_PROFILE {
            continue;
        }
        if !has_d_profile(&ev.tags) {
            continue;
        }
        match latest_by_pubkey.get(&ev.pubkey) {
            Some(existing) if existing.created_at >= ev.created_at => {}
            _ => {
                latest_by_pubkey.insert(ev.pubkey.clone(), ev);
            }
        }
    }

    let mut agents = Vec::new();
    for ev in latest_by_pubkey.into_values() {
        if let Some(agent) = agent_from_event(&ev) {
            agents.push(agent);
        }
    }

    agents.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // If we got nothing at all, bubble up errors so operators can see relay issues quickly.
    if agents.is_empty() && !errors.is_empty() {
        anyhow::bail!(errors.join(" | "));
    }

    Ok(agents)
}

async fn fetch_profile_events(
    url: &str,
    limit: usize,
    timeout: Duration,
    auth_key: Option<[u8; 32]>,
) -> Result<Vec<Event>> {
    let relay = RelayConnection::new(url)?;
    if let Some(key) = auth_key {
        relay.set_auth_key(key).await;
    }
    relay.connect().await?;

    // A unique subscription id per relay fetch.
    let sub_id = format!("registry-{}", uuid::Uuid::now_v7());
    let filters = vec![json!({
        "kinds":[KIND_AGENT_PROFILE],
        "#d":["profile"],
        "limit": limit
    })];

    relay.subscribe(&sub_id, &filters).await?;

    let mut out = Vec::new();
    let start = Instant::now();

    loop {
        let remaining = timeout
            .checked_sub(start.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if remaining.is_zero() {
            break;
        }

        let next = tokio::time::timeout(remaining, relay.recv()).await;
        match next {
            Ok(Ok(Some(msg))) => match msg {
                RelayMessage::Event(sid, ev) if sid == sub_id => out.push(ev),
                RelayMessage::Eose(sid) if sid == sub_id => break,
                _ => {}
            },
            Ok(Ok(None)) => break,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => break,
        }
    }

    // Best-effort cleanup.
    if let Err(e) = relay.unsubscribe(&sub_id).await {
        tracing::debug!(relay = %url, error = %e, "unsubscribe failed");
    }
    if let Err(e) = relay.disconnect().await {
        tracing::debug!(relay = %url, error = %e, "disconnect failed");
    }

    Ok(out)
}

fn load_auth_key(args: &Args) -> Result<Option<[u8; 32]>> {
    if let Some(path) = args.auth_key_file.as_ref() {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read auth key file: {}", path.display()))?;
        return Ok(Some(parse_auth_key(raw.trim())?));
    }
    if let Some(raw) = args.auth_key.as_ref() {
        return Ok(Some(parse_auth_key(raw.trim())?));
    }

    if let Ok(raw) = std::env::var("OPENAGENTS_REGISTRY_AUTH_KEY") {
        let raw = raw.trim().to_string();
        if !raw.is_empty() {
            return Ok(Some(parse_auth_key(&raw)?));
        }
    }

    Ok(None)
}

fn parse_auth_key(raw: &str) -> Result<[u8; 32]> {
    let s = raw.trim();
    if s.is_empty() {
        anyhow::bail!("auth key is empty");
    }

    if s.starts_with("nsec") {
        return nsec_to_private_key(s).map_err(|e| anyhow::anyhow!(e.to_string()));
    }

    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).with_context(|| "invalid auth key hex")?;
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .context("auth key must be 32 bytes")?;
    Ok(bytes)
}

fn agent_from_event(ev: &Event) -> Option<AgentEntry> {
    let content = AgentProfileContent::from_json(&ev.content).ok()?;

    let npub = npub_from_hex_pubkey(&ev.pubkey).ok();
    let operator = first_tag_value(&ev.tags, "operator");
    let lud16 = first_tag_value(&ev.tags, "lud16");

    Some(AgentEntry {
        pubkey: ev.pubkey.clone(),
        npub,
        name: content.name,
        about: content.about,
        picture: content.picture,
        capabilities: content.capabilities,
        autonomy_level: content.autonomy_level,
        version: content.version,
        lud16,
        operator,
        created_at: ev.created_at,
        event_id: ev.id.clone(),
    })
}

fn has_d_profile(tags: &[Vec<String>]) -> bool {
    tags.iter()
        .any(|t| t.len() >= 2 && t[0] == "d" && t[1] == "profile")
}

fn first_tag_value(tags: &[Vec<String>], key: &str) -> Option<String> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == key)
        .and_then(|t| t.get(1))
        .map(|s| s.to_string())
}

fn npub_from_hex_pubkey(pubkey_hex: &str) -> Result<String> {
    let raw = hex::decode(pubkey_hex)
        .with_context(|| format!("invalid pubkey hex: {}", pubkey_hex))?;
    let bytes: [u8; 32] = raw
        .as_slice()
        .try_into()
        .context("pubkey must be 32 bytes")?;
    encode_npub(&bytes).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn now_secs() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    }
}

async fn root_redirect() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        HeaderValue::from_static("/registry"),
    );
    (StatusCode::TEMPORARY_REDIRECT, headers)
}

async fn registry_page() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn api_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cache = state.cache.read().await;
    Json(json!({
        "ok": true,
        "last_updated_at": cache.last_updated_at,
        "agent_count": cache.agents.len(),
        "last_error": cache.last_error,
    }))
}

async fn api_agents(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AgentsQuery>,
) -> Response {
    let cache = state.cache.read().await;
    let mut agents = cache.agents.clone();

    if let Some(cap) = query.capability.as_ref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        agents.retain(|a| a.capabilities.iter().any(|c| c.to_lowercase().contains(&cap)));
    }

    if let Some(aut) = query.autonomy.as_ref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        agents.retain(|a| format!("{:?}", a.autonomy_level).to_lowercase() == aut);
    }

    if let Some(q) = query.q.as_ref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        agents.retain(|a| {
            let hay = format!(
                "{} {} {} {} {}",
                a.name,
                a.about,
                a.capabilities.join(" "),
                a.npub.clone().unwrap_or_default(),
                a.lud16.clone().unwrap_or_default(),
            )
            .to_lowercase();
            hay.contains(&q)
        });
    }

    // Provide a stable response schema for the frontend.
    let body = AgentsResponse {
        agents,
        last_updated_at: cache.last_updated_at,
        last_error: cache.last_error.clone(),
    };

    Json(body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pubkey_hex() -> String {
        // 32 bytes of zeros.
        "00".repeat(32)
    }

    fn mk_event(kind: u16, tags: Vec<Vec<String>>, content: String) -> Event {
        Event {
            id: "e".repeat(64),
            pubkey: test_pubkey_hex(),
            created_at: 123,
            kind,
            tags,
            content,
            sig: "s".repeat(128),
        }
    }

    #[test]
    fn d_profile_tag_is_detected() {
        let tags = vec![vec!["d".to_string(), "profile".to_string()]];
        assert!(has_d_profile(&tags));
    }

    #[test]
    fn agent_profile_event_parses_into_entry() {
        let content = AgentProfileContent::new(
            "TestBot",
            "Does tests",
            AutonomyLevel::Bounded,
            "1.2.3",
        )
        .with_capabilities(vec!["research".to_string(), "summarization".to_string()]);

        let json_content = match content.to_json() {
            Ok(v) => v,
            Err(e) => {
                assert!(false, "to_json failed: {e}");
                return;
            }
        };

        let tags = vec![
            vec!["d".to_string(), "profile".to_string()],
            vec!["operator".to_string(), "abc".to_string()],
            vec!["lud16".to_string(), "bot@example.com".to_string()],
        ];

        let ev = mk_event(KIND_AGENT_PROFILE, tags, json_content);
        let entry = agent_from_event(&ev);
        let Some(entry) = entry else {
            assert!(false, "expected Some(entry)");
            return;
        };
        assert_eq!(entry.name, "TestBot");
        assert_eq!(entry.autonomy_level, AutonomyLevel::Bounded);
        assert_eq!(entry.lud16, Some("bot@example.com".to_string()));
        assert_eq!(entry.operator, Some("abc".to_string()));
        assert_eq!(entry.capabilities.len(), 2);
        assert!(entry.npub.is_some());
    }

    #[test]
    fn auth_key_parses_hex() {
        let k = parse_auth_key(&"00".repeat(32)).expect("parse hex auth key");
        assert_eq!(k, [0u8; 32]);
    }

    #[test]
    fn auth_key_parses_nsec() {
        let k = [0x42u8; 32];
        let nsec = nostr::private_key_to_nsec(&k).expect("encode nsec");
        let parsed = parse_auth_key(&nsec).expect("parse nsec");
        assert_eq!(parsed, k);
    }
}
