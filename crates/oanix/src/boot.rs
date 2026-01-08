//! OANIX boot sequence - environment discovery and initialization.

use crate::discovery::{discover_compute, discover_hardware, discover_identity, discover_network};
use crate::manifest::OanixManifest;
use std::time::Instant;

/// Boot OANIX - discover the environment and return a manifest.
///
/// This runs all discovery phases in sequence:
/// 1. Hardware (CPU, RAM, GPU)
/// 2. Compute backends (Ollama, Apple FM, llama.cpp)
/// 3. Network (internet, relays, swarm)
/// 4. Identity (keys, wallet)
pub async fn boot() -> anyhow::Result<OanixManifest> {
    let start = Instant::now();

    // Phase 1: Hardware discovery
    let hardware = discover_hardware().await?;

    // Phase 2: Compute backend discovery
    let compute = discover_compute().await?;

    // Phase 3: Network discovery
    let network = discover_network().await?;

    // Phase 4: Identity discovery
    let identity = discover_identity().await?;

    Ok(OanixManifest {
        hardware,
        compute,
        network,
        identity,
        discovered_at: start,
    })
}
