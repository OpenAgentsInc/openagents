use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarLinearMemoryExecution, TassadarLinearMemoryExecutionError,
    TassadarLinearMemoryInstruction, TassadarLinearMemoryProgram, TassadarLinearMemoryTraceEvent,
    execute_tassadar_linear_memory_program, tassadar_seeded_linear_memory_copy_fill_program,
};

/// Stable checkpoint-family identifier for persisted dynamic-memory resume artifacts.
pub const TASSADAR_DYNAMIC_MEMORY_RESUME_FAMILY_ID: &str = "tassadar.dynamic_memory_resume.v1";
/// Stable run root for the committed dynamic-memory resume bundle.
pub const TASSADAR_DYNAMIC_MEMORY_RESUME_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_dynamic_memory_resume_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_DYNAMIC_MEMORY_RESUME_BUNDLE_FILE: &str =
    "tassadar_dynamic_memory_resume_bundle.json";

/// Persisted checkpoint over one bounded linear-memory execution prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeCheckpoint {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Number of steps executed before the pause.
    pub paused_after_step_count: usize,
    /// Program counter at which resume should continue.
    pub next_pc: usize,
    /// Stack contents carried across the pause.
    pub stack_values: Vec<i32>,
    /// Current memory size in pages.
    pub current_pages: u32,
    /// Stable digest over the paused memory image.
    pub memory_digest: String,
    /// Full paused memory image as hex.
    pub memory_hex: String,
    /// Stable digest over the checkpoint.
    pub checkpoint_digest: String,
}

