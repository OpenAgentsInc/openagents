//! Network discovery - internet, relays, Pylon providers.

use crate::manifest::{NetworkManifest, RelayStatus};
use nostr::KIND_HANDLER_INFO;
use nostr_client::{RelayConnection, RelayMessage};
use serde_json::json;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::timeout;
/// Default relays to probe.
const DEFAULT_RELAYS: &[&str] = &[
    "wss://nexus.openagents.com",
    "wss://relay.damus.io",
    "wss://nos.lol",
];


/// Discover network connectivity.
pub async fn discover_network() -> anyhow::Result<NetworkManifest> {
    // Check internet connectivity
    let has_internet = check_internet().await;

    // Check relay connectivity (only if internet is available)
    let relays = if has_internet {
        probe_relays().await
    } else {
        Vec::new()
    };

    // Discover providers on connected relays
    let discovery = if relays.iter().any(|r| r.connected) {
        discover_providers(&relays).await
    } else {
        ProviderDiscovery {
            total_providers: 0,
            pylon_count: 0,
            pylons_online: 0,
            pylon_pubkeys: Vec::new(),
        }
    };

    Ok(NetworkManifest {
        has_internet,
        relays,
        total_providers: discovery.total_providers,
        pylon_count: discovery.pylon_count,
        pylons_online: discovery.pylons_online,
        pylon_pubkeys: discovery.pylon_pubkeys,
    })
}

/// Check basic internet connectivity.
async fn check_internet() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    client
        .get("https://1.1.1.1/")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Probe default relays for connectivity.
async fn probe_relays() -> Vec<RelayStatus> {
    let mut relays = Vec::new();

    for url in DEFAULT_RELAYS {
        let status = probe_relay(url).await;
        relays.push(status);
    }

    relays
}

/// Probe a single relay via WebSocket.
async fn probe_relay(url: &str) -> RelayStatus {
    let start = std::time::Instant::now();

    // Try to connect via WebSocket
    let relay = match RelayConnection::new(url) {
        Ok(r) => r,
        Err(_) => {
            return RelayStatus {
                url: url.to_string(),
                connected: false,
                latency_ms: None,
            }
        }
    };

    // Attempt connection with timeout
    let connected = match timeout(Duration::from_secs(5), relay.connect()).await {
        Ok(Ok(())) => true,
        _ => false,
    };

    let latency_ms = if connected {
        Some(start.elapsed().as_millis() as u32)
    } else {
        None
    };

    // Disconnect after probe
    if connected {
        let _ = relay.disconnect().await;
    }

    RelayStatus {
        url: url.to_string(),
        connected,
        latency_ms,
    }
}

/// Result of provider discovery
struct ProviderDiscovery {
    /// Total unique providers (all NIP-89 handlers)
    total_providers: u32,
    /// Pylon providers specifically
    pylon_count: u32,
    /// Pylons that are online (active in last 10 minutes)
    pylons_online: u32,
    /// Pylon pubkeys
    pylon_pubkeys: Vec<String>,
}

/// Discover providers by querying for NIP-89 handler announcements.
async fn discover_providers(relays: &[RelayStatus]) -> ProviderDiscovery {
    let mut all_pubkeys: HashSet<String> = HashSet::new();
    let mut pylon_pubkeys: HashSet<String> = HashSet::new();

    // Try all connected relays (some may require auth)
    for relay_status in relays.iter().filter(|r| r.connected) {
        let relay_url = &relay_status.url;

        let relay = match RelayConnection::new(relay_url) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if relay.connect().await.is_err() {
            continue;
        }

        // Subscribe to NIP-89 handler info events (kind:31990)
        let filters = vec![json!({
            "kinds": [KIND_HANDLER_INFO],
            "limit": 200
        })];

        if relay.subscribe("provider-discovery", &filters).await.is_err() {
            let _ = relay.disconnect().await;
            continue;
        }

        // Collect events with timeout
        let _ = timeout(Duration::from_secs(3), async {
            loop {
                match relay.recv().await {
                    Ok(Some(RelayMessage::Event(_sub_id, event))) => {
                        // Count all providers
                        all_pubkeys.insert(event.pubkey.clone());

                        // Check if this is a Pylon (OpenAgents provider)
                        if is_pylon_provider(&event.content) {
                            pylon_pubkeys.insert(event.pubkey.clone());
                        }
                    }
                    Ok(Some(RelayMessage::Eose(_))) => break,
                    Ok(Some(_)) => continue,
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        })
        .await;

        let _ = relay.unsubscribe("provider-discovery").await;
        let _ = relay.disconnect().await;
    }

    let pylon_pubkeys_vec: Vec<String> = pylon_pubkeys.into_iter().collect();

    // Check liveness for each Pylon (have they been active in last 10 minutes?)
    let pylons_online = check_pylon_liveness(relays, &pylon_pubkeys_vec).await;

    ProviderDiscovery {
        total_providers: all_pubkeys.len() as u32,
        pylon_count: pylon_pubkeys_vec.len() as u32,
        pylons_online,
        pylon_pubkeys: pylon_pubkeys_vec,
    }
}

/// Check which Pylons are online by looking for recent activity.
async fn check_pylon_liveness(relays: &[RelayStatus], pylon_pubkeys: &[String]) -> u32 {
    if pylon_pubkeys.is_empty() {
        return 0;
    }

    let mut online_pubkeys: HashSet<String> = HashSet::new();

    // Query for any events from Pylon pubkeys in the last 10 minutes
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() - 600) // 10 minutes ago
        .unwrap_or(0);

    for relay_status in relays.iter().filter(|r| r.connected) {
        let relay = match RelayConnection::new(&relay_status.url) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if relay.connect().await.is_err() {
            continue;
        }

        // Query recent events from Pylon pubkeys
        let filters = vec![json!({
            "authors": pylon_pubkeys,
            "since": since,
            "limit": 50
        })];

        if relay.subscribe("liveness-check", &filters).await.is_err() {
            let _ = relay.disconnect().await;
            continue;
        }

        let _ = timeout(Duration::from_secs(2), async {
            loop {
                match relay.recv().await {
                    Ok(Some(RelayMessage::Event(_sub_id, event))) => {
                        online_pubkeys.insert(event.pubkey.clone());
                    }
                    Ok(Some(RelayMessage::Eose(_))) => break,
                    Ok(Some(_)) => continue,
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        })
        .await;

        let _ = relay.unsubscribe("liveness-check").await;
        let _ = relay.disconnect().await;
    }

    online_pubkeys.len() as u32
}

/// Check if an event content indicates a Pylon (OpenAgents) provider.
fn is_pylon_provider(content: &str) -> bool {
    // Pylon publishes with "OpenAgents" in the name or "openagents.com" website
    let content_lower = content.to_lowercase();
    content_lower.contains("openagents") || content_lower.contains("pylon")
}
