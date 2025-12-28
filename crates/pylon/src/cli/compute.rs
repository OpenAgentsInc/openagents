//! Compute mix command - show all available compute options
//!
//! Displays:
//! - Local pylon daemon status
//! - Local inference backends (Ollama, Apple FM, etc.)
//! - Cloud API providers (from env vars)
//! - Remote swarm providers (via NIP-89)

use clap::Args;
use serde_json::json;
use std::time::Duration;

use crate::daemon::{is_daemon_running, socket_path, ControlClient, DaemonResponse};

#[derive(Args)]
pub struct ComputeArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: ComputeArgs) -> anyhow::Result<()> {
    // Check pylon daemon status
    let pylon_running = is_daemon_running();
    let pylon_status = get_pylon_status();

    // Detect local backends (async)
    let backends = detect_local_backends().await;

    // Check cloud providers from env vars
    let cloud_providers = detect_cloud_providers();

    // Discover swarm providers (placeholder for now)
    let swarm_providers: Vec<SwarmProviderInfo> = Vec::new();

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
                    println!("  Uptime: {}h {}m", status.uptime_secs / 3600, (status.uptime_secs % 3600) / 60);
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
                println!("  {} ({})", provider.name, &provider.pubkey[..16]);
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

    let models = if available && name == "ollama" {
        get_ollama_models(base_url).await.unwrap_or_default()
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

fn detect_cloud_providers() -> Vec<String> {
    let mut providers = Vec::new();

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        providers.push("anthropic".to_string());
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.push("openai".to_string());
    }
    if std::env::var("OPENROUTER_API_KEY").is_ok() {
        providers.push("openrouter".to_string());
    }
    if std::env::var("GOOGLE_API_KEY").is_ok() {
        providers.push("google".to_string());
    }

    providers
}

#[derive(serde::Serialize)]
struct SwarmProviderInfo {
    pubkey: String,
    name: String,
    price_msats: Option<u64>,
}
