use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use psionic_core::{DeviceKind, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use psionic_runtime::{DeviceDescriptor, RuntimeHealth};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FirstSwarmOpenAdapterContributorReceipt, OPEN_ADAPTER_CUDA_BACKEND_LABEL,
    OpenAdapterPrecisionPolicy, OpenAdapterTrainingExecutionBackend,
    ParameterGolfSingleH100BringupReport, build_first_swarm_open_adapter_contributor_receipt,
    first_swarm_open_adapter_samples, first_swarm_open_adapter_sft_request,
    first_swarm_open_adapter_training_config, first_swarm_run_contract,
};

/// Stable retained inventory source for the first Linux RTX 4080 swarm report.
pub const SWARM_LINUX_4080_SOURCE_INVENTORY_REPORT_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_single_h100_bringup.json";
/// Stable fixture path for the first Linux RTX 4080 swarm report.
pub const SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/swarm_linux_rtx4080_bringup_v1.json";
/// Stable scope window for the first Linux RTX 4080 swarm report.
pub const SWARM_LINUX_4080_BRINGUP_SCOPE_WINDOW: &str = "swarm_linux_rtx4080_bringup_v1";
/// Conservative first-run sequence bound for the Linux CUDA lane.
pub const SWARM_LINUX_SAFE_SEQUENCE_LENGTH_TOKENS: u32 = 512;
/// Conservative first-run microbatch bound for the Linux CUDA lane.
pub const SWARM_LINUX_SAFE_MICROBATCH_SIZE: u32 = 4;
/// Conservative first-run LoRA rank bound for the Linux CUDA lane.
pub const SWARM_LINUX_SAFE_ADAPTER_RANK: u32 = 16;

/// Final disposition for the Linux RTX 4080 swarm bring-up report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmLinuxCudaBringupDisposition {
    /// The retained inventory did not satisfy the RTX 4080 CUDA contract.
    RefusedMachineContract,
    /// The retained inventory and parity harness are both good enough for the first lane.
    ReadyToAttempt,
}

/// Frozen machine thresholds for the first Linux RTX 4080 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmLinuxCudaMachineThresholds {
    /// Required backend label.
    pub required_backend: String,
    /// Required device-kind family.
    pub required_device_kind: String,
    /// Required device-name substring.
    pub required_device_name: String,
    /// Minimum matching-device count.
    pub minimum_matching_device_count: usize,
    /// Conservative sequence bound while the lane remains bounded.
    pub safe_sequence_length_tokens: u32,
    /// Conservative microbatch bound while the lane remains bounded.
    pub safe_microbatch_size: u32,
    /// Conservative adapter-rank bound while the lane remains bounded.
    pub safe_adapter_rank: u32,
    /// Current admitted precision policy.
    pub precision_policy: String,
}

impl FirstSwarmLinuxCudaMachineThresholds {
    /// Returns the canonical thresholds for the first Linux RTX 4080 lane.
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            required_backend: String::from("cuda"),
            required_device_kind: String::from("cuda"),
            required_device_name: String::from("RTX 4080"),
            minimum_matching_device_count: 1,
            safe_sequence_length_tokens: SWARM_LINUX_SAFE_SEQUENCE_LENGTH_TOKENS,
            safe_microbatch_size: SWARM_LINUX_SAFE_MICROBATCH_SIZE,
            safe_adapter_rank: SWARM_LINUX_SAFE_ADAPTER_RANK,
            precision_policy: String::from("f32_reference"),
        }
    }
}

/// Deterministic same-node open-adapter harness report for the Linux CUDA lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmLinuxCudaParityHarnessReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable backend label.
    pub execution_backend_label: String,
    /// Stable adapter family.
    pub adapter_family: String,
    /// Precision policy used by the harness.
    pub precision_policy: String,
    /// Step count executed by the fixed-budget core.
    pub executed_steps: usize,
    /// Packed batch count used by the harness.
    pub batch_count: usize,
    /// Final mean loss from the last gradient batch.
    pub final_mean_loss: f32,
    /// Stable adapter artifact digest emitted by the harness.
    pub adapter_artifact_digest: String,
    /// Stable adapter identity digest emitted by the harness.
    pub adapter_identity_digest: String,
    /// Stable predicted token for one deterministic probe.
    pub probe_top_token_id: usize,
    /// Explicit precision refusal for unsupported later postures.
    pub unsupported_precision_refusal: String,
    /// Shared comparable contributor receipt for the first swarm lane.
    pub contributor_receipt: FirstSwarmOpenAdapterContributorReceipt,
    /// Stable harness digest.
    pub harness_digest: String,
}

