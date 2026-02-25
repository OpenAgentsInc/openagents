//! Pylon daemon integration for autopilot
//!
//! Provides functions to check pylon status, start it if needed,
//! and discover swarm providers.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::preflight::{PylonInfo, SwarmProvider};
use nostr_client::dvm::{DvmClient, DvmProvider};

const ENV_SWARM_RELAYS: &str = "OPENAGENTS_SWARM_RELAYS";
const ENV_SWARM_RELAYS_LEGACY: &str = "OPENAGENTS_SWARM_URL";
const DEFAULT_DISCOVERY_RELAY: &str = "wss://nexus.openagents.com";
const DISCOVERY_JOB_KINDS: [u16; 5] = [5102, 5100, 5101, 5103, 5050];

/// Check if the pylon daemon is running
///
/// Uses the pylon crate's `is_daemon_running()` function which checks:
/// 1. PID file exists at ~/.pylon/pylon.pid
/// 2. Process with that PID is actually running
pub fn check_pylon_running() -> bool {
    pylon::is_daemon_running()
}

/// Get detailed pylon daemon status via control socket
///
/// Returns None if:
/// - Daemon is not running
/// - Control socket doesn't exist
/// - Communication fails
pub fn get_pylon_status() -> Option<PylonInfo> {
    let socket_path = match pylon::socket_path() {
        Ok(p) => p,
        Err(_) => return None,
    };

    if !socket_path.exists() {
        return None;
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.status() {
        Ok(pylon::daemon::DaemonResponse::Status {
            running,
            uptime_secs,
            jobs_completed,
            ..
        }) => {
            // Get PID from pid file
            let pid = get_pylon_pid();

            Some(PylonInfo {
                running,
                pid,
                uptime_secs: Some(uptime_secs),
                jobs_completed,
                models: Vec::new(), // Will be filled by backend detection
            })
        }
        Ok(_) => None,
        Err(e) => {
            debug!("Failed to get pylon status: {}", e);
            None
        }
    }
}

/// Read pylon PID from pid file
fn get_pylon_pid() -> Option<u32> {
    let pid_path = pylon::pid_path().ok()?;
    let content = std::fs::read_to_string(pid_path).ok()?;
    content.trim().parse().ok()
}

pub fn pylon_identity_exists() -> bool {
    let config = match pylon::PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            debug!("Failed to load pylon config: {}", err);
            return false;
        }
    };

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            debug!("Failed to resolve pylon data dir: {}", err);
            return false;
        }
    };

    data_dir.join("identity.mnemonic").exists()
}

pub fn init_pylon_identity() -> anyhow::Result<()> {
    let pylon_bin = find_pylon_binary()?;
    let status = Command::new(&pylon_bin)
        .arg("init")
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run pylon init: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("pylon init failed with status: {}", status))
    }
}

/// Start the pylon daemon in background
///
/// Runs `pylon start` command which will daemonize itself.
/// Returns Ok(()) if the command was spawned successfully.
pub fn start_pylon() -> anyhow::Result<()> {
    info!("Starting pylon daemon...");

    // Try to find the pylon binary
    let pylon_bin = find_pylon_binary()?;

    Command::new(&pylon_bin)
        .arg("start")
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start pylon: {}", e))?;

    // Wait briefly for daemon to start
    std::thread::sleep(Duration::from_millis(1500));

    // Verify it started
    if check_pylon_running() {
        info!("Pylon daemon started successfully");
        Ok(())
    } else {
        warn!("Pylon daemon may not have started correctly");
        Ok(()) // Don't fail - it might still be starting
    }
}

/// Find the pylon binary path
fn find_pylon_binary() -> anyhow::Result<std::path::PathBuf> {
    // First check if `pylon` is in PATH
    if let Ok(output) = Command::new("which").arg("pylon").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path.into());
            }
        }
    }

    // Check common locations
    let candidates = [
        // Cargo target directory
        std::env::current_dir()?.join("target/debug/pylon"),
        std::env::current_dir()?.join("target/release/pylon"),
        // Home directory
        dirs::home_dir()
            .unwrap_or_default()
            .join(".cargo/bin/pylon"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // Fall back to just "pylon" and hope it's in PATH
    Ok("pylon".into())
}

pub fn discover_swarm_providers() -> Vec<SwarmProvider> {
    let relays = configured_discovery_relays();
    if relays.is_empty() {
        debug!("Swarm discovery skipped: no relays configured");
        return Vec::new();
    }

    if tokio::runtime::Handle::try_current().is_ok() {
        warn!(
            "discover_swarm_providers called inside an async runtime; use discover_swarm_providers_async instead"
        );
        return Vec::new();
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            warn!("Failed to create discovery runtime: {}", err);
            return Vec::new();
        }
    };

    runtime.block_on(discover_swarm_providers_async(&relays))
}

