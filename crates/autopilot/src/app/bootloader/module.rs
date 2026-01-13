//! BootloaderModule - DSPy-style composite module for boot sequence.

use super::events::{
    BackendInfo, BootEvent, BootStage, ComputeDetails, HardwareDetails, IdentityDetails,
    NetworkDetails, StageDetails, SummaryDetails, WorkspaceDetails,
};
use super::probes;
use super::signatures::*;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Configuration for the bootloader.
#[derive(Debug, Clone)]
pub struct BootloaderConfig {
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
    /// Skip LLM summary even if available
    pub skip_summary: bool,
    /// Timeout for compute backend probing
    pub compute_timeout: Duration,
    /// Timeout for network operations
    pub network_timeout: Duration,
}

impl Default for BootloaderConfig {
    fn default() -> Self {
        Self {
            skip_hardware: false,
            skip_compute: false,
            skip_network: false,
            skip_identity: false,
            skip_workspace: false,
            skip_summary: false,
            compute_timeout: Duration::from_secs(5),
            network_timeout: Duration::from_secs(3),
        }
    }
}

impl BootloaderConfig {
    /// Minimal config - fast boot, skip slow network/compute.
    pub fn minimal() -> Self {
        Self {
            skip_compute: true,
            skip_network: true,
            ..Default::default()
        }
    }

    /// Offline config - no network operations.
    pub fn offline() -> Self {
        Self {
            skip_network: true,
            ..Default::default()
        }
    }
}

/// BootloaderModule orchestrates the boot sequence with event emission.
pub struct BootloaderModule {
    config: BootloaderConfig,
    event_tx: Option<mpsc::UnboundedSender<BootEvent>>,
}

impl BootloaderModule {
    /// Create a new bootloader with default config.
    pub fn new() -> Self {
        Self {
            config: BootloaderConfig::default(),
            event_tx: None,
        }
    }

    /// Create with custom config.
    pub fn with_config(config: BootloaderConfig) -> Self {
        Self {
            config,
            event_tx: None,
        }
    }

    /// Set event channel for UI updates.
    pub fn with_events(mut self, tx: mpsc::UnboundedSender<BootEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Run the boot sequence, returning the manifest.
    pub async fn run(&self) -> anyhow::Result<BootManifest> {
        let boot_start = Instant::now();
        let total_stages = self.count_enabled_stages();

        self.emit(BootEvent::BootStarted { total_stages });

        // Stage 1: Hardware Probe
        let hardware = self.run_hardware_stage().await;

        // Stage 2: Compute Probe
        let compute = self.run_compute_stage().await;

        // Stage 3: Network Probe
        let network = self.run_network_stage().await;

        // Stage 4: Identity Probe
        let identity = self.run_identity_stage().await;

        // Stage 5: Workspace Probe
        let workspace = self.run_workspace_stage().await;

        // Stage 6: Summary (heuristic-based, optionally LLM if available)
        let summary = if !self.config.skip_summary {
            Some(self.run_summary_stage(&hardware, &compute, &network, &identity, &workspace))
        } else {
            None
        };

        let total_duration = boot_start.elapsed();

        let manifest = BootManifest {
            hardware,
            compute,
            network,
            identity,
            workspace,
            summary: summary.clone(),
            boot_duration: total_duration,
        };

        let summary_text = summary.map(|s| s.capability_summary);
        self.emit(BootEvent::BootCompleted {
            manifest: manifest.clone(),
            total_duration,
            summary: summary_text,
        });

        Ok(manifest)
    }

    // --- Stage Implementations ---

    async fn run_hardware_stage(&self) -> HardwareProbeOutput {
        if self.config.skip_hardware {
            self.emit(BootEvent::StageSkipped {
                stage: BootStage::Hardware,
                reason: "Skipped by config",
            });
            return HardwareProbeOutput::default();
        }

        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Hardware,
            description: BootStage::Hardware.description(),
        });

        let result = probes::probe_hardware().await;
        let duration = start.elapsed();

