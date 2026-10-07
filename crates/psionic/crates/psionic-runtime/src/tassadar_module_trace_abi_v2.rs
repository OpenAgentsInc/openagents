use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarHostImportStub, TassadarHostImportStubKind, TassadarModuleExecution,
    TassadarModuleExecutionError, TassadarModuleExecutionHaltReason,
    TassadarModuleExecutionProgram, TassadarModuleFrameSnapshot, TassadarModuleTraceEvent,
    TassadarModuleTraceStep, TassadarStructuredControlBinaryOp,
};

const TASSADAR_MODULE_TRACE_ARTIFACT_SCHEMA_VERSION: u16 = 1;
const TASSADAR_MODULE_TRACE_LINEAGE_SCHEMA_VERSION: u16 = 1;

/// Stable identifier for the legacy full-snapshot module trace ABI.
pub const TASSADAR_MODULE_TRACE_ABI_V1_ID: &str = "tassadar.module_trace.v1";
/// Stable identifier for the frame-aware delta-oriented module trace ABI.
pub const TASSADAR_MODULE_TRACE_ABI_V2_ID: &str = "tassadar.module_trace.v2";

/// Runtime-owned module-trace ABI contract for the bounded module lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiContract {
    /// Stable ABI identifier.
    pub abi_id: String,
    /// Stable ABI version.
    pub schema_version: u16,
    /// Prior ABI identifier when this ABI is a follow-on version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor_abi_id: Option<String>,
    /// Prior ABI version when this ABI is a follow-on version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predecessor_abi_version: Option<u16>,
    /// Whether the ABI names frame transitions explicitly.
    pub frame_aware: bool,
    /// Whether the ABI emits state deltas rather than full snapshots.
    pub delta_oriented: bool,
    /// Whether the ABI still carries full globals/frame snapshots on each step.
    pub includes_full_state_snapshots: bool,
    /// Explicit replay posture for validators and training consumers.
    pub replay_posture: String,
    /// Explicit proof-lineage posture relative to the prior ABI.
    pub proof_lineage_posture: String,
}

impl TassadarModuleTraceAbiContract {
    /// Returns the legacy module-trace ABI derived from the full-snapshot lane.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            abi_id: String::from(TASSADAR_MODULE_TRACE_ABI_V1_ID),
            schema_version: 1,
            predecessor_abi_id: None,
            predecessor_abi_version: None,
            frame_aware: false,
            delta_oriented: false,
            includes_full_state_snapshots: true,
            replay_posture: String::from(
                "deterministic replay is available through the full-snapshot append-only trace itself",
            ),
            proof_lineage_posture: String::from(
                "legacy module-trace authority is the snapshot-heavy append-only step stream",
            ),
        }
    }

    /// Returns the frame-aware delta-oriented module-trace ABI.
    #[must_use]
    pub fn v2() -> Self {
        Self {
            abi_id: String::from(TASSADAR_MODULE_TRACE_ABI_V2_ID),
            schema_version: 2,
            predecessor_abi_id: Some(String::from(TASSADAR_MODULE_TRACE_ABI_V1_ID)),
            predecessor_abi_version: Some(1),
            frame_aware: true,
            delta_oriented: true,
            includes_full_state_snapshots: false,
            replay_posture: String::from(
                "deterministic replay remains available by rebuilding the snapshot-heavy execution from frame transitions, event values, and local/global deltas",
            ),
            proof_lineage_posture: String::from(
                "machine-truth lineage stays compatible with v1 through shared execution digest, final-state truth, and stable trace authority fields instead of per-step full snapshots",
            ),
        }
    }

    /// Returns a stable digest over the ABI compatibility surface.
    #[must_use]
    pub fn compatibility_digest(&self) -> String {
        stable_digest(b"tassadar_module_trace_abi_contract|", self)
    }
}

/// Full-snapshot trace artifact for the legacy module-trace ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceV1Artifact {
    /// Stable schema version for the artifact itself.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Stable execution digest shared across compatible ABIs.
    pub execution_digest: String,
    /// Number of emitted steps.
    pub step_count: u64,
    /// Optional returned value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Final globals in stable index order.
    pub final_globals: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarModuleExecutionHaltReason,
    /// Ordered snapshot-heavy steps.
    pub steps: Vec<TassadarModuleTraceStep>,
}

impl TassadarModuleTraceV1Artifact {
    /// Builds the legacy v1 artifact from one module execution.
    #[must_use]
    pub fn from_execution(
        artifact_id: impl Into<String>,
        execution: &TassadarModuleExecution,
    ) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_MODULE_TRACE_ARTIFACT_SCHEMA_VERSION,
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            program_id: execution.program_id.clone(),
            trace_abi_id: String::from(TASSADAR_MODULE_TRACE_ABI_V1_ID),
            trace_abi_version: 1,
            execution_digest: execution.execution_digest(),
            step_count: execution.steps.len() as u64,
            returned_value: execution.returned_value,
            final_globals: execution.final_globals.clone(),
            halt_reason: execution.halt_reason,
            steps: execution.steps.clone(),
        };
        artifact.artifact_digest = artifact.compute_digest();
        artifact
    }

    fn compute_digest(&self) -> String {
        stable_digest(b"tassadar_module_trace_v1_artifact|", self)
    }
}

