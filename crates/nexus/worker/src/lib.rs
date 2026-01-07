//! OpenAgents Nexus
//!
//! A Cloudflare Workers-based Nostr relay for the OpenAgents compute marketplace.
//!
//! ## Features
//!
//! - NIP-01: Basic protocol (EVENT, REQ, CLOSE)
//! - NIP-11: Relay information document
//! - NIP-42: Authentication (required for ALL operations)
//! - NIP-89: Handler discovery
//! - NIP-90: Data Vending Machine (DVM) job routing

use serde::Serialize;
use worker::*;

mod nip01;
mod nip28;
mod nip32;
mod nip42;
mod nip90;
mod relay_do;
mod storage;
mod subscription;

pub use relay_do::NexusRelay;

/// NIP-11 Relay Information Document
#[derive(Debug, Clone, Serialize)]
struct RelayInfo {
    name: String,
    description: String,
    pubkey: String,
    contact: String,
    supported_nips: Vec<u32>,
    software: String,
    version: String,
    limitation: RelayLimitation,
}

#[derive(Debug, Clone, Serialize)]
struct RelayLimitation {
    max_message_length: u32,
    max_subscriptions: u32,
    max_filters: u32,
    max_limit: u32,
    max_subid_length: u32,
    auth_required: bool,
    payment_required: bool,
}

/// Stats API response
#[derive(Debug, Clone, Serialize)]
struct RelayStats {
    events: EventStats,
    jobs: JobStats,
    handlers: HandlerStats,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize)]
struct EventStats {
    total: u64,
    last_24h: u64,
    by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize)]
struct JobStats {
    pending: u64,
    completed_24h: u64,
    by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize)]
struct HandlerStats {
    total: u64,
    by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize)]
struct KindCount {
    kind: u16,
    count: u64,
}

impl Default for RelayInfo {
    fn default() -> Self {
        Self {
            name: "nexus.openagents.com".to_string(),
            description: "OpenAgents Nexus relay".to_string(),
            pubkey: String::new(),
            contact: "nexus@openagents.com".to_string(),
            supported_nips: vec![1, 11, 42, 89, 90],
            software: "https://github.com/OpenAgentsInc/openagents".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            limitation: RelayLimitation {
                max_message_length: 524288, // 512 KB
                max_subscriptions: 20,
                max_filters: 10,
                max_limit: 500,
                max_subid_length: 64,
                auth_required: true,
                payment_required: false,
            },
        }
    }
}

fn get_relay_info(env: &Env) -> RelayInfo {
    let mut info = RelayInfo::default();

    if let Ok(name) = env.var("RELAY_NAME") {
        info.name = name.to_string();
    }
    if let Ok(desc) = env.var("RELAY_DESCRIPTION") {
        info.description = desc.to_string();
    }
    if let Ok(pubkey) = env.var("RELAY_PUBKEY") {
        info.pubkey = pubkey.to_string();
    }
    if let Ok(contact) = env.var("RELAY_CONTACT") {
        info.contact = contact.to_string();
    }
    if let Ok(nips) = env.var("SUPPORTED_NIPS") {
        if let Some(parsed) = parse_supported_nips(&nips.to_string()) {
            info.supported_nips = parsed;
        }
    }

    info
}

