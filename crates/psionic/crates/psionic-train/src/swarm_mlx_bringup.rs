use std::{
    fs,
    path::Path,
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use psionic_array::{ArrayContext, ArrayError};
use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FirstSwarmOpenAdapterContributorReceipt, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    OpenAdapterPrecisionPolicy, OpenAdapterTrainingExecutionBackend, SWARM_FIRST_RUN_FAMILY_ID,
    build_first_swarm_open_adapter_contributor_receipt, first_swarm_open_adapter_samples,
    first_swarm_open_adapter_sft_request, first_swarm_open_adapter_training_config,
    first_swarm_run_contract, run_open_adapter_sft_export,
};

/// Stable scope window for the first Mac MLX swarm bring-up report.
pub const SWARM_MAC_MLX_BRINGUP_SCOPE_WINDOW: &str = "swarm_mac_mlx_bringup_v1";
/// Stable fixture path for the first Mac MLX swarm bring-up report.
pub const SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json";
/// Conservative first-run sequence bound for the Mac MLX lane.
pub const SWARM_MAC_SAFE_SEQUENCE_LENGTH_TOKENS: u32 = 512;
/// Conservative first-run microbatch bound for the Mac MLX lane.
pub const SWARM_MAC_SAFE_MICROBATCH_SIZE: u32 = 4;
/// Conservative first-run LoRA rank bound for the Mac MLX lane.
pub const SWARM_MAC_SAFE_ADAPTER_RANK: u32 = 16;

/// Current MLX-backed open-adapter backend posture during the Mac bring-up phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmMlxTrainingBackendPosture {
    /// The current host could not instantiate or complete the bounded MLX gate.
    MissingOpenAdapterBackend,
    /// The current host instantiated the MLX-backed open-adapter backend and may contribute.
    Ready,
}

/// Final Mac bring-up disposition for the first swarm lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmMacMlxBringupDisposition {
    /// The current machine does not satisfy the Mac MLX contract.
    RefusedMachineContract,
    /// The current machine is healthy, but the bounded MLX open-adapter gate still failed.
    RefusedTrainingBackendBlocker,
    /// The machine and backend are both ready for the first swarm lane.
    ReadyToAttempt,
}

/// Frozen machine thresholds for the first Mac swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmMacMachineThresholds {
    /// Required operating system.
    pub required_os: String,
    /// Required architecture.
    pub required_arch: String,
    /// Minimum unified memory expected for the lane.
    pub minimum_unified_memory_bytes: u64,
    /// Required backend label once the shared training backend exists.
    pub required_backend_label: String,
    /// Conservative sequence bound while the lane remains bounded.
    pub safe_sequence_length_tokens: u32,
    /// Conservative microbatch bound while the lane remains bounded.
    pub safe_microbatch_size: u32,
    /// Conservative adapter-rank bound while the lane remains bounded.
    pub safe_adapter_rank: u32,
    /// Current admitted precision policy.
    pub precision_policy: String,
}

impl FirstSwarmMacMachineThresholds {
    /// Returns the canonical first-run thresholds for the Mac MLX node.
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            required_os: String::from("macos"),
            required_arch: String::from("arm64"),
            minimum_unified_memory_bytes: 16 * 1024 * 1024 * 1024,
            required_backend_label: String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL),
            safe_sequence_length_tokens: SWARM_MAC_SAFE_SEQUENCE_LENGTH_TOKENS,
            safe_microbatch_size: SWARM_MAC_SAFE_MICROBATCH_SIZE,
            safe_adapter_rank: SWARM_MAC_SAFE_ADAPTER_RANK,
            precision_policy: String::from("f32_reference"),
        }
    }
}

/// Host identity captured by the Mac bring-up report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmMacHostIdentity {
    /// Hostname used for the report.
    pub hostname: String,
    /// Operating-system name.
    pub os_name: String,
    /// Operating-system version.
    pub os_version: String,
    /// Operating-system build version.
    pub os_build_version: String,
    /// Machine architecture.
    pub architecture: String,
    /// Hardware model identifier when available.
    pub hardware_model: String,
    /// Apple GPU or chip label when available.
    pub chip_name: String,
    /// Reported unified-memory bytes when available.
    pub unified_memory_bytes: u64,
    /// Reported Metal-family support when available.
    pub metal_family_support: Option<String>,
    /// Reported GPU core count when available.
    pub gpu_core_count: Option<u32>,
}

