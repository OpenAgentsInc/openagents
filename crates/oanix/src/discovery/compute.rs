//! Compute backend discovery - Ollama, Apple FM, llama.cpp.

use crate::manifest::{ComputeManifest, InferenceBackend};
use std::process::Command;

/// Discover available compute backends.
pub async fn discover_compute() -> anyhow::Result<ComputeManifest> {
    let mut backends = Vec::new();

    // Check Ollama
    if let Some(backend) = probe_ollama().await {
        backends.push(backend);
    }

    // Check Apple Foundation Models (auto-start if needed)
    if let Some(backend) = probe_or_start_apple_fm().await {
        backends.push(backend);
    }

    // Check llama.cpp
    if let Some(backend) = probe_llamacpp().await {
        backends.push(backend);
    }

    let total_models = backends.iter().map(|b| b.models.len()).sum();

    Ok(ComputeManifest {
        backends,
        total_models,
    })
}

/// Probe Ollama at localhost:11434.
async fn probe_ollama() -> Option<InferenceBackend> {
    let endpoint = "http://localhost:11434";

    // Try to get model list
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client
        .get(format!("{}/api/tags", endpoint))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let body: serde_json::Value = response.json().await.ok()?;
    let models: Vec<String> = body
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name")?.as_str().map(String::from))
        .collect();

    Some(InferenceBackend {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        endpoint: Some(endpoint.to_string()),
        models,
        ready: true,
    })
}

/// Probe Apple Foundation Models at localhost:11435, auto-starting if needed.
async fn probe_or_start_apple_fm() -> Option<InferenceBackend> {
    // Only on macOS Apple Silicon
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        if std::env::consts::ARCH != "aarch64" {
            return None; // Intel Mac - no Apple FM support
        }

        let endpoint = "http://localhost:11435";

        // First try to probe existing server
        if let Some(backend) = probe_apple_fm_endpoint(endpoint).await {
            return Some(backend);
        }

        // Not running - try to auto-start the bridge
        if let Some(bridge_path) = find_fm_bridge() {
            eprintln!("  Starting Apple FM bridge...");

            // Start bridge in background
            let result = Command::new(&bridge_path)
                .arg("11435")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            if result.is_ok() {
                // Wait for it to start (up to 5 seconds)
                for _ in 0..10 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if let Some(backend) = probe_apple_fm_endpoint(endpoint).await {
                        return Some(backend);
                    }
                }
            }
        }

        None
    }
}

/// Find the FM bridge binary.
#[cfg(target_os = "macos")]
fn find_fm_bridge() -> Option<String> {
    // Check common locations
    let candidates = [
        // Relative to current dir (if running from repo)
        "bin/foundation-bridge",
        "swift/foundation-bridge/.build/release/foundation-bridge",
        // Absolute paths
        "/usr/local/bin/foundation-bridge",
    ];

    for path in candidates {
        let full_path = if path.starts_with('/') {
            path.to_string()
        } else {
            // Try relative to current working directory
            std::env::current_dir()
                .ok()?
                .join(path)
                .to_string_lossy()
                .to_string()
        };

        if std::path::Path::new(&full_path).exists() {
            return Some(full_path);
        }
    }

    None
}

/// Probe the Apple FM endpoint.
#[cfg(target_os = "macos")]
async fn probe_apple_fm_endpoint(endpoint: &str) -> Option<InferenceBackend> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client
        .get(format!("{}/health", endpoint))
        .send()
        .await
        .ok()?;

    if response.status().is_success() {
        Some(InferenceBackend {
            id: "apple-fm".to_string(),
            name: "Apple Foundation Models".to_string(),
            endpoint: Some(endpoint.to_string()),
            models: vec!["apple-fm".to_string()],
            ready: true,
        })
    } else {
        None
    }
}

/// Probe llama.cpp server at localhost:8080.
async fn probe_llamacpp() -> Option<InferenceBackend> {
    let endpoint = "http://localhost:8080";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    let response = client
        .get(format!("{}/health", endpoint))
        .send()
        .await
        .ok()?;

    if response.status().is_success() {
        Some(InferenceBackend {
            id: "llamacpp".to_string(),
            name: "llama.cpp".to_string(),
            endpoint: Some(endpoint.to_string()),
            models: vec!["default".to_string()],
            ready: true,
        })
    } else {
        None
    }
}
