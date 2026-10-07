use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    TassadarExecution, TassadarExecutionRefusal, TassadarInstruction, TassadarProgram,
    TassadarTraceEvent, TassadarTraceStep,
};

/// Stable prompt-field identifier for the canonical article route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticlePromptFieldId {
    LocalCount,
    MemorySlots,
    InitialMemory,
    Instructions,
}

/// One prompt-field row in a representation-order-sensitive surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field_id", rename_all = "snake_case")]
pub enum TassadarArticlePromptFieldRow {
    LocalCount { value: usize },
    MemorySlots { value: usize },
    InitialMemory { values: Vec<i32> },
    Instructions { values: Vec<TassadarInstruction> },
}

impl TassadarArticlePromptFieldRow {
    #[must_use]
    pub const fn field_id(&self) -> TassadarArticlePromptFieldId {
        match self {
            Self::LocalCount { .. } => TassadarArticlePromptFieldId::LocalCount,
            Self::MemorySlots { .. } => TassadarArticlePromptFieldId::MemorySlots,
            Self::InitialMemory { .. } => TassadarArticlePromptFieldId::InitialMemory,
            Self::Instructions { .. } => TassadarArticlePromptFieldId::Instructions,
        }
    }
}

/// Explicit prompt surface used to prove field-order invariance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticlePromptFieldSurface {
    pub surface_id: String,
    pub program_id: String,
    pub profile_id: String,
    pub field_rows: Vec<TassadarArticlePromptFieldRow>,
}

impl TassadarArticlePromptFieldSurface {
    /// Builds the canonical prompt-field surface from one program.
    #[must_use]
    pub fn from_program(program: &TassadarProgram) -> Self {
        Self {
            surface_id: format!("{}.prompt_field_surface.v1", program.program_id),
            program_id: program.program_id.clone(),
            profile_id: program.profile_id.clone(),
            field_rows: vec![
                TassadarArticlePromptFieldRow::LocalCount {
                    value: program.local_count,
                },
                TassadarArticlePromptFieldRow::MemorySlots {
                    value: program.memory_slots,
                },
                TassadarArticlePromptFieldRow::InitialMemory {
                    values: program.initial_memory.clone(),
                },
                TassadarArticlePromptFieldRow::Instructions {
                    values: program.instructions.clone(),
                },
            ],
        }
    }

    /// Returns one reordered field surface over the same semantic prompt.
    pub fn reordered(
        &self,
        field_order: &[TassadarArticlePromptFieldId],
    ) -> Result<Self, TassadarArticleRepresentationInvarianceError> {
        if field_order.len() != self.field_rows.len() {
            return Err(
                TassadarArticleRepresentationInvarianceError::InvalidPromptFieldOrderLength {
                    expected: self.field_rows.len(),
                    actual: field_order.len(),
                },
            );
        }
        let mut seen = BTreeSet::new();
        let mut reordered_rows = Vec::with_capacity(field_order.len());
        for field_id in field_order {
            if !seen.insert(*field_id) {
                return Err(
                    TassadarArticleRepresentationInvarianceError::DuplicatePromptFieldOrderEntry {
                        field_id: format!("{field_id:?}").to_lowercase(),
                    },
                );
            }
            let row = self
                .field_rows
                .iter()
                .find(|row| row.field_id() == *field_id)
                .cloned()
                .ok_or_else(|| {
                    TassadarArticleRepresentationInvarianceError::MissingPromptField {
                        field_id: format!("{field_id:?}").to_lowercase(),
                    }
                })?;
            reordered_rows.push(row);
        }
        Ok(Self {
            surface_id: self.surface_id.clone(),
            program_id: self.program_id.clone(),
            profile_id: self.profile_id.clone(),
            field_rows: reordered_rows,
        })
    }