/// One case receipt over the bounded dynamic-memory resume lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeCaseReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Persisted checkpoint used for resume.
    pub checkpoint: TassadarDynamicMemoryResumeCheckpoint,
    /// Outputs emitted before the pause.
    pub prefix_outputs: Vec<i32>,
    /// Outputs emitted by the fresh full run.
    pub fresh_outputs: Vec<i32>,
    /// Outputs reconstructed from prefix plus resumed suffix.
    pub resumed_outputs: Vec<i32>,
    /// Final fresh-run memory digest.
    pub fresh_final_memory_digest: String,
    /// Final resumed-run memory digest.
    pub resumed_final_memory_digest: String,
    /// Whether final outputs and final memory matched exactly.
    pub exact_resume_parity: bool,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded dynamic-memory resume lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeBundle {
    /// Schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Ordered case receipts.
    pub case_receipts: Vec<TassadarDynamicMemoryResumeCaseReceipt>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

/// Build failures for the bounded dynamic-memory resume lane.
#[derive(Debug, Error)]
pub enum TassadarDynamicMemoryResumeError {
    /// Full or resumed execution failed.
    #[error(transparent)]
    Runtime(#[from] TassadarLinearMemoryExecutionError),
    /// The selected pause point exceeded the execution trace.
    #[error(
        "dynamic-memory resume pause point {paused_after_step_count} exceeded execution step_count {step_count}"
    )]
    PausePointOutOfRange {
        paused_after_step_count: usize,
        step_count: usize,
    },
    /// The paused memory image could not be decoded from the checkpoint.
    #[error("failed to decode paused memory image from checkpoint: {0}")]
    DecodeCheckpointMemory(#[from] hex::FromHexError),
}

/// Returns the canonical runtime bundle for the bounded dynamic-memory resume lane.
pub fn build_tassadar_dynamic_memory_resume_bundle()
-> Result<TassadarDynamicMemoryResumeBundle, TassadarDynamicMemoryResumeError> {
    let case_receipts = vec![build_resume_case(
        "copy_fill_pause_after_copy",
        tassadar_seeded_linear_memory_copy_fill_program(),
        4,
    )?];
    let mut bundle = TassadarDynamicMemoryResumeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.dynamic_memory_resume.bundle.v1"),
        checkpoint_family_id: String::from(TASSADAR_DYNAMIC_MEMORY_RESUME_FAMILY_ID),
        case_receipts,
        claim_boundary: String::from(
            "this bundle proves one bounded dynamic-memory pause-and-resume lane over the public linear-memory ABI with explicit checkpoint image, resumed suffix execution, and exact final-memory parity. It does not claim arbitrary checkpointing for arbitrary Wasm or broad served promotion.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(b"tassadar_dynamic_memory_resume_bundle|", &bundle);
    Ok(bundle)
}

fn build_resume_case(
    case_id: &str,
    program: TassadarLinearMemoryProgram,
    paused_after_step_count: usize,
) -> Result<TassadarDynamicMemoryResumeCaseReceipt, TassadarDynamicMemoryResumeError> {
    let fresh = execute_tassadar_linear_memory_program(&program)?;
    if paused_after_step_count == 0 || paused_after_step_count > fresh.steps.len() {
        return Err(TassadarDynamicMemoryResumeError::PausePointOutOfRange {
            paused_after_step_count,
            step_count: fresh.steps.len(),
        });
    }

    let checkpoint = build_checkpoint(case_id, &program, &fresh, paused_after_step_count)?;
    let resumed_program = resume_program_from_checkpoint(&program, &checkpoint)?;
    let resumed = execute_tassadar_linear_memory_program(&resumed_program)?;

    let prefix_outputs = fresh
        .steps
        .iter()
        .take(paused_after_step_count)
        .filter_map(|step| match step.event {
            TassadarLinearMemoryTraceEvent::Output { value } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut resumed_outputs = prefix_outputs.clone();
    resumed_outputs.extend(resumed.outputs.iter().copied());

    let mut receipt = TassadarDynamicMemoryResumeCaseReceipt {
        case_id: String::from(case_id),
        program_id: program.program_id.clone(),
        checkpoint,
        prefix_outputs,
        fresh_outputs: fresh.outputs.clone(),
        resumed_outputs,
        fresh_final_memory_digest: stable_digest(
            b"tassadar_dynamic_memory_resume_final_memory|",
            &fresh.final_memory,
        ),
        resumed_final_memory_digest: stable_digest(
            b"tassadar_dynamic_memory_resume_final_memory|",
            &resumed.final_memory,
        ),
        exact_resume_parity: false,
        receipt_digest: String::new(),
    };
    receipt.exact_resume_parity = receipt.fresh_outputs == receipt.resumed_outputs
        && receipt.fresh_final_memory_digest == receipt.resumed_final_memory_digest
        && fresh.final_stack == resumed.final_stack;
    receipt.receipt_digest = stable_digest(b"tassadar_dynamic_memory_resume_receipt|", &receipt);
    Ok(receipt)
}

fn build_checkpoint(
    case_id: &str,
    program: &TassadarLinearMemoryProgram,
    execution: &TassadarLinearMemoryExecution,
    paused_after_step_count: usize,
) -> Result<TassadarDynamicMemoryResumeCheckpoint, TassadarDynamicMemoryResumeError> {
    let step = &execution.steps[paused_after_step_count - 1];
    let paused_memory = materialize_memory_after_step(execution, paused_after_step_count)?;
    let current_pages = step.memory_size_pages_after;
    let next_pc = step.next_pc;
    let mut checkpoint = TassadarDynamicMemoryResumeCheckpoint {
        checkpoint_id: format!("{case_id}.checkpoint.v1"),
        program_id: program.program_id.clone(),
        paused_after_step_count,
        next_pc,
        stack_values: step.stack_after.clone(),
        current_pages,
        memory_digest: stable_digest(
            b"tassadar_dynamic_memory_resume_checkpoint_memory|",
            &paused_memory,
        ),
        memory_hex: hex::encode(paused_memory),
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest =
        stable_digest(b"tassadar_dynamic_memory_resume_checkpoint|", &checkpoint);
    Ok(checkpoint)
}

fn materialize_memory_after_step(
    execution: &TassadarLinearMemoryExecution,
    paused_after_step_count: usize,
) -> Result<Vec<u8>, TassadarLinearMemoryExecutionError> {
    let mut memory = execution.initial_memory.clone();
    let required_len = linear_memory_len_bytes(execution.initial_pages)?;
    if memory.len() < required_len {
        memory.resize(required_len, 0);
    }
    for step in execution.steps.iter().take(paused_after_step_count) {
        for delta in &step.memory_byte_deltas {
            let index = usize::try_from(delta.address)
                .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
            memory[index] = delta.after;
        }
        if let Some(growth) = &step.memory_growth_delta {
            memory.resize(linear_memory_len_bytes(growth.new_pages)?, 0);
        }
    }
    Ok(memory)
}

fn resume_program_from_checkpoint(
    program: &TassadarLinearMemoryProgram,
    checkpoint: &TassadarDynamicMemoryResumeCheckpoint,
) -> Result<TassadarLinearMemoryProgram, TassadarDynamicMemoryResumeError> {
    let mut instructions = checkpoint
        .stack_values
        .iter()
        .copied()
        .map(|value| TassadarLinearMemoryInstruction::I32Const { value })
        .collect::<Vec<_>>();
    instructions.extend(program.instructions[checkpoint.next_pc..].iter().cloned());
    Ok(TassadarLinearMemoryProgram::new(
        format!("{}.resumed", program.program_id),
        checkpoint.current_pages,
        program.max_pages,
        instructions,
    )
    .with_initial_memory(hex::decode(checkpoint.memory_hex.as_str())?))
}

fn linear_memory_len_bytes(pages: u32) -> Result<usize, TassadarLinearMemoryExecutionError> {
    let bytes = u64::from(pages)
        .checked_mul(u64::from(crate::TASSADAR_LINEAR_MEMORY_PAGE_BYTES))
        .ok_or(TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    usize::try_from(bytes).map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::build_tassadar_dynamic_memory_resume_bundle;

    #[test]
    fn dynamic_memory_resume_bundle_is_exact_on_seeded_case() {
        let bundle = build_tassadar_dynamic_memory_resume_bundle().expect("bundle should build");
        assert_eq!(bundle.case_receipts.len(), 1);
        assert!(bundle.case_receipts[0].exact_resume_parity);
        assert_eq!(bundle.case_receipts[0].prefix_outputs.len(), 0);
    }
}