/// One local delta in the frame-aware v2 trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLocalDelta {
    /// Function owning the local.
    pub function_index: u32,
    /// Stable function name.
    pub function_name: String,
    /// Local index inside the function.
    pub local_index: u32,
    /// Prior local value.
    pub before: i32,
    /// New local value.
    pub after: i32,
}

/// One global delta in the frame-aware v2 trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleGlobalDelta {
    /// Global index in stable order.
    pub global_index: u32,
    /// Prior global value.
    pub before: i32,
    /// New global value.
    pub after: i32,
}

/// Explicit frame transition in the module-trace ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleFrameTransition {
    /// No frame was pushed or popped on this step.
    None,
    /// One new frame was entered.
    Enter {
        /// Caller function when one existed.
        #[serde(skip_serializing_if = "Option::is_none")]
        caller_function_index: Option<u32>,
        /// Entered function index.
        function_index: u32,
        /// Entered function name.
        function_name: String,
    },
    /// One frame exited.
    Exit {
        /// Exited function index.
        function_index: u32,
        /// Exited function name.
        function_name: String,
        /// Caller function when one remained after exit.
        #[serde(skip_serializing_if = "Option::is_none")]
        returned_to_function_index: Option<u32>,
    },
}

/// One frame-aware delta-oriented step in module-trace ABI v2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceV2Step {
    /// Step index in execution order.
    pub step_index: usize,
    /// Current frame depth after the step.
    pub frame_depth_after: usize,
    /// Current active function after the step when one remains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_function_index_after: Option<u32>,
    /// Stable active function name after the step when one remains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_function_name_after: Option<String>,
    /// Event emitted by the step.
    pub event: TassadarModuleTraceEvent,
    /// Explicit frame transition emitted by the step.
    pub frame_transition: TassadarModuleFrameTransition,
    /// Local deltas emitted by the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_deltas: Vec<TassadarModuleLocalDelta>,
    /// Global deltas emitted by the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global_deltas: Vec<TassadarModuleGlobalDelta>,
}

/// Frame-aware delta-oriented trace artifact for module-trace ABI v2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceV2Artifact {
    /// Stable schema version for the artifact itself.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Explicit prior ABI identifier.
    pub predecessor_trace_abi_id: String,
    /// Explicit prior ABI version.
    pub predecessor_trace_abi_version: u16,
    /// Stable execution digest shared across compatible ABIs.
    pub execution_digest: String,
    /// Number of emitted steps.
    pub step_count: u64,
    /// Optional returned value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Final globals in stable index order.
    pub final_globals: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarModuleExecutionHaltReason,
    /// Ordered frame-aware delta-oriented steps.
    pub steps: Vec<TassadarModuleTraceV2Step>,
}

impl TassadarModuleTraceV2Artifact {
    fn compute_digest(&self) -> String {
        stable_digest(b"tassadar_module_trace_v2_artifact|", self)
    }
}

/// Trace-size comparison across the legacy and v2 module trace ABIs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceFootprint {
    /// Serialized byte count for the v1 step stream.
    pub v1_step_stream_bytes: u64,
    /// Serialized byte count for the v2 step stream.
    pub v2_step_stream_bytes: u64,
    /// Saved bytes between v1 and v2.
    pub step_stream_byte_savings: u64,
    /// Savings expressed in basis points against v1.
    pub step_stream_reduction_bps: u32,
}

/// Shared machine-truth lineage receipt binding v1 and v2 to the same execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceLineageCompatibilityReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable program identifier.
    pub program_id: String,
    /// Shared execution digest.
    pub execution_digest: String,
    /// Legacy v1 artifact digest.
    pub trace_v1_artifact_digest: String,
    /// V2 artifact digest.
    pub trace_v2_artifact_digest: String,
    /// Ordered machine-truth fields shared across both ABIs.
    pub shared_machine_truth_fields: Vec<String>,
    /// Stable compatibility digest over the lineage contract.
    pub compatibility_digest: String,
}

/// Replay failure for one module-trace ABI v2 artifact.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleTraceReplayError {
    /// Program validation failed before replay.
    #[error(transparent)]
    ProgramValidation(#[from] TassadarModuleExecutionError),
    /// The supplied artifact carried the wrong ABI identity.
    #[error(
        "module trace replay expected ABI {expected_abi_id}@{expected_abi_version}, got {actual_abi_id}@{actual_abi_version}"
    )]
    AbiMismatch {
        /// Expected ABI identifier.
        expected_abi_id: String,
        /// Expected ABI version.
        expected_abi_version: u16,
        /// Actual ABI identifier.
        actual_abi_id: String,
        /// Actual ABI version.
        actual_abi_version: u16,
    },
    /// The supplied artifact targeted a different program.
    #[error(
        "module trace replay expected program `{expected_program_id}`, got `{actual_program_id}`"
    )]
    ProgramMismatch {
        /// Expected program identifier.
        expected_program_id: String,
        /// Actual program identifier.
        actual_program_id: String,
    },
    /// Replay drifted from the declared step-level truth.
    #[error("module trace replay drift at step {step_index}: {detail}")]
    Drift {
        /// Step index where drift occurred.
        step_index: usize,
        /// Human-readable drift detail.
        detail: String,
    },
}