/// One bounded Metal eval proof for the current MLX bring-up report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmMacMetalEvalProbe {
    /// Stable bounded op slice surfaced by the report.
    pub admitted_ops: Vec<String>,
    /// Stable device identifier used by the eval receipt.
    pub device_id: String,
    /// Stable stream identifier used by the eval receipt.
    pub stream_id: u32,
    /// Evaluated output values for the bounded `constant -> matmul -> add` path.
    pub output: Vec<f32>,
    /// Stable digest over the eval receipt.
    pub eval_receipt_digest: String,
    /// Explicit refusal observed for one op outside the bounded surface.
    pub out_of_slice_refusal: String,
}

/// Deterministic same-node overfit gate for the Mac MLX contributor lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmMacMlxOverfitGate {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable backend label.
    pub execution_backend_label: String,
    /// Stable logical-device kind observed by the backend.
    pub logical_device_kind: String,
    /// Stable logical-device label observed by the backend.
    pub logical_device_label: String,
    /// Stable adapter family emitted by the gate.
    pub adapter_family: String,
    /// Precision policy used by the gate.
    pub precision_policy: String,
    /// Step count executed by the fixed-budget core.
    pub executed_steps: usize,
    /// Packed batch count used by the gate.
    pub batch_count: usize,
    /// Final mean loss from the last gradient batch.
    pub final_mean_loss: f32,
    /// Stable adapter artifact digest emitted by the gate.
    pub adapter_artifact_digest: String,
    /// Stable adapter identity digest emitted by the gate.
    pub adapter_identity_digest: String,
    /// Stable execution-provenance digest emitted by the gate.
    pub execution_provenance_digest: String,
    /// Stable final state-dict digest emitted by the gate.
    pub final_state_dict_digest: String,
    /// Stable predicted token for one deterministic probe.
    pub probe_top_token_id: usize,
    /// Explicit precision refusal for unsupported later postures.
    pub unsupported_precision_refusal: String,
    /// Shared comparable contributor receipt for the first swarm lane.
    pub contributor_receipt: FirstSwarmOpenAdapterContributorReceipt,
    /// Stable gate digest.
    pub gate_digest: String,
}

/// Machine-readable Mac MLX swarm bring-up report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmMacMlxBringupReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family the report applies to.
    pub run_family_id: String,
    /// Stable first-swarm contract digest.
    pub contract_digest: String,
    /// Host identity observed at report time.
    pub host: FirstSwarmMacHostIdentity,
    /// Frozen machine thresholds for the lane.
    pub machine_thresholds: FirstSwarmMacMachineThresholds,
    /// Whether the machine satisfied the Mac MLX contract.
    pub machine_contract_satisfied: bool,
    /// Bounded MLX or Metal op slice currently admitted for the lane.
    pub admitted_metal_slice: Vec<String>,
    /// Bounded Metal eval proof when the runtime was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metal_eval_probe: Option<FirstSwarmMacMetalEvalProbe>,
    /// Current training-backend posture.
    pub training_backend_posture: FirstSwarmMlxTrainingBackendPosture,
    /// Deterministic same-node overfit gate when the backend completed locally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overfit_gate: Option<FirstSwarmMacMlxOverfitGate>,
    /// Explicit training-backend blocker when the local MLX gate still fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_backend_blocker: Option<String>,
    /// Final disposition for the bring-up.
    pub disposition: FirstSwarmMacMlxBringupDisposition,
    /// Primary refusal when the machine or backend blocks the lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Human-readable entrypoint for this report.
    pub psionic_entrypoint: String,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Explicit drift or boundary notes.
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

/// Errors surfaced while building or writing the Mac MLX swarm bring-up report.
#[derive(Debug, Error)]
pub enum FirstSwarmMacMlxBringupError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to encode the Mac MLX bring-up report: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize)]
struct SystemProfilerDisplaysRoot {
    #[serde(rename = "SPDisplaysDataType", default)]
    displays: Vec<SystemProfilerDisplayGpu>,
}

#[derive(Clone, Debug, Deserialize)]
struct SystemProfilerDisplayGpu {
    #[serde(rename = "_name")]
    name: Option<String>,
    #[serde(rename = "sppci_model")]
    model: Option<String>,
    #[serde(rename = "sppci_cores")]
    cores: Option<String>,
    #[serde(rename = "spdisplays_mtlgpufamilysupport")]
    metal_family_support: Option<String>,
}

