//! OANIX manifest types - the discovered environment.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Complete manifest of discovered environment.
#[derive(Debug, Clone)]
pub struct OanixManifest {
    /// Hardware capabilities
    pub hardware: HardwareManifest,
    /// Available compute backends
    pub compute: ComputeManifest,
    /// Network connectivity
    pub network: NetworkManifest,
    /// Identity and wallet status
    pub identity: IdentityManifest,
    /// When discovery completed
    pub discovered_at: Instant,
}

/// Hardware manifest - CPU, RAM, GPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareManifest {
    /// Number of logical CPU cores
    pub cpu_cores: u32,
    /// CPU model name
    pub cpu_model: String,
    /// Total RAM in bytes
    pub ram_bytes: u64,
    /// Available RAM in bytes
    pub ram_available: u64,
    /// Detected GPU devices
    pub gpus: Vec<GpuDevice>,
}

/// GPU device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    /// Device name
    pub name: String,
    /// Backend type (Metal, CUDA, Vulkan)
    pub backend: String,
    /// Whether the GPU is available for compute
    pub available: bool,
}

/// Compute manifest - inference backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeManifest {
    /// Detected inference backends
    pub backends: Vec<InferenceBackend>,
    /// Total models available across all backends
    pub total_models: usize,
}

/// An inference backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceBackend {
    /// Backend identifier
    pub id: String,
    /// Backend name
    pub name: String,
    /// Endpoint URL (if applicable)
    pub endpoint: Option<String>,
    /// Available models
    pub models: Vec<String>,
    /// Whether the backend is ready
    pub ready: bool,
}

/// Network manifest - connectivity status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkManifest {
    /// Whether we have internet connectivity
    pub has_internet: bool,
    /// Nostr relay status
    pub relays: Vec<RelayStatus>,
    /// Total NIP-89 DVM providers discovered
    pub total_providers: u32,
    /// Number of Pylon providers specifically (OpenAgents)
    pub pylon_count: u32,
    /// Number of Pylons that are currently online (recent activity)
    pub pylons_online: u32,
    /// Pubkeys of discovered Pylons
    pub pylon_pubkeys: Vec<String>,
}

/// Status of a Nostr relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayStatus {
    /// Relay URL
    pub url: String,
    /// Whether we're connected
    pub connected: bool,
    /// Latency in milliseconds (if connected)
    pub latency_ms: Option<u32>,
}

/// Identity manifest - keys, wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityManifest {
    /// Whether identity is initialized
    pub initialized: bool,
    /// Nostr npub (if initialized)
    pub npub: Option<String>,
    /// Wallet balance in satoshis (if available)
    pub wallet_balance_sats: Option<u64>,
    /// Bitcoin network (mainnet, regtest, etc.)
    pub network: Option<String>,
}
