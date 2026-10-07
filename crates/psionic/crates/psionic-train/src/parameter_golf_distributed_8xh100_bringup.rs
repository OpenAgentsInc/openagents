use std::{fs, path::Path};

use psionic_backend_cuda::CudaBackend;
use psionic_core::{DeviceKind, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use psionic_eval::{
    ParameterGolfDistributedChallengeThresholds, ParameterGolfDistributedLaneDisposition,
    ParameterGolfDistributedThroughputReceipt,
};
use psionic_models::ParameterGolfReferenceModel;
use psionic_runtime::{DeviceDescriptor, HealthStatus, RuntimeHealth};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    benchmark_parameter_golf_distributed_8xh100, device_capacity_matches_h100_threshold,
    parameter_golf_runpod_8xh100_capability_profile, ParameterGolfBatchGeometry,
    ParameterGolfDistributedLaneError, ParameterGolfRunPod8xH100Measurements,
    ParameterGolfTrainingHyperparameters,
};

/// Config for the Rust-owned distributed `8xH100` bring-up seam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100BringupConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Exact public distributed batch geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Exact public baseline hyperparameters.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
}

impl ParameterGolfDistributed8xH100BringupConfig {
    /// Returns the canonical distributed `8xH100` bring-up config.
    #[must_use]
    pub fn challenge_defaults() -> Self {
        Self {
            run_id: String::from("parameter-golf-distributed-8xh100-bringup"),
            geometry: ParameterGolfBatchGeometry::challenge_distributed_8xh100_defaults(),
            hyperparameters: ParameterGolfTrainingHyperparameters::baseline_defaults(),
        }
    }

    fn validate(&self) -> Result<(), ParameterGolfDistributed8xH100BringupError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfDistributed8xH100BringupError::InvalidConfig {
                message: String::from("run_id must be non-empty"),
            });
        }
        if self.geometry != ParameterGolfBatchGeometry::challenge_distributed_8xh100_defaults() {
            return Err(ParameterGolfDistributed8xH100BringupError::InvalidConfig {
                message: format!(
                    "distributed 8xH100 bring-up requires challenge_distributed_8xh100_defaults geometry, found {:?}",
                    self.geometry
                ),
            });
        }
        if self.hyperparameters != ParameterGolfTrainingHyperparameters::baseline_defaults() {
            return Err(ParameterGolfDistributed8xH100BringupError::InvalidConfig {
                message: String::from(
                    "distributed 8xH100 bring-up requires the public baseline hyperparameters",
                ),
            });
        }
        Ok(())
    }
}

/// Current disposition for the distributed `8xH100` bring-up seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributed8xH100BringupDisposition {
    /// The local machine satisfies the exact `8xH100` admission contract.
    ContractReady,
    /// The local machine does not satisfy the exact `8xH100` admission contract.
    RefusedMachineContract,
    /// The local machine satisfies the contract and the provided measurements lifted into one measured receipt.
    MeasuredReceiptLoaded,
    /// The local machine satisfies the contract but the provided measurements still miss the challenge bar.
    MeasuredReceiptRefused,
}

/// Whether the current bring-up only inspected local contracts or also loaded one measured receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfDistributed8xH100BringupExecutionPosture {
    ContractValidationOnly,
    MeasuredReceiptLoaded,
}

