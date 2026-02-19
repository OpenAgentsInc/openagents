//! Network discovery - check connectivity and relay status.

use crate::manifest::{NetworkManifest, RelayStatus};

/// Default relays to probe.
const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];

/// Discover network connectivity.
pub async fn discover_network() -> anyhow::Result<NetworkManifest> {
    // Check internet connectivity
    let has_internet = check_internet().await;

    // Probe relays if we have internet
    let relays = if has_internet {
        probe_relays().await
    } else {
        Vec::new()
    };

    Ok(NetworkManifest {
        has_internet,
        relays,
        total_providers: 0, // Would need NIP-89 query
        pylon_count: 0,
        pylons_online: 0,
        pylon_pubkeys: Vec::new(),
    })
}

/// Check if we have internet connectivity.
async fn check_internet() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Try a few reliable endpoints
    for endpoint in &["https://1.1.1.1", "https://8.8.8.8"] {
        if client.get(*endpoint).send().await.is_ok() {
            return true;
        }
    }

    false
}

/// Probe relay endpoints for connectivity.
async fn probe_relays() -> Vec<RelayStatus> {
    let mut relays = Vec::new();

    for url in DEFAULT_RELAYS {
        let status = probe_relay(url).await;
        relays.push(status);
    }

    relays
}

/// Probe a single relay.
async fn probe_relay(url: &str) -> RelayStatus {
    // For now, just check if we can resolve the host
    // Full WebSocket probing would require nostr-client
    let connected = false;

    RelayStatus {
        url: url.to_string(),
        connected,
        latency_ms: None,
    }
}
