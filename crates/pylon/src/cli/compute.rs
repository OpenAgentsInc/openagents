//! Compute mix command - show all available compute options
//!
//! Displays:
//! - Local pylon daemon status
//! - Local inference backends (Ollama, Apple FM, etc.)
//! - Cloud API providers (from env vars)
//! - Remote swarm providers (via NIP-89)

use clap::Args;
use nostr_client::dvm::{DvmClient, DvmProvider};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::config::PylonConfig;
use crate::daemon::{ControlClient, DaemonResponse, is_daemon_running, socket_path};

const ENV_SWARM_RELAYS: &str = "OPENAGENTS_SWARM_RELAYS";
const ENV_SWARM_RELAYS_LEGACY: &str = "OPENAGENTS_SWARM_URL";
const DEFAULT_DISCOVERY_RELAY: &str = "wss://nexus.openagents.com";
const DISCOVERY_JOB_KINDS: [u16; 5] = [5102, 5100, 5101, 5103, 5050];

#[derive(Args)]
pub struct ComputeArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: ComputeArgs) -> anyhow::Result<()> {
    let config = PylonConfig::load().unwrap_or_else(|_| PylonConfig::default());

    // Check pylon daemon status
    let pylon_running = is_daemon_running();
    let pylon_status = get_pylon_status();

    // Detect local backends (async)
    let backends = detect_local_backends().await;

    // Check cloud providers from env vars
    let cloud_providers = detect_cloud_providers();

    // Discover swarm providers from configured relays
    let swarm_providers = discover_swarm_providers(&config.relays).await;

