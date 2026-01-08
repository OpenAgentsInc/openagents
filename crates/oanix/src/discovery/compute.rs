//! Compute backend discovery - Ollama, Apple FM, llama.cpp.

use crate::manifest::{ComputeManifest, InferenceBackend};

/// Discover available compute backends.
pub async fn discover_compute() -> anyhow::Result<ComputeManifest> {
    let mut backends = Vec::new();

    // Check Ollama
    if let Some(backend) = probe_ollama().await {
        backends.push(backend);
    }

    // Check Apple Foundation Models
    if let Some(backend) = probe_apple_fm().await {
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

/// Probe Apple Foundation Models at localhost:11435.
async fn probe_apple_fm() -> Option<InferenceBackend> {
    // Only on macOS
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let endpoint = "http://localhost:11435";

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
