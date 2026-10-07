use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarCallFrameError, TassadarCallFrameExecution, TassadarCallFrameFunction,
    TassadarCallFrameHaltReason, TassadarCallFrameInstruction, TassadarCallFrameProgram,
    TassadarCallFrameSnapshot, TassadarCallFrameTraceEvent, TassadarCallFrameTraceStep,
    TassadarResumeRefusal, TassadarResumeRefusalKind, TassadarStructuredControlBinaryOp,
    execute_tassadar_call_frame_program, tassadar_seeded_call_frame_multi_function_program,
    tassadar_seeded_call_frame_recursive_sum_program,
};

const TASSADAR_CALL_FRAME_RESUME_MAX_STEPS: usize = 4_096;
const TASSADAR_CALL_FRAME_RESUME_MAX_CHECKPOINT_BYTES: usize = 768;

/// Stable profile id for the resumable multi-slice checkpoint-promotion lane.
pub const TASSADAR_CALL_FRAME_RESUME_PROFILE_ID: &str =
    "tassadar.internal_compute.resumable_multi_slice.v1";
/// Stable checkpoint-family identifier for persisted call-frame resume artifacts.
pub const TASSADAR_CALL_FRAME_RESUME_FAMILY_ID: &str = "tassadar.call_frame_resume.v1";
/// Stable run root for the committed call-frame resume bundle.
pub const TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_call_frame_resume_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_CALL_FRAME_RESUME_BUNDLE_FILE: &str =
    "tassadar_call_frame_resume_bundle.json";

/// Persisted checkpoint over one bounded call-frame execution prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameResumeCheckpoint {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable resumable profile identifier.
    pub profile_id: String,
    /// Number of executed steps before the pause.
    pub paused_after_step_count: usize,
    /// Next step index to execute after resume.
    pub next_step_index: usize,
    /// Current frame stack carried across the pause.
    pub frame_stack: Vec<TassadarCallFrameSnapshot>,
    /// Stable digest over the carried frame stack.
    pub frame_stack_digest: String,
    /// Stable replay identity for the resumed execution lineage.
    pub replay_identity: String,
    /// Later checkpoint that superseded this one, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_checkpoint_id: Option<String>,
    /// Stable digest over the full checkpoint contract.
    pub checkpoint_digest: String,
}

/// One case receipt over the bounded call-frame resume lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameResumeCaseReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Persisted checkpoint used for resume.
    pub checkpoint: TassadarCallFrameResumeCheckpoint,
    /// Fresh full-run execution digest.
    pub fresh_execution_digest: String,
    /// Reconstructed prefix-plus-resumed execution digest.
    pub resumed_execution_digest: String,
    /// Final returned value of the fresh run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_returned_value: Option<i32>,
    /// Final halt reason of the fresh run.
    pub final_halt_reason: TassadarCallFrameHaltReason,
    /// Whether prefix-plus-resumed execution matched the fresh trajectory exactly.
    pub exact_resume_parity: bool,
    /// Typed resume refusals exercised against the emitted checkpoint.
    pub refusal_cases: Vec<TassadarResumeRefusal>,
    /// Plain-language case note.
    pub note: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded call-frame resume lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameResumeBundle {
    /// Schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable resumable profile identifier.
    pub profile_id: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Ordered case receipts.
    pub case_receipts: Vec<TassadarCallFrameResumeCaseReceipt>,
    /// Number of exact fresh-vs-resumed parity rows.
    pub exact_resume_parity_count: u32,
    /// Number of typed refusal rows.
    pub refusal_case_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

/// Build failures for the bounded call-frame resume lane.
#[derive(Debug, Error)]
pub enum TassadarCallFrameResumeError {
    /// Full or resumed call-frame execution failed.
    #[error(transparent)]
    Runtime(#[from] TassadarCallFrameError),
    /// The selected pause point exceeded the execution trace.
    #[error(
        "call-frame resume pause point {paused_after_step_count} exceeded execution step_count {step_count}"
    )]
    PausePointOutOfRange {
        paused_after_step_count: usize,
        step_count: usize,
    },
    /// The pause point produced no live frame stack.
    #[error("call-frame resume pause point {paused_after_step_count} produced an empty frame stack")]
    EmptyFrameStack { paused_after_step_count: usize },
}

