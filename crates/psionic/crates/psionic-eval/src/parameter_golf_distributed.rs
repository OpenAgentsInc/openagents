use psionic_runtime::{
    BackendSelection, ClusterCommunicationClass, ClusterTransportClass, TrainingCollectiveKind,
    TrainingCollectiveQuantization, TrainingDeviceMeshAxis,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::EvalArtifact;

/// Canonical benchmark reference for the distributed `8xH100` Parameter Golf lane.
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF: &str =
    "benchmark://openagents/psionic/parameter_golf/distributed_8xh100";

/// Claim boundary carried by the distributed `8xH100` receipt lane.
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY: &str = "exact 8xH100 DDP-style topology, communication, wallclock, and memory receipts are explicit, but challenge-ready CUDA kernel and runtime widening remains a separate closure item";

/// Explicit challenge bar for the distributed `8xH100` lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedChallengeThresholds {
    /// Required effective backend.
    pub required_backend: String,
    /// Required device-family substring.
    pub required_device_name: String,
    /// Required world size.
    pub required_world_size: usize,
    /// Maximum total observed wallclock for the run.
    pub max_total_wallclock_ms: u64,
    /// Maximum peak device bytes per worker.
    pub max_peak_device_bytes_per_worker: u64,
    /// Maximum peak host bytes per worker.
    pub max_peak_host_bytes_per_worker: u64,
}

impl ParameterGolfDistributedChallengeThresholds {
    /// Returns the canonical public `8xH100` challenge thresholds.
    #[must_use]
    pub fn challenge_8xh100() -> Self {
        Self {
            required_backend: String::from("cuda"),
            required_device_name: String::from("H100"),
            required_world_size: 8,
            max_total_wallclock_ms: 600_000,
            max_peak_device_bytes_per_worker: 80 * 1024 * 1024 * 1024,
            max_peak_host_bytes_per_worker: 16 * 1024 * 1024 * 1024,
        }
    }
}

/// Final disposition for one distributed Parameter Golf lane receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributedLaneDisposition {
    /// The lane was admitted and the measured run stayed inside the declared bar.
    Measured,
    /// The lane was refused because admission or measured facts failed the declared bar.
    Refused,
}

/// Refusal class for one distributed Parameter Golf lane receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributedLaneRefusalKind {
    /// The provided inventory does not satisfy the required `8xH100` posture.
    DeviceInventoryMismatch,
    /// The runtime capability profile does not expose the required collective posture.
    CapabilityMismatch,
    /// No observed training timings were supplied for the receipt.
    MeasurementsMissing,
    /// The measured or planned memory posture exceeded the declared device or host bar.
    MemoryBudgetExceeded,
    /// The measured wallclock exceeded the declared challenge cap.
    WallclockExceeded,
}

/// Stable topology and backend-selection facts for one distributed Parameter Golf receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedTopologyReceipt {
    /// Runtime selection that owned the distributed lane.
    pub backend_selection: BackendSelection,
    /// Stable topology digest for the selected execution path.
    pub topology_digest: String,
    /// Stable names of the selected devices.
    pub selected_device_names: Vec<String>,
    /// Whether every selected device matched the required model family.
    pub all_devices_match_required_model: bool,
}

/// One communication stage in the distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedCommunicationStageReceipt {
    /// Stable stage identifier.
    pub stage_id: String,
    /// High-level collective kind.
    pub collective_kind: TrainingCollectiveKind,
    /// Communication quantization used by the stage.
    pub quantization: TrainingCollectiveQuantization,
    /// Logical payload bytes reduced or broadcast by the stage.
    pub payload_bytes: u64,
    /// Estimated wire bytes for the chosen collective plan.
    pub estimated_wire_bytes: u64,
    /// Number of workers participating in the stage.
    pub worker_count: usize,
    /// Plain-language detail.
    pub detail: String,
}

/// Aggregate communication posture for one distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedCommunicationReceipt {
    /// Communication class required by the lane.
    pub communication_class: ClusterCommunicationClass,
    /// Transport used for the benchmarked collective mesh.
    pub transport: ClusterTransportClass,
    /// Stable mesh identifier.
    pub mesh_id: String,
    /// Explicit device-mesh axes.
    pub axes: Vec<TrainingDeviceMeshAxis>,
    /// Ordered collective stages realized by the lane.
    pub stages: Vec<ParameterGolfDistributedCommunicationStageReceipt>,
}

