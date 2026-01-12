//! OANIX boot sequence - environment discovery and initialization.

use crate::discovery::{
    discover_compute, discover_hardware, discover_identity, discover_network, discover_workspace,
};
use crate::manifest::{
    BootConfig, ComputeManifest, HardwareManifest, IdentityManifest, NetworkManifest, OanixManifest,
};
use std::time::Instant;
use tracing::{debug, warn};

/// Boot OANIX with default configuration.
///
/// This runs all discovery phases in sequence:
/// 1. Hardware (CPU, RAM, GPU)
/// 2. Compute backends (Ollama, Apple FM, llama.cpp)
/// 3. Network (internet, relays, swarm)
/// 4. Identity (keys, wallet)
/// 5. Workspace (.openagents/ folder)
pub async fn boot() -> anyhow::Result<OanixManifest> {
    boot_with_config(BootConfig::default()).await
}

/// Boot OANIX with custom configuration.
///
/// Respects skip flags and retry settings from the config.
/// Runs all discovery phases in parallel for faster startup.
pub async fn boot_with_config(config: BootConfig) -> anyhow::Result<OanixManifest> {
    let start = Instant::now();

    // Run all discovery phases in parallel
    let (hardware_result, compute_result, network_result, identity_result, workspace_result) = tokio::join!(
        async {
            if config.skip_hardware {
                debug!("Skipping hardware discovery");
                Ok(HardwareManifest::unknown())
            } else {
                retry_discovery("hardware", config.retries, || discover_hardware()).await
            }
        },
        async {
            if config.skip_compute {
                debug!("Skipping compute discovery");
                Ok(ComputeManifest::empty())
            } else {
                retry_discovery("compute", config.retries, || discover_compute()).await
            }
        },
        async {
            if config.skip_network {
                debug!("Skipping network discovery");
                Ok(NetworkManifest::offline())
            } else {
                retry_discovery("network", config.retries, || discover_network()).await
            }
        },
        async {
            if config.skip_identity {
                debug!("Skipping identity discovery");
                Ok(IdentityManifest::unknown())
            } else {
                retry_discovery("identity", config.retries, || discover_identity()).await
            }
        },
        async {
            if config.skip_workspace {
                debug!("Skipping workspace discovery");
                Ok(None)
            } else {
                retry_discovery("workspace", config.retries, || discover_workspace()).await
            }
        },
    );

    Ok(OanixManifest {
        hardware: hardware_result?,
        compute: compute_result?,
        network: network_result?,
        identity: identity_result?,
        workspace: workspace_result?,
        discovered_at: start,
    })
}

/// Retry a discovery function with exponential backoff.
async fn retry_discovery<T, F, Fut>(name: &str, retries: u32, f: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut attempts = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < retries => {
                attempts += 1;
                warn!(
                    "{} discovery failed (attempt {}/{}): {}",
                    name,
                    attempts,
                    retries + 1,
                    e
                );
                tokio::time::sleep(std::time::Duration::from_millis(250 * (1 << attempts))).await;
            }
            Err(e) => return Err(e),
        }
    }
}
