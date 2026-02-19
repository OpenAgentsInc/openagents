//! Probe implementations - wrappers around adjutant discovery functions.

use super::signatures::*;
use anyhow::Result;
use std::path::Path;

/// Probe hardware using adjutant's discover_hardware().
pub async fn probe_hardware() -> Result<HardwareProbeOutput> {
    let manifest = adjutant::discovery::discover_hardware().await?;

    let apple_silicon = cfg!(target_os = "macos") && std::env::consts::ARCH == "aarch64";

    Ok(HardwareProbeOutput {
        cpu_cores: manifest.cpu_cores,
        cpu_model: manifest.cpu_model,
        ram_gb: manifest.ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        ram_available_gb: manifest.ram_available as f64 / (1024.0 * 1024.0 * 1024.0),
        gpus: manifest
            .gpus
            .into_iter()
            .map(|g| GpuInfo {
                name: g.name,
                backend: g.backend,
                available: g.available,
            })
            .collect(),
        apple_silicon,
    })
}

/// Probe compute backends using adjutant's discover_compute().
pub async fn probe_compute(_input: ComputeProbeInput) -> Result<ComputeProbeOutput> {
    let manifest = adjutant::discovery::discover_compute().await?;

    // Check for additional tools
    let has_codex_cli = check_codex_cli();
    let has_cerebras = std::env::var("CEREBRAS_API_KEY").is_ok();

    Ok(ComputeProbeOutput {
        backends: manifest
            .backends
            .into_iter()
            .map(|b| InferenceBackendInfo {
                id: b.id,
                name: b.name,
                endpoint: b.endpoint,
                models: b.models,
                ready: b.ready,
            })
            .collect(),
        total_models: manifest.total_models,
        has_codex_cli,
        has_cerebras,
    })
}

/// Check if Codex CLI is installed.
fn check_codex_cli() -> bool {
    // Check common locations for codex binary
    let candidates = ["codex", "claude"];

    for name in candidates {
        if which::which(name).is_ok() {
            return true;
        }
    }

    false
}

/// Probe network using adjutant's discover_network().
pub async fn probe_network(_input: NetworkProbeInput) -> Result<NetworkProbeOutput> {
    let manifest = adjutant::discovery::discover_network().await?;

    Ok(NetworkProbeOutput {
        has_internet: manifest.has_internet,
        relays: manifest
            .relays
            .into_iter()
            .map(|r| RelayInfo {
                url: r.url,
                connected: r.connected,
                latency_ms: r.latency_ms,
            })
            .collect(),
    })
}

/// Probe identity using adjutant's discover_identity().
pub async fn probe_identity(_input: IdentityProbeInput) -> Result<IdentityProbeOutput> {
    let manifest = adjutant::discovery::discover_identity().await?;

    Ok(IdentityProbeOutput {
        initialized: manifest.initialized,
        npub: manifest.npub,
        wallet_balance_sats: manifest.wallet_balance_sats,
        network: manifest.network,
    })
}

/// Probe workspace using adjutant's discover_workspace().
pub async fn probe_workspace(_input: WorkspaceProbeInput) -> Result<Option<WorkspaceProbeOutput>> {
    let manifest = adjutant::discovery::discover_workspace().await?;

    let Some(ws) = manifest else {
        return Ok(None);
    };

    // Detect git repo
    let is_git_repo = ws.root.join(".git").exists();

    // Detect language hints from file extensions
    let language_hints = detect_languages(&ws.root);

    Ok(Some(WorkspaceProbeOutput {
        root: ws.root,
        is_git_repo,
        project_name: ws.project_name,
        language_hints,
        has_openagents: ws.has_openagents,
        open_issues: ws.open_issues,
        pending_issues: ws.pending_issues,
        active_directive: ws.active_directive,
    }))
}

/// Detect programming languages from common project files.
fn detect_languages(root: &Path) -> Vec<String> {
    let mut languages = Vec::new();

    let patterns = [
        ("Cargo.toml", "Rust"),
        ("package.json", "JavaScript/TypeScript"),
        ("go.mod", "Go"),
        ("requirements.txt", "Python"),
        ("pyproject.toml", "Python"),
        ("Gemfile", "Ruby"),
        ("build.gradle", "Java/Kotlin"),
        ("pom.xml", "Java"),
        ("*.swift", "Swift"),
        ("*.cs", "C#"),
        ("CMakeLists.txt", "C/C++"),
    ];

    for (file, lang) in patterns {
        if file.starts_with('*') {
            // Glob pattern - skip for now (would need walkdir)
            continue;
        }
        if root.join(file).exists() && !languages.contains(&lang.to_string()) {
            languages.push(lang.to_string());
        }
    }

    languages
}