/// Returns the canonical runtime bundle for the bounded call-frame resume lane.
pub fn build_tassadar_call_frame_resume_bundle()
-> Result<TassadarCallFrameResumeBundle, TassadarCallFrameResumeError> {
    let case_receipts = vec![
        build_resume_case(
            "recursive_sum_pause_mid_stack",
            tassadar_seeded_call_frame_recursive_sum_program(),
            5,
        )?,
        build_resume_case(
            "multi_function_pause_after_direct_call",
            tassadar_seeded_call_frame_multi_function_program(),
            4,
        )?,
    ];
    let exact_resume_parity_count = case_receipts
        .iter()
        .filter(|receipt| receipt.exact_resume_parity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|receipt| receipt.refusal_cases.len() as u32)
        .sum();
    let mut bundle = TassadarCallFrameResumeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.call_frame_resume.bundle.v1"),
        profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        checkpoint_family_id: String::from(TASSADAR_CALL_FRAME_RESUME_FAMILY_ID),
        case_receipts,
        exact_resume_parity_count,
        refusal_case_count,
        claim_boundary: String::from(
            "this bundle proves one bounded call-frame checkpoint-and-resume lane over committed direct-call programs with explicit frame-stack checkpoints, exact prefix-plus-resumed replay, and typed profile, supersession, and oversized-state refusal. It does not claim arbitrary Wasm checkpointing, imports, or served promotion by itself.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(b"tassadar_call_frame_resume_bundle|", &bundle);
    Ok(bundle)
}

/// Resumes one call-frame program from a persisted checkpoint.
pub fn resume_tassadar_call_frame_program(
    program: &TassadarCallFrameProgram,
    checkpoint: &TassadarCallFrameResumeCheckpoint,
    expected_profile_id: Option<&str>,
    max_checkpoint_bytes: usize,
) -> Result<TassadarCallFrameExecution, TassadarResumeRefusal> {
    program.validate().map_err(|error| TassadarResumeRefusal {
        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
        detail: format!("call-frame resume refused because the program is invalid: {error}"),
    })?;
    if checkpoint.program_id != program.program_id {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "call-frame resume refused because checkpoint program `{}` did not match requested program `{}`",
                checkpoint.program_id, program.program_id
            ),
        });
    }
    if checkpoint
        .superseded_by_checkpoint_id
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::StaleCheckpointSuperseded,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "call-frame resume refused because checkpoint `{}` was superseded by `{}`",
                checkpoint.checkpoint_id,
                checkpoint
                    .superseded_by_checkpoint_id
                    .as_deref()
                    .unwrap_or_default()
            ),
        });
    }
    if let Some(expected_profile_id) = expected_profile_id {
        if checkpoint.profile_id != expected_profile_id {
            return Err(TassadarResumeRefusal {
                refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                detail: format!(
                    "call-frame resume refused because checkpoint profile `{}` did not match expected profile `{expected_profile_id}`",
                    checkpoint.profile_id
                ),
            });
        }
    }
    let checkpoint_bytes = serde_json::to_vec(checkpoint).unwrap_or_default();
    if checkpoint_bytes.len() > max_checkpoint_bytes {
        return Err(TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::OversizedCheckpointState,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: format!(
                "call-frame resume refused because checkpoint `{}` serialized to {} bytes, exceeding the maximum {} bytes",
                checkpoint.checkpoint_id,
                checkpoint_bytes.len(),
                max_checkpoint_bytes,
            ),
        });
    }

    let mut state = ResumeState {
        frames: checkpoint
            .frame_stack
            .iter()
            .cloned()
            .map(ResumeExecutionFrame::from_snapshot)
            .collect(),
        steps: Vec::new(),
        step_index: checkpoint.next_step_index,
    };
    let mut halt_reason = TassadarCallFrameHaltReason::FellOffEnd;
    let mut returned_value = None;

    while !state.frames.is_empty() {
        if state.step_index >= TASSADAR_CALL_FRAME_RESUME_MAX_STEPS {
            return Err(TassadarResumeRefusal {
                refusal_kind: TassadarResumeRefusalKind::SliceLimitExceeded,
                checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                detail: format!(
                    "call-frame resume refused because resumed execution exceeded the step limit of {}",
                    TASSADAR_CALL_FRAME_RESUME_MAX_STEPS
                ),
            });
        }
        let current_index = state.frames.len() - 1;
        let function_index = state.frames[current_index].function_index;
        let function = &program.functions[function_index as usize];
        if state.frames[current_index].pc >= function.instructions.len() {
            let value =
                finalize_function_return(&mut state.frames[current_index], function, false).map_err(
                    |error| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: error.to_string(),
                    },
                )?;
            state.frames.pop();
            if let Some(caller) = state.frames.last_mut() {
                if let Some(value) = value {
                    caller.operand_stack.push(value);
                }
                state
                    .push_step(TassadarCallFrameTraceEvent::Return {
                        function_index,
                        value,
                        implicit: true,
                    })
                    .map_err(|error| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: error.to_string(),
                    })?;
                continue;
            }
            halt_reason = TassadarCallFrameHaltReason::FellOffEnd;
            returned_value = value;
            state
                .push_step(TassadarCallFrameTraceEvent::Return {
                    function_index,
                    value,
                    implicit: true,
                })
                .map_err(|error| TassadarResumeRefusal {
                    refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                    checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                    detail: error.to_string(),
                })?;
            break;
        }

        let instruction = function.instructions[state.frames[current_index].pc].clone();
        match instruction {
            TassadarCallFrameInstruction::I32Const { value } => {
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::ConstPush { value })
            }
            TassadarCallFrameInstruction::LocalGet { local_index } => {
                let value = *state.frames[current_index]
                    .locals
                    .get(local_index as usize)
                    .ok_or_else(|| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: TassadarCallFrameError::LocalOutOfRange {
                            function_index,
                            local_index,
                            local_count: function.local_count,
                        }
                        .to_string(),
                    })?;
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalGet { local_index, value })
            }
            TassadarCallFrameInstruction::LocalSet { local_index } => {
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "local.set",
                )
                .map_err(|error| TassadarResumeRefusal {
                    refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                    checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                    detail: error.to_string(),
                })?;
                *state.frames[current_index]
                    .locals
                    .get_mut(local_index as usize)
                    .ok_or_else(|| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: TassadarCallFrameError::LocalOutOfRange {
                            function_index,
                            local_index,
                            local_count: function.local_count,
                        }
                        .to_string(),
                    })? = value;
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalSet { local_index, value })
            }
            TassadarCallFrameInstruction::LocalTee { local_index } => {
                let value = *state.frames[current_index]
                    .operand_stack
                    .last()
                    .ok_or_else(|| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: TassadarCallFrameError::StackUnderflow {
                            function_index,
                            pc: state.frames[current_index].pc,
                            context: String::from("local.tee"),
                            needed: 1,
                            available: 0,
                        }
                        .to_string(),
                    })?;
                *state.frames[current_index]
                    .locals
                    .get_mut(local_index as usize)
                    .ok_or_else(|| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: TassadarCallFrameError::LocalOutOfRange {
                            function_index,
                            local_index,
                            local_count: function.local_count,
                        }
                        .to_string(),
                    })? = value;
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalTee { local_index, value })
            }
            TassadarCallFrameInstruction::BinaryOp { op } => {
                let right = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "binary_op",
                )
                .map_err(|error| TassadarResumeRefusal {
                    refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                    checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                    detail: error.to_string(),
                })?;
                let left = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "binary_op",
                )
                .map_err(|error| TassadarResumeRefusal {
                    refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                    checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                    detail: error.to_string(),
                })?;
                let result = execute_binary_op(op, left, right);
                state.frames[current_index].operand_stack.push(result);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::BinaryOp {
                    op,
                    left,
                    right,
                    result,
                })
            }
            TassadarCallFrameInstruction::Jump { target_pc } => {
                let from_pc = state.frames[current_index].pc;
                state.frames[current_index].pc = target_pc;
                state.push_step(TassadarCallFrameTraceEvent::Jump {
                    function_index,
                    from_pc,
                    target_pc,
                })
            }
            TassadarCallFrameInstruction::JumpIfZero { target_pc } => {
                let from_pc = state.frames[current_index].pc;
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "jump_if_zero",
                )
                .map_err(|error| TassadarResumeRefusal {
                    refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                    checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                    detail: error.to_string(),
                })?;
                let taken = value == 0;
                state.frames[current_index].pc = if taken { target_pc } else { from_pc + 1 };
                state.push_step(TassadarCallFrameTraceEvent::JumpIfZero {
                    function_index,
                    from_pc,
                    target_pc,
                    value,
                    taken,
                })
            }
            TassadarCallFrameInstruction::Call {
                function_index: callee_index,
            } => {
                if state.frames.len() as u32 >= program.max_call_depth {
                    return Err(TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::SliceLimitExceeded,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: TassadarCallFrameError::RecursionDepthExceeded {
                            attempted_function_index: callee_index,
                            max_call_depth: program.max_call_depth,
                        }
                        .to_string(),
                    });
                }
                let callee = &program.functions[callee_index as usize];
                let args = pop_call_args(&mut state.frames[current_index], function_index, callee)
                    .map_err(|error| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: error.to_string(),
                    })?;
                state.frames[current_index].pc += 1;
                state.frames.push(ResumeExecutionFrame::new(callee, args.clone()));
                state.push_step(TassadarCallFrameTraceEvent::Call {
                    caller_function_index: function_index,
                    callee_function_index: callee_index,
                    args,
                })
            }
            TassadarCallFrameInstruction::Return => {
                let value = finalize_function_return(&mut state.frames[current_index], function, true)
                    .map_err(|error| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: error.to_string(),
                    })?;
                state.frames.pop();
                if let Some(caller) = state.frames.last_mut() {
                    if let Some(value) = value {
                        caller.operand_stack.push(value);
                    }
                    state
                        .push_step(TassadarCallFrameTraceEvent::Return {
                            function_index,
                            value,
                            implicit: false,
                        })
                        .map_err(|error| TassadarResumeRefusal {
                            refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                            detail: error.to_string(),
                        })?;
                    continue;
                }
                halt_reason = TassadarCallFrameHaltReason::Returned;
                returned_value = value;
                state
                    .push_step(TassadarCallFrameTraceEvent::Return {
                        function_index,
                        value,
                        implicit: false,
                    })
                    .map_err(|error| TassadarResumeRefusal {
                        refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
                        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
                        detail: error.to_string(),
                    })?;
                break;
            }
        }
        .map_err(|error| TassadarResumeRefusal {
            refusal_kind: TassadarResumeRefusalKind::ProfileMismatch,
            checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
            detail: error.to_string(),
        })?;
    }

    Ok(TassadarCallFrameExecution {
        program_id: program.program_id.clone(),
        steps: state.steps,
        returned_value,
        halt_reason,
    })
}

