//! Pylon daemon integration for autopilot
//!
//! Provides functions to check pylon status, start it if needed,
//! detect local inference backends, discover swarm providers, and
//! query neobank treasury status.

use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::preflight::{LocalBackend, NeobankInfo, PylonInfo, SwarmProvider};

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

/// Detect local inference backends synchronously
///
/// Checks well-known ports for inference servers:
/// - Ollama: localhost:11434
/// - Apple FM: localhost:11435
/// - Llama.cpp/GPT-OSS: localhost:8080
/// - FM-Bridge: localhost:8081
pub fn detect_local_backends() -> Vec<LocalBackend> {
    let mut backends = Vec::new();

    // Check Ollama
    let ollama = check_backend_http("ollama", "http://localhost:11434", "/api/tags");
    backends.push(ollama);

    // Check Apple FM
    let apple_fm = check_backend_http("apple_fm", "http://localhost:11435", "/v1/models");
    backends.push(apple_fm);

    // Check Llama.cpp / GPT-OSS
    let llamacpp = check_backend_http("llamacpp", "http://localhost:8080", "/health");
    backends.push(llamacpp);

    // Check FM-Bridge
    let fm_bridge = check_backend_http("fm-bridge", "http://localhost:8081", "/v1/models");
    backends.push(fm_bridge);

    backends
}

/// Check if a backend is available via HTTP
fn check_backend_http(name: &str, base_url: &str, health_path: &str) -> LocalBackend {
    let url = format!("{}{}", base_url, health_path);

    // Use a short timeout for detection
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(500))
        .build();

    let available = match client {
        Ok(c) => c.get(&url).send().is_ok(),
        Err(_) => false,
    };

    // Get models from the appropriate endpoint based on backend type
    let models = if available {
        match name {
            "ollama" => get_ollama_models(base_url).unwrap_or_default(),
            "llamacpp" | "apple_fm" | "fm-bridge" => {
                get_openai_compatible_models(base_url).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    LocalBackend {
        name: name.to_string(),
        available,
        endpoint: if available {
            Some(base_url.to_string())
        } else {
            None
        },
        models,
    }
}

/// Get available Ollama models
fn get_ollama_models(base_url: &str) -> Option<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let url = format!("{}/api/tags", base_url);
    let resp = client.get(&url).send().ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    let models = json
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name")?.as_str().map(|s| s.to_string()))
        .collect();

    Some(models)
}

/// Get models from OpenAI-compatible backends (llama.cpp, apple_fm, fm-bridge)
fn get_openai_compatible_models(base_url: &str) -> Option<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let url = format!("{}/v1/models", base_url);
    let resp = client.get(&url).send().ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;

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

/// Discover swarm providers via NIP-89 (stub for now)
///
/// In the future, this will:
/// 1. Connect to configured Nostr relays
/// 2. Subscribe to kind 31990 (handler info) events
/// 3. Filter for compute providers
/// 4. Parse and return SwarmProvider list
pub fn discover_swarm_providers() -> Vec<SwarmProvider> {
    // TODO: Implement NIP-89 discovery
    // For now, return empty list with a note about future implementation
    debug!("NIP-89 swarm discovery not yet implemented");
    Vec::new()
}

/// Async version of swarm provider discovery
///
/// This will be used in contexts where we can run async code
pub async fn discover_swarm_providers_async(_relays: &[String]) -> Vec<SwarmProvider> {
    // TODO: Implement async NIP-89 discovery using nostr-sdk
    // For now, return empty list
    Vec::new()
}

/// Query neobank treasury status from pylon daemon
///
/// Returns None if:
/// - Daemon is not running
/// - Neobank is not initialized
/// - Communication fails
pub fn get_neobank_status() -> Option<NeobankInfo> {
    let socket_path = match pylon::socket_path() {
        Ok(p) => p,
        Err(_) => return None,
    };

    if !socket_path.exists() {
        return None;
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.neobank_status() {
        Ok(pylon::daemon::DaemonResponse::NeobankStatus {
            btc_balance_sats,
            usd_balance_cents,
            treasury_active,
            btc_usd_rate,
        }) => Some(NeobankInfo {
            available: true,
            btc_balance_sats,
            usd_balance_cents,
            treasury_active,
            btc_usd_rate,
        }),
        Ok(pylon::daemon::DaemonResponse::Error(_)) => Some(NeobankInfo {
            available: false,
            ..Default::default()
        }),
        _ => None,
    }
}

/// Get neobank balance for a specific currency
pub fn get_neobank_balance(currency: &str) -> Option<u64> {
    let socket_path = match pylon::socket_path() {
        Ok(p) => p,
        Err(_) => return None,
    };

    if !socket_path.exists() {
        return None;
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.neobank_balance(currency) {
        Ok(pylon::daemon::DaemonResponse::NeobankBalance { sats }) => Some(sats),
        _ => None,
    }
}

/// Pay a Lightning invoice via neobank
pub fn neobank_pay(bolt11: &str) -> Result<String, String> {
    let socket_path = pylon::socket_path().map_err(|e| e.to_string())?;

    if !socket_path.exists() {
        return Err("Pylon daemon not running".to_string());
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.neobank_pay(bolt11) {
        Ok(pylon::daemon::DaemonResponse::NeobankPayment { preimage }) => Ok(preimage),
        Ok(pylon::daemon::DaemonResponse::Error(e)) => Err(e),
        Err(e) => Err(e.to_string()),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Send Cashu tokens via neobank
pub fn neobank_send(amount_sats: u64, currency: &str) -> Result<String, String> {
    let socket_path = pylon::socket_path().map_err(|e| e.to_string())?;

    if !socket_path.exists() {
        return Err("Pylon daemon not running".to_string());
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.neobank_send(amount_sats, currency) {
        Ok(pylon::daemon::DaemonResponse::NeobankSend { token }) => Ok(token),
        Ok(pylon::daemon::DaemonResponse::Error(e)) => Err(e),
        Err(e) => Err(e.to_string()),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Receive Cashu tokens via neobank
pub fn neobank_receive(token: &str) -> Result<u64, String> {
    let socket_path = pylon::socket_path().map_err(|e| e.to_string())?;

    if !socket_path.exists() {
        return Err("Pylon daemon not running".to_string());
    }

    let client = pylon::daemon::ControlClient::new(socket_path);

    match client.neobank_receive(token) {
        Ok(pylon::daemon::DaemonResponse::NeobankReceive { amount_sats }) => Ok(amount_sats),
        Ok(pylon::daemon::DaemonResponse::Error(e)) => Err(e),
        Err(e) => Err(e.to_string()),
        _ => Err("Unexpected response".to_string()),
    }
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
    fn test_detect_local_backends_returns_all() {
        let backends = detect_local_backends();
        assert_eq!(backends.len(), 4); // ollama, apple_fm, llamacpp, fm-bridge
    }

    #[test]
    fn test_detect_local_backends_names() {
        let backends = detect_local_backends();
        let names: Vec<&str> = backends.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"ollama"));
        assert!(names.contains(&"apple_fm"));
        assert!(names.contains(&"llamacpp"));
        assert!(names.contains(&"fm-bridge"));
    }

    #[test]
    fn test_discover_swarm_providers_returns_empty() {
        // Until implemented, should return empty
        let providers = discover_swarm_providers();
        assert!(providers.is_empty());
    }

    #[test]
    fn test_check_backend_http_unavailable() {
        // Check a port that's definitely not running anything
        let backend = check_backend_http("test", "http://localhost:59999", "/health");
        assert!(!backend.available);
        assert!(backend.models.is_empty());
    }
}