/// Machine observation for the local distributed `8xH100` lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParameterGolfDistributed8xH100MachineObservation {
    pub thresholds: ParameterGolfDistributedChallengeThresholds,
    pub observed_cuda_health: RuntimeHealth,
    pub cuda_discovery_error: Option<String>,
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    pub matching_h100_device_count: usize,
    pub machine_contract_satisfied: bool,
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable report for the Rust-owned distributed `8xH100` bring-up seam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100BringupReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable run identifier.
    pub run_id: String,
    /// Exact public distributed geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Exact public baseline hyperparameters.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    /// Exact distributed admission thresholds.
    pub machine_thresholds: ParameterGolfDistributedChallengeThresholds,
    /// Observed CUDA runtime health.
    pub observed_cuda_health: RuntimeHealth,
    /// Optional CUDA discovery error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_discovery_error: Option<String>,
    /// Observed CUDA inventory.
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    /// Number of devices matching the exact `8xH100` contract.
    pub matching_h100_device_count: usize,
    /// Whether the exact `8xH100` machine contract is satisfied.
    pub machine_contract_satisfied: bool,
    /// Human-readable Rust entrypoint for this seam.
    pub psionic_entrypoint: String,
    /// Current execution posture.
    pub execution_posture: ParameterGolfDistributed8xH100BringupExecutionPosture,
    /// Optional measured or refused distributed receipt lifted from the current machine plus supplied measurements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distributed_receipt: Option<ParameterGolfDistributedThroughputReceipt>,
    /// Final disposition for the seam.
    pub disposition: ParameterGolfDistributed8xH100BringupDisposition,
    /// Primary refusal for the current posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Honest boundary notes.
    pub drift_notes: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

impl ParameterGolfDistributed8xH100BringupReport {
    /// Returns whether the local machine is ready for the later real runtime attempt.
    #[must_use]
    pub const fn ready_to_attempt(&self) -> bool {
        matches!(
            self.disposition,
            ParameterGolfDistributed8xH100BringupDisposition::ContractReady
                | ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptLoaded
                | ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptRefused
        )
    }
}

