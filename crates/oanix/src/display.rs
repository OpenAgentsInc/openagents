//! Display formatting for OANIX boot output.

use crate::manifest::OanixManifest;
use crate::situation::SituationAssessment;

/// Print the full manifest in a nice format.
pub fn print_manifest(manifest: &OanixManifest) {
    print_hardware(&manifest);
    println!();
    print_compute(&manifest);
    println!();
    print_network(&manifest);
    println!();
    print_identity(&manifest);
    println!();
    print_situation(&manifest);
}

fn print_hardware(manifest: &OanixManifest) {
    println!("Hardware");
    println!(
        "  CPU: {} ({} cores)",
        manifest.hardware.cpu_model, manifest.hardware.cpu_cores
    );

    let ram_gb = manifest.hardware.ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let ram_avail_gb = manifest.hardware.ram_available as f64 / (1024.0 * 1024.0 * 1024.0);
    println!("  RAM: {:.0} GB ({:.1} GB available)", ram_gb, ram_avail_gb);

    if manifest.hardware.gpus.is_empty() {
        println!("  GPU: None detected");
    } else {
        for gpu in &manifest.hardware.gpus {
            let status = if gpu.available { "" } else { " (unavailable)" };
            println!("  GPU: {} ({}){}", gpu.name, gpu.backend, status);
        }
    }
}

fn print_compute(manifest: &OanixManifest) {
    println!("Compute Backends");

    if manifest.compute.backends.is_empty() {
        println!("  [--] No backends running");
        // Give context-aware suggestions based on hardware
        let has_gpu = !manifest.hardware.gpus.is_empty();
        let is_apple_silicon = manifest.hardware.gpus.iter()
            .any(|g| g.backend.contains("Metal"));

        if is_apple_silicon {
            println!("       Start Ollama: `ollama serve`");
        } else if has_gpu {
            println!("       Start Ollama: `ollama serve`");
            println!("       Or llama.cpp: `llama-server -m <model.gguf>`");
        } else {
            println!("       Install Ollama: https://ollama.com");
        }
    } else {
        for backend in &manifest.compute.backends {
            let model_count = backend.models.len();
            let models_str = if model_count == 1 {
                "1 model".to_string()
            } else {
                format!("{} models", model_count)
            };

            if backend.ready {
                if let Some(endpoint) = &backend.endpoint {
                    println!("  [OK] {} ({}) - {}", backend.name, endpoint, models_str);
                } else {
                    println!("  [OK] {} - {}", backend.name, models_str);
                }

                // List models if few
                if model_count <= 5 {
                    for model in &backend.models {
                        println!("       - {}", model);
                    }
                }
            } else {
                println!("  [--] {} not ready", backend.name);
            }
        }
    }
}

fn print_network(manifest: &OanixManifest) {
    println!("Network");

    if manifest.network.has_internet {
        println!("  [OK] Internet connectivity");
    } else {
        println!("  [--] No internet connectivity");
    }

    if manifest.network.relays.is_empty() {
        println!("  [--] Nostr relays: none configured");
    } else {
        let connected_count = manifest.network.relays.iter().filter(|r| r.connected).count();
        if connected_count > 0 {
            println!("  [OK] Nostr relays: {} connected", connected_count);
            for relay in &manifest.network.relays {
                if relay.connected {
                    let latency = relay
                        .latency_ms
                        .map(|ms| format!("{}ms", ms))
                        .unwrap_or_else(|| "?".to_string());
                    println!("       - {} ({})", relay.url, latency);
                }
            }
        } else {
            println!("  [--] Nostr relays: not connected");
        }
    }

    // Show total providers and Pylons separately
    if manifest.network.total_providers > 0 {
        if manifest.network.pylon_count > 0 {
            println!(
                "  [OK] Providers: {} total ({} Pylons, {} online)",
                manifest.network.total_providers,
                manifest.network.pylon_count,
                manifest.network.pylons_online
            );
        } else {
            println!(
                "  [OK] Providers: {} total (no Pylons)",
                manifest.network.total_providers
            );
        }
    } else {
        println!("  [--] Providers: none discovered");
    }
}

fn print_identity(manifest: &OanixManifest) {
    println!("Identity");

    if manifest.identity.initialized {
        if let Some(npub) = &manifest.identity.npub {
            println!("  [OK] Pubkey: {}", npub);
        } else {
            println!("  [OK] Initialized");
        }

        if let Some(balance) = manifest.identity.wallet_balance_sats {
            let network = manifest
                .identity
                .network
                .as_deref()
                .unwrap_or("unknown");
            println!("  Wallet: {} sats ({})", balance, network);
        }
    } else {
        println!("  [--] Not initialized");
        println!("       Run 'pylon init' to create identity");
    }
}

fn print_situation(manifest: &OanixManifest) {
    let situation = SituationAssessment::from_manifest(manifest);

    println!("Situation Assessment");

    // Environment
    let env_str = match &situation.environment {
        crate::situation::Environment::Developer { os } => format!("Developer ({})", os),
        crate::situation::Environment::Server => "Server".to_string(),
        crate::situation::Environment::Container => "Container".to_string(),
        crate::situation::Environment::Unknown => "Unknown".to_string(),
    };
    println!("  Environment: {}", env_str);

    // Compute power
    let compute_str = match situation.compute_power {
        crate::situation::ComputePower::High => "High (can run large models)",
        crate::situation::ComputePower::Medium => "Medium (can run 7B models)",
        crate::situation::ComputePower::Low => "Low (small models or API)",
        crate::situation::ComputePower::SwarmOnly => "Swarm only (no local inference)",
    };
    println!("  Compute: {}", compute_str);

    // Connectivity
    let conn_str = match situation.connectivity {
        crate::situation::Connectivity::Full => "Full (internet + nostr)",
        crate::situation::Connectivity::InternetOnly => "Internet only",
        crate::situation::Connectivity::Offline => "Offline",
    };
    println!("  Connectivity: {}", conn_str);

    // Recommended action
    let action_str = match situation.recommended_action {
        crate::situation::RecommendedAction::AwaitUser => "Awaiting user direction",
        crate::situation::RecommendedAction::InitializeIdentity => "Initialize identity first (run 'pylon init')",
        crate::situation::RecommendedAction::ConnectNetwork => "Connect to network",
        crate::situation::RecommendedAction::StartProvider => "Could start provider mode",
    };
    println!("\nRecommended: {}", action_str);
}