/// Async version of swarm provider discovery
///
/// This will be used in contexts where we can run async code
pub async fn discover_swarm_providers_async(relays: &[String]) -> Vec<SwarmProvider> {
    let relays = if relays.is_empty() {
        configured_discovery_relays()
    } else {
        sanitize_relays(relays.iter().cloned())
    };
    if relays.is_empty() {
        return Vec::new();
    }

    let relay_refs: Vec<&str> = relays.iter().map(String::as_str).collect();
    let client = match DvmClient::new(discovery_private_key()) {
        Ok(client) => client,
        Err(err) => {
            warn!("Failed to create DVM discovery client: {}", err);
            return Vec::new();
        }
    };

    let mut providers_by_pubkey: BTreeMap<String, AggregatedProvider> = BTreeMap::new();
    for job_kind in DISCOVERY_JOB_KINDS {
        match client.discover_providers(job_kind, &relay_refs).await {
            Ok(providers) => {
                merge_discovered_providers(&mut providers_by_pubkey, job_kind, providers, &relays);
            }
            Err(err) => {
                warn!(
                    job_kind,
                    relays = relays.join(","),
                    "Failed to discover providers for kind {}: {}",
                    job_kind,
                    err
                );
            }
        }
    }

    let mut mapped = providers_by_pubkey
        .into_iter()
        .map(|(pubkey, aggregated)| {
            let name = aggregated
                .name
                .unwrap_or_else(|| short_pubkey_label(&pubkey));
            let relay = aggregated.relays.iter().next().cloned().unwrap_or_default();
            let supported_kinds = aggregated
                .supported_kinds
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let relay_count = aggregated.relays.len();
            let health = if relay_count > 0 && !supported_kinds.is_empty() {
                "healthy"
            } else {
                "degraded"
            };

            SwarmProvider {
                pubkey,
                name,
                price_msats: None,
                relay,
                supported_kinds,
                relay_count,
                health: health.to_string(),
            }
        })
        .collect::<Vec<_>>();

    mapped.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.pubkey.cmp(&b.pubkey)));
    mapped
}

#[derive(Debug, Default)]
struct AggregatedProvider {
    name: Option<String>,
    relays: BTreeSet<String>,
    supported_kinds: BTreeSet<u16>,
}

fn discovery_private_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    key[31] = 1;
    key
}

fn configured_discovery_relays() -> Vec<String> {
    if let Some(raw) = std::env::var(ENV_SWARM_RELAYS).ok() {
        let parsed = parse_env_relays(&raw);
        if !parsed.is_empty() {
            return parsed;
        }
    }
    if let Some(raw) = std::env::var(ENV_SWARM_RELAYS_LEGACY).ok() {
        let parsed = parse_env_relays(&raw);
        if !parsed.is_empty() {
            return parsed;
        }
    }

    if let Ok(config) = pylon::PylonConfig::load() {
        let parsed = sanitize_relays(config.relays.into_iter());
        if !parsed.is_empty() {
            return parsed;
        }
    }

    vec![DEFAULT_DISCOVERY_RELAY.to_string()]
}

fn parse_env_relays(raw: &str) -> Vec<String> {
    sanitize_relays(
        raw.split([',', ' ', '\n', '\t'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    )
}

fn sanitize_relays(relays: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for relay in relays {
        let trimmed = relay.trim().trim_end_matches('/').to_string();
        if trimmed.is_empty() {
            continue;
        }
        match reqwest::Url::parse(&trimmed) {
            Ok(url)
                if matches!(url.scheme(), "ws" | "wss")
                    && url.host_str().is_some()
                    && seen.insert(trimmed.clone()) =>
            {
                out.push(trimmed);
            }
            Ok(_) => debug!("Ignoring non websocket relay URL: {}", trimmed),
            Err(err) => debug!("Ignoring invalid relay URL {}: {}", trimmed, err),
        }
    }
    out
}

fn merge_discovered_providers(
    providers_by_pubkey: &mut BTreeMap<String, AggregatedProvider>,
    job_kind: u16,
    discovered: Vec<DvmProvider>,
    discovery_relays: &[String],
) {
    for provider in discovered {
        let entry = providers_by_pubkey
            .entry(provider.pubkey.clone())
            .or_default();

        if entry.name.is_none() {
            entry.name = provider.name.clone();
        }
        entry.supported_kinds.extend(provider.supported_kinds);
        entry.supported_kinds.insert(job_kind);

        let provider_relays = sanitize_relays(provider.relays.into_iter());
        if provider_relays.is_empty() {
            entry.relays.extend(discovery_relays.iter().cloned());
        } else {
            entry.relays.extend(provider_relays);
        }
    }
}

fn short_pubkey_label(pubkey: &str) -> String {
    let short = pubkey.chars().take(12).collect::<String>();
    format!("provider-{}", short)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_pylon_running_doesnt_panic() {
        // Should return false if pylon isn't running, but shouldn't panic
        let _ = check_pylon_running();
    }

    #[test]
    fn test_parse_env_relays_sanitizes_and_dedupes() {
        let relays = parse_env_relays(
            "wss://nexus.openagents.com, wss://nexus.openagents.com, ws://relay.damus.io, https://invalid",
        );
        assert_eq!(relays.len(), 2);
        assert!(relays.contains(&"wss://nexus.openagents.com".to_string()));
        assert!(relays.contains(&"ws://relay.damus.io".to_string()));
    }

    #[test]
    fn test_merge_discovered_providers_aggregates_metadata() {
        let mut map = BTreeMap::new();
        let providers = vec![
            DvmProvider {
                pubkey: "pubkey_a".to_string(),
                name: Some("Provider A".to_string()),
                about: None,
                supported_kinds: vec![5102],
                relays: vec!["wss://relay.one".to_string()],
            },
            DvmProvider {
                pubkey: "pubkey_a".to_string(),
                name: None,
                about: None,
                supported_kinds: vec![5050],
                relays: vec!["wss://relay.two".to_string()],
            },
        ];

        merge_discovered_providers(
            &mut map,
            5102,
            providers,
            &["wss://nexus.openagents.com".to_string()],
        );

        let provider = map.get("pubkey_a").expect("provider should be present");
        assert_eq!(provider.name.as_deref(), Some("Provider A"));
        assert!(provider.supported_kinds.contains(&5102));
        assert!(provider.supported_kinds.contains(&5050));
        assert!(provider.relays.contains("wss://relay.one"));
        assert!(provider.relays.contains("wss://relay.two"));
    }

    #[test]
    fn test_short_pubkey_label_falls_back() {
        let label = short_pubkey_label("abcdef0123456789");
        assert_eq!(label, "provider-abcdef012345");
    }
}