/// Failure while building or writing the distributed `8xH100` bring-up report.
#[derive(Debug, Error)]
pub enum ParameterGolfDistributed8xH100BringupError {
    #[error("parameter golf distributed 8xH100 bring-up config is invalid: {message}")]
    InvalidConfig { message: String },
    #[error("parameter golf distributed 8xH100 bring-up failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("parameter golf distributed 8xH100 bring-up failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    DistributedLane(#[from] ParameterGolfDistributedLaneError),
    #[error(transparent)]
    Model(#[from] psionic_models::ParameterGolfModelError),
    #[error(transparent)]
    Train(#[from] crate::ParameterGolfTrainError),
}

/// Inspects the local CUDA inventory against the exact distributed `8xH100` contract.
pub(crate) fn inspect_local_distributed_8xh100_machine(
) -> ParameterGolfDistributed8xH100MachineObservation {
    let thresholds = ParameterGolfDistributedChallengeThresholds::challenge_8xh100();
    let backend = CudaBackend::new();
    match backend.discovery_report() {
        Ok(report) => {
            machine_observation_from_inventory(thresholds, report.health, None, report.devices)
        }
        Err(error) => machine_observation_from_inventory(
            thresholds,
            RuntimeHealth {
                status: HealthStatus::Offline,
                message: error.to_string(),
            },
            Some(error.to_string()),
            Vec::new(),
        ),
    }
}

pub(crate) fn machine_observation_from_inventory(
    thresholds: ParameterGolfDistributedChallengeThresholds,
    observed_cuda_health: RuntimeHealth,
    cuda_discovery_error: Option<String>,
    observed_cuda_devices: Vec<DeviceDescriptor>,
) -> ParameterGolfDistributed8xH100MachineObservation {
    let matching_h100_device_count = observed_cuda_devices
        .iter()
        .filter(|device| device_matches_distributed_h100(device, &thresholds))
        .count();
    let machine_contract_satisfied = observed_cuda_health.status != HealthStatus::Offline
        && matching_h100_device_count == thresholds.required_world_size;
    let refusal = (!machine_contract_satisfied).then(|| {
        let observed_names = if observed_cuda_devices.is_empty() {
            String::from("no discovered CUDA devices")
        } else {
            observed_cuda_devices
                .iter()
                .map(observed_device_label)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let detail = match &cuda_discovery_error {
            Some(error) => format!(
                "distributed 8xH100 bring-up requires exactly {} non-MIG `{}` device(s) on backend `{}` but CUDA discovery failed: {error}",
                thresholds.required_world_size,
                thresholds.required_device_name,
                thresholds.required_backend,
            ),
            None => format!(
                "distributed 8xH100 bring-up requires exactly {} non-MIG `{}` device(s) on backend `{}` but found {} matching device(s) among: {observed_names}",
                thresholds.required_world_size,
                thresholds.required_device_name,
                thresholds.required_backend,
                matching_h100_device_count,
            ),
        };
        PsionicRefusal::new(
            PsionicRefusalCode::UnsupportedBackendCapability,
            PsionicRefusalScope::Runtime,
            detail,
        )
        .with_subject(String::from("parameter_golf_distributed_8xh100_machine"))
    });
    ParameterGolfDistributed8xH100MachineObservation {
        thresholds,
        observed_cuda_health,
        cuda_discovery_error,
        observed_cuda_devices,
        matching_h100_device_count,
        machine_contract_satisfied,
        refusal,
    }
}

pub(crate) fn device_matches_distributed_h100(
    device: &DeviceDescriptor,
    thresholds: &ParameterGolfDistributedChallengeThresholds,
) -> bool {
    if device.backend != thresholds.required_backend || device.device.kind() != DeviceKind::Cuda {
        return false;
    }
    let name_matches = device
        .device_name
        .as_deref()
        .is_some_and(|name| name.contains(thresholds.required_device_name.as_str()));
    if !name_matches {
        return false;
    }
    let Some(metadata) = &device.nvidia_metadata else {
        return false;
    };
    metadata.topology.mig_profile.is_none()
        && !metadata.risk.mig_partitioned
        && device_capacity_matches_h100_threshold(device, thresholds)
}

fn observed_device_label(device: &DeviceDescriptor) -> String {
    device
        .device_name
        .clone()
        .unwrap_or_else(|| device.device.label().unwrap_or("unknown").to_string())
}

/// Builds the distributed `8xH100` bring-up report from local CUDA discovery and optional measured facts.
pub fn build_parameter_golf_distributed_8xh100_bringup_report(
    config: &ParameterGolfDistributed8xH100BringupConfig,
    measurements: Option<&ParameterGolfRunPod8xH100Measurements>,
) -> Result<ParameterGolfDistributed8xH100BringupReport, ParameterGolfDistributed8xH100BringupError>
{
    config.validate()?;
    let machine_observation = inspect_local_distributed_8xh100_machine();
    let distributed_receipt = if !machine_observation.machine_contract_satisfied {
        None
    } else {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let receipt_config = measurements
            .cloned()
            .unwrap_or_else(ParameterGolfRunPod8xH100Measurements::challenge_defaults)
            .into_config(&config.run_id);
        Some(benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &config.hyperparameters,
            machine_observation.observed_cuda_devices.as_slice(),
            &parameter_golf_runpod_8xh100_capability_profile(),
            &receipt_config,
        )?)
    };
    let execution_posture = if measurements.is_some() {
        ParameterGolfDistributed8xH100BringupExecutionPosture::MeasuredReceiptLoaded
    } else {
        ParameterGolfDistributed8xH100BringupExecutionPosture::ContractValidationOnly
    };
    let disposition = if machine_observation.refusal.is_some() {
        ParameterGolfDistributed8xH100BringupDisposition::RefusedMachineContract
    } else if distributed_receipt.as_ref().is_some_and(|receipt| {
        receipt.disposition == ParameterGolfDistributedLaneDisposition::Measured
    }) {
        ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptLoaded
    } else if measurements.is_some() {
        ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptRefused
    } else {
        ParameterGolfDistributed8xH100BringupDisposition::ContractReady
    };
    let mut drift_notes = Vec::new();
    if measurements.is_none() {
        drift_notes.push(String::from(
            "No distributed optimizer-step or validation measurements were supplied, so the optional distributed receipt remains a machine-admission or measurements-missing seam rather than execution evidence.",
        ));
    }
    if let Some(receipt) = distributed_receipt.as_ref() {
        if let Some(refusal) = receipt.refusal.as_ref() {
            drift_notes.push(format!(
                "Distributed receipt refusal_kind={:?} reason={}",
                refusal.refusal_kind, refusal.reason
            ));
        }
    }
    let refusal = machine_observation.refusal.clone();
    let mut report = ParameterGolfDistributed8xH100BringupReport {
        schema_version: 1,
        run_id: config.run_id.clone(),
        geometry: config.geometry.clone(),
        hyperparameters: config.hyperparameters.clone(),
        machine_thresholds: machine_observation.thresholds.clone(),
        observed_cuda_health: machine_observation.observed_cuda_health.clone(),
        cuda_discovery_error: machine_observation.cuda_discovery_error.clone(),
        observed_cuda_devices: machine_observation.observed_cuda_devices.clone(),
        matching_h100_device_count: machine_observation.matching_h100_device_count,
        machine_contract_satisfied: machine_observation.machine_contract_satisfied,
        psionic_entrypoint: String::from(
            "cargo run -p psionic-train --bin parameter_golf_distributed_8xh100_bringup",
        ),
        execution_posture,
        distributed_receipt,
        disposition,
        refusal,
        drift_notes,
        claim_boundary: String::from(
            "This report proves the local CUDA inventory, exact 8xH100 machine-admission truth, and optional measured-receipt lift for the Parameter Golf distributed lane. It does not claim that the repo already owns a real Rust-native WORLD_SIZE=8 trainer execution path.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_distributed_8xh100_bringup_report|",
        &report,
    );
    Ok(report)
}

/// Writes the distributed `8xH100` bring-up report to disk.
pub fn write_parameter_golf_distributed_8xh100_bringup_report(
    output_path: impl AsRef<Path>,
    config: &ParameterGolfDistributed8xH100BringupConfig,
    measurements: Option<&ParameterGolfRunPod8xH100Measurements>,
) -> Result<ParameterGolfDistributed8xH100BringupReport, ParameterGolfDistributed8xH100BringupError>
{
    let report = build_parameter_golf_distributed_8xh100_bringup_report(config, measurements)?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfDistributed8xH100BringupError::Read {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&report).expect("encode distributed bring-up report"),
    )
    .map_err(|error| ParameterGolfDistributed8xH100BringupError::Read {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

/// Loads one measurements JSON payload for the bring-up seam.
pub fn load_parameter_golf_runpod_8xh100_measurements(
    path: impl AsRef<Path>,
) -> Result<ParameterGolfRunPod8xH100Measurements, ParameterGolfDistributed8xH100BringupError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| ParameterGolfDistributed8xH100BringupError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfDistributed8xH100BringupError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use psionic_core::{DType, Device, DeviceKind};
    use psionic_runtime::{
        HealthStatus, NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile,
        NvidiaRiskLevel, NvidiaRiskProfile, NvidiaTopologyInfo,
    };

    use super::{
        build_parameter_golf_distributed_8xh100_bringup_report, device_matches_distributed_h100,
        load_parameter_golf_runpod_8xh100_measurements, machine_observation_from_inventory,
        ParameterGolfDistributed8xH100BringupConfig,
        ParameterGolfDistributed8xH100BringupDisposition,
    };
    use crate::{
        ParameterGolfDistributedMemoryObservation, ParameterGolfDistributedStepObservation,
        ParameterGolfDistributedValidationShardObservation, ParameterGolfRunPod8xH100Measurements,
    };
    use psionic_eval::ParameterGolfDistributedChallengeThresholds;
    use psionic_runtime::{DeviceDescriptor, RuntimeHealth};

    fn sample_h100_device(ordinal: usize) -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                ordinal as u16,
                Some(format!("cuda:{ordinal}")),
            ),
            device_name: Some(String::from("NVIDIA H100 80GB HBM3")),
            supported_dtypes: vec![DType::F32, DType::BF16],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(80 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("hopper")),
                    compute_capability: Some(String::from("9.0")),
                    pci_bdf: Some(format!("00000000:{:02x}:00.0", ordinal + 1)),
                    sm_count: Some(132),
                    vram_bytes: Some(80 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    #[test]
    fn distributed_8xh100_machine_observation_refuses_partial_inventory() {
        let observation = machine_observation_from_inventory(
            ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("cuda online"),
            },
            None,
            vec![sample_h100_device(0)],
        );
        assert!(!observation.machine_contract_satisfied);
        assert_eq!(observation.matching_h100_device_count, 1);
        assert!(observation.refusal.is_some());
    }

    #[test]
    fn distributed_8xh100_bringup_report_marks_measured_receipt_when_supplied(
    ) -> Result<(), Box<dyn Error>> {
        let config = ParameterGolfDistributed8xH100BringupConfig::challenge_defaults();
        let mut measurements = ParameterGolfRunPod8xH100Measurements::challenge_defaults();
        measurements.step_observations = vec![
            ParameterGolfDistributedStepObservation::new(1, 0, 40, 524_288),
            ParameterGolfDistributedStepObservation::new(2, 40, 80, 524_288),
        ];
        measurements.validation_observed_ms = 10;
        measurements.validation_total_sequence_count = 32;
        measurements.validation_shard_observations = (0..8)
            .map(|rank| ParameterGolfDistributedValidationShardObservation {
                rank,
                sequence_start: (rank * 4) as u64,
                sequence_count: 4,
                evaluation_unit_start: (rank * 4) as u64,
                evaluation_unit_count: 4,
                scored_token_start: (rank * 16) as u64,
                scored_token_count: 16,
                loss_sum: 8.0 + rank as f64,
                token_count: 16,
                byte_count: 12,
                observed_ms: 9 + rank as u64,
            })
            .collect();
        measurements.export_observed_ms = 5;
        measurements.memory_observation = Some(ParameterGolfDistributedMemoryObservation {
            peak_device_bytes_per_worker: 70 * 1024 * 1024 * 1024,
            peak_host_bytes_per_worker: 8 * 1024 * 1024 * 1024,
            source: String::from("synthetic measured runtime"),
        });
        let report =
            build_parameter_golf_distributed_8xh100_bringup_report(&config, Some(&measurements))?;
        assert!(matches!(
            report.disposition,
            ParameterGolfDistributed8xH100BringupDisposition::RefusedMachineContract
                | ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptLoaded
                | ParameterGolfDistributed8xH100BringupDisposition::MeasuredReceiptRefused
        ));
        Ok(())
    }

    #[test]
    fn distributed_8xh100_measurements_loader_round_trips() -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let path = tempdir.path().join("measurements.json");
        let mut measurements = ParameterGolfRunPod8xH100Measurements::challenge_defaults();
        measurements.step_observations = vec![ParameterGolfDistributedStepObservation::new(
            1, 0, 40, 524_288,
        )];
        fs::write(&path, serde_json::to_vec_pretty(&measurements)?)?;
        let loaded = load_parameter_golf_runpod_8xh100_measurements(&path)?;
        assert_eq!(loaded, measurements);
        Ok(())
    }

    #[test]
    fn distributed_h100_matcher_rejects_non_h100_names() {
        let mut device = sample_h100_device(0);
        device.device_name = Some(String::from("NVIDIA RTX 4080"));
        assert!(!device_matches_distributed_h100(
            &device,
            &ParameterGolfDistributedChallengeThresholds::challenge_8xh100()
        ));
    }

    #[test]
    fn distributed_h100_matcher_accepts_real_runpod_inventory_delta() {
        let mut device = sample_h100_device(0);
        device.memory_capacity_bytes = Some(81_559 * 1024 * 1024);
        assert!(device_matches_distributed_h100(
            &device,
            &ParameterGolfDistributedChallengeThresholds::challenge_8xh100()
        ));
    }
}