    /// Materializes one semantic program from the field surface.
    pub fn materialize_program(
        &self,
    ) -> Result<TassadarProgram, TassadarArticleRepresentationInvarianceError> {
        let mut local_count = None;
        let mut memory_slots = None;
        let mut initial_memory = None;
        let mut instructions = None;

        for row in &self.field_rows {
            match row {
                TassadarArticlePromptFieldRow::LocalCount { value } => {
                    if local_count.replace(*value).is_some() {
                        return Err(
                            TassadarArticleRepresentationInvarianceError::DuplicatePromptField {
                                field_id: String::from("local_count"),
                            },
                        );
                    }
                }
                TassadarArticlePromptFieldRow::MemorySlots { value } => {
                    if memory_slots.replace(*value).is_some() {
                        return Err(
                            TassadarArticleRepresentationInvarianceError::DuplicatePromptField {
                                field_id: String::from("memory_slots"),
                            },
                        );
                    }
                }
                TassadarArticlePromptFieldRow::InitialMemory { values } => {
                    if initial_memory.replace(values.clone()).is_some() {
                        return Err(
                            TassadarArticleRepresentationInvarianceError::DuplicatePromptField {
                                field_id: String::from("initial_memory"),
                            },
                        );
                    }
                }
                TassadarArticlePromptFieldRow::Instructions { values } => {
                    if instructions.replace(values.clone()).is_some() {
                        return Err(
                            TassadarArticleRepresentationInvarianceError::DuplicatePromptField {
                                field_id: String::from("instructions"),
                            },
                        );
                    }
                }
            }
        }

        Ok(TassadarProgram {
            program_id: self.program_id.clone(),
            profile_id: self.profile_id.clone(),
            local_count: local_count.ok_or_else(|| {
                TassadarArticleRepresentationInvarianceError::MissingPromptField {
                    field_id: String::from("local_count"),
                }
            })?,
            memory_slots: memory_slots.ok_or_else(|| {
                TassadarArticleRepresentationInvarianceError::MissingPromptField {
                    field_id: String::from("memory_slots"),
                }
            })?,
            initial_memory: initial_memory.ok_or_else(|| {
                TassadarArticleRepresentationInvarianceError::MissingPromptField {
                    field_id: String::from("initial_memory"),
                }
            })?,
            instructions: instructions.ok_or_else(|| {
                TassadarArticleRepresentationInvarianceError::MissingPromptField {
                    field_id: String::from("instructions"),
                }
            })?,
        })
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleRepresentationInvarianceError {
    #[error("prompt field `{field_id}` is missing")]
    MissingPromptField { field_id: String },
    #[error("prompt field `{field_id}` appears more than once")]
    DuplicatePromptField { field_id: String },
    #[error("prompt field order length {actual} does not match required field count {expected}")]
    InvalidPromptFieldOrderLength { expected: usize, actual: usize },
    #[error("prompt field order repeats `{field_id}`")]
    DuplicatePromptFieldOrderEntry { field_id: String },
    #[error("invalid local permutation for local_count={local_count}: {detail}")]
    InvalidLocalPermutation { local_count: usize, detail: String },
    #[error(
        "locals vector length {actual} does not match program-local count {expected} while remapping locals"
    )]
    InvalidLocalVectorLength { expected: usize, actual: usize },
    #[error("unreachable instruction suffix requires a return-terminated program")]
    UnreachableSuffixRequiresReturn,
    #[error(transparent)]
    RuntimeExecution(#[from] TassadarExecutionRefusal),
}

/// Inverts one `old_index -> new_index` local permutation.
pub fn invert_tassadar_local_permutation(
    old_to_new: &[usize],
) -> Result<Vec<usize>, TassadarArticleRepresentationInvarianceError> {
    validate_local_permutation(old_to_new, old_to_new.len())?;
    let mut inverse = vec![0; old_to_new.len()];
    for (old_index, new_index) in old_to_new.iter().copied().enumerate() {
        inverse[new_index] = old_index;
    }
    Ok(inverse)
}

/// Remaps local-slot references in one program under an `old -> new`
/// permutation.
pub fn remap_tassadar_program_local_indices(
    program: &TassadarProgram,
    old_to_new: &[usize],
) -> Result<TassadarProgram, TassadarArticleRepresentationInvarianceError> {
    validate_local_permutation(old_to_new, program.local_count)?;
    let instructions = program
        .instructions
        .iter()
        .map(|instruction| remap_instruction_local_indices(instruction, old_to_new))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TassadarProgram {
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        local_count: program.local_count,
        memory_slots: program.memory_slots,
        initial_memory: program.initial_memory.clone(),
        instructions,
    })
}

/// Remaps local-slot references and local-state vectors in one execution under
/// a `current -> new` permutation.
pub fn remap_tassadar_execution_local_indices(
    execution: &TassadarExecution,
    current_to_new: &[usize],
) -> Result<TassadarExecution, TassadarArticleRepresentationInvarianceError> {
    let local_count = execution.final_locals.len();
    validate_local_permutation(current_to_new, local_count)?;
    let steps = execution
        .steps
        .iter()
        .map(|step| remap_trace_step_local_indices(step, current_to_new, local_count))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TassadarExecution {
        program_id: execution.program_id.clone(),
        profile_id: execution.profile_id.clone(),
        runner_id: execution.runner_id.clone(),
        trace_abi: execution.trace_abi.clone(),
        steps,
        outputs: execution.outputs.clone(),
        final_locals: remap_local_values(&execution.final_locals, current_to_new, local_count)?,
        final_memory: execution.final_memory.clone(),
        final_stack: execution.final_stack.clone(),
        halt_reason: execution.halt_reason,
    })
}