/// Builds the frame-aware delta-oriented module-trace ABI v2 artifact.
pub fn build_tassadar_module_trace_v2_artifact(
    artifact_id: impl Into<String>,
    program: &TassadarModuleExecutionProgram,
    execution: &TassadarModuleExecution,
) -> Result<TassadarModuleTraceV2Artifact, TassadarModuleExecutionError> {
    program.validate()?;
    let mut previous_globals = program
        .globals
        .iter()
        .map(|global| global.initial_value)
        .collect::<Vec<_>>();
    let mut previous_frames = initial_frame_stack(program)?;
    let mut steps = Vec::with_capacity(execution.steps.len());

    for step in &execution.steps {
        steps.push(TassadarModuleTraceV2Step {
            step_index: step.step_index,
            frame_depth_after: step.frame_depth_after,
            active_function_index_after: step
                .frame_stack_after
                .last()
                .map(|frame| frame.function_index),
            active_function_name_after: step
                .frame_stack_after
                .last()
                .map(|frame| frame.function_name.clone()),
            event: step.event.clone(),
            frame_transition: frame_transition_for_snapshots(
                &previous_frames,
                &step.frame_stack_after,
            ),
            local_deltas: local_deltas_for_snapshots(&previous_frames, &step.frame_stack_after),
            global_deltas: global_deltas_for_values(
                previous_globals.as_slice(),
                step.globals_after.as_slice(),
            ),
        });
        previous_globals = step.globals_after.clone();
        previous_frames = step.frame_stack_after.clone();
    }

    let mut artifact = TassadarModuleTraceV2Artifact {
        schema_version: TASSADAR_MODULE_TRACE_ARTIFACT_SCHEMA_VERSION,
        artifact_id: artifact_id.into(),
        artifact_digest: String::new(),
        program_id: execution.program_id.clone(),
        trace_abi_id: String::from(TASSADAR_MODULE_TRACE_ABI_V2_ID),
        trace_abi_version: 2,
        predecessor_trace_abi_id: String::from(TASSADAR_MODULE_TRACE_ABI_V1_ID),
        predecessor_trace_abi_version: 1,
        execution_digest: execution.execution_digest(),
        step_count: execution.steps.len() as u64,
        returned_value: execution.returned_value,
        final_globals: execution.final_globals.clone(),
        halt_reason: execution.halt_reason,
        steps,
    };
    artifact.artifact_digest = artifact.compute_digest();
    Ok(artifact)
}

/// Builds the shared lineage receipt binding v1 and v2 to one execution.
#[must_use]
pub fn build_tassadar_module_trace_lineage_compatibility_receipt(
    v1: &TassadarModuleTraceV1Artifact,
    v2: &TassadarModuleTraceV2Artifact,
) -> TassadarModuleTraceLineageCompatibilityReceipt {
    let mut receipt = TassadarModuleTraceLineageCompatibilityReceipt {
        schema_version: TASSADAR_MODULE_TRACE_LINEAGE_SCHEMA_VERSION,
        program_id: v1.program_id.clone(),
        execution_digest: v1.execution_digest.clone(),
        trace_v1_artifact_digest: v1.artifact_digest.clone(),
        trace_v2_artifact_digest: v2.artifact_digest.clone(),
        shared_machine_truth_fields: vec![
            String::from("program_id"),
            String::from("execution_digest"),
            String::from("step_count"),
            String::from("returned_value"),
            String::from("final_globals"),
            String::from("halt_reason"),
        ],
        compatibility_digest: String::new(),
    };
    receipt.compatibility_digest =
        stable_digest(b"tassadar_module_trace_lineage_receipt|", &receipt);
    receipt
}

/// Computes the serialized step-stream footprint across module-trace ABIs.
pub fn summarize_tassadar_module_trace_footprint(
    v1: &TassadarModuleTraceV1Artifact,
    v2: &TassadarModuleTraceV2Artifact,
) -> Result<TassadarModuleTraceFootprint, serde_json::Error> {
    let v1_step_stream_bytes = serde_json::to_vec(&v1.steps)?.len() as u64;
    let v2_step_stream_bytes = serde_json::to_vec(&v2.steps)?.len() as u64;
    let step_stream_byte_savings = v1_step_stream_bytes.saturating_sub(v2_step_stream_bytes);
    let step_stream_reduction_bps = if v1_step_stream_bytes == 0 {
        0
    } else {
        ((step_stream_byte_savings * 10_000) / v1_step_stream_bytes) as u32
    };
    Ok(TassadarModuleTraceFootprint {
        v1_step_stream_bytes,
        v2_step_stream_bytes,
        step_stream_byte_savings,
        step_stream_reduction_bps,
    })
}

