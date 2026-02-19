//! Boot sequence - discover environment and create manifest.

use crate::discovery::{
    discover_compute, discover_hardware, discover_identity, discover_network, discover_workspace,
};
use crate::manifest::{
    BootConfig, ComputeManifest, HardwareManifest, IdentityManifest, NetworkManifest, OanixManifest,
};
use std::time::Instant;

/// Boot with default configuration.
pub async fn boot() -> anyhow::Result<OanixManifest> {
    boot_with_config(BootConfig::default()).await
}

/// Boot with custom configuration.
pub async fn boot_with_config(config: BootConfig) -> anyhow::Result<OanixManifest> {
    // Hardware discovery
    let hardware = if config.skip_hardware {
        HardwareManifest::unknown()
    } else {
        discover_hardware().await?
    };

    // Compute discovery
    let compute = if config.skip_compute {
        ComputeManifest::empty()
    } else {
        discover_compute().await?
    };

    // Network discovery
    let network = if config.skip_network {
        NetworkManifest::offline()
    } else {
        discover_network().await?
    };

    // Identity discovery
    let identity = if config.skip_identity {
        IdentityManifest::unknown()
    } else {
        discover_identity().await?
    };

    // Workspace discovery
    let workspace = if config.skip_workspace {
        None
    } else {
        discover_workspace().await?
    };

    Ok(OanixManifest {
        hardware,
        compute,
        network,
        identity,
        workspace,
        discovered_at: Instant::now(),
    })
}
