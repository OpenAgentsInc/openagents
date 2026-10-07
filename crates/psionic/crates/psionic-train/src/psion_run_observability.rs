use std::collections::{BTreeMap, BTreeSet};

use psionic_runtime::DeliveredExecutionContext;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionPretrainStageRunReceipt, TrainingInstabilitySignalKind, TrainingInstabilityTelemetry,
    TrainingStabilityVerdict, TrainingStageKind,
};

/// Stable schema version for the first Psion pretrain-run observability receipt.
pub const PSION_PRETRAIN_RUN_OBSERVABILITY_SCHEMA_VERSION: &str =
    "psion.pretrain_run_observability_receipt.v1";
/// Stable schema version for the first Psion pretrain-stage observability summary.
pub const PSION_PRETRAIN_STAGE_OBSERVABILITY_SUMMARY_SCHEMA_VERSION: &str =
    "psion.pretrain_stage_observability_summary.v1";

/// Scale profile for one bounded Psion pretraining run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPretrainRunScaleProfile {
    /// Small first run used to validate the lane and receipts.
    Pilot,
    /// Broader pretraining run used to plan scale-up and recovery posture.
    BroaderPretraining,
}

/// Cost basis used for one observability receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPretrainRunCostBasis {
    /// Cost was estimated from declared rates.
    EstimatedUsd,
    /// Cost was metered from the realized run.
    MeteredUsd,
}

/// Structured cost breakdown for one Psion pretraining run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainRunCostReceipt {
    /// Cost basis used by the receipt.
    pub cost_basis: PsionPretrainRunCostBasis,
    /// ISO-style currency code.
    pub currency_code: String,
    /// Compute spend in micro-USD.
    pub compute_cost_microusd: u64,
    /// Storage spend in micro-USD.
    pub storage_cost_microusd: u64,
    /// Network spend in micro-USD.
    pub network_cost_microusd: u64,
    /// Total spend in micro-USD.
    pub total_cost_microusd: u64,
    /// Short explanation of the cost posture.
    pub detail: String,
}

impl PsionPretrainRunCostReceipt {
    fn validate(&self) -> Result<(), PsionPretrainRunObservabilityError> {
        ensure_nonempty(
            self.currency_code.as_str(),
            "pretrain_run_observability.cost.currency_code",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "pretrain_run_observability.cost.detail",
        )?;
        let expected_total = self
            .compute_cost_microusd
            .saturating_add(self.storage_cost_microusd)
            .saturating_add(self.network_cost_microusd);
        if expected_total != self.total_cost_microusd || self.total_cost_microusd == 0 {
            return Err(PsionPretrainRunObservabilityError::InvalidCostBreakdown);
        }
        Ok(())
    }
}

/// Structured throughput and token-rate summary for one Psion pretraining run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainRunThroughputReceipt {
    /// Train tokens processed by the run.
    pub train_tokens_processed: u64,
    /// Validation tokens processed by the run.
    pub validation_tokens_processed: u64,
    /// Held-out tokens scored without entering the train mix.
    pub held_out_tokens_scored: u64,
    /// Optimizer steps completed by the run.
    pub optimizer_steps_completed: u32,
    /// Wall-clock run duration.
    pub wall_clock_ms: u64,
    /// Mean observed token rate.
    pub mean_tokens_per_second: u64,
    /// Peak observed token rate.
    pub peak_tokens_per_second: u64,
    /// Mean observed sequence rate in milli-sequences per second.
    pub mean_sequences_per_second_milli: u32,
    /// Mean observed step latency.
    pub mean_step_latency_ms: u64,
    /// Mean checkpoint-write throughput observed during the run.
    pub checkpoint_write_throughput_bytes_per_second: u64,
}

impl PsionPretrainRunThroughputReceipt {
    fn validate(&self) -> Result<(), PsionPretrainRunObservabilityError> {
        if self.total_tokens_processed() == 0 {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("total_tokens_processed"),
                },
            );
        }
        if self.optimizer_steps_completed == 0 {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("optimizer_steps_completed"),
                },
            );
        }
        if self.wall_clock_ms == 0 {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("wall_clock_ms"),
                },
            );
        }
        if self.mean_tokens_per_second == 0 || self.peak_tokens_per_second == 0 {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("tokens_per_second"),
                },
            );
        }
        if self.peak_tokens_per_second < self.mean_tokens_per_second {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("peak_tokens_per_second"),
                },
            );
        }
        if self.mean_sequences_per_second_milli == 0
            || self.mean_step_latency_ms == 0
            || self.checkpoint_write_throughput_bytes_per_second == 0
        {
            return Err(
                PsionPretrainRunObservabilityError::InvalidThroughputMetric {
                    field: String::from("sequence_or_step_or_checkpoint_rate"),
                },
            );
        }
        Ok(())
    }

    #[must_use]
    pub const fn total_tokens_processed(&self) -> u64 {
        self.train_tokens_processed
            .saturating_add(self.validation_tokens_processed)
            .saturating_add(self.held_out_tokens_scored)
    }
}

/// Checkpoint-size and artifact-surface summary for one promoted pretrain checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainCheckpointArtifactReceipt {
    /// Stable promoted-checkpoint label.
    pub promoted_checkpoint_label: String,
    /// Checkpoint family emitted by the run.
    pub checkpoint_family: String,
    /// Promoted checkpoint object digest.
    pub checkpoint_object_digest: String,
    /// Checkpoint tensor payload size in bytes.
    pub checkpoint_size_bytes: u64,
    /// Optimizer-state payload size in bytes.
    pub optimizer_state_size_bytes: u64,
    /// Ancillary manifest, descriptor, and receipt bytes.
    pub ancillary_artifact_size_bytes: u64,
    /// Total artifact bytes attributable to the run.
    pub total_artifact_size_bytes: u64,
    /// Number of checkpoint shards emitted by the run.
    pub shard_count: u16,
    /// Short explanation of the artifact posture.
    pub detail: String,
}