/// Appends one semantically dead instruction suffix after a final `return`.
pub fn append_tassadar_unreachable_instruction_suffix(
    program: &TassadarProgram,
    suffix: Vec<TassadarInstruction>,
) -> Result<TassadarProgram, TassadarArticleRepresentationInvarianceError> {
    if !matches!(
        program.instructions.last(),
        Some(TassadarInstruction::Return)
    ) {
        return Err(TassadarArticleRepresentationInvarianceError::UnreachableSuffixRequiresReturn);
    }
    let mut instructions = program.instructions.clone();
    instructions.extend(suffix);
    Ok(TassadarProgram {
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        local_count: program.local_count,
        memory_slots: program.memory_slots,
        initial_memory: program.initial_memory.clone(),
        instructions,
    })
}

fn validate_local_permutation(
    permutation: &[usize],
    local_count: usize,
) -> Result<(), TassadarArticleRepresentationInvarianceError> {
    if permutation.len() != local_count {
        return Err(
            TassadarArticleRepresentationInvarianceError::InvalidLocalPermutation {
                local_count,
                detail: format!(
                    "permutation length {} does not match local_count {}",
                    permutation.len(),
                    local_count
                ),
            },
        );
    }
    let expected = (0..local_count).collect::<BTreeSet<_>>();
    let actual = permutation.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(
            TassadarArticleRepresentationInvarianceError::InvalidLocalPermutation {
                local_count,
                detail: format!("permutation entries {actual:?} do not match {expected:?}"),
            },
        );
    }
    if permutation.iter().any(|index| *index > u8::MAX as usize) {
        return Err(
            TassadarArticleRepresentationInvarianceError::InvalidLocalPermutation {
                local_count,
                detail: format!(
                    "permutation exceeds the supported u8 local index range: {permutation:?}"
                ),
            },
        );
    }
    Ok(())
}

fn remap_trace_step_local_indices(
    step: &TassadarTraceStep,
    current_to_new: &[usize],
    local_count: usize,
) -> Result<TassadarTraceStep, TassadarArticleRepresentationInvarianceError> {
    Ok(TassadarTraceStep {
        step_index: step.step_index,
        pc: step.pc,
        next_pc: step.next_pc,
        instruction: remap_instruction_local_indices(&step.instruction, current_to_new)?,
        event: remap_event_local_indices(&step.event, current_to_new)?,
        stack_before: step.stack_before.clone(),
        stack_after: step.stack_after.clone(),
        locals_after: remap_local_values(&step.locals_after, current_to_new, local_count)?,
        memory_after: step.memory_after.clone(),
    })
}

fn remap_instruction_local_indices(
    instruction: &TassadarInstruction,
    current_to_new: &[usize],
) -> Result<TassadarInstruction, TassadarArticleRepresentationInvarianceError> {
    Ok(match instruction {
        TassadarInstruction::LocalGet { local } => TassadarInstruction::LocalGet {
            local: remap_local_index(*local, current_to_new)?,
        },
        TassadarInstruction::LocalSet { local } => TassadarInstruction::LocalSet {
            local: remap_local_index(*local, current_to_new)?,
        },
        _ => instruction.clone(),
    })
}

fn remap_event_local_indices(
    event: &TassadarTraceEvent,
    current_to_new: &[usize],
) -> Result<TassadarTraceEvent, TassadarArticleRepresentationInvarianceError> {
    Ok(match event {
        TassadarTraceEvent::LocalGet { local, value } => TassadarTraceEvent::LocalGet {
            local: remap_local_index(*local, current_to_new)?,
            value: *value,
        },
        TassadarTraceEvent::LocalSet { local, value } => TassadarTraceEvent::LocalSet {
            local: remap_local_index(*local, current_to_new)?,
            value: *value,
        },
        _ => event.clone(),
    })
}

fn remap_local_values(
    values: &[i32],
    current_to_new: &[usize],
    local_count: usize,
) -> Result<Vec<i32>, TassadarArticleRepresentationInvarianceError> {
    if values.len() != local_count {
        return Err(
            TassadarArticleRepresentationInvarianceError::InvalidLocalVectorLength {
                expected: local_count,
                actual: values.len(),
            },
        );
    }
    let mut remapped = vec![0; values.len()];
    for (current_index, value) in values.iter().copied().enumerate() {
        remapped[current_to_new[current_index]] = value;
    }
    Ok(remapped)
}