/// Machine-readable Linux RTX 4080 swarm bring-up report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmLinuxCudaBringupReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Stable first-swarm contract digest.
    pub contract_digest: String,
    /// Source inventory report path used by this report.
    pub source_inventory_report_path: PathBuf,
    /// Digest of the source inventory report bytes.
    pub source_inventory_report_digest: String,
    /// Observed CUDA backend health from the retained inventory.
    pub observed_cuda_health: RuntimeHealth,
    /// Observed CUDA devices from the retained inventory.
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    /// Count of retained devices matching the RTX 4080 contract.
    pub matching_rtx4080_device_count: usize,
    /// Whether the retained inventory satisfies the first RTX 4080 contract.
    pub machine_contract_satisfied: bool,
    /// Frozen machine thresholds for the lane.
    pub machine_thresholds: FirstSwarmLinuxCudaMachineThresholds,
    /// Deterministic same-node parity harness report.
    pub parity_harness: FirstSwarmLinuxCudaParityHarnessReport,
    /// Final disposition for the lane.
    pub disposition: FirstSwarmLinuxCudaBringupDisposition,
    /// Primary refusal when the retained inventory misses the lane contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Human-readable entrypoint for this report.
    pub psionic_entrypoint: String,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Explicit drift and boundary notes.
    pub drift_notes: Vec<String>,
    /// Observed report start time.
    pub started_at_ms: u64,
    /// Observed report finish time.
    pub finished_at_ms: u64,
    /// Observed wallclock for the report.
    pub observed_wallclock_ms: u64,
    /// Stable report digest.
    pub report_digest: String,
}