/// Deterministically replays one module-trace ABI v2 artifact back into the
/// legacy snapshot-heavy execution view.
pub fn replay_tassadar_module_trace_v2_artifact(
    program: &TassadarModuleExecutionProgram,
    artifact: &TassadarModuleTraceV2Artifact,
) -> Result<TassadarModuleExecution, TassadarModuleTraceReplayError> {
    program.validate()?;
    let expected_abi = TassadarModuleTraceAbiContract::v2();
    if artifact.trace_abi_id != expected_abi.abi_id
        || artifact.trace_abi_version != expected_abi.schema_version
    {
        return Err(TassadarModuleTraceReplayError::AbiMismatch {
            expected_abi_id: expected_abi.abi_id,
            expected_abi_version: expected_abi.schema_version,
            actual_abi_id: artifact.trace_abi_id.clone(),
            actual_abi_version: artifact.trace_abi_version,
        });
    }
    if artifact.program_id != program.program_id {
        return Err(TassadarModuleTraceReplayError::ProgramMismatch {
            expected_program_id: program.program_id.clone(),
            actual_program_id: artifact.program_id.clone(),
        });
    }

    let mut globals = program
        .globals
        .iter()
        .map(|global| global.initial_value)
        .collect::<Vec<_>>();
    let mut frames = initial_replay_frames(program)?;
    let mut replayed_steps = Vec::with_capacity(artifact.steps.len());
    let mut returned_value = None;
    let mut halt_reason = TassadarModuleExecutionHaltReason::Returned;

    for (expected_step_index, step) in artifact.steps.iter().enumerate() {
        if step.step_index != expected_step_index {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: expected_step_index,
                detail: format!(
                    "expected step_index={expected_step_index}, got {}",
                    step.step_index
                ),
            });
        }

        let previous_globals = globals.clone();
        let previous_snapshots = snapshots_from_replay_frames(program, frames.as_slice());
        apply_trace_event(
            program,
            &mut globals,
            &mut frames,
            &mut returned_value,
            &mut halt_reason,
            step,
        )?;
        let current_snapshots = snapshots_from_replay_frames(program, frames.as_slice());

        let actual_transition =
            frame_transition_for_snapshots(&previous_snapshots, &current_snapshots);
        if actual_transition != step.frame_transition {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: String::from("frame transition mismatched the replayed state"),
            });
        }
        let actual_local_deltas =
            local_deltas_for_snapshots(&previous_snapshots, &current_snapshots);
        if actual_local_deltas != step.local_deltas {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: String::from("local deltas mismatched the replayed state"),
            });
        }
        let actual_global_deltas =
            global_deltas_for_values(previous_globals.as_slice(), globals.as_slice());
        if actual_global_deltas != step.global_deltas {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: String::from("global deltas mismatched the replayed state"),
            });
        }
        if frames.len() != step.frame_depth_after {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: format!(
                    "expected frame_depth_after={}, got {}",
                    step.frame_depth_after,
                    frames.len()
                ),
            });
        }
        if frames.last().map(|frame| frame.function_index) != step.active_function_index_after {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: String::from("active_function_index_after mismatched replayed state"),
            });
        }
        if frames
            .last()
            .map(|frame| frame.function_name(program))
            .as_deref()
            != step.active_function_name_after.as_deref()
        {
            return Err(TassadarModuleTraceReplayError::Drift {
                step_index: step.step_index,
                detail: String::from("active_function_name_after mismatched replayed state"),
            });
        }

        replayed_steps.push(TassadarModuleTraceStep {
            step_index: step.step_index,
            frame_depth_after: frames.len(),
            event: step.event.clone(),
            globals_after: globals.clone(),
            memory_byte_deltas: Vec::new(),
            memory_growth_delta: None,
            memory_size_pages_after: Vec::new(),
            frame_stack_after: current_snapshots,
        });
    }

    let replayed = TassadarModuleExecution {
        program_id: program.program_id.clone(),
        steps: replayed_steps,
        returned_value,
        final_globals: globals,
        final_memory_digests: Vec::new(),
        final_memory_pages: Vec::new(),
        halt_reason,
    };
    if replayed.execution_digest() != artifact.execution_digest {
        return Err(TassadarModuleTraceReplayError::Drift {
            step_index: artifact.steps.len(),
            detail: format!(
                "execution digest drifted: expected={}, actual={}",
                artifact.execution_digest,
                replayed.execution_digest()
            ),
        });
    }
    if replayed.returned_value != artifact.returned_value
        || replayed.final_globals != artifact.final_globals
        || replayed.halt_reason != artifact.halt_reason
    {
        return Err(TassadarModuleTraceReplayError::Drift {
            step_index: artifact.steps.len(),
            detail: String::from("final-state truth drifted from the artifact"),
        });
    }
    Ok(replayed)
}

