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

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    match path {
        // NIP-11: Relay Information Document (non-WebSocket requests to root)
        "/" if !is_websocket_upgrade(&req) => {
            // Check Accept header for application/nostr+json
            let accept = req.headers().get("Accept")?.unwrap_or_default();
            if accept.contains("application/nostr+json") {
                let info = get_relay_info(&env);
                let mut headers = Headers::new();
                headers.set("Content-Type", "application/nostr+json")?;
                headers.set("Access-Control-Allow-Origin", "*")?;
                let json = serde_json::to_string(&info)?;
                Ok(Response::from_body(ResponseBody::Body(json.into_bytes()))?.with_headers(headers))
            } else {
                // Return simple HTML for browsers
                let html = r#"<!DOCTYPE html>
<html>
<head><title>OpenAgents Nexus</title></head>
<body>
<h1>OpenAgents Nexus Relay</h1>
<p>This is a Nostr relay for the OpenAgents compute marketplace.</p>
<p>Connect with a Nostr client: <code>wss://nexus.openagents.com</code></p>
<p>Supported NIPs: 1, 11, 42, 89, 90</p>
<p><strong>Note:</strong> Authentication (NIP-42) is required for all operations.</p>
</body>
</html>"#;
                let mut headers = Headers::new();
                headers.set("Content-Type", "text/html")?;
                Ok(Response::from_body(ResponseBody::Body(html.as_bytes().to_vec()))?.with_headers(headers))
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

        // CORS preflight
        _ if req.method() == Method::Options => {
            let mut headers = Headers::new();
            headers.set("Access-Control-Allow-Origin", "*")?;
            headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
            headers.set("Access-Control-Allow-Headers", "Content-Type, Upgrade, Connection")?;
            Ok(Response::empty()?.with_headers(headers))
        }

        _ => Response::error("Not Found", 404),
    }
}