/// Errors surfaced while building or writing the Linux RTX 4080 swarm report.
#[derive(Debug, Error)]
pub enum FirstSwarmLinuxCudaBringupError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode inventory report `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    OpenAdapter(#[from] crate::OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    OpenAdapterSft(#[from] crate::OpenAdapterSftError),
    #[error(transparent)]
    TrainingCore(#[from] crate::TrainingCoreError),
    #[error("failed to execute the Linux CUDA parity harness probe: {detail}")]
    ParityRuntime { detail: String },
}

/// Builds the first Linux RTX 4080 swarm bring-up report from retained inventory evidence.
pub fn build_first_swarm_linux_cuda_bringup_report(
    inventory_report_path: impl AsRef<Path>,
) -> Result<FirstSwarmLinuxCudaBringupReport, FirstSwarmLinuxCudaBringupError> {
    let inventory_report_path = inventory_report_path.as_ref();
    let started_at_ms = now_ms();
    let started = Instant::now();
    let inventory_bytes =
        fs::read(inventory_report_path).map_err(|error| FirstSwarmLinuxCudaBringupError::Read {
            path: inventory_report_path.display().to_string(),
            error,
        })?;
    let inventory: ParameterGolfSingleH100BringupReport = serde_json::from_slice(&inventory_bytes)
        .map_err(|error| FirstSwarmLinuxCudaBringupError::Deserialize {
            path: inventory_report_path.display().to_string(),
            error,
        })?;
    let source_inventory_report_digest = hex::encode(Sha256::digest(inventory_bytes.as_slice()));
    let machine_thresholds = FirstSwarmLinuxCudaMachineThresholds::canonical();
    let matching_rtx4080_device_count = inventory
        .observed_cuda_devices
        .iter()
        .filter(|device| device_matches_rtx4080(device, &machine_thresholds))
        .count();
    let machine_contract_satisfied =
        matching_rtx4080_device_count >= machine_thresholds.minimum_matching_device_count;
    let refusal = (!machine_contract_satisfied).then(|| {
        PsionicRefusal::new(
            PsionicRefusalCode::UnsupportedBackendCapability,
            PsionicRefusalScope::Runtime,
            format!(
                "first swarm Linux CUDA bring-up requires at least {} `{}` device(s) on backend `{}` but found {} matching device(s)",
                machine_thresholds.minimum_matching_device_count,
                machine_thresholds.required_device_name,
                machine_thresholds.required_backend,
                matching_rtx4080_device_count
            ),
        )
        .with_subject(String::from("first_swarm_linux_cuda_machine"))
    });
    let parity_harness = run_first_swarm_linux_cuda_parity_harness()?;
    let contract = first_swarm_run_contract();
    let finished_at_ms = now_ms();
    let observed_wallclock_ms = (started.elapsed().as_millis() as u64).max(1);
    let mut drift_notes = vec![
        format!(
            "This report reuses retained CUDA inventory from `{}` instead of pretending to probe a remote Linux desktop from the current host.",
            inventory_report_path.display()
        ),
        String::from(
            "The deterministic same-node harness proves open-adapter execution, export, and unsupported precision refusal under the CUDA backend label, but it is still a bounded sanity harness rather than a live CUDA-kernel benchmark.",
        ),
    ];
    if let Some(device) = inventory
        .observed_cuda_devices
        .iter()
        .find(|device| device_matches_rtx4080(device, &machine_thresholds))
        .and_then(|device| device.device_name.clone())
    {
        drift_notes.push(format!(
            "The retained inventory still exposes the expected CUDA worker device `{device}`."
        ));
    }
    let claim_boundary = String::from(
        "This report proves one retained Linux RTX 4080 CUDA inventory contract, one deterministic same-node open-adapter execution and export harness under the CUDA backend label, the first conservative sequence, microbatch, rank, and precision bounds for the swarm lane, and explicit refusal for unsupported mixed-precision posture. It does not claim challenge-equivalent H100 hardware, live distributed execution by itself, or a benchmark result for the later mixed-hardware swarm lane.",
    );
    let mut report = FirstSwarmLinuxCudaBringupReport {
        schema_version: 1,
        scope_window: String::from(SWARM_LINUX_4080_BRINGUP_SCOPE_WINDOW),
        run_family_id: contract.run_family_id.clone(),
        contract_digest: contract.contract_digest,
        source_inventory_report_path: inventory_report_path.to_path_buf(),
        source_inventory_report_digest,
        observed_cuda_health: inventory.observed_cuda_health.clone(),
        observed_cuda_devices: inventory.observed_cuda_devices.clone(),
        matching_rtx4080_device_count,
        machine_contract_satisfied,
        machine_thresholds,
        parity_harness,
        disposition: if machine_contract_satisfied {
            FirstSwarmLinuxCudaBringupDisposition::ReadyToAttempt
        } else {
            FirstSwarmLinuxCudaBringupDisposition::RefusedMachineContract
        },
        refusal,
        psionic_entrypoint: format!(
            "cargo run -q -p psionic-train --bin swarm_linux_cuda_bringup -- {} {}",
            SWARM_LINUX_4080_SOURCE_INVENTORY_REPORT_PATH, SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH
        ),
        claim_boundary,
        drift_notes,
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms,
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_first_swarm_linux_cuda_bringup_report|", &report);
    Ok(report)
}

/// Writes the first Linux RTX 4080 swarm bring-up report to one JSON path.
pub fn write_first_swarm_linux_cuda_bringup_report(
    inventory_report_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmLinuxCudaBringupReport, FirstSwarmLinuxCudaBringupError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FirstSwarmLinuxCudaBringupError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_first_swarm_linux_cuda_bringup_report(inventory_report_path)?;
    let encoded = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmLinuxCudaBringupError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn run_first_swarm_linux_cuda_parity_harness()
-> Result<FirstSwarmLinuxCudaParityHarnessReport, FirstSwarmLinuxCudaBringupError> {
    let config = first_swarm_open_adapter_training_config(
        "swarm-linux-cuda-parity",
        "swarm.open_adapter.cuda.same_node",
        OPEN_ADAPTER_CUDA_BACKEND_LABEL,
    );
    let samples = first_swarm_open_adapter_samples("swarm-cuda").map_err(|error| {
        FirstSwarmLinuxCudaBringupError::ParityRuntime {
            detail: error.to_string(),
        }
    })?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config, samples)?;
    let outcome = crate::run_open_adapter_sft_export(
        &backend,
        &first_swarm_open_adapter_sft_request("swarm-linux-cuda", "r1", 1_774_393_600_000, 25),
    )?;
    let unsupported_precision_refusal = OpenAdapterTrainingExecutionBackend::new(
        crate::OpenAdapterExecutionConfig {
            precision_policy: OpenAdapterPrecisionPolicy::Bf16Mixed,
            ..backend.config().clone()
        },
        vec![crate::OpenAdapterHiddenStateSample::new(
            "unsupported",
            vec![1.0, 0.0, 0.0, 0.0],
            2,
            1,
        )?],
    )
    .expect_err("bf16 should stay unsupported")
    .to_string();
    let final_mean_loss = outcome
        .gradient_records
        .last()
        .map(|record| record.mean_loss)
        .unwrap_or_default();
    let adapter = outcome.load_lm_head_lora_artifact()?;
    let mut logits = vec![0.0_f32; backend.config().model.vocab_size];
    adapter
        .apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())
        .map_err(|error| FirstSwarmLinuxCudaBringupError::ParityRuntime {
            detail: error.to_string(),
        })?;
    let probe_top_token_id = logits
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.partial_cmp(right.1).expect("finite logits"))
        .map(|(index, _)| index)
        .unwrap_or_default();
    let contributor_receipt = build_first_swarm_open_adapter_contributor_receipt(
        "swarm.linux.cuda.rtx4080.contributor",
        &backend,
        &outcome,
        probe_top_token_id,
        unsupported_precision_refusal.clone(),
    )
    .map_err(|error| FirstSwarmLinuxCudaBringupError::ParityRuntime {
        detail: error.to_string(),
    })?;
    let mut report = FirstSwarmLinuxCudaParityHarnessReport {
        run_id: backend.config().run_id.clone(),
        execution_backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
        adapter_family: backend.provenance().adapter_family.clone(),
        precision_policy: String::from("f32_reference"),
        executed_steps: outcome.step_receipts.len(),
        batch_count: backend.batches().len(),
        final_mean_loss,
        adapter_artifact_digest: outcome.summary.adapter_artifact_digest.clone(),
        adapter_identity_digest: outcome.summary.adapter_identity_digest.clone(),
        probe_top_token_id,
        unsupported_precision_refusal,
        contributor_receipt,
        harness_digest: String::new(),
    };
    report.harness_digest =
        stable_digest(b"psionic_first_swarm_linux_cuda_parity_harness|", &report);
    Ok(report)
}

fn device_matches_rtx4080(
    device: &DeviceDescriptor,
    thresholds: &FirstSwarmLinuxCudaMachineThresholds,
) -> bool {
    if device.backend != thresholds.required_backend || device.device.kind() != DeviceKind::Cuda {
        return false;
    }
    device
        .device_name
        .as_deref()
        .is_some_and(|name| name.contains(thresholds.required_device_name.as_str()))
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
fn inventory_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/parameter_golf/reports/parameter_golf_single_h100_bringup.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_parameter_golf_inventory_still_exposes_rtx4080_contract() {
        let report = build_first_swarm_linux_cuda_bringup_report(inventory_fixture_path())
            .expect("retained inventory should parse");
        assert!(report.machine_contract_satisfied);
        assert_eq!(report.matching_rtx4080_device_count, 1);
        assert_eq!(
            report.disposition,
            FirstSwarmLinuxCudaBringupDisposition::ReadyToAttempt
        );
    }

    #[test]
    fn linux_cuda_parity_harness_is_deterministic_and_refuses_bf16() {
        let harness =
            run_first_swarm_linux_cuda_parity_harness().expect("parity harness should execute");
        assert_eq!(
            harness.execution_backend_label,
            OPEN_ADAPTER_CUDA_BACKEND_LABEL
        );
        assert_eq!(harness.adapter_family, "gpt_oss.decoder_lm_head_lora");
        assert_eq!(harness.precision_policy, "f32_reference");
        assert!(harness.executed_steps > 0);
        assert!(harness.final_mean_loss > 0.0);
        assert!(
            harness
                .unsupported_precision_refusal
                .contains("does not yet support precision policy")
        );
    }
}
