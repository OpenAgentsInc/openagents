//! Boot events for progressive UI updates.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::signatures::BootManifest;

/// Events emitted during the boot sequence for real-time UI updates.
#[derive(Debug, Clone)]
pub enum BootEvent {
    /// Boot sequence starting
    BootStarted {
        total_stages: u8,
    },

    /// A stage has started
    StageStarted {
        stage: BootStage,
        description: &'static str,
    },

    /// Progress within a stage (for stages with sub-steps)
    StageProgress {
        stage: BootStage,
        message: String,
    },

    /// A stage completed successfully
    StageCompleted {
        stage: BootStage,
        duration: Duration,
        details: StageDetails,
    },

    /// A stage failed (non-fatal, boot continues)
    StageFailed {
        stage: BootStage,
        duration: Duration,
        error: String,
    },

    /// A stage was skipped (via config)
    StageSkipped {
        stage: BootStage,
        reason: &'static str,
    },

    /// Entire boot sequence completed
    BootCompleted {
        manifest: BootManifest,
        total_duration: Duration,
        summary: Option<String>,
    },

    /// Boot failed entirely (fatal error)
    BootFailed {
        error: String,
    },
}

/// Identifiers for each boot stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BootStage {
    Hardware,
    Compute,
    Network,
    Identity,
    Workspace,
    Summary,
}

impl BootStage {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hardware => "Hardware",
            Self::Compute => "Compute",
            Self::Network => "Network",
            Self::Identity => "Identity",
            Self::Workspace => "Workspace",
            Self::Summary => "Summary",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Hardware => "Detecting CPU, RAM, and GPU",
            Self::Compute => "Probing inference backends",
            Self::Network => "Checking connectivity and relays",
            Self::Identity => "Loading Nostr identity and wallet",
            Self::Workspace => "Scanning project context",
            Self::Summary => "Generating capability summary",
        }
    }
}

/// Details for a completed stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageDetails {
    Hardware(HardwareDetails),
    Compute(ComputeDetails),
    Network(NetworkDetails),
    Identity(IdentityDetails),
    Workspace(WorkspaceDetails),
    Summary(SummaryDetails),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareDetails {
    pub cpu_cores: u32,
    pub cpu_model: String,
    pub ram_gb: f64,
    pub apple_silicon: bool,
    pub gpu_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDetails {
    pub backends: Vec<BackendInfo>,
    pub total_models: usize,
    pub has_local_llm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendInfo {
    pub id: String,
    pub name: String,
    pub ready: bool,
    pub model_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDetails {
    pub has_internet: bool,
    pub relays_connected: usize,
    pub relays_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityDetails {
    pub initialized: bool,
    pub npub: Option<String>,
    pub has_wallet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDetails {
    pub is_git_repo: bool,
    pub project_name: Option<String>,
    pub language_hints: Vec<String>,
    pub open_issues: u32,
    pub active_directive: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryDetails {
    pub capability_summary: String,
    pub recommended_lane: String,
}