fn initial_frame_stack(
    program: &TassadarModuleExecutionProgram,
) -> Result<Vec<TassadarModuleFrameSnapshot>, TassadarModuleExecutionError> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.function_index == program.entry_function_index)
        .ok_or(TassadarModuleExecutionError::MissingEntryFunction {
            entry_function_index: program.entry_function_index,
        })?;
    Ok(vec![TassadarModuleFrameSnapshot {
        function_index: entry.function_index,
        function_name: entry.function_name.clone(),
        pc: 0,
        locals: vec![0; entry.local_count],
        operand_stack: Vec::new(),
    }])
}

fn frame_transition_for_snapshots(
    previous_frames: &[TassadarModuleFrameSnapshot],
    current_frames: &[TassadarModuleFrameSnapshot],
) -> TassadarModuleFrameTransition {
    if current_frames.len() > previous_frames.len() {
        let caller_function_index = previous_frames.last().map(|frame| frame.function_index);
        let entered = current_frames
            .last()
            .expect("current_frames should contain the entered frame");
        TassadarModuleFrameTransition::Enter {
            caller_function_index,
            function_index: entered.function_index,
            function_name: entered.function_name.clone(),
        }
    } else if current_frames.len() < previous_frames.len() {
        let exited = previous_frames
            .last()
            .expect("previous_frames should contain the exited frame");
        TassadarModuleFrameTransition::Exit {
            function_index: exited.function_index,
            function_name: exited.function_name.clone(),
            returned_to_function_index: current_frames.last().map(|frame| frame.function_index),
        }
    } else {
        TassadarModuleFrameTransition::None
    }
}