/// Returns the current MLX-backed open-adapter backend posture.
#[must_use]
pub fn current_first_swarm_mlx_training_backend_posture() -> FirstSwarmMlxTrainingBackendPosture {
    match run_first_swarm_mac_mlx_overfit_gate() {
        Ok(_) => FirstSwarmMlxTrainingBackendPosture::Ready,
        Err(_) => FirstSwarmMlxTrainingBackendPosture::MissingOpenAdapterBackend,
    }
}

/// Builds the current Mac MLX swarm bring-up report.
pub fn build_first_swarm_mac_mlx_bringup_report()
-> Result<FirstSwarmMacMlxBringupReport, FirstSwarmMacMlxBringupError> {
    let started_at_ms = now_ms();
    let started = Instant::now();
    let contract = first_swarm_run_contract();
    let host = observe_mac_host_identity();
    let machine_thresholds = FirstSwarmMacMachineThresholds::canonical();
    let machine_refusal = evaluate_machine_contract(&host, &machine_thresholds);
    let metal_eval_probe = if machine_refusal.is_none() {
        run_bounded_metal_eval_probe()
    } else {
        None
    };
    let (training_backend_posture, overfit_gate, training_backend_blocker) =
        if machine_refusal.is_none() {
            match run_first_swarm_mac_mlx_overfit_gate() {
                Ok(gate) => (FirstSwarmMlxTrainingBackendPosture::Ready, Some(gate), None),
                Err(detail) => (
                    FirstSwarmMlxTrainingBackendPosture::MissingOpenAdapterBackend,
                    None,
                    Some(detail),
                ),
            }
        } else {
            (
                current_first_swarm_mlx_training_backend_posture(),
                None,
                None,
            )
        };
    let refusal = machine_refusal.clone().or_else(|| {
        training_backend_blocker.as_ref().map(|detail| {
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                detail.clone(),
            )
            .with_subject(String::from("first_swarm_mac_mlx_training_backend"))
        })
    });
    let disposition = if machine_refusal.is_some() {
        FirstSwarmMacMlxBringupDisposition::RefusedMachineContract
    } else if training_backend_blocker.is_some() {
        FirstSwarmMacMlxBringupDisposition::RefusedTrainingBackendBlocker
    } else {
        FirstSwarmMacMlxBringupDisposition::ReadyToAttempt
    };
    let finished_at_ms = now_ms();
    let observed_wallclock_ms = (started.elapsed().as_millis() as u64).max(1);
    let admitted_metal_slice = vec![
        String::from("psionic-array::ArrayContext::metal"),
        String::from("dense_f32.constant"),
        String::from("dense_f32.add"),
        String::from("dense_f32.matmul"),
    ];
    let mut drift_notes = vec![
        String::from(
            "The current MLX or Metal admission truth remains bounded to the public dense-f32 ArrayContext metal slice rather than blanket MLX training closure.",
        ),
        String::from(
            "This report is refusal-proof: it records real local host and bounded Metal runtime facts first, then records whether the bounded open-adapter gate produced a real adapter delta on this host.",
        ),
    ];
    if let Some(metal_family) = &host.metal_family_support {
        drift_notes.push(format!(
            "Observed Metal-family support `{metal_family}` on chip `{}`.",
            host.chip_name
        ));
    }
    if let Some(blocker) = &training_backend_blocker {
        drift_notes.push(format!("Current swarm blocker: {blocker}"));
    }
    if let Some(gate) = &overfit_gate {
        drift_notes.push(format!(
            "The bounded MLX open-adapter gate completed {} steps on logical device `{}` and emitted adapter digest `{}`.",
            gate.executed_steps, gate.logical_device_label, gate.adapter_artifact_digest
        ));
    }
    if let Some(probe) = &metal_eval_probe {
        drift_notes.push(format!(
            "The bounded Metal eval probe executed through device `{}` stream `{}` and kept explicit refusal for reshape-backed flatten outside the admitted slice.",
            probe.device_id, probe.stream_id
        ));
    }
    let claim_boundary = String::from(
        "This report proves one exact local Mac host identity, one bounded Metal eval slice through psionic-array, the first conservative sequence, microbatch, rank, and precision bounds for the swarm lane, and, when the gate succeeds, one bounded local open-adapter run that emits a real adapter artifact under the MLX Metal backend label. It does not claim blanket MLX training closure, mixed-backend distributed execution, or general MLX parity outside this bounded swarm lane.",
    );
    let mut report = FirstSwarmMacMlxBringupReport {
        schema_version: 1,
        scope_window: String::from(SWARM_MAC_MLX_BRINGUP_SCOPE_WINDOW),
        run_family_id: String::from(SWARM_FIRST_RUN_FAMILY_ID),
        contract_digest: contract.contract_digest,
        host,
        machine_thresholds,
        machine_contract_satisfied: machine_refusal.is_none(),
        admitted_metal_slice,
        metal_eval_probe,
        training_backend_posture,
        overfit_gate,
        training_backend_blocker,
        disposition,
        refusal,
        psionic_entrypoint: format!(
            "cargo run -q -p psionic-train --bin swarm_mac_mlx_bringup -- {}",
            SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH
        ),
        claim_boundary,
        drift_notes,
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_first_swarm_mac_mlx_bringup_report|", &report);
    Ok(report)
}