/// Timing facts for one distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedTimingReceipt {
    /// Stable description of how the timing facts were collected.
    pub measurement_posture: String,
    /// Number of measured optimizer steps.
    pub step_count: u64,
    /// Total train tokens represented by the measured steps.
    pub total_train_tokens: u64,
    /// Summed observed step duration.
    pub training_step_observed_ms: u64,
    /// Observed validation duration.
    pub validation_observed_ms: u64,
    /// Observed export or roundtrip duration.
    pub export_observed_ms: u64,
    /// Total observed duration across training, validation, and export.
    pub total_observed_ms: u64,
    /// Mean observed step duration.
    pub mean_step_duration_ms: u64,
    /// Tail observed step duration.
    pub tail_step_duration_ms: u64,
    /// Whole train tokens per second realized by the measured steps.
    pub train_tokens_per_second: u64,
    /// Declared wallclock cap applied to this receipt.
    pub wallclock_cap_ms: u64,
    /// Whether the measured total stayed within the cap.
    pub within_wallclock_cap: bool,
}

/// One rank-local validation shard in the distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedValidationShardReceipt {
    /// Zero-based rank identifier.
    pub rank: usize,
    /// Zero-based global validation-sequence offset covered by this rank's scored tokens.
    pub sequence_start: u64,
    /// Number of validation sequences covered by this rank's scored tokens.
    pub sequence_count: u64,
    /// Zero-based global validation evaluation-unit offset owned by this rank.
    pub evaluation_unit_start: u64,
    /// Number of validation evaluation units owned by this rank.
    pub evaluation_unit_count: u64,
    /// Zero-based scored-token offset owned by this rank.
    pub scored_token_start: u64,
    /// Number of scored tokens owned by this rank.
    pub scored_token_count: u64,
    /// Expected local batch size in sequences or windows for this rank.
    pub local_batch_sequences: u64,
    /// Rank-local summed loss over the shard.
    pub loss_sum: f64,
    /// Rank-local evaluated token count.
    pub token_count: u64,
    /// Rank-local evaluated byte count.
    pub byte_count: u64,
    /// Rank-local observed validation wallclock.
    pub observed_ms: u64,
}

/// Aggregated distributed validation facts for one Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedValidationAggregationReceipt {
    /// Stable description of how the validation facts were collected.
    pub measurement_posture: String,
    /// Validation evaluation mode used by the lane.
    pub eval_mode: String,
    /// World size used by the distributed validation lane.
    pub world_size: usize,
    /// Total validation sequence count across all ranks.
    pub total_sequence_count: u64,
    /// Total validation evaluation-unit count across all ranks.
    pub total_evaluation_unit_count: u64,
    /// Expected local batch size in sequences or windows for each rank.
    pub local_batch_sequences: u64,
    /// Aggregated loss sum reduced across ranks.
    pub aggregated_loss_sum: f64,
    /// Aggregated evaluated token count reduced across ranks.
    pub aggregated_token_count: u64,
    /// Aggregated evaluated byte count reduced across ranks.
    pub aggregated_byte_count: u64,
    /// Final aggregated validation mean loss.
    pub mean_loss: f64,
    /// Final aggregated validation bits-per-byte.
    pub bits_per_byte: f64,
    /// Honest end-to-end validation wallclock for the lane.
    pub observed_ms: u64,
    /// Ordered rank-local shard receipts.
    pub shards: Vec<ParameterGolfDistributedValidationShardReceipt>,
}

/// Memory facts for one distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedMemoryReceipt {
    /// Stable description of how the memory facts were collected.
    pub measurement_posture: String,
    /// Declared device-memory bar per worker.
    pub declared_device_budget_bytes_per_worker: u64,
    /// Declared host-memory bar per worker.
    pub declared_host_budget_bytes_per_worker: u64,
    /// Logical parameter bytes across the run.
    pub parameter_logical_bytes: u64,
    /// Logical gradient bytes across the run.
    pub gradient_logical_bytes: u64,
    /// Logical optimizer-state bytes across the run.
    pub optimizer_state_logical_bytes: u64,
    /// Logical master-weight bytes across the run.
    pub master_weight_logical_bytes: u64,
    /// Estimated activation peak bytes per worker.
    pub activation_peak_bytes: u64,
    /// Activation bytes saved by checkpointing, when any.
    pub activation_bytes_saved: u64,
    /// Peak device bytes per worker.
    pub peak_device_bytes_per_worker: u64,
    /// Peak host bytes per worker.
    pub peak_host_bytes_per_worker: u64,
    /// Remote-offloaded optimizer bytes excluded from host or device peaks.
    pub remote_offloaded_optimizer_bytes: u64,
    /// Whether the measured or planned device posture stayed within the declared bar.
    pub within_device_budget: bool,
    /// Whether the measured or planned host posture stayed within the declared bar.
    pub within_host_budget: bool,
    /// Stable planner digest that produced the memory facts.
    pub planner_receipt_digest: String,
}