impl PsionPretrainCheckpointArtifactReceipt {
    fn validate_against_stage(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
    ) -> Result<(), PsionPretrainRunObservabilityError> {
        check_string_match(
            self.promoted_checkpoint_label.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .as_str(),
            "checkpoint_artifact.promoted_checkpoint_label",
        )?;
        check_string_match(
            self.checkpoint_family.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .as_str(),
            "checkpoint_artifact.checkpoint_family",
        )?;
        check_string_match(
            self.checkpoint_object_digest.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .as_str(),
            "checkpoint_artifact.checkpoint_object_digest",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_artifact.detail")?;
        if self.checkpoint_size_bytes == 0
            || self.optimizer_state_size_bytes == 0
            || self.ancillary_artifact_size_bytes == 0
            || self.shard_count == 0
        {
            return Err(PsionPretrainRunObservabilityError::InvalidCheckpointArtifactSize);
        }
        let expected_total = self
            .checkpoint_size_bytes
            .saturating_add(self.optimizer_state_size_bytes)
            .saturating_add(self.ancillary_artifact_size_bytes);
        if expected_total != self.total_artifact_size_bytes {
            return Err(PsionPretrainRunObservabilityError::InvalidCheckpointArtifactSize);
        }
        Ok(())
    }
}

/// Hardware-topology facts preserved for one Psion pretraining run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainHardwareTopologyReceipt {
    /// Stable digest over the hardware-topology facts.
    pub topology_digest: String,
    /// Number of workers observed by the run.
    pub observed_worker_count: u16,
    /// Delivered execution facts for the realized path.
    pub delivered_execution: DeliveredExecutionContext,
    /// Short explanation of the realized hardware posture.
    pub detail: String,
}

impl PsionPretrainHardwareTopologyReceipt {
    /// Creates one hardware-topology receipt and computes its stable digest.
    pub fn new(
        observed_worker_count: u16,
        delivered_execution: DeliveredExecutionContext,
        detail: impl Into<String>,
    ) -> Result<Self, PsionPretrainRunObservabilityError> {
        let mut receipt = Self {
            topology_digest: String::new(),
            observed_worker_count,
            delivered_execution,
            detail: detail.into(),
        };
        receipt.topology_digest = stable_hardware_topology_digest(&receipt);
        receipt.validate()?;
        Ok(receipt)
    }

    fn validate(&self) -> Result<(), PsionPretrainRunObservabilityError> {
        if self.observed_worker_count == 0 {
            return Err(
                PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                    detail: String::from("observed_worker_count must be greater than zero"),
                },
            );
        }
        ensure_nonempty(
            self.delivered_execution.runtime_backend.as_str(),
            "hardware_topology.delivered_execution.runtime_backend",
        )?;
        ensure_nonempty(self.detail.as_str(), "hardware_topology.detail")?;
        if self.delivered_execution.selected_devices.is_empty() {
            return Err(
                PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                    detail: String::from("delivered_execution.selected_devices must not be empty"),
                },
            );
        }
        if let Some(execution_topology) = &self.delivered_execution.execution_topology {
            if execution_topology.effective_backend != self.delivered_execution.runtime_backend {
                return Err(
                    PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                        detail: String::from(
                            "execution_topology.effective_backend must match runtime_backend",
                        ),
                    },
                );
            }
            if execution_topology.assignments.len()
                != self.delivered_execution.selected_devices.len()
            {
                return Err(
                    PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                        detail: String::from(
                            "execution_topology.assignments must match selected_devices length",
                        ),
                    },
                );
            }
        } else if self.delivered_execution.selected_devices.len() > 1 {
            return Err(PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                detail: String::from(
                    "multi-device observability receipts require an explicit execution_topology",
                ),
            });
        }
        if let Some(cluster_execution) = &self.delivered_execution.cluster_execution {
            if let (Some(cluster_topology), Some(delivered_topology)) = (
                &cluster_execution.execution_topology,
                &self.delivered_execution.execution_topology,
            ) {
                if cluster_topology != delivered_topology {
                    return Err(PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                        detail: String::from(
                            "cluster execution topology must match delivered execution topology",
                        ),
                    });
                }
            }
            if !cluster_execution.selected_nodes.is_empty()
                && cluster_execution.selected_nodes.len() != self.observed_worker_count as usize
            {
                return Err(
                    PsionPretrainRunObservabilityError::InvalidHardwareTopology {
                        detail: String::from(
                            "selected_nodes length must match observed_worker_count when surfaced",
                        ),
                    },
                );
            }
        }
        if self.topology_digest != stable_hardware_topology_digest(self) {
            return Err(PsionPretrainRunObservabilityError::HardwareTopologyDigestMismatch);
        }
        Ok(())
    }
}

/// Run-level observability receipt for one explicit Psion pretrain run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPretrainRunObservabilityReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable observability receipt identifier.
    pub receipt_id: String,
    /// Scale profile for the run.
    pub run_profile: PsionPretrainRunScaleProfile,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stage kind.
    pub stage_kind: TrainingStageKind,
    /// Stable model id.
    pub model_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable dataset identity.
    pub dataset_identity: String,
    /// Stable sampling-policy identifier.
    pub sampling_policy_id: String,
    /// Stable sampling-policy version.
    pub sampling_policy_version: String,
    /// Stable digest of the bound pretrain-stage receipt.
    pub pretrain_stage_receipt_digest: String,
    /// Cost surface for the run.
    pub cost: PsionPretrainRunCostReceipt,
    /// Throughput surface for the run.
    pub throughput: PsionPretrainRunThroughputReceipt,
    /// Checkpoint and artifact-size surface for the run.
    pub checkpoint_artifact: PsionPretrainCheckpointArtifactReceipt,
    /// Hardware-topology surface for the run.
    pub hardware_topology: PsionPretrainHardwareTopologyReceipt,
    /// Aggregated instability telemetry for the run.
    pub instability_telemetry: TrainingInstabilityTelemetry,
    /// Structured instability markers and policy outcome when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability_verdict: Option<TrainingStabilityVerdict>,
    /// Short run summary.
    pub summary: String,
    /// Stable digest over the full observability receipt.
    pub observability_digest: String,
}