fn parse_supported_nips(raw: &str) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed.parse::<u32>() {
            Ok(value) => out.push(value),
            Err(_) => return None,
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn is_websocket_upgrade(req: &Request) -> bool {
    req.headers()
        .get("Upgrade")
        .ok()
        .flatten()
        .map(|v| v.to_lowercase() == "websocket")
        .unwrap_or(false)
}

/// Query aggregate stats from D1 database
async fn query_relay_stats(env: &Env) -> Result<RelayStats> {
    let db = env.d1("DB")?;
    let now = (js_sys::Date::now() / 1000.0) as u64;
    let day_ago = now - 86400;

    // Total events
    #[derive(serde::Deserialize)]
    struct CountRow {
        cnt: f64,
    }

    let total: u64 = db
        .prepare("SELECT COUNT(*) as cnt FROM events")
        .first::<CountRow>(None)
        .await?
        .map(|r| r.cnt as u64)
        .unwrap_or(0);

    // Events in last 24h
    let last_24h: u64 = db
        .prepare("SELECT COUNT(*) as cnt FROM events WHERE created_at > ?")
        .bind(&[(day_ago as f64).into()])?
        .first::<CountRow>(None)
        .await?
        .map(|r| r.cnt as u64)
        .unwrap_or(0);

    // Events by kind
    #[derive(serde::Deserialize)]
    struct KindRow {
        kind: f64,
        cnt: f64,
    }

    let by_kind_rows = db
        .prepare("SELECT kind, COUNT(*) as cnt FROM events GROUP BY kind ORDER BY cnt DESC LIMIT 20")
        .all()
        .await?;
    let events_by_kind: Vec<KindCount> = by_kind_rows
        .results::<KindRow>()?
        .into_iter()
        .map(|r| KindCount {
            kind: r.kind as u16,
            count: r.cnt as u64,
        })
        .collect();

    // Job requests (kinds 5000-5999)
    let job_requests: u64 = db
        .prepare("SELECT COUNT(*) as cnt FROM events WHERE kind >= 5000 AND kind < 6000 AND created_at > ?")
        .bind(&[(day_ago as f64).into()])?
        .first::<CountRow>(None)
        .await?
        .map(|r| r.cnt as u64)
        .unwrap_or(0);

    // Job results (kinds 6000-6999)
    let job_results: u64 = db
        .prepare("SELECT COUNT(*) as cnt FROM events WHERE kind >= 6000 AND kind < 7000 AND created_at > ?")
        .bind(&[(day_ago as f64).into()])?
        .first::<CountRow>(None)
        .await?
        .map(|r| r.cnt as u64)
        .unwrap_or(0);

    // Jobs by kind (5000-5999)
    let jobs_by_kind_rows = db
        .prepare("SELECT kind, COUNT(*) as cnt FROM events WHERE kind >= 5000 AND kind < 6000 GROUP BY kind ORDER BY cnt DESC LIMIT 10")
        .all()
        .await?;
    let jobs_by_kind: Vec<KindCount> = jobs_by_kind_rows
        .results::<KindRow>()?
        .into_iter()
        .map(|r| KindCount {
            kind: r.kind as u16,
            count: r.cnt as u64,
        })
        .collect();

    // Pending = requests without corresponding results (simplified: requests - results)
    let pending = job_requests.saturating_sub(job_results);

    // Handler announcements (kind 31990)
    let handlers_total: u64 = db
        .prepare("SELECT COUNT(DISTINCT pubkey) as cnt FROM events WHERE kind = 31990")
        .first::<CountRow>(None)
        .await?
        .map(|r| r.cnt as u64)
        .unwrap_or(0);

    // Handlers supporting each job kind (parse from content/tags - simplified to count by pubkey)
    let handlers_by_kind_rows = db
        .prepare("SELECT 5050 as kind, COUNT(DISTINCT pubkey) as cnt FROM events WHERE kind = 31990")
        .all()
        .await?;
    let handlers_by_kind: Vec<KindCount> = handlers_by_kind_rows
        .results::<KindRow>()?
        .into_iter()
        .map(|r| KindCount {
            kind: r.kind as u16,
            count: r.cnt as u64,
        })
        .collect();

    Ok(RelayStats {
        events: EventStats {
            total,
            last_24h,
            by_kind: events_by_kind,
        },
        jobs: JobStats {
            pending,
            completed_24h: job_results,
            by_kind: jobs_by_kind,
        },
        handlers: HandlerStats {
            total: handlers_total,
            by_kind: handlers_by_kind,
        },
        timestamp: now,
    })
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    match path {
        // NIP-11 or HUD: Non-WebSocket requests to root
        "/" if !is_websocket_upgrade(&req) => {
            // Check Accept header for application/nostr+json (NIP-11)
            let accept = req.headers().get("Accept")?.unwrap_or_default();
            if accept.contains("application/nostr+json") {
                // NIP-11: Relay Information Document
                let info = get_relay_info(&env);
                let mut headers = Headers::new();
                headers.set("Content-Type", "application/nostr+json")?;
                headers.set("Access-Control-Allow-Origin", "*")?;
                let json = serde_json::to_string(&info)?;
                Ok(Response::from_body(ResponseBody::Body(json.into_bytes()))?.with_headers(headers))
            } else {
                // Browser request: Serve HUD from assets binding
                let assets: Fetcher = env.get_binding("ASSETS")?;
                let asset_req = Request::new("https://assets/index.html", Method::Get)?;
                let http_resp = assets.fetch_request(asset_req).await?;
                Response::try_from(http_resp)
            }
        }

        // WebSocket upgrade -> route to Durable Object
        "/" | "/ws" if is_websocket_upgrade(&req) => {
            let namespace = env.durable_object("NEXUS_RELAY")?;
            let id = namespace.id_from_name("main")?;
            let stub = id.get_stub()?;
            stub.fetch_with_request(req).await
        }

        // Health check endpoint
        "/health" => {
            Response::ok("ok")
        }

        // Stats API endpoint
        "/api/stats" => {
            let stats = query_relay_stats(&env).await?;
            let mut headers = Headers::new();
            headers.set("Content-Type", "application/json")?;
            headers.set("Access-Control-Allow-Origin", "*")?;
            headers.set("Cache-Control", "public, max-age=5")?;
            Ok(Response::from_json(&stats)?.with_headers(headers))
        }

        // CORS preflight
        _ if req.method() == Method::Options => {
            let mut headers = Headers::new();
            headers.set("Access-Control-Allow-Origin", "*")?;
            headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
            headers.set("Access-Control-Allow-Headers", "Content-Type, Upgrade, Connection")?;
            Ok(Response::empty()?.with_headers(headers))
        }

        // Static assets (pkg/, static/)
        _ if path.starts_with("/pkg/") || path.starts_with("/static/") => {
            let assets: Fetcher = env.get_binding("ASSETS")?;
            let asset_req = Request::new(&format!("https://assets{}", path), Method::Get)?;
            let http_resp = assets.fetch_request(asset_req).await?;
            Response::try_from(http_resp)
        }

        // SPA fallback for browser requests, 404 for others
        _ => {
            let accept = req.headers().get("Accept")?.unwrap_or_default();
            if accept.contains("text/html") {
                let assets: Fetcher = env.get_binding("ASSETS")?;
                let asset_req = Request::new("https://assets/index.html", Method::Get)?;
                let http_resp = assets.fetch_request(asset_req).await?;
                Response::try_from(http_resp)
            } else {
                Response::error("Not Found", 404)
            }
        }
    }
}