/// Refusal posture for one distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedLaneRefusal {
    /// Stable refusal class.
    pub refusal_kind: ParameterGolfDistributedLaneRefusalKind,
    /// Plain-language refusal detail.
    pub reason: String,
    /// Benchmark reference preserved as the fallback review lane.
    pub fallback_benchmark_ref: String,
}

/// Aggregate throughput receipt for one distributed Parameter Golf lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributedThroughputReceipt {
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable Parameter Golf model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable optimizer-plan digest.
    pub optimizer_plan_digest: String,
    /// Declared challenge thresholds.
    pub thresholds: ParameterGolfDistributedChallengeThresholds,
    /// Stable topology and backend-selection facts.
    pub topology: ParameterGolfDistributedTopologyReceipt,
    /// Stable communication facts.
    pub communication: ParameterGolfDistributedCommunicationReceipt,
    /// Stable digest of the aligned CUDA training capability report.
    pub training_capability_report_digest: String,
    /// Stable blocker case identifiers for remaining CUDA train-path gaps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenge_kernel_blockers: Vec<String>,
    /// Final receipt disposition.
    pub disposition: ParameterGolfDistributedLaneDisposition,
    /// Timing facts when they exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<ParameterGolfDistributedTimingReceipt>,
    /// Distributed validation shard and aggregation facts when they exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_aggregation: Option<ParameterGolfDistributedValidationAggregationReceipt>,
    /// Memory facts when they exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<ParameterGolfDistributedMemoryReceipt>,
    /// Refusal posture when the lane did not clear the bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<ParameterGolfDistributedLaneRefusal>,
    /// Honest boundary notes preserved with the receipt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boundary_notes: Vec<String>,
    /// Explicit claim boundary for the receipt.
    pub claim_boundary: String,
    /// Stable digest over the receipt contents.
    pub receipt_digest: String,
}

