use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_CPU_REFERENCE_RUNNER_ID, TASSADAR_FIXTURE_RUNNER_ID, TassadarBranchMode,
    TassadarExecutionSummary, TassadarExecutionSummaryEvidenceBundle, TassadarExecutorDecodeMode,
    TassadarExecutorSelectionReason, TassadarExecutorSelectionState, TassadarFixtureWeights,
    TassadarInstruction, TassadarMillionStepMeasurementPosture, TassadarOpcode, TassadarProgram,
    TassadarProgramArtifact, TassadarProgramArtifactError, TassadarTraceAbi, TassadarWasmProfile,
    build_tassadar_execution_summary_evidence_bundle, execute_program_direct_summary,
    execute_program_hull_cache_summary,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const LONG_HORIZON_PROFILE_ID: &str = "tassadar.wasm.article_runtime_closeout.v1";
const DIRECT_THROUGHPUT_FLOOR_STEPS_PER_SECOND: f64 = 250_000.0;

pub const TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF: &str =
    "fixtures/tassadar/runs/article_runtime_closeout_v1";
pub const TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE: &str =
    "article_runtime_closeout_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleRuntimeWorkloadFamily {
    LongLoopKernel,
    StateMachineKernel,
}

impl TassadarArticleRuntimeWorkloadFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LongLoopKernel => "long_loop_kernel",
            Self::StateMachineKernel => "state_machine_kernel",
        }
    }

    #[must_use]
    pub const fn source_ref(self) -> &'static str {
        match self {
            Self::LongLoopKernel => "fixtures/tassadar/sources/tassadar_long_loop_kernel.rs",
            Self::StateMachineKernel => {
                "fixtures/tassadar/sources/tassadar_state_machine_kernel.rs"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleRuntimeHorizon {
    MillionStep,
    TwoMillionStep,
}

impl TassadarArticleRuntimeHorizon {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MillionStep => "million_step",
            Self::TwoMillionStep => "two_million_step",
        }
    }

    #[must_use]
    pub const fn target_step_floor(self) -> u64 {
        match self {
            Self::MillionStep => 1_000_000,
            Self::TwoMillionStep => 2_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleRuntimeFloorStatus {
    Passed,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleRuntimeDecodeReceipt {
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub selection_state: TassadarExecutorSelectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub measurement_posture: TassadarMillionStepMeasurementPosture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps_per_second: Option<f64>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleRuntimeHorizonReceipt {
    pub workload_family: TassadarArticleRuntimeWorkloadFamily,
    pub workload_family_id: String,
    pub source_ref: String,
    pub horizon: TassadarArticleRuntimeHorizon,
    pub horizon_id: String,
    pub iteration_count: u64,
    pub exact_step_count: u64,
    pub cpu_reference_summary: TassadarExecutionSummary,
    pub direct_executor_summary: TassadarExecutionSummary,
    pub direct_steps_per_second: f64,
    pub throughput_floor_steps_per_second: f64,
    pub floor_status: TassadarArticleRuntimeFloorStatus,
    pub exactness_bps: u32,
    pub hull_cache_exactness_bps: u32,
    pub hull_cache_floor_status: TassadarArticleRuntimeFloorStatus,
    pub evidence_bundle: TassadarExecutionSummaryEvidenceBundle,
    pub reference_linear: TassadarArticleRuntimeDecodeReceipt,
    pub hull_cache: TassadarArticleRuntimeDecodeReceipt,
    pub sparse_top_k: TassadarArticleRuntimeDecodeReceipt,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleRuntimeCloseoutBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub bundle_root_ref: String,
    pub generated_from_refs: Vec<String>,
    pub workload_family_ids: Vec<String>,
    pub horizon_receipts: Vec<TassadarArticleRuntimeHorizonReceipt>,
    pub exact_horizon_count: u32,
    pub floor_pass_count: u32,
    pub floor_refusal_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl TassadarArticleRuntimeCloseoutBundle {
    fn new(horizon_receipts: Vec<TassadarArticleRuntimeHorizonReceipt>) -> Self {
        let generated_from_refs = horizon_receipts
            .iter()
            .map(|receipt| receipt.source_ref.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let workload_family_ids = horizon_receipts
            .iter()
            .map(|receipt| receipt.workload_family_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let exact_horizon_count = horizon_receipts
            .iter()
            .filter(|receipt| receipt.exactness_bps == 10_000)
            .count() as u32;
        let floor_pass_count = horizon_receipts
            .iter()
            .filter(|receipt| receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed)
            .count() as u32;
        let floor_refusal_count = horizon_receipts.len() as u32 - floor_pass_count;
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.article_runtime_closeout.bundle.v1"),
            bundle_root_ref: String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF),
            generated_from_refs,
            workload_family_ids,
            horizon_receipts,
            exact_horizon_count,
            floor_pass_count,
            floor_refusal_count,
            claim_boundary: String::from(
                "this bundle closes the Rust-only runtime floor for the committed long-loop and state-machine kernel families at the declared million-step and two-million-step horizons on the direct reference-linear CPU executor path, and it now also records direct measured HullCache throughput on that same bounded kernel set. SparseTopK remains explicit fallback-only here, and this bundle still does not widen served profile closure beyond the committed benchmark lane",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
        bundle.summary = format!(
            "Rust-only article runtime closeout now freezes {} exact horizon receipts across {} workload families with reference_linear_floor_passes={}, reference_linear_floor_refusals={}, and hull_cache_floor_passes={}.",
            bundle.horizon_receipts.len(),
            bundle.workload_family_ids.len(),
            bundle.floor_pass_count,
            bundle.floor_refusal_count,
            bundle
                .horizon_receipts
                .iter()
                .filter(|receipt| {
                    receipt.hull_cache_floor_status == TassadarArticleRuntimeFloorStatus::Passed
                })
                .count(),
        );
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_article_runtime_closeout_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleRuntimeCloseoutError {
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    #[error(transparent)]
    Execution(#[from] crate::TassadarExecutionRefusal),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

#[must_use]
pub fn tassadar_article_runtime_closeout_root_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF)
}

#[must_use]
pub fn tassadar_article_runtime_closeout_bundle_path() -> PathBuf {
    tassadar_article_runtime_closeout_root_path()
        .join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE)
}

pub fn build_tassadar_article_runtime_closeout_bundle()
-> Result<TassadarArticleRuntimeCloseoutBundle, TassadarArticleRuntimeCloseoutError> {
    let mut horizon_receipts = Vec::new();
    for workload_family in [
        TassadarArticleRuntimeWorkloadFamily::LongLoopKernel,
        TassadarArticleRuntimeWorkloadFamily::StateMachineKernel,
    ] {
        for horizon in [
            TassadarArticleRuntimeHorizon::MillionStep,
            TassadarArticleRuntimeHorizon::TwoMillionStep,
        ] {
            horizon_receipts.push(build_horizon_receipt(workload_family, horizon)?);
        }
    }
    Ok(TassadarArticleRuntimeCloseoutBundle::new(horizon_receipts))
}

pub fn write_tassadar_article_runtime_closeout_bundle(
    output_root: impl AsRef<Path>,
) -> Result<TassadarArticleRuntimeCloseoutBundle, TassadarArticleRuntimeCloseoutError> {
    let output_root = output_root.as_ref();
    fs::create_dir_all(output_root).map_err(|error| {
        TassadarArticleRuntimeCloseoutError::CreateDir {
            path: output_root.display().to_string(),
            error,
        }
    })?;
    let bundle = build_tassadar_article_runtime_closeout_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    let output_path = output_root.join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE);
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRuntimeCloseoutError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_horizon_receipt(
    workload_family: TassadarArticleRuntimeWorkloadFamily,
    horizon: TassadarArticleRuntimeHorizon,
) -> Result<TassadarArticleRuntimeHorizonReceipt, TassadarArticleRuntimeCloseoutError> {
    let profile = long_horizon_profile();
    let trace_abi = TassadarTraceAbi {
        profile_id: String::from(LONG_HORIZON_PROFILE_ID),
        ..TassadarTraceAbi::article_i32_compute_v1()
    };
    let (iteration_count, throughput_floor_steps_per_second) =
        horizon_spec(workload_family, horizon);
    let program = program_for(workload_family, &profile, iteration_count as i32);
    let cpu_reference_summary = execute_program_direct_summary(
        &program,
        &profile,
        &trace_abi,
        TASSADAR_CPU_REFERENCE_RUNNER_ID,
    )?;
    let direct_executor_summary =
        execute_program_direct_summary(&program, &profile, &trace_abi, TASSADAR_FIXTURE_RUNNER_ID)?;
    let exactness_bps = u32::from(
        cpu_reference_summary.trace_digest == direct_executor_summary.trace_digest
            && cpu_reference_summary.behavior_digest == direct_executor_summary.behavior_digest
            && cpu_reference_summary.outputs == direct_executor_summary.outputs
            && cpu_reference_summary.halt_reason == direct_executor_summary.halt_reason,
    ) * 10_000;
    let hull_cache_summary = execute_long_horizon_program_hull_cache_summary(&program)?;
    let hull_cache_exactness_bps = u32::from(
        cpu_reference_summary.trace_digest == hull_cache_summary.trace_digest
            && cpu_reference_summary.behavior_digest == hull_cache_summary.behavior_digest
            && cpu_reference_summary.outputs == hull_cache_summary.outputs
            && cpu_reference_summary.halt_reason == hull_cache_summary.halt_reason,
    ) * 10_000;
    let direct_steps_per_second =
        benchmark_steps_per_second(direct_executor_summary.step_count, || {
            execute_program_direct_summary(
                &program,
                &profile,
                &trace_abi,
                TASSADAR_FIXTURE_RUNNER_ID,
            )
        })?;
    let hull_cache_steps_per_second =
        benchmark_steps_per_second(hull_cache_summary.step_count, || {
            execute_long_horizon_program_hull_cache_summary(&program)
        })?;
    let floor_status = if direct_steps_per_second >= throughput_floor_steps_per_second {
        TassadarArticleRuntimeFloorStatus::Passed
    } else {
        TassadarArticleRuntimeFloorStatus::Refused
    };
    let hull_cache_floor_status = if hull_cache_steps_per_second >= throughput_floor_steps_per_second {
        TassadarArticleRuntimeFloorStatus::Passed
    } else {
        TassadarArticleRuntimeFloorStatus::Refused
    };
    let program_artifact = TassadarProgramArtifact::fixture_reference(
        format!(
            "tassadar://artifact/article_runtime_closeout/{}/{}",
            workload_family.as_str(),
            horizon.as_str()
        ),
        &profile,
        &trace_abi,
        program.clone(),
    )?;
    let evidence_bundle = build_tassadar_execution_summary_evidence_bundle(
        format!(
            "tassadar-article-runtime-closeout-{}-{}",
            workload_family.as_str(),
            horizon.as_str()
        ),
        stable_digest(
            b"psionic_tassadar_article_runtime_closeout_request|",
            &(
                workload_family.as_str(),
                horizon.as_str(),
                direct_executor_summary.trace_digest.as_str(),
            ),
        ),
        "psionic.tassadar.article_runtime_closeout.v1",
        format!(
            "model://tassadar/{}/{}",
            TASSADAR_FIXTURE_RUNNER_ID, LONG_HORIZON_PROFILE_ID
        ),
        stable_digest(
            b"psionic_tassadar_article_runtime_closeout_model|",
            &(TASSADAR_FIXTURE_RUNNER_ID, LONG_HORIZON_PROFILE_ID),
        ),
        vec![format!(
            "env.openagents.tassadar.article_runtime_closeout.{}",
            workload_family.as_str()
        )],
        &program_artifact,
        &direct_executor_summary,
    );
    Ok(TassadarArticleRuntimeHorizonReceipt {
        workload_family,
        workload_family_id: format!("rust.{}", workload_family.as_str()),
        source_ref: String::from(workload_family.source_ref()),
        horizon,
        horizon_id: format!("{}.{}", workload_family.as_str(), horizon.as_str()),
        iteration_count,
        exact_step_count: direct_executor_summary.step_count,
        cpu_reference_summary,
        direct_executor_summary,
        direct_steps_per_second,
        throughput_floor_steps_per_second,
        floor_status,
        exactness_bps,
        hull_cache_exactness_bps,
        hull_cache_floor_status,
        evidence_bundle,
        reference_linear: TassadarArticleRuntimeDecodeReceipt {
            requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            selection_state: TassadarExecutorSelectionState::Direct,
            selection_reason: None,
            effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
            measurement_posture: TassadarMillionStepMeasurementPosture::Measured,
            steps_per_second: Some(direct_steps_per_second),
            note: String::from(
                "reference-linear remains the baseline measured runtime lane for the committed Rust-only closeout kernels at these horizons",
            ),
        },
        hull_cache: fallback_decode_receipt(
            TassadarExecutorDecodeMode::HullCache,
            &program,
            Some(hull_cache_steps_per_second),
            "HullCache now stays direct and measured on the committed long-horizon kernel digests, so this receipt records fast-route exactness and throughput on that bounded benchmark-only subset",
        ),
        sparse_top_k: fallback_decode_receipt(
            TassadarExecutorDecodeMode::SparseTopK,
            &program,
            None,
            "SparseTopK remains explicit fallback-only on the long-horizon backward-branch kernels, so this receipt records selection truth without claiming direct article-floor closure",
        ),
        note: format!(
            "{} at `{}` stayed exact against CPU reference and {} the declared direct-throughput floor, while HullCache {} the same floor on the bounded fast-route kernel subset",
            workload_family.as_str(),
            horizon.as_str(),
            if floor_status == TassadarArticleRuntimeFloorStatus::Passed {
                "cleared"
            } else {
                "missed"
            },
            if hull_cache_floor_status == TassadarArticleRuntimeFloorStatus::Passed {
                "cleared"
            } else {
                "missed"
            },
        ),
    })
}

fn horizon_spec(
    workload_family: TassadarArticleRuntimeWorkloadFamily,
    horizon: TassadarArticleRuntimeHorizon,
) -> (u64, f64) {
    match (workload_family, horizon) {
        (
            TassadarArticleRuntimeWorkloadFamily::LongLoopKernel,
            TassadarArticleRuntimeHorizon::MillionStep,
        ) => (131_071, DIRECT_THROUGHPUT_FLOOR_STEPS_PER_SECOND),
        (
            TassadarArticleRuntimeWorkloadFamily::LongLoopKernel,
            TassadarArticleRuntimeHorizon::TwoMillionStep,
        ) => (262_143, DIRECT_THROUGHPUT_FLOOR_STEPS_PER_SECOND),
        (
            TassadarArticleRuntimeWorkloadFamily::StateMachineKernel,
            TassadarArticleRuntimeHorizon::MillionStep,
        ) => (76_923, DIRECT_THROUGHPUT_FLOOR_STEPS_PER_SECOND),
        (
            TassadarArticleRuntimeWorkloadFamily::StateMachineKernel,
            TassadarArticleRuntimeHorizon::TwoMillionStep,
        ) => (153_846, DIRECT_THROUGHPUT_FLOOR_STEPS_PER_SECOND),
    }
}

fn program_for(
    workload_family: TassadarArticleRuntimeWorkloadFamily,
    profile: &TassadarWasmProfile,
    iteration_count: i32,
) -> TassadarProgram {
    match workload_family {
        TassadarArticleRuntimeWorkloadFamily::LongLoopKernel => TassadarProgram::new(
            format!("tassadar.article_runtime_closeout.long_loop.{iteration_count}.v1"),
            profile,
            1,
            0,
            vec![
                TassadarInstruction::I32Const {
                    value: iteration_count,
                },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::BrIf { target_pc: 7 },
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 2 },
            ],
        ),
        TassadarArticleRuntimeWorkloadFamily::StateMachineKernel => TassadarProgram::new(
            format!("tassadar.article_runtime_closeout.state_machine.{iteration_count}.v1"),
            profile,
            2,
            1,
            vec![
                TassadarInstruction::I32Const {
                    value: iteration_count,
                },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::I32Store { slot: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::BrIf { target_pc: 11 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::I32Store { slot: 0 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 6 },
            ],
        ),
    }
}

pub(crate) fn article_runtime_closeout_hull_cache_direct_programs() -> Vec<TassadarProgram> {
    let profile = long_horizon_profile();
    let mut programs = Vec::new();
    for workload_family in [
        TassadarArticleRuntimeWorkloadFamily::LongLoopKernel,
        TassadarArticleRuntimeWorkloadFamily::StateMachineKernel,
    ] {
        for horizon in [
            TassadarArticleRuntimeHorizon::MillionStep,
            TassadarArticleRuntimeHorizon::TwoMillionStep,
        ] {
            let (iteration_count, _) = horizon_spec(workload_family, horizon);
            programs.push(program_for(workload_family, &profile, iteration_count as i32));
        }
    }
    programs
}

fn fallback_decode_receipt(
    requested_decode_mode: TassadarExecutorDecodeMode,
    program: &TassadarProgram,
    measured_steps_per_second: Option<f64>,
    note: &str,
) -> TassadarArticleRuntimeDecodeReceipt {
    let measured_direct = measured_steps_per_second.is_some();
    let (selection_state, selection_reason) =
        match requested_decode_mode {
            TassadarExecutorDecodeMode::ReferenceLinear => {
                (TassadarExecutorSelectionState::Direct, None)
            }
            TassadarExecutorDecodeMode::HullCache => {
                if measured_direct {
                    (TassadarExecutorSelectionState::Direct, None)
                } else if program.instructions.iter().enumerate().any(|(pc, instruction)| {
                    matches!(
                        instruction,
                        TassadarInstruction::BrIf { target_pc } if usize::from(*target_pc) <= pc
                    )
                }) {
                    (
                        TassadarExecutorSelectionState::Fallback,
                        Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported),
                    )
                } else {
                    (TassadarExecutorSelectionState::Direct, None)
                }
            }
            TassadarExecutorDecodeMode::SparseTopK => {
                if program.instructions.len() > 64
                || program
                    .instructions
                    .iter()
                    .enumerate()
                    .any(|(pc, instruction)| matches!(
                        instruction,
                        TassadarInstruction::BrIf { target_pc } if usize::from(*target_pc) <= pc
                    ))
            {
                (
                    TassadarExecutorSelectionState::Fallback,
                    Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported),
                )
            } else {
                (TassadarExecutorSelectionState::Direct, None)
            }
            }
        };
    TassadarArticleRuntimeDecodeReceipt {
        requested_decode_mode,
        selection_state,
        selection_reason,
        effective_decode_mode: Some(
            if selection_state == TassadarExecutorSelectionState::Direct {
                requested_decode_mode
            } else {
                TassadarExecutorDecodeMode::ReferenceLinear
            },
        ),
        measurement_posture: if measured_direct {
            TassadarMillionStepMeasurementPosture::Measured
        } else {
            TassadarMillionStepMeasurementPosture::SelectionOnly
        },
        steps_per_second: measured_steps_per_second,
        note: String::from(note),
    }
}

fn execute_long_horizon_program_hull_cache_summary(
    program: &TassadarProgram,
) -> Result<TassadarExecutionSummary, crate::TassadarExecutionRefusal> {
    let profile = long_horizon_profile();
    let trace_abi = TassadarTraceAbi {
        profile_id: String::from(LONG_HORIZON_PROFILE_ID),
        ..TassadarTraceAbi::article_i32_compute_v1()
    };
    let fixture_weights = long_horizon_fixture_weights();
    execute_program_hull_cache_summary(
        program,
        &profile,
        &trace_abi,
        crate::TASSADAR_HULL_CACHE_RUNNER_ID,
        &fixture_weights,
    )
}

fn long_horizon_profile() -> TassadarWasmProfile {
    TassadarWasmProfile {
        profile_id: String::from(LONG_HORIZON_PROFILE_ID),
        allowed_opcodes: TassadarOpcode::ALL.to_vec(),
        max_locals: 8,
        max_memory_slots: 8,
        max_program_len: 128,
        max_steps: 4_194_304,
        branch_mode: TassadarBranchMode::BrIfNonZero,
        host_output_opcode: true,
    }
}

fn long_horizon_fixture_weights() -> TassadarFixtureWeights {
    TassadarFixtureWeights {
        profile_id: String::from(LONG_HORIZON_PROFILE_ID),
        trace_abi_id: String::from("tassadar.trace.v1"),
        opcode_rules: TassadarFixtureWeights::core_i32_v1().opcode_rules,
    }
}

fn benchmark_steps_per_second<F>(
    steps_per_run: u64,
    mut runner: F,
) -> Result<f64, crate::TassadarExecutionRefusal>
where
    F: FnMut() -> Result<TassadarExecutionSummary, crate::TassadarExecutionRefusal>,
{
    let started = Instant::now();
    let execution = runner()?;
    let elapsed = started.elapsed().as_secs_f64().max(1e-9);
    let measured_steps = execution.step_count.max(steps_per_run.max(1));
    Ok(measured_steps as f64 / elapsed)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-runtime should live under <repo>/crates/psionic-runtime")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleRuntimeCloseoutError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarArticleRuntimeCloseoutError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarArticleRuntimeCloseoutError::Decode {
        path: format!("{} ({artifact_kind})", path.display()),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE, TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF,
        TassadarArticleRuntimeCloseoutBundle, TassadarArticleRuntimeFloorStatus,
        TassadarArticleRuntimeHorizon, TassadarArticleRuntimeWorkloadFamily,
        build_tassadar_article_runtime_closeout_bundle, read_repo_json,
        tassadar_article_runtime_closeout_bundle_path,
        write_tassadar_article_runtime_closeout_bundle,
    };
    use crate::TassadarExecutorSelectionState;

    fn normalized_bundle_value(bundle: &TassadarArticleRuntimeCloseoutBundle) -> serde_json::Value {
        let mut value = serde_json::to_value(bundle).expect("bundle serializes");
        value["bundle_digest"] = serde_json::Value::Null;
        for receipt in value["horizon_receipts"]
            .as_array_mut()
            .expect("horizon_receipts array")
        {
            receipt["direct_steps_per_second"] = serde_json::Value::Null;
            receipt["reference_linear"]["steps_per_second"] = serde_json::Value::Null;
            receipt["hull_cache"]["steps_per_second"] = serde_json::Value::Null;
        }
        value
    }

    #[test]
    fn article_runtime_closeout_bundle_is_machine_legible() {
        let bundle = build_tassadar_article_runtime_closeout_bundle().expect("bundle");

        assert_eq!(
            bundle.bundle_root_ref,
            TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF
        );
        assert_eq!(bundle.horizon_receipts.len(), 4);
        assert_eq!(bundle.exact_horizon_count, 4);
        assert_eq!(bundle.floor_pass_count, 4);
        assert!(bundle.horizon_receipts.iter().any(|receipt| {
            receipt.workload_family == TassadarArticleRuntimeWorkloadFamily::LongLoopKernel
                && receipt.horizon == TassadarArticleRuntimeHorizon::TwoMillionStep
                && receipt.exact_step_count >= 2_000_000
                && receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed
        }));
        assert!(bundle.horizon_receipts.iter().all(|receipt| {
            receipt.hull_cache.selection_state == TassadarExecutorSelectionState::Direct
                && receipt.hull_cache.selection_reason.is_none()
                && receipt.hull_cache_exactness_bps == 10_000
                && receipt.hull_cache_floor_status == TassadarArticleRuntimeFloorStatus::Passed
                && receipt.sparse_top_k.selection_state == TassadarExecutorSelectionState::Fallback
        }));
    }

    #[test]
    fn article_runtime_closeout_bundle_matches_committed_truth() {
        let generated = build_tassadar_article_runtime_closeout_bundle().expect("bundle");
        let committed: TassadarArticleRuntimeCloseoutBundle = read_repo_json(
            &format!(
                "{}/{}",
                TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF,
                TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE
            ),
            "tassadar_article_runtime_closeout_bundle",
        )
        .expect("committed bundle");
        assert_eq!(
            normalized_bundle_value(&generated),
            normalized_bundle_value(&committed)
        );
    }

    #[test]
    fn write_article_runtime_closeout_bundle_persists_current_truth() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let written =
            write_tassadar_article_runtime_closeout_bundle(temp_dir.path()).expect("write bundle");
        let persisted: TassadarArticleRuntimeCloseoutBundle = serde_json::from_slice(
            &std::fs::read(
                temp_dir
                    .path()
                    .join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE),
            )
            .expect("read"),
        )
        .expect("decode");
        assert_eq!(
            normalized_bundle_value(&written),
            normalized_bundle_value(&persisted)
        );
        assert_eq!(
            tassadar_article_runtime_closeout_bundle_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE)
        );
    }
}