fn remap_local_index(
    local: u8,
    current_to_new: &[usize],
) -> Result<u8, TassadarArticleRepresentationInvarianceError> {
    let local = usize::from(local);
    if local >= current_to_new.len() {
        return Err(
            TassadarArticleRepresentationInvarianceError::InvalidLocalPermutation {
                local_count: current_to_new.len(),
                detail: format!("local index {local} is out of range"),
            },
        );
    }
    Ok(current_to_new[local] as u8)
}

#[cfg(test)]
mod tests {
    use super::{
        append_tassadar_unreachable_instruction_suffix, invert_tassadar_local_permutation,
        remap_tassadar_execution_local_indices, remap_tassadar_program_local_indices,
        TassadarArticlePromptFieldId, TassadarArticlePromptFieldSurface,
    };
    use crate::{tassadar_article_class_corpus, TassadarCpuReferenceRunner, TassadarInstruction};

    #[test]
    fn prompt_field_surface_materializes_program_independently_of_field_order() {
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article case");
        let canonical_surface = TassadarArticlePromptFieldSurface::from_program(&case.program);
        let reordered_surface = canonical_surface
            .reordered(&[
                TassadarArticlePromptFieldId::Instructions,
                TassadarArticlePromptFieldId::InitialMemory,
                TassadarArticlePromptFieldId::MemorySlots,
                TassadarArticlePromptFieldId::LocalCount,
            ])
            .expect("reordered field surface");

        assert_ne!(canonical_surface.field_rows, reordered_surface.field_rows);
        assert_eq!(
            canonical_surface
                .materialize_program()
                .expect("canonical prompt surface"),
            case.program
        );
        assert_eq!(
            reordered_surface
                .materialize_program()
                .expect("reordered prompt surface"),
            case.program
        );
    }

    #[test]
    fn local_renaming_is_semantically_equivalent_after_inverse_remap() {
        let case = tassadar_article_class_corpus()
            .into_iter()
            .find(|case| case.program.local_count >= 2)
            .expect("article case with multiple locals");
        let mut rename = (0..case.program.local_count).collect::<Vec<_>>();
        rename.swap(0, 1);
        let renamed_program =
            remap_tassadar_program_local_indices(&case.program, &rename).expect("renamed program");
        let canonical_execution = TassadarCpuReferenceRunner::for_program(&case.program)
            .expect("canonical runner")
            .execute(&case.program)
            .expect("canonical execution");
        let renamed_execution = TassadarCpuReferenceRunner::for_program(&renamed_program)
            .expect("renamed runner")
            .execute(&renamed_program)
            .expect("renamed execution");
        let inverse = invert_tassadar_local_permutation(&rename).expect("inverse permutation");
        let normalized_program =
            remap_tassadar_program_local_indices(&renamed_program, &inverse).expect("normalized");
        let normalized_execution =
            remap_tassadar_execution_local_indices(&renamed_execution, &inverse)
                .expect("normalized execution");

        assert_eq!(renamed_execution.outputs, canonical_execution.outputs);
        assert_eq!(normalized_program.instructions, case.program.instructions);
        assert_eq!(normalized_execution.steps, canonical_execution.steps);
        assert_eq!(
            normalized_execution.final_locals,
            canonical_execution.final_locals
        );
        assert_eq!(
            normalized_execution.final_memory,
            canonical_execution.final_memory
        );
        assert_eq!(
            normalized_execution.final_stack,
            canonical_execution.final_stack
        );
        assert_eq!(
            normalized_execution.halt_reason,
            canonical_execution.halt_reason
        );
    }

    #[test]
    fn unreachable_suffix_keeps_execution_behavior_stable() {
        let case = tassadar_article_class_corpus()
            .into_iter()
            .find(|case| {
                matches!(
                    case.program.instructions.last(),
                    Some(TassadarInstruction::Return)
                )
            })
            .expect("return-terminated article case");
        let canonical_execution = TassadarCpuReferenceRunner::for_program(&case.program)
            .expect("canonical runner")
            .execute(&case.program)
            .expect("canonical execution");
        let perturbed_program = append_tassadar_unreachable_instruction_suffix(
            &case.program,
            vec![
                TassadarInstruction::I32Const { value: 99 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        )
        .expect("unreachable suffix");
        let perturbed_execution = TassadarCpuReferenceRunner::for_program(&perturbed_program)
            .expect("perturbed runner")
            .execute(&perturbed_program)
            .expect("perturbed execution");

        assert_eq!(perturbed_execution.steps, canonical_execution.steps);
        assert_eq!(perturbed_execution.outputs, canonical_execution.outputs);
        assert_eq!(
            perturbed_execution.behavior_digest(),
            canonical_execution.behavior_digest()
        );
    }
}