    if args.json {
        let json = json!({
            "pylon": {
                "running": pylon_running,
                "status": pylon_status,
            },
            "local_backends": backends,
            "cloud_providers": cloud_providers,
            "swarm_providers": swarm_providers,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Compute Mix");
        println!("===========\n");

        // Pylon status
        println!("Local Pylon:");
        if pylon_running {
            println!("  Status: Running");
            if let Some(status) = pylon_status {
                if status.uptime_secs > 3600 {
                    println!(
                        "  Uptime: {}h {}m",
                        status.uptime_secs / 3600,
                        (status.uptime_secs % 3600) / 60
                    );
                } else {
                    println!("  Uptime: {}m", status.uptime_secs / 60);
                }
                if status.jobs_completed > 0 {
                    println!("  Jobs completed: {}", status.jobs_completed);
                }
            }
        } else {
            println!("  Status: Stopped");
            println!("  Run 'openagents pylon start' to start");
        }

        // Local backends
        println!("\nLocal Backends:");
        let available_backends: Vec<_> = backends.iter().filter(|b| b.available).collect();
        if available_backends.is_empty() {
            println!("  None detected");
        } else {
            for backend in &backends {
                if backend.available {
                    let models = if backend.models.is_empty() {
                        String::new()
                    } else {
                        format!(" - {}", backend.models.join(", "))
                    };
                    println!("  [OK] {} ({}){}", backend.name, backend.endpoint, models);
                } else {
                    println!("  [--] {} (not running)", backend.name);
                }
            }
        }

        // Cloud providers
        println!("\nCloud Providers:");
        if cloud_providers.is_empty() {
            println!("  None configured");
        } else {
            for provider in &cloud_providers {
                println!("  [OK] {}", provider);
            }
        }

        // Swarm providers
        println!("\nSwarm Providers (NIP-89):");
        if swarm_providers.is_empty() {
            println!("  None discovered");
        } else {
            for provider in &swarm_providers {
                let kinds = if provider.supported_kinds.is_empty() {
                    String::new()
                } else {
                    format!(" kinds={:?}", provider.supported_kinds)
                };
                let price = provider
                    .price_msats
                    .map(|msats| format!(" price={}msats", msats))
                    .unwrap_or_default();
                println!(
                    "  {} ({}){}{}",
                    provider.name,
                    &provider.pubkey[..16],
                    kinds,
                    price
                );
            }
        }

        // Summary
        println!("\nSummary:");
        println!(
            "  {} local backend(s), {} cloud provider(s), {} swarm provider(s)",
            available_backends.len(),
            cloud_providers.len(),
            swarm_providers.len()
        );
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct PylonStatus {
    uptime_secs: u64,
    jobs_completed: u64,
    provider_active: bool,
    host_active: bool,
}

fn get_pylon_status() -> Option<PylonStatus> {
    let socket = socket_path().ok()?;
    if !socket.exists() {
        return None;
    }

    let client = ControlClient::new(socket);
    match client.status() {
        Ok(DaemonResponse::Status {
            uptime_secs,
            jobs_completed,
            provider_active,
            host_active,
            ..
        }) => Some(PylonStatus {
            uptime_secs,
            jobs_completed,
            provider_active,
            host_active,
        }),
        _ => None,
    }
}

#[derive(serde::Serialize)]
struct BackendInfo {
    name: String,
    available: bool,
    endpoint: String,
    models: Vec<String>,
}

async fn detect_local_backends() -> Vec<BackendInfo> {
    let mut backends = Vec::new();

    // Check all backends concurrently
    let (ollama, apple_fm, llamacpp, fm_bridge) = tokio::join!(
        check_backend("ollama", "http://localhost:11434", "/api/tags"),
        check_backend("apple_fm", "http://localhost:11435", "/v1/models"),
        check_backend("llamacpp", "http://localhost:8080", "/health"),
        check_backend("fm-bridge", "http://localhost:8081", "/v1/models"),
    );

    backends.push(ollama);
    backends.push(apple_fm);
    backends.push(llamacpp);
    backends.push(fm_bridge);

    backends
}

async fn check_backend(name: &str, base_url: &str, health_path: &str) -> BackendInfo {
    let url = format!("{}{}", base_url, health_path);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build();

    let available = match client {
        Ok(c) => c.get(&url).send().await.is_ok(),
        Err(_) => false,
    };

    // Get models from the appropriate endpoint based on backend type
    let models = if available {
        match name {
            "ollama" => get_ollama_models(base_url).await.unwrap_or_default(),
            "llamacpp" | "apple_fm" | "fm-bridge" => get_openai_compatible_models(base_url)
                .await
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    BackendInfo {
        name: name.to_string(),
        available,
        endpoint: base_url.to_string(),
        models,
    }
}

async fn get_ollama_models(base_url: &str) -> Option<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let url = format!("{}/api/tags", base_url);
    let resp = client.get(&url).send().await.ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let models = json
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name")?.as_str().map(|s| s.to_string()))
        .collect();

    Some(models)
}

/// Get models from OpenAI-compatible backends (llama.cpp, apple_fm, fm-bridge)
async fn get_openai_compatible_models(base_url: &str) -> Option<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let url = format!("{}/v1/models", base_url);
    let resp = client.get(&url).send().await.ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;

    // Handle both array and {data: [...]} formats (OpenAI-compatible API)
    let models_array = if json.is_array() {
        json.as_array()?
    } else {
        json.get("data")?.as_array()?
    };

    let models = models_array
        .iter()
        .filter_map(|m| {
            // Try "id" first (OpenAI format), then "name"
            m.get("id")
                .or_else(|| m.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    Some(models)
}

fn detect_cloud_providers() -> Vec<String> {
    let mut providers = BTreeSet::new();

    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.insert("openai".to_string());
    }
    if std::env::var("OPENROUTER_API_KEY").is_ok() {
        providers.insert("openrouter".to_string());
    }
    if std::env::var("GOOGLE_API_KEY").is_ok() {
        providers.insert("google".to_string());
    }

    providers.into_iter().collect()
}

#[derive(serde::Serialize)]
struct SwarmProviderInfo {
    pubkey: String,
    name: String,
    price_msats: Option<u64>,
    #[serde(default)]
    supported_kinds: Vec<u16>,
}

#[derive(Debug, Default)]
struct AggregatedProvider {
    name: Option<String>,
    supported_kinds: BTreeSet<u16>,
    price_msats: Option<u64>,
}

async fn discover_swarm_providers(config_relays: &[String]) -> Vec<SwarmProviderInfo> {
    let relays = configured_discovery_relays(config_relays);
    if relays.is_empty() {
        return Vec::new();
    }

    let client = match DvmClient::new(discovery_private_key()) {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };

    let relay_refs: Vec<&str> = relays.iter().map(String::as_str).collect();
    let mut providers_by_pubkey: BTreeMap<String, AggregatedProvider> = BTreeMap::new();

    for kind in DISCOVERY_JOB_KINDS {
        if let Ok(Ok(discovered)) = tokio::time::timeout(
            Duration::from_millis(1200),
            client.discover_providers(kind, &relay_refs),
        )
        .await
        {
            merge_discovered_providers(&mut providers_by_pubkey, kind, discovered);
        }
    }

    let mut providers = providers_by_pubkey
        .into_iter()
        .map(|(pubkey, aggregated)| SwarmProviderInfo {
            name: aggregated
                .name
                .unwrap_or_else(|| short_pubkey_label(&pubkey)),
            pubkey,
            price_msats: aggregated.price_msats,
            supported_kinds: aggregated.supported_kinds.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    providers.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.pubkey.cmp(&b.pubkey)));
    providers
}

fn merge_discovered_providers(
    providers_by_pubkey: &mut BTreeMap<String, AggregatedProvider>,
    kind: u16,
    discovered: Vec<DvmProvider>,
) {
    for provider in discovered {
        let entry = providers_by_pubkey
            .entry(provider.pubkey.clone())
            .or_default();
        if entry.name.is_none() {
            entry.name = provider.name.clone();
        }
        entry.supported_kinds.extend(provider.supported_kinds);
        entry.supported_kinds.insert(kind);
    }
}

fn discovery_private_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    key[31] = 1;
    key
}

fn configured_discovery_relays(config_relays: &[String]) -> Vec<String> {
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

    let parsed = sanitize_relays(config_relays.iter().cloned());
    if !parsed.is_empty() {
        return parsed;
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
        let normalized = relay.trim().trim_end_matches('/').to_string();
        if normalized.is_empty() {
            continue;
        }
        match reqwest::Url::parse(&normalized) {
            Ok(url)
                if matches!(url.scheme(), "ws" | "wss")
                    && url.host_str().is_some()
                    && seen.insert(normalized.clone()) =>
            {
                out.push(normalized);
            }
            _ => {}
        }
    }
    out
}

fn short_pubkey_label(pubkey: &str) -> String {
    let short = pubkey.chars().take(12).collect::<String>();
    format!("provider-{}", short)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cloud_providers() {
        // Just verify the function doesn't panic and returns reasonable results
        let providers = detect_cloud_providers();
        // Should return at most 3 providers
        assert!(providers.len() <= 3);
        // All returned providers should be valid names
        for provider in &providers {
            assert!(["openai", "openrouter", "google"].contains(&provider.as_str()));
        }
    }

    #[tokio::test]
    async fn test_detect_local_backends_returns_all() {
        let backends = detect_local_backends().await;
        assert!(backends.len() >= 4); // ollama, apple_fm, llamacpp, fm-bridge
    }

    #[tokio::test]
    async fn test_detect_local_backends_names() {
        let backends = detect_local_backends().await;
        let names: Vec<&str> = backends.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"ollama"));
        assert!(names.contains(&"apple_fm"));
        assert!(names.contains(&"llamacpp"));
        assert!(names.contains(&"fm-bridge"));
    }

    #[tokio::test]
    async fn test_check_backend_unavailable() {
        // Check a port that's definitely not running anything
        let backend = check_backend("test", "http://localhost:59999", "/health").await;
        assert!(!backend.available);
        assert!(backend.models.is_empty());
    }

    #[tokio::test]
    async fn test_check_backend_model_detection_llamacpp() {
        // If llamacpp is running, it should have models detected
        let backend = check_backend("llamacpp", "http://localhost:8080", "/health").await;
        // If available, models should be fetched (may be empty if not running)
        // This test just ensures no panic occurs
        assert!(backend.name == "llamacpp");
    }
}