fn build_resume_case(
    case_id: &str,
    program: TassadarCallFrameProgram,
    paused_after_step_count: usize,
) -> Result<TassadarCallFrameResumeCaseReceipt, TassadarCallFrameResumeError> {
    let fresh = execute_tassadar_call_frame_program(&program)?;
    if paused_after_step_count == 0 || paused_after_step_count > fresh.steps.len() {
        return Err(TassadarCallFrameResumeError::PausePointOutOfRange {
            paused_after_step_count,
            step_count: fresh.steps.len(),
        });
    }
    let checkpoint = build_checkpoint(case_id, &program, &fresh, paused_after_step_count)?;
    let resumed = resume_tassadar_call_frame_program(
        &program,
        &checkpoint,
        Some(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        TASSADAR_CALL_FRAME_RESUME_MAX_CHECKPOINT_BYTES,
    )
    .expect("seeded call-frame checkpoint should resume exactly");
    let mut resumed_steps = fresh.steps[..paused_after_step_count].to_vec();
    resumed_steps.extend(resumed.steps.clone());
    let reconstructed = TassadarCallFrameExecution {
        program_id: program.program_id.clone(),
        steps: resumed_steps,
        returned_value: resumed.returned_value,
        halt_reason: resumed.halt_reason,
    };

    let oversized_refusal = resume_tassadar_call_frame_program(
        &program,
        &checkpoint,
        Some(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        32,
    )
    .expect_err("oversized checkpoint must refuse");
    let profile_mismatch_refusal =
        resume_tassadar_call_frame_program(&program, &checkpoint, Some("other.profile"), 4_096)
            .expect_err("profile mismatch must refuse");
    let mut superseded_checkpoint = checkpoint.clone();
    superseded_checkpoint.superseded_by_checkpoint_id =
        Some(String::from("recursive_sum_pause_mid_stack.superseded"));
    superseded_checkpoint.checkpoint_digest = stable_digest(
        b"tassadar_call_frame_resume_checkpoint|",
        &superseded_checkpoint,
    );
    let stale_refusal = resume_tassadar_call_frame_program(
        &program,
        &superseded_checkpoint,
        Some(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        4_096,
    )
    .expect_err("superseded checkpoint must refuse");

    let mut receipt = TassadarCallFrameResumeCaseReceipt {
        case_id: String::from(case_id),
        program_id: program.program_id.clone(),
        checkpoint,
        fresh_execution_digest: fresh.execution_digest(),
        resumed_execution_digest: reconstructed.execution_digest(),
        final_returned_value: fresh.returned_value,
        final_halt_reason: fresh.halt_reason,
        exact_resume_parity: fresh == reconstructed,
        refusal_cases: vec![oversized_refusal, profile_mismatch_refusal, stale_refusal],
        note: String::from(
            "call-frame resume parity is exact only for the committed paused prefixes under the resumable multi-slice profile; arbitrary checkpoint surgery remains out of scope",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_call_frame_resume_receipt|", &receipt);
    Ok(receipt)
}

fn build_checkpoint(
    case_id: &str,
    program: &TassadarCallFrameProgram,
    execution: &TassadarCallFrameExecution,
    paused_after_step_count: usize,
) -> Result<TassadarCallFrameResumeCheckpoint, TassadarCallFrameResumeError> {
    let step = &execution.steps[paused_after_step_count - 1];
    if step.frame_stack_after.is_empty() {
        return Err(TassadarCallFrameResumeError::EmptyFrameStack {
            paused_after_step_count,
        });
    }
    let mut checkpoint = TassadarCallFrameResumeCheckpoint {
        checkpoint_id: format!("{case_id}.checkpoint.v1"),
        program_id: program.program_id.clone(),
        profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        paused_after_step_count,
        next_step_index: paused_after_step_count,
        frame_stack: step.frame_stack_after.clone(),
        frame_stack_digest: stable_digest(
            b"tassadar_call_frame_resume_frame_stack|",
            &step.frame_stack_after,
        ),
        replay_identity: stable_digest(
            b"tassadar_call_frame_resume_replay_identity|",
            &(program.program_id.as_str(), paused_after_step_count),
        ),
        superseded_by_checkpoint_id: None,
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest =
        stable_digest(b"tassadar_call_frame_resume_checkpoint|", &checkpoint);
    Ok(checkpoint)
}

#[derive(Clone)]
struct ResumeExecutionFrame {
    function_index: u32,
    function_name: String,
    pc: usize,
    locals: Vec<i32>,
    operand_stack: Vec<i32>,
}

impl ResumeExecutionFrame {
    fn new(function: &TassadarCallFrameFunction, args: Vec<i32>) -> Self {
        let mut locals = vec![0; function.local_count];
        for (index, value) in args.into_iter().enumerate() {
            locals[index] = value;
        }
        Self {
            function_index: function.function_index,
            function_name: function.function_name.clone(),
            pc: 0,
            locals,
            operand_stack: Vec::new(),
        }
    }

    fn from_snapshot(snapshot: TassadarCallFrameSnapshot) -> Self {
        Self {
            function_index: snapshot.function_index,
            function_name: snapshot.function_name,
            pc: snapshot.pc,
            locals: snapshot.locals,
            operand_stack: snapshot.operand_stack,
        }
    }
}

struct ResumeState {
    frames: Vec<ResumeExecutionFrame>,
    steps: Vec<TassadarCallFrameTraceStep>,
    step_index: usize,
}

impl ResumeState {
    fn push_step(&mut self, event: TassadarCallFrameTraceEvent) -> Result<(), TassadarCallFrameError> {
        self.steps.push(TassadarCallFrameTraceStep {
            step_index: self.step_index,
            frame_depth_after: self.frames.len(),
            event,
            frame_stack_after: self
                .frames
                .iter()
                .map(|frame| TassadarCallFrameSnapshot {
                    function_index: frame.function_index,
                    function_name: frame.function_name.clone(),
                    pc: frame.pc,
                    locals: frame.locals.clone(),
                    operand_stack: frame.operand_stack.clone(),
                })
                .collect(),
        });
        self.step_index = self.step_index.saturating_add(1);
        Ok(())
    }
}

fn pop_operand(
    frame: &mut ResumeExecutionFrame,
    function_index: u32,
    context: &str,
) -> Result<i32, TassadarCallFrameError> {
    let available = frame.operand_stack.len();
    frame
        .operand_stack
        .pop()
        .ok_or_else(|| TassadarCallFrameError::StackUnderflow {
            function_index,
            pc: frame.pc,
            context: String::from(context),
            needed: 1,
            available,
        })
}

fn pop_call_args(
    frame: &mut ResumeExecutionFrame,
    caller_function_index: u32,
    callee: &TassadarCallFrameFunction,
) -> Result<Vec<i32>, TassadarCallFrameError> {
    let needed = usize::from(callee.param_count);
    let available = frame.operand_stack.len();
    if available < needed {
        return Err(TassadarCallFrameError::StackUnderflow {
            function_index: caller_function_index,
            pc: frame.pc,
            context: String::from("call_args"),
            needed,
            available,
        });
    }
    let mut args = Vec::with_capacity(needed);
    for _ in 0..needed {
        args.push(frame.operand_stack.pop().expect("availability checked"));
    }
    args.reverse();
    Ok(args)
}

fn finalize_function_return(
    frame: &mut ResumeExecutionFrame,
    function: &TassadarCallFrameFunction,
    advance_pc: bool,
) -> Result<Option<i32>, TassadarCallFrameError> {
    if advance_pc {
        frame.pc += 1;
    }
    match function.result_count {
        0 => Ok(None),
        1 => {
            let available = frame.operand_stack.len();
            frame.operand_stack.pop().map(Some).ok_or_else(|| {
                TassadarCallFrameError::StackUnderflow {
                    function_index: function.function_index,
                    pc: frame.pc.saturating_sub(usize::from(advance_pc)),
                    context: String::from("return_value"),
                    needed: 1,
                    available,
                }
            })
        }
        result_count => Err(TassadarCallFrameError::UnsupportedResultCount {
            function_index: function.function_index,
            result_count,
        }),
    }
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
        TassadarStructuredControlBinaryOp::ShrU => ((left as u32).wrapping_shr(right as u32)) as i32,
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
        TASSADAR_CALL_FRAME_RESUME_PROFILE_ID, build_tassadar_call_frame_resume_bundle,
        resume_tassadar_call_frame_program,
    };
    use crate::tassadar_seeded_call_frame_recursive_sum_program;

    #[test]
    fn call_frame_resume_bundle_is_exact_on_seeded_cases() {
        let bundle = build_tassadar_call_frame_resume_bundle().expect("bundle");
        assert_eq!(bundle.exact_resume_parity_count, 2);
        assert_eq!(bundle.refusal_case_count, 6);
    }

    #[test]
    fn call_frame_resume_refuses_profile_mismatch() {
        let bundle = build_tassadar_call_frame_resume_bundle().expect("bundle");
        let checkpoint = &bundle.case_receipts[0].checkpoint;
        let refusal = resume_tassadar_call_frame_program(
            &tassadar_seeded_call_frame_recursive_sum_program(),
            checkpoint,
            Some("other.profile"),
            4_096,
        )
        .expect_err("profile mismatch should refuse");
        assert_eq!(refusal.refusal_kind, crate::TassadarResumeRefusalKind::ProfileMismatch);
        assert_eq!(checkpoint.profile_id, TASSADAR_CALL_FRAME_RESUME_PROFILE_ID);
    }
}