/// Writes the current Mac MLX swarm bring-up report to one JSON path.
pub fn write_first_swarm_mac_mlx_bringup_report(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmMacMlxBringupReport, FirstSwarmMacMlxBringupError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FirstSwarmMacMlxBringupError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_first_swarm_mac_mlx_bringup_report()?;
    let encoded = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmMacMlxBringupError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn observe_mac_host_identity() -> FirstSwarmMacHostIdentity {
    let os_name =
        command_stdout("sw_vers", &["-productName"]).unwrap_or_else(|| String::from("unknown"));
    let os_version =
        command_stdout("sw_vers", &["-productVersion"]).unwrap_or_else(|| String::from("unknown"));
    let os_build_version =
        command_stdout("sw_vers", &["-buildVersion"]).unwrap_or_else(|| String::from("unknown"));
    let architecture =
        command_stdout("uname", &["-m"]).unwrap_or_else(|| std::env::consts::ARCH.to_string());
    let hostname = command_stdout("hostname", &[]).unwrap_or_else(|| String::from("unknown-host"));
    let hardware_model =
        command_stdout("sysctl", &["-n", "hw.model"]).unwrap_or_else(|| String::from("unknown"));
    let unified_memory_bytes = command_stdout("sysctl", &["-n", "hw.memsize"])
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    let (chip_name, metal_family_support, gpu_core_count) =
        system_profiler_gpu_summary().unwrap_or_else(|| (String::from("unknown"), None, None));
    FirstSwarmMacHostIdentity {
        hostname,
        os_name: os_name.to_lowercase(),
        os_version,
        os_build_version,
        architecture,
        hardware_model,
        chip_name,
        unified_memory_bytes,
        metal_family_support,
        gpu_core_count,
    }
}

fn evaluate_machine_contract(
    host: &FirstSwarmMacHostIdentity,
    thresholds: &FirstSwarmMacMachineThresholds,
) -> Option<PsionicRefusal> {
    if host.os_name != thresholds.required_os {
        return Some(
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "first swarm Mac MLX bring-up requires os `{}` but observed `{}`",
                    thresholds.required_os, host.os_name
                ),
            )
            .with_subject(String::from("first_swarm_mac_machine")),
        );
    }
    if host.architecture != thresholds.required_arch {
        return Some(
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "first swarm Mac MLX bring-up requires architecture `{}` but observed `{}`",
                    thresholds.required_arch, host.architecture
                ),
            )
            .with_subject(String::from("first_swarm_mac_machine")),
        );
    }
    if host.unified_memory_bytes < thresholds.minimum_unified_memory_bytes {
        return Some(
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                format!(
                    "first swarm Mac MLX bring-up requires at least {} unified-memory bytes but observed {}",
                    thresholds.minimum_unified_memory_bytes, host.unified_memory_bytes
                ),
            )
            .with_subject(String::from("first_swarm_mac_machine")),
        );
    }
    if host.metal_family_support.is_none() {
        return Some(
            PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedBackendCapability,
                PsionicRefusalScope::Runtime,
                String::from(
                    "first swarm Mac MLX bring-up requires visible Metal-family support, but system_profiler did not surface one",
                ),
            )
            .with_subject(String::from("first_swarm_mac_machine")),
        );
    }
    None
}

