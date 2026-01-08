//! Situation assessment - what should OANIX do?

use crate::OanixManifest;
use serde::{Deserialize, Serialize};

/// Complete situation assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SituationAssessment {
    /// Where is OANIX running?
    pub environment: Environment,
    /// What compute power is available?
    pub compute_power: ComputePower,
    /// What connectivity do we have?
    pub connectivity: Connectivity,
    /// What should we do first?
    pub recommended_action: RecommendedAction,
}

/// Environment type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    /// Developer machine (interactive, resources available)
    Developer { os: String },
    /// Server (headless, persistent)
    Server,
    /// Container (isolated, ephemeral)
    Container,
    /// Unknown
    Unknown,
}

/// Available compute power.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputePower {
    /// Can run large models locally (70B+)
    High,
    /// Can run medium models (7B-13B)
    Medium,
    /// Limited to small models or API only
    Low,
    /// No local inference, swarm only
    SwarmOnly,
}

/// Network connectivity level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Connectivity {
    /// Full connectivity (internet + nostr + swarm)
    Full,
    /// Internet only (no nostr/swarm)
    InternetOnly,
    /// Limited or no connectivity
    Offline,
}

/// Recommended first action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendedAction {
    /// Await user input
    AwaitUser,
    /// Initialize identity first
    InitializeIdentity,
    /// Connect to network
    ConnectNetwork,
    /// Start provider mode (earn sats)
    StartProvider,
}

impl SituationAssessment {
    /// Assess the situation from a manifest.
    pub fn from_manifest(manifest: &OanixManifest) -> Self {
        // Determine environment
        let environment = if std::env::var("CONTAINER").is_ok() {
            Environment::Container
        } else if std::env::var("SSH_CONNECTION").is_ok() {
            Environment::Server
        } else {
            Environment::Developer {
                os: std::env::consts::OS.to_string(),
            }
        };

        // Determine compute power
        let compute_power = if manifest.compute.backends.is_empty() {
            ComputePower::SwarmOnly
        } else {
            // Check RAM - rough heuristic
            let ram_gb = manifest.hardware.ram_bytes / (1024 * 1024 * 1024);
            if ram_gb >= 32 && !manifest.hardware.gpus.is_empty() {
                ComputePower::High
            } else if ram_gb >= 16 {
                ComputePower::Medium
            } else {
                ComputePower::Low
            }
        };

        // Determine connectivity
        let connectivity = if manifest.network.has_internet {
            if !manifest.network.relays.is_empty()
                && manifest.network.relays.iter().any(|r| r.connected)
            {
                Connectivity::Full
            } else {
                Connectivity::InternetOnly
            }
        } else {
            Connectivity::Offline
        };

        // Determine recommended action
        let recommended_action = if !manifest.identity.initialized {
            RecommendedAction::InitializeIdentity
        } else if matches!(connectivity, Connectivity::Offline) {
            RecommendedAction::ConnectNetwork
        } else if !manifest.compute.backends.is_empty() {
            // Has compute, could be a provider
            RecommendedAction::AwaitUser
        } else {
            RecommendedAction::AwaitUser
        };

        Self {
            environment,
            compute_power,
            connectivity,
            recommended_action,
        }
    }
}