impl PsionPretrainRunObservabilityReceipt {
    /// Validates the receipt against one explicit pretrain-stage receipt.
    pub fn validate_against_stage(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
    ) -> Result<(), PsionPretrainRunObservabilityError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pretrain_run_observability.schema_version",
        )?;
        if self.schema_version != PSION_PRETRAIN_RUN_OBSERVABILITY_SCHEMA_VERSION {
            return Err(PsionPretrainRunObservabilityError::SchemaVersionMismatch {
                expected: String::from(PSION_PRETRAIN_RUN_OBSERVABILITY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "pretrain_run_observability.receipt_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_receipt.stage_id.as_str(),
            "stage_id",
        )?;
        if self.stage_kind != TrainingStageKind::Pretrain {
            return Err(PsionPretrainRunObservabilityError::StageKindMismatch {
                expected: TrainingStageKind::Pretrain,
                actual: self.stage_kind,
            });
        }
        check_string_match(
            self.model_id.as_str(),
            stage_receipt.model_id.as_str(),
            "model_id",
        )?;
        check_string_match(
            self.model_descriptor_digest.as_str(),
            stage_receipt.model_descriptor_digest.as_str(),
            "model_descriptor_digest",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            stage_receipt.dataset_identity.as_str(),
            "dataset_identity",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            stage_receipt.sampling_policy_id.as_str(),
            "sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            stage_receipt.sampling_policy_version.as_str(),
            "sampling_policy_version",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            stage_receipt.receipt_digest.as_str(),
            "pretrain_stage_receipt_digest",
        )?;
        self.cost.validate()?;
        self.throughput.validate()?;
        self.checkpoint_artifact
            .validate_against_stage(stage_receipt)?;
        self.hardware_topology.validate()?;
        self.validate_stability_surface()?;
        ensure_nonempty(self.summary.as_str(), "pretrain_run_observability.summary")?;
        if self.observability_digest != stable_pretrain_run_observability_digest(self) {
            return Err(PsionPretrainRunObservabilityError::ObservabilityDigestMismatch);
        }
        Ok(())
    }

    fn validate_stability_surface(&self) -> Result<(), PsionPretrainRunObservabilityError> {
        if let Some(stability_verdict) = &self.stability_verdict {
            ensure_nonempty(
                stability_verdict.policy_digest.as_str(),
                "pretrain_run_observability.stability_verdict.policy_digest",
            )?;
            ensure_nonempty(
                stability_verdict.verdict_digest.as_str(),
                "pretrain_run_observability.stability_verdict.verdict_digest",
            )?;
            for signal_receipt in &stability_verdict.signal_receipts {
                let Some(observed_value) =
                    signal_value_from_telemetry(signal_receipt.signal, &self.instability_telemetry)
                else {
                    return Err(
                        PsionPretrainRunObservabilityError::MissingTelemetryForSignal {
                            signal: signal_kind_label(signal_receipt.signal).to_owned(),
                        },
                    );
                };
                if (observed_value - signal_receipt.observed_value).abs() > 0.0001 {
                    return Err(PsionPretrainRunObservabilityError::SignalReceiptMismatch {
                        signal: signal_kind_label(signal_receipt.signal).to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// One stage-summary row projected from one observability receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainStageObservabilitySummaryRow {
    /// Run profile represented by the row.
    pub run_profile: PsionPretrainRunScaleProfile,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Stable observability receipt identifier.
    pub observability_receipt_id: String,
    /// Stable observability digest.
    pub observability_digest: String,
    /// Total run cost in micro-USD.
    pub total_cost_microusd: u64,
    /// Mean observed token rate.
    pub mean_tokens_per_second: u64,
    /// Total checkpoint and artifact size in bytes.
    pub checkpoint_artifact_size_bytes: u64,
    /// Number of instability markers surfaced by policy.
    pub instability_marker_count: u16,
    /// Stable hardware-topology digest for the run.
    pub topology_digest: String,
    /// Short summary of the row.
    pub summary: String,
}

impl PsionPretrainStageObservabilitySummaryRow {
    fn from_receipt(receipt: &PsionPretrainRunObservabilityReceipt) -> Self {
        Self {
            run_profile: receipt.run_profile,
            run_id: receipt.run_id.clone(),
            stage_id: receipt.stage_id.clone(),
            model_id: receipt.model_id.clone(),
            observability_receipt_id: receipt.receipt_id.clone(),
            observability_digest: receipt.observability_digest.clone(),
            total_cost_microusd: receipt.cost.total_cost_microusd,
            mean_tokens_per_second: receipt.throughput.mean_tokens_per_second,
            checkpoint_artifact_size_bytes: receipt.checkpoint_artifact.total_artifact_size_bytes,
            instability_marker_count: receipt
                .stability_verdict
                .as_ref()
                .map(|verdict| verdict.signal_receipts.len() as u16)
                .unwrap_or(0),
            topology_digest: receipt.hardware_topology.topology_digest.clone(),
            summary: format!(
                "{} run `{}` recorded {} train tokens at {} tok/s mean throughput with {} total checkpoint bytes.",
                run_profile_label(receipt.run_profile),
                receipt.run_id,
                receipt.throughput.train_tokens_processed,
                receipt.throughput.mean_tokens_per_second,
                receipt.checkpoint_artifact.total_artifact_size_bytes
            ),
        }
    }
}

/// Stage-level observability summary spanning pilot and broader-pretraining runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainStageObservabilitySummary {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable summary identifier.
    pub summary_id: String,
    /// Stage kind covered by the summary.
    pub stage_kind: TrainingStageKind,
    /// Stable dataset identity shared by the summarized runs.
    pub dataset_identity: String,
    /// Comparison rows derived from the run receipts.
    pub rows: Vec<PsionPretrainStageObservabilitySummaryRow>,
    /// Short summary of the stage-level comparison.
    pub summary: String,
    /// Stable digest over the summary.
    pub summary_digest: String,
}

impl PsionPretrainStageObservabilitySummary {
    /// Validates the summary against the source observability receipts.
    pub fn validate_against_receipts(
        &self,
        receipts: &[PsionPretrainRunObservabilityReceipt],
    ) -> Result<(), PsionPretrainRunObservabilityError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pretrain_stage_observability_summary.schema_version",
        )?;
        if self.schema_version != PSION_PRETRAIN_STAGE_OBSERVABILITY_SUMMARY_SCHEMA_VERSION {
            return Err(PsionPretrainRunObservabilityError::SchemaVersionMismatch {
                expected: String::from(PSION_PRETRAIN_STAGE_OBSERVABILITY_SUMMARY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.summary_id.as_str(),
            "pretrain_stage_observability_summary.summary_id",
        )?;
        if self.stage_kind != TrainingStageKind::Pretrain {
            return Err(PsionPretrainRunObservabilityError::StageKindMismatch {
                expected: TrainingStageKind::Pretrain,
                actual: self.stage_kind,
            });
        }
        ensure_nonempty(
            self.dataset_identity.as_str(),
            "pretrain_stage_observability_summary.dataset_identity",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "pretrain_stage_observability_summary.summary",
        )?;
        if self.rows.len() != receipts.len() {
            return Err(
                PsionPretrainRunObservabilityError::SummaryReceiptCountMismatch {
                    expected: receipts.len(),
                    actual: self.rows.len(),
                },
            );
        }
        let receipt_map = receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let mut seen_profiles = BTreeSet::new();
        for row in &self.rows {
            ensure_nonempty(
                row.observability_receipt_id.as_str(),
                "pretrain_stage_observability_summary.rows[].observability_receipt_id",
            )?;
            ensure_nonempty(
                row.summary.as_str(),
                "pretrain_stage_observability_summary.rows[].summary",
            )?;
            if !seen_profiles.insert(row.run_profile) {
                return Err(PsionPretrainRunObservabilityError::DuplicateRunProfile {
                    run_profile: run_profile_label(row.run_profile).to_owned(),
                });
            }
            let Some(receipt) = receipt_map.get(row.observability_receipt_id.as_str()) else {
                return Err(
                    PsionPretrainRunObservabilityError::SummaryRowReceiptMismatch {
                        receipt_id: row.observability_receipt_id.clone(),
                    },
                );
            };
            if receipt.stage_kind != TrainingStageKind::Pretrain
                || receipt.dataset_identity != self.dataset_identity
            {
                return Err(
                    PsionPretrainRunObservabilityError::SummaryRowReceiptMismatch {
                        receipt_id: row.observability_receipt_id.clone(),
                    },
                );
            }
            if row.run_profile != receipt.run_profile
                || row.run_id != receipt.run_id
                || row.stage_id != receipt.stage_id
                || row.model_id != receipt.model_id
                || row.observability_digest != receipt.observability_digest
                || row.total_cost_microusd != receipt.cost.total_cost_microusd
                || row.mean_tokens_per_second != receipt.throughput.mean_tokens_per_second
                || row.checkpoint_artifact_size_bytes
                    != receipt.checkpoint_artifact.total_artifact_size_bytes
                || row.topology_digest != receipt.hardware_topology.topology_digest
                || row.instability_marker_count
                    != receipt
                        .stability_verdict
                        .as_ref()
                        .map(|verdict| verdict.signal_receipts.len() as u16)
                        .unwrap_or(0)
            {
                return Err(
                    PsionPretrainRunObservabilityError::SummaryRowReceiptMismatch {
                        receipt_id: row.observability_receipt_id.clone(),
                    },
                );
            }
        }
        for required_profile in [
            PsionPretrainRunScaleProfile::Pilot,
            PsionPretrainRunScaleProfile::BroaderPretraining,
        ] {
            if !seen_profiles.contains(&required_profile) {
                return Err(PsionPretrainRunObservabilityError::MissingRunProfile {
                    run_profile: run_profile_label(required_profile).to_owned(),
                });
            }
        }
        if self.summary_digest != stable_pretrain_stage_observability_summary_digest(self) {
            return Err(PsionPretrainRunObservabilityError::SummaryDigestMismatch);
        }
        Ok(())
    }
}

/// Creates one typed run-observability receipt for an explicit Psion pretrain stage.
pub fn record_psion_pretrain_run_observability(
    receipt_id: impl Into<String>,
    run_profile: PsionPretrainRunScaleProfile,
    cost: PsionPretrainRunCostReceipt,
    throughput: PsionPretrainRunThroughputReceipt,
    checkpoint_artifact: PsionPretrainCheckpointArtifactReceipt,
    hardware_topology: PsionPretrainHardwareTopologyReceipt,
    instability_telemetry: TrainingInstabilityTelemetry,
    stability_verdict: Option<TrainingStabilityVerdict>,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
) -> Result<PsionPretrainRunObservabilityReceipt, PsionPretrainRunObservabilityError> {
    let mut receipt = PsionPretrainRunObservabilityReceipt {
        schema_version: String::from(PSION_PRETRAIN_RUN_OBSERVABILITY_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_profile,
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        stage_kind: TrainingStageKind::Pretrain,
        model_id: stage_receipt.model_id.clone(),
        model_descriptor_digest: stage_receipt.model_descriptor_digest.clone(),
        dataset_identity: stage_receipt.dataset_identity.clone(),
        sampling_policy_id: stage_receipt.sampling_policy_id.clone(),
        sampling_policy_version: stage_receipt.sampling_policy_version.clone(),
        pretrain_stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        cost,
        throughput,
        checkpoint_artifact,
        hardware_topology,
        instability_telemetry,
        stability_verdict,
        summary: summary.into(),
        observability_digest: String::new(),
    };
    receipt.observability_digest = stable_pretrain_run_observability_digest(&receipt);
    receipt.validate_against_stage(stage_receipt)?;
    Ok(receipt)
}

/// Summarizes the pilot and broader-pretraining observability receipts into one stage artifact.
pub fn summarize_psion_pretrain_observability_runs(
    summary_id: impl Into<String>,
    receipts: &[PsionPretrainRunObservabilityReceipt],
    summary: impl Into<String>,
) -> Result<PsionPretrainStageObservabilitySummary, PsionPretrainRunObservabilityError> {
    let dataset_identity = receipts
        .first()
        .map(|receipt| receipt.dataset_identity.clone())
        .ok_or(PsionPretrainRunObservabilityError::MissingField {
            field: String::from("pretrain_stage_observability_summary.rows"),
        })?;
    let mut rows = receipts
        .iter()
        .map(PsionPretrainStageObservabilitySummaryRow::from_receipt)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.run_profile);
    let mut stage_summary = PsionPretrainStageObservabilitySummary {
        schema_version: String::from(PSION_PRETRAIN_STAGE_OBSERVABILITY_SUMMARY_SCHEMA_VERSION),
        summary_id: summary_id.into(),
        stage_kind: TrainingStageKind::Pretrain,
        dataset_identity,
        rows,
        summary: summary.into(),
        summary_digest: String::new(),
    };
    stage_summary.summary_digest =
        stable_pretrain_stage_observability_summary_digest(&stage_summary);
    stage_summary.validate_against_receipts(receipts)?;
    Ok(stage_summary)
}

/// Error returned by the Psion pretrain-run observability contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionPretrainRunObservabilityError {
    /// One required field was missing or empty.
    #[error("Psion pretrain observability field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version drifted from the expected contract.
    #[error("Psion pretrain observability expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One string field drifted from the expected value.
    #[error(
        "Psion pretrain observability field `{field}` expected `{expected}`, found `{actual}`"
    )]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Stage kind drifted from the explicit pretrain contract.
    #[error("Psion pretrain observability expected stage kind `{expected:?}`, found `{actual:?}`")]
    StageKindMismatch {
        /// Expected stage kind.
        expected: TrainingStageKind,
        /// Actual stage kind.
        actual: TrainingStageKind,
    },
    /// Cost breakdown did not add up.
    #[error("Psion pretrain observability cost breakdown must sum exactly to the total cost and stay non-zero")]
    InvalidCostBreakdown,
    /// One throughput field was invalid.
    #[error("Psion pretrain observability throughput field `{field}` is invalid")]
    InvalidThroughputMetric {
        /// Field name.
        field: String,
    },
    /// Checkpoint-size accounting drifted from the declared artifact surface.
    #[error("Psion pretrain observability checkpoint artifact sizes must stay non-zero and sum exactly to the total artifact size")]
    InvalidCheckpointArtifactSize,
    /// Hardware-topology facts were malformed.
    #[error("Psion pretrain observability hardware topology is invalid: {detail}")]
    InvalidHardwareTopology {
        /// Plain-language error detail.
        detail: String,
    },
    /// Hardware-topology digest drifted from the payload.
    #[error("Psion pretrain observability hardware-topology digest drifted from the payload")]
    HardwareTopologyDigestMismatch,
    /// Observability digest drifted from the payload.
    #[error("Psion pretrain observability digest drifted from the payload")]
    ObservabilityDigestMismatch,
    /// One stability signal receipt did not map back to the surfaced telemetry.
    #[error("Psion pretrain observability stability signal `{signal}` did not match the surfaced telemetry")]
    SignalReceiptMismatch {
        /// Signal label.
        signal: String,
    },
    /// One stability signal receipt omitted the corresponding telemetry value.
    #[error("Psion pretrain observability stability signal `{signal}` requires a surfaced telemetry value")]
    MissingTelemetryForSignal {
        /// Signal label.
        signal: String,
    },
    /// One run profile was repeated in the stage summary.
    #[error("Psion pretrain observability summary repeated run profile `{run_profile}`")]
    DuplicateRunProfile {
        /// Repeated profile label.
        run_profile: String,
    },
    /// One required run profile was missing from the stage summary.
    #[error("Psion pretrain observability summary requires run profile `{run_profile}`")]
    MissingRunProfile {
        /// Missing profile label.
        run_profile: String,
    },
    /// The summary row count did not match the source receipt count.
    #[error("Psion pretrain observability summary expected `{expected}` source receipts, found `{actual}` rows")]
    SummaryReceiptCountMismatch {
        /// Expected count.
        expected: usize,
        /// Actual count.
        actual: usize,
    },
    /// One summary row drifted from the referenced receipt.
    #[error("Psion pretrain observability summary row drifted from receipt `{receipt_id}`")]
    SummaryRowReceiptMismatch {
        /// Receipt identifier.
        receipt_id: String,
    },
    /// Summary digest drifted from the payload.
    #[error("Psion pretrain observability stage-summary digest drifted from the payload")]
    SummaryDigestMismatch,
}

fn stable_hardware_topology_digest(receipt: &PsionPretrainHardwareTopologyReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_hardware_topology|");
    hasher.update(receipt.observed_worker_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.detail.as_bytes());
    hasher.update(b"|");
    hasher.update(
        serde_json::to_vec(&receipt.delivered_execution)
            .expect("delivered execution should serialize for digest"),
    );
    hex::encode(hasher.finalize())
}

fn stable_pretrain_run_observability_digest(
    receipt: &PsionPretrainRunObservabilityReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_run_observability|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(run_profile_label(receipt.run_profile).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(receipt.stage_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_descriptor_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.dataset_identity.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.cost.total_cost_microusd.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .throughput
            .mean_tokens_per_second
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .checkpoint_artifact
            .total_artifact_size_bytes
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.hardware_topology.topology_digest.as_bytes());
    if let Some(stability_verdict) = &receipt.stability_verdict {
        hasher.update(b"|stability|");
        hasher.update(stability_verdict.verdict_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_pretrain_stage_observability_summary_digest(
    summary: &PsionPretrainStageObservabilitySummary,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_stage_observability_summary|");
    hasher.update(summary.summary_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_stage_kind_label(summary.stage_kind).as_bytes());
    hasher.update(b"|");
    hasher.update(summary.dataset_identity.as_bytes());
    hasher.update(b"|");
    hasher.update(summary.summary.as_bytes());
    for row in &summary.rows {
        hasher.update(b"|row|");
        hasher.update(run_profile_label(row.run_profile).as_bytes());
        hasher.update(b"|");
        hasher.update(row.run_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.stage_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.observability_receipt_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.observability_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(row.total_cost_microusd.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.mean_tokens_per_second.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.checkpoint_artifact_size_bytes.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.instability_marker_count.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.topology_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn signal_value_from_telemetry(
    signal: TrainingInstabilitySignalKind,
    telemetry: &TrainingInstabilityTelemetry,
) -> Option<f64> {
    match signal {
        TrainingInstabilitySignalKind::MaxGradientNormL2 => {
            telemetry.max_gradient_norm_l2.map(f64::from)
        }
        TrainingInstabilitySignalKind::MeanClippingRatio => {
            telemetry.mean_clipping_ratio.map(f64::from)
        }
        TrainingInstabilitySignalKind::EntropyDriftBps => {
            telemetry.entropy_drift_bps.map(f64::from)
        }
        TrainingInstabilitySignalKind::StaleRolloutDropRateBps => {
            Some(f64::from(telemetry.stale_rollout_drop_rate_bps))
        }
        TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs => telemetry
            .checkpoint_catchup_latency_ms
            .map(|value| value as f64),
        TrainingInstabilitySignalKind::TopologyChurnEvents => {
            Some(f64::from(telemetry.topology_churn_events))
        }
        TrainingInstabilitySignalKind::EnvironmentFailureRateBps => {
            Some(f64::from(telemetry.environment_failure_rate_bps))
        }
        TrainingInstabilitySignalKind::SandboxFailureRateBps => {
            Some(f64::from(telemetry.sandbox_failure_rate_bps))
        }
    }
}

fn training_stage_kind_label(kind: TrainingStageKind) -> &'static str {
    match kind {
        TrainingStageKind::Pretrain => "pretrain",
        TrainingStageKind::GeneralSft => "general_sft",
        TrainingStageKind::AgenticSft => "agentic_sft",
        TrainingStageKind::Rl => "rl",
    }
}

fn run_profile_label(run_profile: PsionPretrainRunScaleProfile) -> &'static str {
    match run_profile {
        PsionPretrainRunScaleProfile::Pilot => "pilot",
        PsionPretrainRunScaleProfile::BroaderPretraining => "broader_pretraining",
    }
}

fn signal_kind_label(signal: TrainingInstabilitySignalKind) -> &'static str {
    match signal {
        TrainingInstabilitySignalKind::MaxGradientNormL2 => "max_gradient_norm_l2",
        TrainingInstabilitySignalKind::MeanClippingRatio => "mean_clipping_ratio",
        TrainingInstabilitySignalKind::EntropyDriftBps => "entropy_drift_bps",
        TrainingInstabilitySignalKind::StaleRolloutDropRateBps => "stale_rollout_drop_rate_bps",
        TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs => {
            "checkpoint_catchup_latency_ms"
        }
        TrainingInstabilitySignalKind::TopologyChurnEvents => "topology_churn_events",
        TrainingInstabilitySignalKind::EnvironmentFailureRateBps => "environment_failure_rate_bps",
        TrainingInstabilitySignalKind::SandboxFailureRateBps => "sandbox_failure_rate_bps",
    }
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPretrainRunObservabilityError> {
    if value.trim().is_empty() {
        return Err(PsionPretrainRunObservabilityError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPretrainRunObservabilityError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionPretrainRunObservabilityError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_data::PsionTokenizedCorpusManifest;
    use psionic_models::PsionCompactDecoderDescriptor;
    use psionic_runtime::{
        ClusterCommunicationClass, ClusterExecutionContext, ClusterExecutionDisposition,
        ClusterSelectedNode, ClusterTransportClass, DeviceInventoryQualifiers, DeviceMemoryClass,
        DevicePerformanceClass, ExecutionTopologyKind, ExecutionTopologyPlan,
        TrainingCheckpointAvailability, TrainingCollectiveContext, TrainingCollectiveKind,
        TrainingCollectiveQuantization, TrainingDeviceMeshContext,
        TrainingElasticMembershipContext, TrainingRecoveryContext, TrainingRecoveryPosture,
    };

    use crate::{
        record_psion_pretrain_run_observability, run_psion_pretrain_stage,
        summarize_psion_pretrain_observability_runs, PsionPretrainCheckpointLineageReceipt,
        PsionPretrainLossNormalization, PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind,
        PsionPretrainReplayReceipt, PsionPretrainSourceFamilyReportRow, PsionPretrainStageConfig,
        PsionSamplingPolicyManifest, TrainingInstabilityPolicy, TrainingInstabilityRule,
        TrainingOperationalAction, TrainingStabilityController,
    };

    fn pilot_stage_receipt() -> PsionPretrainStageRunReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"
        ))
        .expect("pilot stage receipt should parse")
    }

    fn pilot_observability_receipt() -> PsionPretrainRunObservabilityReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json"
        ))
        .expect("pilot observability receipt should parse")
    }

    fn broader_observability_receipt() -> PsionPretrainRunObservabilityReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/observability/psion_broader_pretrain_run_observability_receipt_v1.json"
        ))
        .expect("broader observability receipt should parse")
    }

    fn observability_summary() -> PsionPretrainStageObservabilitySummary {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/observability/psion_pretrain_stage_observability_summary_v1.json"
        ))
        .expect("observability summary should parse")
    }

    fn broader_stage_receipt() -> PsionPretrainStageRunReceipt {
        let model_descriptor: PsionCompactDecoderDescriptor = serde_json::from_str(include_str!(
            "../../../fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json"
        ))
        .expect("internal model descriptor should parse");
        let tokenized_corpus: PsionTokenizedCorpusManifest = serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"
        ))
        .expect("tokenized corpus should parse");
        let sampling_policy: PsionSamplingPolicyManifest = serde_json::from_str(include_str!(
            "../../../fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"
        ))
        .expect("sampling policy should parse");
        let stage_config = PsionPretrainStageConfig::new(
            "run-psion-broad",
            "run-psion-broad-stage-1-pretrain",
            PsionPretrainObjectiveConfig {
                objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
                loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
                label_smoothing_bps: 20,
                tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
                dataset_identity: tokenized_corpus
                    .replay_contract
                    .stable_dataset_identity
                    .clone(),
                max_context_tokens: model_descriptor.config.max_context,
            },
            &model_descriptor,
            &tokenized_corpus,
            &sampling_policy,
        )
        .expect("broader stage config should validate");
        let replay_receipt = PsionPretrainReplayReceipt::new(
            "psion-broad-pretrain-replay-v1",
            tokenized_corpus.replay_contract.stable_dataset_identity.clone(),
            tokenized_corpus.replay_contract.iteration_mode,
            tokenized_corpus.replay_contract.shard_ordering,
            tokenized_corpus.replay_contract.deterministic_shuffle_seed,
            3,
            true,
            "Broader run replay checks matched the tokenized-corpus contract across three recovery rehearsals.",
        );
        let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
            "psion-broad-pretrain-checkpoint-lineage-v1",
            psionic_runtime::TrainingCheckpointReference::new(
                "train.psion.decoder",
                "stream-psion-broad-pretrain-final-v1",
                "manifest-psion-broad-pretrain-final-v1",
                "object-psion-broad-pretrain-final-v1",
                "node-psion-b",
                7,
                "cluster-state-digest-psion-broad-v1",
                "topology-digest-psion-broad-v1",
                1_742_620_000_000,
            )
            .with_checkpoint_ref("checkpoint://psion/broad/pretrain/final")
            .with_step(16384)
            .with_durable_at_ms(1_742_620_900_000),
            None,
            "broader-pretrain-final",
            model_descriptor.model.model_id.clone(),
            model_descriptor.stable_digest(),
        );
        run_psion_pretrain_stage(
            &stage_config,
            vec![
                PsionPretrainSourceFamilyReportRow {
                    split_name: String::from("held_out"),
                    split_kind: psionic_data::DatasetSplitKind::HeldOut,
                    source_family_id: String::from("evaluation_only_benchmark_material"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    token_share_bps_within_split: 10_000,
                    sequence_share_bps_within_split: 10_000,
                    mean_next_token_loss_milli: 1210,
                    detail: String::from(
                        "Held-out benchmark material remains isolated for broader pretraining evaluation.",
                    ),
                },
                PsionPretrainSourceFamilyReportRow {
                    split_name: String::from("train"),
                    split_kind: psionic_data::DatasetSplitKind::Train,
                    source_family_id: String::from("computer_architecture_history"),
                    source_ids: vec![String::from("arch_textbook_foster_1985")],
                    token_share_bps_within_split: 5550,
                    sequence_share_bps_within_split: 5450,
                    mean_next_token_loss_milli: 980,
                    detail: String::from(
                        "Broader run keeps prose slightly ahead while reducing train loss materially.",
                    ),
                },
                PsionPretrainSourceFamilyReportRow {
                    split_name: String::from("train"),
                    split_kind: psionic_data::DatasetSplitKind::Train,
                    source_family_id: String::from("normative_specs"),
                    source_ids: vec![String::from("wasm_core_spec_release_2")],
                    token_share_bps_within_split: 4450,
                    sequence_share_bps_within_split: 4550,
                    mean_next_token_loss_milli: 1035,
                    detail: String::from(
                        "Broader run preserves heavy spec coverage alongside the prose anchor.",
                    ),
                },
                PsionPretrainSourceFamilyReportRow {
                    split_name: String::from("validation"),
                    split_kind: psionic_data::DatasetSplitKind::Validation,
                    source_family_id: String::from("computer_architecture_history"),
                    source_ids: vec![String::from("arch_textbook_foster_1985")],
                    token_share_bps_within_split: 5200,
                    sequence_share_bps_within_split: 5150,
                    mean_next_token_loss_milli: 1015,
                    detail: String::from(
                        "Validation prose stays slightly dominant for broader-run reasoning checks.",
                    ),
                },
                PsionPretrainSourceFamilyReportRow {
                    split_name: String::from("validation"),
                    split_kind: psionic_data::DatasetSplitKind::Validation,
                    source_family_id: String::from("normative_specs"),
                    source_ids: vec![String::from("wasm_core_spec_release_2")],
                    token_share_bps_within_split: 4800,
                    sequence_share_bps_within_split: 4850,
                    mean_next_token_loss_milli: 1080,
                    detail: String::from(
                        "Validation spec coverage remains high so interpretation drift is visible.",
                    ),
                },
            ],
            replay_receipt,
            checkpoint_lineage,
            "Broader Psion pretrain stage scales the explicit next-token lane onto the internal compact decoder while preserving replay and checkpoint lineage.",
            &model_descriptor,
            &tokenized_corpus,
            &sampling_policy,
        )
        .expect("broader stage receipt should validate")
    }

    fn device(
        stable_device_id: &str,
        topology_key: &str,
        total_memory_bytes: u64,
        free_memory_bytes: u64,
    ) -> DeviceInventoryQualifiers {
        DeviceInventoryQualifiers {
            stable_device_id: String::from(stable_device_id),
            topology_key: Some(String::from(topology_key)),
            performance_class: DevicePerformanceClass::DiscreteAccelerator,
            memory_class: DeviceMemoryClass::DedicatedDevice,
            total_memory_bytes: Some(total_memory_bytes),
            free_memory_bytes: Some(free_memory_bytes),
        }
    }

    fn broader_stage_observability_fixture_rebuild(
    ) -> Result<PsionPretrainRunObservabilityReceipt, Box<dyn std::error::Error>> {
        let stage_receipt = broader_stage_receipt();
        let devices = vec![
            device(
                "cuda:h100-0",
                "0000:81:00.0",
                80 * 1024 * 1024 * 1024,
                63 * 1024 * 1024 * 1024,
            ),
            device(
                "cuda:h100-1",
                "0000:82:00.0",
                80 * 1024 * 1024 * 1024,
                61 * 1024 * 1024 * 1024,
            ),
            device(
                "cuda:h100-2",
                "0000:83:00.0",
                80 * 1024 * 1024 * 1024,
                60 * 1024 * 1024 * 1024,
            ),
            device(
                "cuda:h100-3",
                "0000:84:00.0",
                80 * 1024 * 1024 * 1024,
                59 * 1024 * 1024 * 1024,
            ),
        ];
        let topology = ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            0,
            vec![
                (devices[0].clone(), 0, 256),
                (devices[1].clone(), 256, 512),
                (devices[2].clone(), 512, 768),
                (devices[3].clone(), 768, 1024),
            ],
        );
        let membership = TrainingElasticMembershipContext::new(
            7,
            "cluster-state-digest-psion-broad-v1",
            "topology-digest-psion-broad-v1",
            vec![
                String::from("worker-a"),
                String::from("worker-b"),
                String::from("worker-c"),
                String::from("worker-d"),
            ],
        );
        let training_recovery = TrainingRecoveryContext::new(
            TrainingRecoveryPosture::ElasticReconfiguration,
            TrainingCheckpointAvailability::Durable,
            membership.clone(),
        )
        .with_latest_checkpoint(stage_receipt.checkpoint_lineage.promoted_checkpoint.clone())
        .with_recovering_node_ids(vec![String::from("worker-d")])
        .with_requested_at_ms(1_742_620_500_000)
        .with_detail(
            "One worker rejoined after a short topology churn event during the broader run.",
        );
        let collective = TrainingCollectiveContext::new(
            TrainingDeviceMeshContext::new(
                "psion-broad-mesh",
                7,
                "cuda",
                ClusterCommunicationClass::TensorCollectiveMesh,
                membership,
                vec![
                    String::from("worker-a"),
                    String::from("worker-b"),
                    String::from("worker-c"),
                    String::from("worker-d"),
                ],
            ),
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            512 * 1024 * 1024,
            192 * 1024 * 1024,
            4,
        )
        .with_benchmark("psion-broad-collective-benchmark-v1", 1670, 12)
        .with_detail(
            "Tensor-parallel gradient reductions stayed on the justified int8 collective lane.",
        );
        let cluster_execution = ClusterExecutionContext::new(
            "cluster-psion-trusted-a",
            "cluster-state-digest-psion-broad-v1",
            "topology-digest-psion-broad-v1",
            "scheduler-psion-a",
            ClusterTransportClass::TrustedLanStream,
            ClusterExecutionDisposition::Sharded,
        )
        .with_execution_topology(topology.clone())
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(devices[0].clone()),
            ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(devices[1].clone()),
            ClusterSelectedNode::new("worker-c", "cuda").with_device_inventory(devices[2].clone()),
            ClusterSelectedNode::new("worker-d", "cuda").with_device_inventory(devices[3].clone()),
        ])
        .with_training_recovery(training_recovery)
        .with_training_collective(collective);
        let hardware_topology = PsionPretrainHardwareTopologyReceipt::new(
            4,
            DeliveredExecutionContext::new("cuda", Some(topology), devices)
                .with_cluster_execution(cluster_execution),
            "Broader pretraining run preserved explicit tensor-sharded cluster topology and recovery facts.",
        )?;
        let telemetry = TrainingInstabilityTelemetry::default()
            .with_entropy_drift_bps(180)
            .with_checkpoint_catchup_latency_ms(2400)
            .with_topology_churn_events(2)
            .with_environment_failure_rate_bps(120)
            .with_sandbox_failure_rate_bps(45);
        let stability_verdict = TrainingStabilityController::new(TrainingInstabilityPolicy::new(
            vec![
                TrainingInstabilityRule {
                    signal: TrainingInstabilitySignalKind::EntropyDriftBps,
                    max_value: 100.0,
                    action: TrainingOperationalAction::Continue,
                },
                TrainingInstabilityRule {
                    signal: TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs,
                    max_value: 1500.0,
                    action: TrainingOperationalAction::Quarantine,
                },
                TrainingInstabilityRule {
                    signal: TrainingInstabilitySignalKind::TopologyChurnEvents,
                    max_value: 0.0,
                    action: TrainingOperationalAction::Continue,
                },
            ],
            Vec::new(),
        ))
        .evaluate(&telemetry, &[]);
        Ok(record_psion_pretrain_run_observability(
            "psion-broader-pretrain-observability-v1",
            PsionPretrainRunScaleProfile::BroaderPretraining,
            PsionPretrainRunCostReceipt {
                cost_basis: PsionPretrainRunCostBasis::MeteredUsd,
                currency_code: String::from("USD"),
                compute_cost_microusd: 487_250_000,
                storage_cost_microusd: 18_600_000,
                network_cost_microusd: 6_400_000,
                total_cost_microusd: 512_250_000,
                detail: String::from(
                    "Broader run cost reflects metered trusted-cluster accelerator time plus checkpoint storage and east-west traffic.",
                ),
            },
            PsionPretrainRunThroughputReceipt {
                train_tokens_processed: 1_073_741_824,
                validation_tokens_processed: 33_554_432,
                held_out_tokens_scored: 8_388_608,
                optimizer_steps_completed: 16_384,
                wall_clock_ms: 3_780_000,
                mean_tokens_per_second: 296_214,
                peak_tokens_per_second: 331_442,
                mean_sequences_per_second_milli: 72_500,
                mean_step_latency_ms: 231,
                checkpoint_write_throughput_bytes_per_second: 1_476_395_008,
            },
            PsionPretrainCheckpointArtifactReceipt {
                promoted_checkpoint_label: stage_receipt
                    .checkpoint_lineage
                    .promoted_checkpoint_label
                    .clone(),
                checkpoint_family: stage_receipt
                    .checkpoint_lineage
                    .promoted_checkpoint
                    .checkpoint_family
                    .clone(),
                checkpoint_object_digest: stage_receipt
                    .checkpoint_lineage
                    .promoted_checkpoint
                    .object_digest
                    .clone(),
                checkpoint_size_bytes: 1_546_182_656,
                optimizer_state_size_bytes: 773_091_328,
                ancillary_artifact_size_bytes: 14_680_064,
                total_artifact_size_bytes: 2_333_954_048,
                shard_count: 8,
                detail: String::from(
                    "Broader run artifact surface includes sharded weights, optimizer state, and receipt/descriptor sidecars.",
                ),
            },
            hardware_topology,
            telemetry,
            Some(stability_verdict),
            "Broader pretraining observability receipt records scale-up throughput, metered cost, checkpoint size, cluster topology, and structured instability markers.",
            &stage_receipt,
        )?)
    }

    #[test]
    fn pilot_pretrain_run_observability_receipt_validates_against_stage_receipt() {
        pilot_observability_receipt()
            .validate_against_stage(&pilot_stage_receipt())
            .expect("pilot observability receipt should validate");
    }

    #[test]
    fn broader_pretrain_run_observability_receipt_preserves_cluster_topology_and_instability_markers(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let broader_receipt = broader_observability_receipt();
        broader_receipt.validate_against_stage(&broader_stage_receipt())?;
        let rebuilt = broader_stage_observability_fixture_rebuild()?;
        assert_eq!(
            rebuilt.observability_digest,
            broader_receipt.observability_digest
        );
        assert_eq!(
            broader_receipt
                .stability_verdict
                .as_ref()
                .map(|verdict| verdict.signal_receipts.len()),
            Some(3)
        );
        assert_eq!(
            broader_receipt
                .hardware_topology
                .delivered_execution
                .topology_kind(),
            Some(ExecutionTopologyKind::TensorSharded)
        );
        Ok(())
    }

    #[test]
    fn stage_observability_summary_requires_pilot_and_broader_profiles() {
        let mut summary = observability_summary();
        summary.rows.pop();
        let error = summary
            .validate_against_receipts(&[
                pilot_observability_receipt(),
                broader_observability_receipt(),
            ])
            .expect_err("summary should require both run profiles");
        assert!(matches!(
            error,
            PsionPretrainRunObservabilityError::SummaryReceiptCountMismatch { .. }
                | PsionPretrainRunObservabilityError::MissingRunProfile { .. }
        ));
    }

    #[test]
    fn checkpoint_artifact_must_match_promoted_checkpoint_identity() {
        let mut receipt = pilot_observability_receipt();
        receipt.checkpoint_artifact.checkpoint_object_digest = String::from("wrong-object-digest");
        let error = receipt
            .validate_against_stage(&pilot_stage_receipt())
            .expect_err("checkpoint artifact mismatch should be rejected");
        assert!(matches!(
            error,
            PsionPretrainRunObservabilityError::FieldMismatch { .. }
        ));
    }

    #[test]
    fn stage_observability_summary_validates_against_source_receipts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let receipts = vec![
            pilot_observability_receipt(),
            broader_observability_receipt(),
        ];
        observability_summary().validate_against_receipts(receipts.as_slice())?;
        let rebuilt = summarize_psion_pretrain_observability_runs(
            "psion-pretrain-stage-observability-v1",
            receipts.as_slice(),
            "Pilot and broader-pretraining observability receipts now expose the minimum budgeting, checkpoint, topology, and instability surface required for Psion scale-up decisions.",
        )?;
        assert_eq!(
            rebuilt.summary_digest,
            observability_summary().summary_digest
        );
        Ok(())
    }
}