fn run_bounded_metal_eval_probe() -> Option<FirstSwarmMacMetalEvalProbe> {
    let context = match ArrayContext::metal() {
        Ok(context) => context,
        Err(ArrayError::BackendUnavailable { .. }) => return None,
        Err(_) => return None,
    };
    let stream_context = match context.with_stream(context.new_stream()) {
        Ok(context) => context,
        Err(_) => return None,
    };
    let input = stream_context
        .constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])
        .ok()?;
    let weights = stream_context
        .constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])
        .ok()?;
    let bias = stream_context
        .constant_f32(Shape::new(vec![1, 2]), vec![0.5, 0.5])
        .ok()?;
    let projected = input.matmul(&weights).ok()?;
    let shifted = projected.add(&bias).ok()?;
    let evaluated = shifted.eval().ok()?;
    let output = evaluated
        .to_host_data()
        .ok()?
        .as_f32_slice()
        .map(|values| values.to_vec())
        .unwrap_or_default();
    let matrix = stream_context.ones_f32(Shape::new(vec![2, 2])).ok()?;
    let out_of_slice_refusal = matrix
        .flatten()
        .ok()
        .and_then(|flattened| flattened.eval().err())
        .map(|error| error.to_string())
        .unwrap_or_else(|| {
            String::from(
                "bounded Metal eval did not surface the expected explicit refusal for flatten",
            )
        });
    Some(FirstSwarmMacMetalEvalProbe {
        admitted_ops: vec![
            String::from("constant_f32"),
            String::from("matmul"),
            String::from("add"),
        ],
        device_id: evaluated.receipt().device_id.clone(),
        stream_id: evaluated.receipt().stream_id,
        output,
        eval_receipt_digest: stable_digest(
            b"psionic_first_swarm_mac_mlx_eval_receipt|",
            evaluated.receipt(),
        ),
        out_of_slice_refusal,
    })
}

fn run_first_swarm_mac_mlx_overfit_gate() -> Result<FirstSwarmMacMlxOverfitGate, String> {
    let config = first_swarm_open_adapter_training_config(
        "swarm-mac-mlx-overfit",
        "swarm.open_adapter.mlx.same_node",
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    );
    let samples =
        first_swarm_open_adapter_samples("swarm-mlx").map_err(|error| error.to_string())?;
    let backend = OpenAdapterTrainingExecutionBackend::new(config, samples)
        .map_err(|error| error.to_string())?;
    let outcome = run_open_adapter_sft_export(
        &backend,
        &first_swarm_open_adapter_sft_request("swarm-mac-mlx", "r1", 1_774_393_600_000, 25),
    )
    .map_err(|error| error.to_string())?;
    let unsupported_precision_refusal = OpenAdapterTrainingExecutionBackend::new(
        crate::OpenAdapterExecutionConfig {
            precision_policy: OpenAdapterPrecisionPolicy::Bf16Mixed,
            ..backend.config().clone()
        },
        vec![
            crate::OpenAdapterHiddenStateSample::new("unsupported", vec![1.0, 0.0, 0.0, 0.0], 2, 1)
                .map_err(|error| error.to_string())?,
        ],
    )
    .expect_err("bf16 should stay unsupported")
    .to_string();
    let adapter = outcome
        .load_lm_head_lora_artifact()
        .map_err(|error| error.to_string())?;
    let mut logits = vec![0.0_f32; backend.config().model.vocab_size];
    adapter
        .apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())
        .map_err(|error| error.to_string())?;
    let probe_top_token_id = logits
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.partial_cmp(right.1).expect("finite logits"))
        .map(|(index, _)| index)
        .unwrap_or_default();
    let contributor_receipt = build_first_swarm_open_adapter_contributor_receipt(
        "swarm.mac.mlx.coordinator_validator_contributor",
        &backend,
        &outcome,
        probe_top_token_id,
        unsupported_precision_refusal.clone(),
    )
    .map_err(|error| error.to_string())?;
    let mut gate = FirstSwarmMacMlxOverfitGate {
        run_id: backend.config().run_id.clone(),
        execution_backend_label: String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL),
        logical_device_kind: backend.provenance().logical_device_kind.to_string(),
        logical_device_label: backend.provenance().logical_device_label.clone(),
        adapter_family: backend.provenance().adapter_family.clone(),
        precision_policy: String::from("f32_reference"),
        executed_steps: outcome.step_receipts.len(),
        batch_count: backend.batches().len(),
        final_mean_loss: outcome
            .gradient_records
            .last()
            .map(|record| record.mean_loss)
            .unwrap_or_default(),
        adapter_artifact_digest: outcome.summary.adapter_artifact_digest.clone(),
        adapter_identity_digest: outcome.summary.adapter_identity_digest.clone(),
        execution_provenance_digest: outcome.summary.execution_provenance.stable_digest(),
        final_state_dict_digest: outcome.summary.final_state_dict_digest.clone(),
        probe_top_token_id,
        unsupported_precision_refusal,
        contributor_receipt,
        gate_digest: String::new(),
    };
    gate.gate_digest = stable_digest(b"psionic_first_swarm_mac_mlx_overfit_gate|", &gate);
    Ok(gate)
}