/// Generate system summary using a simple heuristic (no LLM).
///
/// This is a fallback that determines recommended lane based on detected capabilities.
pub fn generate_summary_heuristic(input: &SystemSummaryInput) -> SystemSummaryOutput {
    let recommended_lane = determine_lane(input);
    let capability_summary = build_capability_summary(input, &recommended_lane);

    SystemSummaryOutput {
        capability_summary,
        recommended_lane,
        confidence: 0.8,
    }
}

/// Determine the recommended execution lane based on detected capabilities.
fn determine_lane(input: &SystemSummaryInput) -> String {
    // Priority: Codex > Local LLM > Cerebras > Analysis-only
    if input.compute.has_codex_cli {
        return "codex".to_string();
    }

    if input.compute.backends.iter().any(|b| b.ready) {
        return "local_llm".to_string();
    }

    if input.compute.has_cerebras {
        return "tiered".to_string();
    }

    "analysis_only".to_string()
}

/// Build a natural language summary of capabilities.
fn build_capability_summary(input: &SystemSummaryInput, lane: &str) -> String {
    let mut parts = Vec::new();

    // Hardware summary
    if input.hardware.apple_silicon {
        parts.push("Apple Silicon".to_string());
    } else {
        parts.push(format!("{} cores", input.hardware.cpu_cores));
    }

    // Compute summary
    let ready_backends: Vec<_> = input
        .compute
        .backends
        .iter()
        .filter(|b| b.ready)
        .map(|b| b.name.as_str())
        .collect();

    if !ready_backends.is_empty() {
        parts.push(format!("{} ready", ready_backends.join(", ")));
    }

    if input.compute.has_codex_cli {
        parts.push("Codex CLI available".to_string());
    }

    // Network summary
    if input.network.has_internet {
        parts.push("online".to_string());
    } else {
        parts.push("offline".to_string());
    }

    // Workspace summary
    if let Some(ws) = &input.workspace {
        if let Some(name) = &ws.project_name {
            parts.push(format!("project: {}", name));
        }
        if ws.open_issues > 0 {
            parts.push(format!("{} issues", ws.open_issues));
        }
    }

    let summary = parts.join(" | ");

    // Add lane recommendation
    let lane_desc = match lane {
        "codex" => "Ready for autonomous coding with Codex.",
        "local_llm" => "Local LLM inference available.",
        "tiered" => "Cloud inference via Cerebras.",
        _ => "Analysis mode only.",
    };

    format!("{}\n{}", summary, lane_desc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_probe_hardware() {
        let result = probe_hardware().await;
        assert!(result.is_ok());
        let hw = result.unwrap();
        assert!(hw.cpu_cores > 0);
        assert!(hw.ram_gb > 0.0);
    }

    #[test]
    fn test_determine_lane() {
        // Test with codex
        let input_codex = SystemSummaryInput {
            hardware: HardwareProbeOutput::default(),
            compute: ComputeProbeOutput {
                has_codex_cli: true,
                ..Default::default()
            },
            network: NetworkProbeOutput::offline(),
            identity: IdentityProbeOutput::unknown(),
            workspace: None,
        };
        assert_eq!(determine_lane(&input_codex), "codex");

        // Test with local LLM
        let input_local = SystemSummaryInput {
            hardware: HardwareProbeOutput::default(),
            compute: ComputeProbeOutput {
                backends: vec![InferenceBackendInfo {
                    id: "ollama".to_string(),
                    name: "Ollama".to_string(),
                    endpoint: None,
                    models: vec!["llama3".to_string()],
                    ready: true,
                }],
                ..Default::default()
            },
            network: NetworkProbeOutput::offline(),
            identity: IdentityProbeOutput::unknown(),
            workspace: None,
        };
        assert_eq!(determine_lane(&input_local), "local_llm");

        // Test analysis only
        let input_analysis = SystemSummaryInput {
            hardware: HardwareProbeOutput::default(),
            compute: ComputeProbeOutput::default(),
            network: NetworkProbeOutput::offline(),
            identity: IdentityProbeOutput::unknown(),
            workspace: None,
        };
        assert_eq!(determine_lane(&input_analysis), "analysis_only");
    }
}