        match result {
            Ok(output) => {
                let details = StageDetails::Hardware(HardwareDetails {
                    cpu_cores: output.cpu_cores,
                    cpu_model: output.cpu_model.clone(),
                    ram_gb: output.ram_gb,
                    apple_silicon: output.apple_silicon,
                    gpu_count: output.gpus.len(),
                });
                self.emit(BootEvent::StageCompleted {
                    stage: BootStage::Hardware,
                    duration,
                    details,
                });
                output
            }
            Err(e) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Hardware,
                    duration,
                    error: e.to_string(),
                });
                HardwareProbeOutput::default()
            }
        }
    }

    async fn run_compute_stage(&self) -> ComputeProbeOutput {
        if self.config.skip_compute {
            self.emit(BootEvent::StageSkipped {
                stage: BootStage::Compute,
                reason: "Skipped by config",
            });
            return ComputeProbeOutput::default();
        }

        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Compute,
            description: BootStage::Compute.description(),
        });

        // Emit progress for each backend check
        self.emit(BootEvent::StageProgress {
            stage: BootStage::Compute,
            message: "Checking Ollama...".to_string(),
        });

        let input = ComputeProbeInput {
            timeout_ms: self.config.compute_timeout.as_millis() as u64,
            ..Default::default()
        };

        let result = probes::probe_compute(input).await;
        let duration = start.elapsed();

        match result {
            Ok(output) => {
                let has_local_llm = output.backends.iter().any(|b| b.ready);
                let details = StageDetails::Compute(ComputeDetails {
                    backends: output
                        .backends
                        .iter()
                        .map(|b| BackendInfo {
                            id: b.id.clone(),
                            name: b.name.clone(),
                            ready: b.ready,
                            model_count: b.models.len(),
                        })
                        .collect(),
                    total_models: output.total_models,
                    has_local_llm,
                });
                self.emit(BootEvent::StageCompleted {
                    stage: BootStage::Compute,
                    duration,
                    details,
                });
                output
            }
            Err(e) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Compute,
                    duration,
                    error: e.to_string(),
                });
                ComputeProbeOutput::default()
            }
        }
    }

    async fn run_network_stage(&self) -> NetworkProbeOutput {
        if self.config.skip_network {
            self.emit(BootEvent::StageSkipped {
                stage: BootStage::Network,
                reason: "Skipped by config",
            });
            return NetworkProbeOutput::offline();
        }

        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Network,
            description: BootStage::Network.description(),
        });

        let input = NetworkProbeInput {
            timeout_ms: self.config.network_timeout.as_millis() as u64,
            ..Default::default()
        };

        let result = probes::probe_network(input).await;
        let duration = start.elapsed();

        match result {
            Ok(output) => {
                let connected = output.relays.iter().filter(|r| r.connected).count();
                let details = StageDetails::Network(NetworkDetails {
                    has_internet: output.has_internet,
                    relays_connected: connected,
                    relays_total: output.relays.len(),
                });
                self.emit(BootEvent::StageCompleted {
                    stage: BootStage::Network,
                    duration,
                    details,
                });
                output
            }
            Err(e) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Network,
                    duration,
                    error: e.to_string(),
                });
                NetworkProbeOutput::offline()
            }
        }
    }

    async fn run_identity_stage(&self) -> IdentityProbeOutput {
        if self.config.skip_identity {
            self.emit(BootEvent::StageSkipped {
                stage: BootStage::Identity,
                reason: "Skipped by config",
            });
            return IdentityProbeOutput::unknown();
        }

        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Identity,
            description: BootStage::Identity.description(),
        });

        let input = IdentityProbeInput { data_dir: None };
        let result = probes::probe_identity(input).await;
        let duration = start.elapsed();

        match result {
            Ok(output) => {
                let details = StageDetails::Identity(IdentityDetails {
                    initialized: output.initialized,
                    npub: output.npub.clone(),
                    has_wallet: output.wallet_balance_sats.is_some(),
                });
                self.emit(BootEvent::StageCompleted {
                    stage: BootStage::Identity,
                    duration,
                    details,
                });
                output
            }
            Err(e) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Identity,
                    duration,
                    error: e.to_string(),
                });
                IdentityProbeOutput::unknown()
            }
        }
    }

    async fn run_workspace_stage(&self) -> Option<WorkspaceProbeOutput> {
        if self.config.skip_workspace {
            self.emit(BootEvent::StageSkipped {
                stage: BootStage::Workspace,
                reason: "Skipped by config",
            });
            return None;
        }

        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Workspace,
            description: BootStage::Workspace.description(),
        });

        let cwd = std::env::current_dir().unwrap_or_default();
        let input = WorkspaceProbeInput { cwd };
        let result = probes::probe_workspace(input).await;
        let duration = start.elapsed();

        match result {
            Ok(Some(output)) => {
                let details = StageDetails::Workspace(WorkspaceDetails {
                    is_git_repo: output.is_git_repo,
                    project_name: output.project_name.clone(),
                    language_hints: output.language_hints.clone(),
                    open_issues: output.open_issues,
                    active_directive: output.active_directive.clone(),
                });
                self.emit(BootEvent::StageCompleted {
                    stage: BootStage::Workspace,
                    duration,
                    details,
                });
                Some(output)
            }
            Ok(None) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Workspace,
                    duration,
                    error: "No .openagents/ folder found".to_string(),
                });
                None
            }
            Err(e) => {
                self.emit(BootEvent::StageFailed {
                    stage: BootStage::Workspace,
                    duration,
                    error: e.to_string(),
                });
                None
            }
        }
    }

    fn run_summary_stage(
        &self,
        hardware: &HardwareProbeOutput,
        compute: &ComputeProbeOutput,
        network: &NetworkProbeOutput,
        identity: &IdentityProbeOutput,
        workspace: &Option<WorkspaceProbeOutput>,
    ) -> SystemSummaryOutput {
        let start = Instant::now();
        self.emit(BootEvent::StageStarted {
            stage: BootStage::Summary,
            description: BootStage::Summary.description(),
        });

        let input = SystemSummaryInput {
            hardware: hardware.clone(),
            compute: compute.clone(),
            network: network.clone(),
            identity: identity.clone(),
            workspace: workspace.clone(),
        };

        // Use heuristic-based summary (no LLM required)
        let output = probes::generate_summary_heuristic(&input);
        let duration = start.elapsed();

        let details = StageDetails::Summary(SummaryDetails {
            capability_summary: output.capability_summary.clone(),
            recommended_lane: output.recommended_lane.clone(),
        });

        self.emit(BootEvent::StageCompleted {
            stage: BootStage::Summary,
            duration,
            details,
        });

        output
    }

    // --- Helpers ---

    fn emit(&self, event: BootEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event);
        }
    }

    fn count_enabled_stages(&self) -> u8 {
        let mut count = 0;
        if !self.config.skip_hardware {
            count += 1;
        }
        if !self.config.skip_compute {
            count += 1;
        }
        if !self.config.skip_network {
            count += 1;
        }
        if !self.config.skip_identity {
            count += 1;
        }
        if !self.config.skip_workspace {
            count += 1;
        }
        if !self.config.skip_summary {
            count += 1;
        }
        count
    }
}

impl Default for BootloaderModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bootloader_minimal() {
        let config = BootloaderConfig::minimal();
        let bootloader = BootloaderModule::with_config(config);

        let manifest = bootloader.run().await.unwrap();

        // Hardware should be populated
        assert!(manifest.hardware.cpu_cores > 0);

        // Compute should be empty (skipped)
        assert!(manifest.compute.backends.is_empty());
    }

    #[tokio::test]
    async fn test_bootloader_with_events() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let config = BootloaderConfig::minimal();
        let bootloader = BootloaderModule::with_config(config).with_events(tx);

        tokio::spawn(async move {
            let _ = bootloader.run().await;
        });

        // Should receive BootStarted
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, BootEvent::BootStarted { .. }));
    }
}