#[cfg(test)]
fn training_backend_blocker_detail(
    posture: FirstSwarmMlxTrainingBackendPosture,
) -> Option<&'static str> {
    match posture {
        FirstSwarmMlxTrainingBackendPosture::MissingOpenAdapterBackend => Some(
            "the current host could not instantiate and complete the bounded MLX-backed Metal open-adapter gate, so the Mac node cannot yet contribute a real swarm adapter delta",
        ),
        FirstSwarmMlxTrainingBackendPosture::Ready => None,
    }
}

fn system_profiler_gpu_summary() -> Option<(String, Option<String>, Option<u32>)> {
    let raw = command_stdout("system_profiler", &["SPDisplaysDataType", "-json"])?;
    let parsed: SystemProfilerDisplaysRoot = serde_json::from_str(raw.as_str()).ok()?;
    let gpu = parsed.displays.first()?;
    let chip_name = gpu
        .model
        .clone()
        .or_else(|| gpu.name.clone())
        .unwrap_or_else(|| String::from("unknown"));
    let gpu_core_count = gpu
        .cores
        .as_deref()
        .and_then(|value| value.parse::<u32>().ok());
    Some((chip_name, gpu.metal_family_support.clone(), gpu_core_count))
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
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
mod tests {
    use super::*;

    #[test]
    fn missing_posture_keeps_generic_blocker_explicit() {
        assert!(
            training_backend_blocker_detail(
                FirstSwarmMlxTrainingBackendPosture::MissingOpenAdapterBackend
            )
            .is_some()
        );
        assert!(
            training_backend_blocker_detail(FirstSwarmMlxTrainingBackendPosture::Ready).is_none()
        );
    }

    #[test]
    fn sample_system_profiler_json_parses_chip_and_metal_family() {
        let sample = r#"{
          "SPDisplaysDataType": [
            {
              "_name": "Apple M2 Pro",
              "sppci_model": "Apple M2 Pro",
              "sppci_cores": "19",
              "spdisplays_mtlgpufamilysupport": "spdisplays_metal4"
            }
          ]
        }"#;
        let parsed: SystemProfilerDisplaysRoot =
            serde_json::from_str(sample).expect("sample json should parse");
        let gpu = parsed.displays.first().expect("gpu entry should exist");
        assert_eq!(gpu.model.as_deref(), Some("Apple M2 Pro"));
        assert_eq!(
            gpu.metal_family_support.as_deref(),
            Some("spdisplays_metal4")
        );
        assert_eq!(gpu.cores.as_deref(), Some("19"));
    }

    #[test]
    fn machine_contract_requires_macos_arm64_and_metal_support() {
        let thresholds = FirstSwarmMacMachineThresholds::canonical();
        let host = FirstSwarmMacHostIdentity {
            hostname: String::from("host"),
            os_name: String::from("linux"),
            os_version: String::from("1"),
            os_build_version: String::from("1"),
            architecture: String::from("x86_64"),
            hardware_model: String::from("pc"),
            chip_name: String::from("gpu"),
            unified_memory_bytes: thresholds.minimum_unified_memory_bytes,
            metal_family_support: Some(String::from("spdisplays_metal4")),
            gpu_core_count: Some(10),
        };
        let refusal =
            evaluate_machine_contract(&host, &thresholds).expect("non-macos host should refuse");
        assert!(refusal.detail.contains("requires os"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_mlx_overfit_gate_emits_real_adapter_delta() {
        let gate = run_first_swarm_mac_mlx_overfit_gate()
            .expect("local mac should complete the bounded MLX overfit gate");
        assert_eq!(
            gate.execution_backend_label,
            OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL
        );
        assert_eq!(gate.logical_device_kind, "metal");
        assert_eq!(gate.logical_device_label, "metal:0");
        assert_eq!(gate.adapter_family, "gpt_oss.decoder_lm_head_lora");
        assert_eq!(gate.probe_top_token_id, 2);
        assert!(gate.final_mean_loss > 0.0);
        assert!(
            gate.unsupported_precision_refusal
                .contains("does not yet support precision policy")
        );
        assert!(!gate.adapter_artifact_digest.is_empty());
        assert!(!gate.gate_digest.is_empty());
    }
}
