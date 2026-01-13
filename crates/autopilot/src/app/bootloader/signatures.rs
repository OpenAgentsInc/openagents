//! DSPy-style signatures for the bootloader.
//!
//! These are primarily tool-based (deterministic probes), with an optional
//! LLM signature for system summary generation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

// =============================================================================
// Tool-based Signatures (deterministic, no LLM required)
// =============================================================================

/// Hardware probe output - CPU, RAM, GPU info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProbeOutput {
    pub cpu_cores: u32,
    pub cpu_model: String,
    pub ram_gb: f64,
    pub ram_available_gb: f64,
    pub gpus: Vec<GpuInfo>,
    pub apple_silicon: bool,
}

impl Default for HardwareProbeOutput {
    fn default() -> Self {
        Self {
            cpu_cores: 0,
            cpu_model: "unknown".to_string(),
            ram_gb: 0.0,
            ram_available_gb: 0.0,
            gpus: vec![],
            apple_silicon: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub backend: String,
    pub available: bool,
}

/// Compute probe input configuration.
#[derive(Debug, Clone)]
pub struct ComputeProbeInput {
    pub timeout_ms: u64,
    pub auto_start_apple_fm: bool,
}

impl Default for ComputeProbeInput {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            auto_start_apple_fm: true,
        }
    }
}

/// Compute probe output - inference backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeProbeOutput {
    pub backends: Vec<InferenceBackendInfo>,
    pub total_models: usize,
    pub has_codex_cli: bool,
    pub has_cerebras: bool,
}

impl Default for ComputeProbeOutput {
    fn default() -> Self {
        Self {
            backends: vec![],
            total_models: 0,
            has_codex_cli: false,
            has_cerebras: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceBackendInfo {
    pub id: String,
    pub name: String,
    pub endpoint: Option<String>,
    pub models: Vec<String>,
    pub ready: bool,
}

/// Network probe input configuration.
#[derive(Debug, Clone)]
pub struct NetworkProbeInput {
    pub relay_urls: Vec<String>,
    pub timeout_ms: u64,
}

impl Default for NetworkProbeInput {
    fn default() -> Self {
        Self {
            relay_urls: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.primal.net".to_string(),
            ],
            timeout_ms: 3000,
        }
    }
}

/// Network probe output - connectivity status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProbeOutput {
    pub has_internet: bool,
    pub relays: Vec<RelayInfo>,
}

impl NetworkProbeOutput {
    pub fn offline() -> Self {
        Self {
            has_internet: false,
            relays: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    pub url: String,
    pub connected: bool,
    pub latency_ms: Option<u32>,
}

/// Identity probe input configuration.
#[derive(Debug, Clone)]
pub struct IdentityProbeInput {
    pub data_dir: Option<PathBuf>,
}

/// Identity probe output - Nostr identity and wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProbeOutput {
    pub initialized: bool,
    pub npub: Option<String>,
    pub wallet_balance_sats: Option<u64>,
    pub network: Option<String>,
}

impl IdentityProbeOutput {
    pub fn unknown() -> Self {
        Self {
            initialized: false,
            npub: None,
            wallet_balance_sats: None,
            network: None,
        }
    }
}

/// Workspace probe input configuration.
#[derive(Debug, Clone)]
pub struct WorkspaceProbeInput {
    pub cwd: PathBuf,
}

/// Workspace probe output - project context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProbeOutput {
    pub root: PathBuf,
    pub is_git_repo: bool,
    pub project_name: Option<String>,
    pub language_hints: Vec<String>,
    pub has_openagents: bool,
    pub open_issues: u32,
    pub pending_issues: u32,
    pub active_directive: Option<String>,
}

// =============================================================================
// LLM-based Signature (optional, for summary generation)
// =============================================================================

/// System summary input - all manifests combined.
#[derive(Debug, Clone)]
pub struct SystemSummaryInput {
    pub hardware: HardwareProbeOutput,
    pub compute: ComputeProbeOutput,
    pub network: NetworkProbeOutput,
    pub identity: IdentityProbeOutput,
    pub workspace: Option<WorkspaceProbeOutput>,
}

/// System summary output - LLM-generated capability summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSummaryOutput {
    /// Natural language summary of system capabilities
    pub capability_summary: String,
    /// Recommended execution lane: "codex", "local_llm", "tiered", "analysis_only"
    pub recommended_lane: String,
    /// Confidence in the recommendation (0.0 - 1.0)
    pub confidence: f32,
}

// =============================================================================
// Boot Manifest (combined output)
// =============================================================================

/// Complete manifest from the boot sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootManifest {
    pub hardware: HardwareProbeOutput,
    pub compute: ComputeProbeOutput,
    pub network: NetworkProbeOutput,
    pub identity: IdentityProbeOutput,
    pub workspace: Option<WorkspaceProbeOutput>,
    pub summary: Option<SystemSummaryOutput>,
    #[serde(with = "duration_serde")]
    pub boot_duration: Duration,
}

/// Serde support for Duration
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}