fn local_deltas_for_snapshots(
    previous_frames: &[TassadarModuleFrameSnapshot],
    current_frames: &[TassadarModuleFrameSnapshot],
) -> Vec<TassadarModuleLocalDelta> {
    previous_frames
        .iter()
        .zip(current_frames.iter())
        .flat_map(|(before, after)| {
            if before.function_index != after.function_index {
                return Vec::new();
            }
            before
                .locals
                .iter()
                .zip(after.locals.iter())
                .enumerate()
                .filter_map(|(local_index, (before_value, after_value))| {
                    if before_value == after_value {
                        None
                    } else {
                        Some(TassadarModuleLocalDelta {
                            function_index: after.function_index,
                            function_name: after.function_name.clone(),
                            local_index: local_index as u32,
                            before: *before_value,
                            after: *after_value,
                        })
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn global_deltas_for_values(
    previous_globals: &[i32],
    current_globals: &[i32],
) -> Vec<TassadarModuleGlobalDelta> {
    previous_globals
        .iter()
        .zip(current_globals.iter())
        .enumerate()
        .filter_map(|(global_index, (before, after))| {
            if before == after {
                None
            } else {
                Some(TassadarModuleGlobalDelta {
                    global_index: global_index as u32,
                    before: *before,
                    after: *after,
                })
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReplayFrame {
    function_index: u32,
    pc: usize,
    locals: Vec<i32>,
    operand_stack: Vec<i32>,
}

impl ReplayFrame {
    fn function_name<'a>(&self, program: &'a TassadarModuleExecutionProgram) -> String {
        program.functions[self.function_index as usize]
            .function_name
            .clone()
    }
}

fn initial_replay_frames(
    program: &TassadarModuleExecutionProgram,
) -> Result<Vec<ReplayFrame>, TassadarModuleExecutionError> {
    let entry = program
        .functions
        .iter()
        .find(|function| function.function_index == program.entry_function_index)
        .ok_or(TassadarModuleExecutionError::MissingEntryFunction {
            entry_function_index: program.entry_function_index,
        })?;
    Ok(vec![ReplayFrame {
        function_index: entry.function_index,
        pc: 0,
        locals: vec![0; entry.local_count],
        operand_stack: Vec::new(),
    }])
}

fn snapshots_from_replay_frames(
    program: &TassadarModuleExecutionProgram,
    frames: &[ReplayFrame],
) -> Vec<TassadarModuleFrameSnapshot> {
    frames
        .iter()
        .map(|frame| TassadarModuleFrameSnapshot {
            function_index: frame.function_index,
            function_name: program.functions[frame.function_index as usize]
                .function_name
                .clone(),
            pc: frame.pc,
            locals: frame.locals.clone(),
            operand_stack: frame.operand_stack.clone(),
        })
        .collect()
}

fn apply_trace_event(
    program: &TassadarModuleExecutionProgram,
    globals: &mut [i32],
    frames: &mut Vec<ReplayFrame>,
    returned_value: &mut Option<i32>,
    halt_reason: &mut TassadarModuleExecutionHaltReason,
    step: &TassadarModuleTraceV2Step,
) -> Result<(), TassadarModuleTraceReplayError> {
    let top_function_index = frames.last().map(|frame| frame.function_index);
    match &step.event {
        TassadarModuleTraceEvent::ElementSegmentApplied { .. } => {}
        TassadarModuleTraceEvent::DataSegmentApplied { .. } => {}
        TassadarModuleTraceEvent::ConstPush { value } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            frame.operand_stack.push(*value);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::LocalGet { local_index, value } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let actual = *frame.locals.get(*local_index as usize).ok_or_else(|| {
                TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("local.get referenced missing local {local_index}"),
                }
            })?;
            if actual != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("local.get expected value {value}, replay saw {actual}"),
                });
            }
            frame.operand_stack.push(*value);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::LocalSet { local_index, value } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let popped =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("local.set underflowed the replay operand stack"),
                    })?;
            if popped != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("local.set expected popped value {value}, replay saw {popped}"),
                });
            }
            let local = frame.locals.get_mut(*local_index as usize).ok_or_else(|| {
                TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("local.set referenced missing local {local_index}"),
                }
            })?;
            *local = *value;
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::GlobalGet {
            global_index,
            value,
        } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let actual = *globals.get(*global_index as usize).ok_or_else(|| {
                TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("global.get referenced missing global {global_index}"),
                }
            })?;
            if actual != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("global.get expected value {value}, replay saw {actual}"),
                });
            }
            frame.operand_stack.push(*value);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::GlobalSet {
            global_index,
            value,
        } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let popped =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("global.set underflowed the replay operand stack"),
                    })?;
            if popped != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "global.set expected popped value {value}, replay saw {popped}"
                    ),
                });
            }
            let global = globals.get_mut(*global_index as usize).ok_or_else(|| {
                TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("global.set referenced missing global {global_index}"),
                }
            })?;
            *global = *value;
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::Drop { value } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let popped =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("drop underflowed the replay operand stack"),
                    })?;
            if popped != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("drop expected value {value}, replay saw {popped}"),
                });
            }
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::BinaryOp {
            op,
            left,
            right,
            result,
        } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let replay_right =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("binary_op missing right operand"),
                    })?;
            let replay_left =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("binary_op missing left operand"),
                    })?;
            if replay_left != *left || replay_right != *right {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "binary_op expected left/right=({left},{right}), replay saw ({replay_left},{replay_right})"
                    ),
                });
            }
            let replay_result = execute_binary_op(*op, replay_left, replay_right);
            if replay_result != *result {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "binary_op expected result {result}, replay computed {replay_result}"
                    ),
                });
            }
            frame.operand_stack.push(replay_result);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::Load { value, .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let _ =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("load missing dynamic address"),
                    })?;
            frame.operand_stack.push(*value);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::Store { value, .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let replay_value =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("store missing value"),
                    })?;
            if replay_value != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("store expected value {value}, replay saw {replay_value}"),
                });
            }
            let _ =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("store missing dynamic address"),
                    })?;
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::MemorySize { pages, .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            frame
                .operand_stack
                .push(i32::try_from(*pages).unwrap_or(i32::MAX));
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::MemoryGrow { result, .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            let _ =
                frame
                    .operand_stack
                    .pop()
                    .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("memory.grow missing requested delta"),
                    })?;
            frame.operand_stack.push(*result);
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::MemoryCopy { .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            for detail in ["memory.copy len", "memory.copy src", "memory.copy dst"] {
                let _ = frame.operand_stack.pop().ok_or_else(|| {
                    TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from(detail),
                    }
                })?;
            }
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::MemoryFill { .. } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            for detail in ["memory.fill len", "memory.fill value", "memory.fill dst"] {
                let _ = frame.operand_stack.pop().ok_or_else(|| {
                    TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from(detail),
                    }
                })?;
            }
            frame.pc = frame.pc.saturating_add(1);
        }
        TassadarModuleTraceEvent::Call { function_index } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            frame.pc = frame.pc.saturating_add(1);
            let function = program
                .functions
                .get(*function_index as usize)
                .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("call target function {function_index} is missing"),
                })?;
            frames.push(ReplayFrame {
                function_index: function.function_index,
                pc: 0,
                locals: vec![0; function.local_count],
                operand_stack: Vec::new(),
            });
        }
        TassadarModuleTraceEvent::CallIndirect {
            table_index,
            selector,
            function_index,
        } => {
            let selector_value = {
                let frame = current_frame_mut(frames, step.step_index)?;
                let selector_value = frame.operand_stack.pop().ok_or_else(|| {
                    TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("call_indirect missing selector"),
                    }
                })?;
                frame.pc = frame.pc.saturating_add(1);
                selector_value
            };
            if selector_value != *selector {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "call_indirect expected selector {selector}, replay saw {selector_value}"
                    ),
                });
            }
            let table = program.tables.get(*table_index as usize).ok_or_else(|| {
                TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("call_indirect referenced missing table {table_index}"),
                }
            })?;
            let resolved = instantiated_table_elements(program, table)
                .get(*selector as usize)
                .copied()
                .flatten()
                .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: String::from("call_indirect selector did not resolve during replay"),
                })?;
            if resolved != *function_index {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "call_indirect expected function {function_index}, replay resolved {resolved}"
                    ),
                });
            }
            let function = program
                .functions
                .get(*function_index as usize)
                .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!("call_indirect target function {function_index} is missing"),
                })?;
            frames.push(ReplayFrame {
                function_index: function.function_index,
                pc: 0,
                locals: vec![0; function.local_count],
                operand_stack: Vec::new(),
            });
        }
        TassadarModuleTraceEvent::HostCall {
            import_ref,
            stub_kind,
            result,
        } => {
            let frame = current_frame_mut(frames, step.step_index)?;
            frame.pc = frame.pc.saturating_add(1);
            match program.imports.iter().find(|import| match import {
                TassadarHostImportStub::DeterministicI32Const {
                    import_ref: candidate,
                    ..
                }
                | TassadarHostImportStub::UnsupportedHostCall {
                    import_ref: candidate,
                    ..
                } => candidate == import_ref,
            }) {
                Some(TassadarHostImportStub::DeterministicI32Const { value, .. }) => {
                    if *stub_kind != TassadarHostImportStubKind::DeterministicI32Const {
                        return Err(TassadarModuleTraceReplayError::Drift {
                            step_index: step.step_index,
                            detail: String::from(
                                "host_call stub kind drifted from deterministic import",
                            ),
                        });
                    }
                    if Some(*value) != *result {
                        return Err(TassadarModuleTraceReplayError::Drift {
                            step_index: step.step_index,
                            detail: format!(
                                "host_call expected deterministic result {:?}, replay saw {:?}",
                                result,
                                Some(*value)
                            ),
                        });
                    }
                    frame.operand_stack.push(*value);
                }
                Some(TassadarHostImportStub::UnsupportedHostCall { .. }) | None => {
                    return Err(TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from(
                            "host_call referenced an unsupported or missing import during replay",
                        ),
                    });
                }
            }
        }
        TassadarModuleTraceEvent::Return {
            function_index,
            value,
            implicit,
        } => {
            if top_function_index != Some(*function_index) {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "return expected frame {function_index}, replay saw {:?}",
                        top_function_index
                    ),
                });
            }
            if !*implicit {
                let frame = current_frame_mut(frames, step.step_index)?;
                let current_instruction = program.functions[*function_index as usize]
                    .instructions
                    .get(frame.pc);
                if current_instruction != Some(&crate::TassadarModuleInstruction::Return) {
                    return Err(TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from(
                            "explicit return did not point at a return instruction",
                        ),
                    });
                }
            }
            let result_count = program.functions[*function_index as usize].result_count;
            let mut frame = frames
                .pop()
                .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: String::from("return popped an empty frame stack"),
                })?;
            let replay_value = match result_count {
                0 => None,
                1 => Some(frame.operand_stack.pop().ok_or_else(|| {
                    TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: String::from("return expected one operand-stack value"),
                    }
                })?),
                other => {
                    return Err(TassadarModuleTraceReplayError::Drift {
                        step_index: step.step_index,
                        detail: format!("unsupported result_count {other} during replay"),
                    });
                }
            };
            if replay_value != *value {
                return Err(TassadarModuleTraceReplayError::Drift {
                    step_index: step.step_index,
                    detail: format!(
                        "return expected value {:?}, replay saw {:?}",
                        value, replay_value
                    ),
                });
            }
            if let Some(caller) = frames.last_mut() {
                if let Some(replay_value) = replay_value {
                    caller.operand_stack.push(replay_value);
                }
            } else {
                *returned_value = replay_value;
                *halt_reason = if *implicit {
                    TassadarModuleExecutionHaltReason::FellOffEnd
                } else {
                    TassadarModuleExecutionHaltReason::Returned
                };
            }
        }
    }
    Ok(())
}