impl ParameterGolfDistributedThroughputReceipt {
    /// Returns a digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_distributed_throughput_receipt|",
            &digestible,
        )
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }

    /// Returns the receipt as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "parameter_golf_distributed_throughput_receipt",
            "parameter_golf_distributed_throughput_receipt.json",
            &bytes,
        )
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use psionic_runtime::{
        BackendSelection, ClusterCommunicationClass, ClusterTransportClass, TrainingCollectiveKind,
        TrainingCollectiveQuantization, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    };

    use crate::{
        ParameterGolfDistributedChallengeThresholds, ParameterGolfDistributedCommunicationReceipt,
        ParameterGolfDistributedCommunicationStageReceipt, ParameterGolfDistributedLaneDisposition,
        ParameterGolfDistributedMemoryReceipt, ParameterGolfDistributedThroughputReceipt,
        ParameterGolfDistributedTimingReceipt, ParameterGolfDistributedTopologyReceipt,
        ParameterGolfDistributedValidationAggregationReceipt,
        ParameterGolfDistributedValidationShardReceipt,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY,
    };

    #[test]
    fn parameter_golf_distributed_throughput_receipt_round_trips() -> Result<(), Box<dyn Error>> {
        let receipt = ParameterGolfDistributedThroughputReceipt {
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            run_id: String::from("parameter-golf-distributed-test"),
            model_descriptor_digest: String::from("model-digest"),
            optimizer_plan_digest: String::from("optimizer-digest"),
            thresholds: ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            topology: ParameterGolfDistributedTopologyReceipt {
                backend_selection: BackendSelection::direct(
                    "cuda",
                    None,
                    vec![String::from("parameter_golf_distributed_train")],
                ),
                topology_digest: String::from("topology-digest"),
                selected_device_names: vec![String::from("NVIDIA H100 80GB HBM3"); 8],
                all_devices_match_required_model: true,
            },
            communication: ParameterGolfDistributedCommunicationReceipt {
                communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
                transport: ClusterTransportClass::Loopback,
                mesh_id: String::from("mesh.parameter_golf.8xh100"),
                axes: vec![TrainingDeviceMeshAxis::new(
                    "dp",
                    TrainingDeviceMeshAxisKind::DataParallel,
                    8,
                )
                .with_collective_group_size(8)],
                stages: vec![
                    ParameterGolfDistributedCommunicationStageReceipt {
                        stage_id: String::from("ddp_gradient_all_reduce"),
                        collective_kind: TrainingCollectiveKind::AllReduce,
                        quantization: TrainingCollectiveQuantization::None,
                        payload_bytes: 1024,
                        estimated_wire_bytes: 2048,
                        worker_count: 8,
                        detail: String::from("DDP gradient synchronization"),
                    },
                    ParameterGolfDistributedCommunicationStageReceipt {
                        stage_id: String::from("muon_matrix_update_all_reduce"),
                        collective_kind: TrainingCollectiveKind::AllReduce,
                        quantization: TrainingCollectiveQuantization::None,
                        payload_bytes: 2048,
                        estimated_wire_bytes: 4096,
                        worker_count: 8,
                        detail: String::from("Muon rank-local update synchronization"),
                    },
                ],
            },
            training_capability_report_digest: String::from("cuda-coverage-digest"),
            challenge_kernel_blockers: Vec::new(),
            disposition: ParameterGolfDistributedLaneDisposition::Measured,
            timing: Some(ParameterGolfDistributedTimingReceipt {
                measurement_posture: String::from("observed_step_wallclock"),
                step_count: 2,
                total_train_tokens: 262_144,
                training_step_observed_ms: 140,
                validation_observed_ms: 20,
                export_observed_ms: 8,
                total_observed_ms: 168,
                mean_step_duration_ms: 70,
                tail_step_duration_ms: 75,
                train_tokens_per_second: 1_872_457,
                wallclock_cap_ms: 600_000,
                within_wallclock_cap: true,
            }),
            validation_aggregation: Some(ParameterGolfDistributedValidationAggregationReceipt {
                measurement_posture: String::from(
                    "rank_local_validation_shards_plus_metric_all_reduce",
                ),
                eval_mode: String::from("non_overlapping"),
                world_size: 8,
                total_sequence_count: 512,
                total_evaluation_unit_count: 512,
                local_batch_sequences: 64,
                aggregated_loss_sum: 96.0,
                aggregated_token_count: 131_072,
                aggregated_byte_count: 104_857,
                mean_loss: 96.0 / 131_072.0,
                bits_per_byte: 1.271828,
                observed_ms: 20,
                shards: vec![
                    ParameterGolfDistributedValidationShardReceipt {
                        rank: 0,
                        sequence_start: 0,
                        sequence_count: 64,
                        evaluation_unit_start: 0,
                        evaluation_unit_count: 64,
                        scored_token_start: 0,
                        scored_token_count: 16_384,
                        local_batch_sequences: 64,
                        loss_sum: 12.0,
                        token_count: 16_384,
                        byte_count: 13_107,
                        observed_ms: 19,
                    },
                    ParameterGolfDistributedValidationShardReceipt {
                        rank: 1,
                        sequence_start: 64,
                        sequence_count: 64,
                        evaluation_unit_start: 64,
                        evaluation_unit_count: 64,
                        scored_token_start: 16_384,
                        scored_token_count: 16_384,
                        local_batch_sequences: 64,
                        loss_sum: 12.0,
                        token_count: 16_384,
                        byte_count: 13_107,
                        observed_ms: 20,
                    },
                ],
            }),
            memory: Some(ParameterGolfDistributedMemoryReceipt {
                measurement_posture: String::from("analytic_optimizer_contract_upper_bound"),
                declared_device_budget_bytes_per_worker: 80 * 1024 * 1024 * 1024,
                declared_host_budget_bytes_per_worker: 16 * 1024 * 1024 * 1024,
                parameter_logical_bytes: 10_000_000,
                gradient_logical_bytes: 10_000_000,
                optimizer_state_logical_bytes: 20_000_000,
                master_weight_logical_bytes: 20_000_000,
                activation_peak_bytes: 2_000_000_000,
                activation_bytes_saved: 0,
                peak_device_bytes_per_worker: 2_060_000_000,
                peak_host_bytes_per_worker: 0,
                remote_offloaded_optimizer_bytes: 0,
                within_device_budget: true,
                within_host_budget: true,
                planner_receipt_digest: String::from("planner-digest"),
            }),
            refusal: None,
            boundary_notes: vec![String::from(
                "CUDA kernel and runtime widening is tracked separately from this receipt lane.",
            )],
            claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        }
        .with_stable_digest();
        let artifact = receipt.as_artifact();
        assert_eq!(
            artifact.artifact_kind,
            "parameter_golf_distributed_throughput_receipt"
        );
        let decoded: ParameterGolfDistributedThroughputReceipt =
            serde_json::from_slice(&serde_json::to_vec(&receipt)?)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.receipt_digest, receipt.stable_digest());
        Ok(())
    }
}
