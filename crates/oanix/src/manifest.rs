//! OANIX manifest types - the discovered environment.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Configuration for the boot sequence.
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// Skip hardware discovery
    pub skip_hardware: bool,
    /// Skip compute backend discovery
    pub skip_compute: bool,
    /// Skip network discovery
    pub skip_network: bool,
    /// Skip identity discovery
    pub skip_identity: bool,
    /// Skip workspace discovery
    pub skip_workspace: bool,
    /// Timeout for network operations
    pub timeout: Duration,
    /// Number of retries for transient failures
    pub retries: u32,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            skip_hardware: false,
            skip_compute: false,
            skip_network: false,
            skip_identity: false,
            skip_workspace: false,
            timeout: Duration::from_secs(5),
            retries: 2,
        }
    }
}

impl BootConfig {
    /// Create a minimal config that only discovers what's fast and local.
    pub fn minimal() -> Self {
        Self {
            skip_hardware: false,
            skip_compute: true,
            skip_network: true,
            skip_identity: true,
            skip_workspace: false,
            timeout: Duration::from_secs(2),
            retries: 0,
        }
    }

    /// Create a config for offline mode (no network).
    pub fn offline() -> Self {
        Self {
            skip_network: true,
            ..Default::default()
        }
    }
}

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
    /// Workspace context (.openagents/)
    pub workspace: Option<WorkspaceManifest>,
    /// When discovery completed
    pub discovered_at: Instant,
}

/// Workspace manifest - project context from .openagents/ folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    /// Project root directory
    pub root: PathBuf,
    /// Project name (from directory name)
    pub project_name: Option<String>,
    /// Whether .openagents/ folder exists
    pub has_openagents: bool,
    /// Discovered directives
    pub directives: Vec<DirectiveSummary>,
    /// Discovered issues
    pub issues: Vec<IssueSummary>,
    /// Number of open issues
    pub open_issues: u32,
    /// Number of pending issues (not yet triaged)
    pub pending_issues: u32,
    /// Currently active directive ID
    pub active_directive: Option<String>,
}

/// Summary of a directive from .openagents/directives/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectiveSummary {
    /// Directive ID (e.g., "d-027")
    pub id: String,
    /// Directive title
    pub title: String,
    /// Status (active, completed, paused, etc.)
    pub status: String,
    /// Priority (urgent, high, medium, low)
    pub priority: Option<String>,
    /// Progress percentage (0-100)
    pub progress_pct: Option<u8>,
}

/// Summary of an issue from .openagents/issues.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    /// Issue number
    pub number: u32,
    /// Issue title
    pub title: String,
    /// Issue description (for staleness checks)
    #[serde(default)]
    pub description: Option<String>,
    /// Issue type (bug, feature, task)
    #[serde(default)]
    pub issue_type: Option<String>,
    /// Status (open, in_progress, completed)
    pub status: String,
    /// Priority
    pub priority: String,
    /// Whether the issue is blocked
    pub is_blocked: bool,
    /// Reason for being blocked (if any)
    pub blocked_reason: Option<String>,
    /// When the issue was created (ISO-8601)
    #[serde(default)]
    pub created_at: Option<String>,
    /// When the issue was last updated (ISO-8601)
    #[serde(default)]
    pub updated_at: Option<String>,
    /// When the issue was last validated by an agent (ISO-8601)
    #[serde(default)]
    pub last_checked: Option<String>,
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

impl HardwareManifest {
    /// Create an unknown hardware manifest (when discovery is skipped).
    pub fn unknown() -> Self {
        Self {
            cpu_cores: 0,
            cpu_model: "unknown".to_string(),
            ram_bytes: 0,
            ram_available: 0,
            gpus: vec![],
        }
    }
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

impl ComputeManifest {
    /// Create an empty compute manifest (when discovery is skipped).
    pub fn empty() -> Self {
        Self {
            backends: vec![],
            total_models: 0,
        }
    }
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

impl NetworkManifest {
    /// Create an offline network manifest (when discovery is skipped).
    pub fn offline() -> Self {
        Self {
            has_internet: false,
            relays: vec![],
            total_providers: 0,
            pylon_count: 0,
            pylons_online: 0,
            pylon_pubkeys: vec![],
        }
    }
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

impl IdentityManifest {
    /// Create an unknown identity manifest (when discovery is skipped).
    pub fn unknown() -> Self {
        Self {
            initialized: false,
            npub: None,
            wallet_balance_sats: None,
            network: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_manifest_unknown_defaults() {
        let manifest = HardwareManifest::unknown();
        assert_eq!(manifest.cpu_cores, 0);
        assert_eq!(manifest.cpu_model, "unknown");
        assert_eq!(manifest.ram_bytes, 0);
        assert_eq!(manifest.ram_available, 0);
        assert!(manifest.gpus.is_empty());
    }

    #[test]
    fn compute_manifest_empty_defaults() {
        let manifest = ComputeManifest::empty();
        assert!(manifest.backends.is_empty());
        assert_eq!(manifest.total_models, 0);
    }

    #[test]
    fn network_manifest_offline_defaults() {
        let manifest = NetworkManifest::offline();
        assert!(!manifest.has_internet);
        assert!(manifest.relays.is_empty());
        assert_eq!(manifest.total_providers, 0);
        assert_eq!(manifest.pylon_count, 0);
        assert_eq!(manifest.pylons_online, 0);
        assert!(manifest.pylon_pubkeys.is_empty());
    }

    #[test]
    fn identity_manifest_unknown_defaults() {
        let manifest = IdentityManifest::unknown();
        assert!(!manifest.initialized);
        assert!(manifest.npub.is_none());
        assert!(manifest.wallet_balance_sats.is_none());
        assert!(manifest.network.is_none());
    }
}