fn instantiated_table_elements(
    program: &TassadarModuleExecutionProgram,
    table: &crate::TassadarModuleTable,
) -> Vec<Option<u32>> {
    let mut elements = table.elements.clone();
    for segment in &program.element_segments {
        if segment.table_index != table.table_index {
            continue;
        }
        let start = segment.offset as usize;
        let end = start + segment.elements.len();
        if end > elements.len() {
            continue;
        }
        for (slot, function_index) in segment.elements.iter().enumerate() {
            elements[start + slot] = *function_index;
        }
    }
    elements
}

fn current_frame_mut(
    frames: &mut [ReplayFrame],
    step_index: usize,
) -> Result<&mut ReplayFrame, TassadarModuleTraceReplayError> {
    frames
        .last_mut()
        .ok_or_else(|| TassadarModuleTraceReplayError::Drift {
            step_index,
            detail: String::from("trace replay reached a step without an active frame"),
        })
}

fn execute_binary_op(op: TassadarStructuredControlBinaryOp, left: i32, right: i32) -> i32 {
    match op {
        TassadarStructuredControlBinaryOp::Add => left.saturating_add(right),
        TassadarStructuredControlBinaryOp::Sub => left.saturating_sub(right),
        TassadarStructuredControlBinaryOp::Mul => left.saturating_mul(right),
        TassadarStructuredControlBinaryOp::Eq => i32::from(left == right),
        TassadarStructuredControlBinaryOp::Ne => i32::from(left != right),
        TassadarStructuredControlBinaryOp::LtS => i32::from(left < right),
        TassadarStructuredControlBinaryOp::LtU => i32::from((left as u32) < (right as u32)),
        TassadarStructuredControlBinaryOp::GtS => i32::from(left > right),
        TassadarStructuredControlBinaryOp::GtU => i32::from((left as u32) > (right as u32)),
        TassadarStructuredControlBinaryOp::LeS => i32::from(left <= right),
        TassadarStructuredControlBinaryOp::LeU => i32::from((left as u32) <= (right as u32)),
        TassadarStructuredControlBinaryOp::GeS => i32::from(left >= right),
        TassadarStructuredControlBinaryOp::GeU => i32::from((left as u32) >= (right as u32)),
        TassadarStructuredControlBinaryOp::And => left & right,
        TassadarStructuredControlBinaryOp::Or => left | right,
        TassadarStructuredControlBinaryOp::Xor => left ^ right,
        TassadarStructuredControlBinaryOp::Shl => left.wrapping_shl(right as u32),
        TassadarStructuredControlBinaryOp::ShrS => left.wrapping_shr(right as u32),
        TassadarStructuredControlBinaryOp::ShrU => {
            ((left as u32).wrapping_shr(right as u32)) as i32
        }
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleTraceAbiContract, TassadarModuleTraceV1Artifact,
        build_tassadar_module_trace_lineage_compatibility_receipt,
        build_tassadar_module_trace_v2_artifact, replay_tassadar_module_trace_v2_artifact,
        summarize_tassadar_module_trace_footprint,
    };
    use crate::{
        execute_tassadar_module_execution_program, tassadar_seeded_module_call_indirect_program,
        tassadar_seeded_module_deterministic_import_program,
        tassadar_seeded_module_global_state_program,
    };

    #[test]
    fn module_trace_abi_v2_has_explicit_versioning_relative_to_v1() {
        let v1 = TassadarModuleTraceAbiContract::v1();
        let v2 = TassadarModuleTraceAbiContract::v2();

        assert_eq!(v1.abi_id, "tassadar.module_trace.v1");
        assert_eq!(v2.predecessor_abi_id.as_deref(), Some(v1.abi_id.as_str()));
        assert_eq!(v2.predecessor_abi_version, Some(v1.schema_version));
        assert!(v2.frame_aware);
        assert!(v2.delta_oriented);
        assert!(!v2.includes_full_state_snapshots);
    }

    #[test]
    fn module_trace_abi_v2_replays_call_indirect_execution_exactly() {
        let program = tassadar_seeded_module_call_indirect_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        let artifact = build_tassadar_module_trace_v2_artifact(
            "tassadar://module_trace_v2/call_indirect",
            &program,
            &execution,
        )
        .expect("build");

        let replayed =
            replay_tassadar_module_trace_v2_artifact(&program, &artifact).expect("replay");
        assert_eq!(replayed, execution);
    }

    #[test]
    fn module_trace_abi_v2_preserves_lineage_against_v1() {
        let program = tassadar_seeded_module_global_state_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        let v1 = TassadarModuleTraceV1Artifact::from_execution(
            "tassadar://module_trace_v1/global_state",
            &execution,
        );
        let v2 = build_tassadar_module_trace_v2_artifact(
            "tassadar://module_trace_v2/global_state",
            &program,
            &execution,
        )
        .expect("build");
        let receipt = build_tassadar_module_trace_lineage_compatibility_receipt(&v1, &v2);

        assert_eq!(receipt.program_id, program.program_id);
        assert_eq!(receipt.execution_digest, execution.execution_digest());
        assert_eq!(receipt.shared_machine_truth_fields.len(), 6);
        assert!(!receipt.compatibility_digest.is_empty());
    }

    #[test]
    fn module_trace_abi_v2_reduces_step_stream_size() {
        let program = tassadar_seeded_module_deterministic_import_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        let v1 = TassadarModuleTraceV1Artifact::from_execution(
            "tassadar://module_trace_v1/import_stub",
            &execution,
        );
        let v2 = build_tassadar_module_trace_v2_artifact(
            "tassadar://module_trace_v2/import_stub",
            &program,
            &execution,
        )
        .expect("build");
        let footprint = summarize_tassadar_module_trace_footprint(&v1, &v2).expect("footprint");

        assert!(footprint.v2_step_stream_bytes < footprint.v1_step_stream_bytes);
        assert!(footprint.step_stream_byte_savings > 0);
        assert!(footprint.step_stream_reduction_bps > 0);
    }
}
