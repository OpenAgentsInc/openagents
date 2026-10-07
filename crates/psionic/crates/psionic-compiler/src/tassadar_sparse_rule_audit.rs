use std::collections::{BTreeMap, BTreeSet};

use psionic_ir::{
    TassadarSparseRuleSourceKind, TassadarSparseRuleSourceRef, TassadarSparseRuleTargetKind,
    TassadarSparseTransitionRule, TassadarSymbolicExpr, TassadarSymbolicOperand,
    TassadarSymbolicProgram, TassadarSymbolicProgramError, TassadarSymbolicStatement,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    compile_tassadar_symbolic_program_to_artifact_bundle, TassadarSymbolicArtifactBundleError,
};

/// One machine-legible minimality audit over one sparse-rule projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleMinimalityAudit {
    /// Total number of sparse rules in the current program.
    pub total_rule_count: usize,
    /// Rules required when full trace supervision is available.
    pub full_trace_rule_ids: Vec<String>,
    /// Rules required when only final outputs plus final memory remain visible.
    pub final_state_required_rule_ids: Vec<String>,
    /// Rules required when only final outputs remain visible.
    pub io_only_required_rule_ids: Vec<String>,
    /// Rules that do not affect final outputs or final memory.
    pub dead_rule_ids: Vec<String>,
    /// Rules required for final-state truth but hidden by IO-only supervision.
    pub io_only_underconstrained_rule_ids: Vec<String>,
    /// Label-neutral duplicate-signature groups inside the current program.
    pub duplicate_signature_rule_groups: Vec<Vec<String>>,
    /// Stable digest over the minimality audit.
    pub audit_digest: String,
}

impl TassadarSparseRuleMinimalityAudit {
    fn new(
        total_rule_count: usize,
        full_trace_rule_ids: Vec<String>,
        final_state_required_rule_ids: Vec<String>,
        io_only_required_rule_ids: Vec<String>,
        dead_rule_ids: Vec<String>,
        io_only_underconstrained_rule_ids: Vec<String>,
        duplicate_signature_rule_groups: Vec<Vec<String>>,
    ) -> Self {
        let mut audit = Self {
            total_rule_count,
            full_trace_rule_ids,
            final_state_required_rule_ids,
            io_only_required_rule_ids,
            dead_rule_ids,
            io_only_underconstrained_rule_ids,
            duplicate_signature_rule_groups,
            audit_digest: String::new(),
        };
        audit.audit_digest = stable_digest(b"tassadar_sparse_rule_minimality_audit|", &audit);
        audit
    }
}

/// One compiler-side sparse-rule audit over one symbolic program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleCompilerAudit {
    /// Stable symbolic program identifier.
    pub symbolic_program_id: String,
    /// Stable symbolic program digest.
    pub symbolic_program_digest: String,
    /// Target runtime profile identifier.
    pub target_profile_id: String,
    /// Digest-bound bundle identifier produced by lowering.
    pub bundle_id: String,
    /// Validated runtime instruction count for the lowered artifact.
    pub validated_instruction_count: usize,
    /// Ordered lowering opcode requirements for the symbolic program.
    pub required_lowering_opcodes: Vec<psionic_ir::TassadarSymbolicLoweringOpcode>,
    /// Statement-projected sparse-rule view of the program.
    pub rules: Vec<TassadarSparseTransitionRule>,
    /// Minimality and under-specification audit facts for the sparse-rule view.
    pub minimality_audit: TassadarSparseRuleMinimalityAudit,
    /// Stable digest over the compiler audit.
    pub audit_digest: String,
}

impl TassadarSparseRuleCompilerAudit {
    fn new(
        symbolic_program_id: String,
        symbolic_program_digest: String,
        target_profile_id: String,
        bundle_id: String,
        validated_instruction_count: usize,
        required_lowering_opcodes: Vec<psionic_ir::TassadarSymbolicLoweringOpcode>,
        rules: Vec<TassadarSparseTransitionRule>,
        minimality_audit: TassadarSparseRuleMinimalityAudit,
    ) -> Self {
        let mut audit = Self {
            symbolic_program_id,
            symbolic_program_digest,
            target_profile_id,
            bundle_id,
            validated_instruction_count,
            required_lowering_opcodes,
            rules,
            minimality_audit,
            audit_digest: String::new(),
        };
        audit.audit_digest = stable_digest(b"tassadar_sparse_rule_compiler_audit|", &audit);
        audit
    }
}

/// Sparse-rule compiler-audit assembly failure.
#[derive(Debug, Error)]
pub enum TassadarSparseRuleCompilerAuditError {
    /// The symbolic program or input assignments were invalid.
    #[error(transparent)]
    Symbolic(#[from] TassadarSymbolicProgramError),
    /// Artifact-bundle assembly failed on the bounded lowering lane.
    #[error(transparent)]
    Compiler(#[from] TassadarSymbolicArtifactBundleError),
}

/// Builds one sparse-rule compiler audit over one symbolic program and concrete
/// input assignment set.
pub fn build_tassadar_sparse_rule_compiler_audit(
    program: &TassadarSymbolicProgram,
    input_assignments: &BTreeMap<String, i32>,
) -> Result<TassadarSparseRuleCompilerAudit, TassadarSparseRuleCompilerAuditError> {
    program.validate()?;
    let bundle = compile_tassadar_symbolic_program_to_artifact_bundle(program, input_assignments)?;
    let rules = build_sparse_transition_rules(program);
    let minimality_audit = build_minimality_audit(program, rules.as_slice());

    Ok(TassadarSparseRuleCompilerAudit::new(
        bundle.symbolic_program_id.clone(),
        bundle.symbolic_program_digest.clone(),
        bundle.program_artifact.wasm_profile_id.clone(),
        bundle.bundle_id.clone(),
        bundle.program_artifact.validated_program.instructions.len(),
        bundle.required_lowering_opcodes.clone(),
        rules,
        minimality_audit,
    ))
}

fn build_sparse_transition_rules(
    program: &TassadarSymbolicProgram,
) -> Vec<TassadarSparseTransitionRule> {
    program
        .statements
        .iter()
        .enumerate()
        .map(|(statement_index, statement)| {
            let (operation, source_refs) = statement_operation_and_sources(program, statement);
            let (target_kind, target_label) = statement_target(statement);
            TassadarSparseTransitionRule::new(
                format!("rule_{statement_index:02}"),
                statement_index,
                operation,
                target_kind,
                target_label,
                source_refs,
            )
        })
        .collect()
}

fn build_minimality_audit(
    program: &TassadarSymbolicProgram,
    rules: &[TassadarSparseTransitionRule],
) -> TassadarSparseRuleMinimalityAudit {
    let full_trace_indices = rules
        .iter()
        .map(|rule| rule.statement_index)
        .collect::<BTreeSet<_>>();
    let final_state_roots = final_state_root_indices(program);
    let io_only_roots = rules
        .iter()
        .filter(|rule| rule.target_kind == TassadarSparseRuleTargetKind::Output)
        .map(|rule| rule.statement_index)
        .collect::<Vec<_>>();
    let final_state_indices = backward_slice_statement_indices(program, rules, final_state_roots);
    let io_only_indices = backward_slice_statement_indices(program, rules, io_only_roots);

    let dead_rule_indices = full_trace_indices
        .difference(&final_state_indices)
        .copied()
        .collect::<BTreeSet<_>>();
    let io_only_underconstrained_indices = final_state_indices
        .difference(&io_only_indices)
        .copied()
        .collect::<BTreeSet<_>>();

    TassadarSparseRuleMinimalityAudit::new(
        rules.len(),
        ordered_rule_ids(rules, &full_trace_indices),
        ordered_rule_ids(rules, &final_state_indices),
        ordered_rule_ids(rules, &io_only_indices),
        ordered_rule_ids(rules, &dead_rule_indices),
        ordered_rule_ids(rules, &io_only_underconstrained_indices),
        duplicate_signature_rule_groups(rules),
    )
}

fn final_state_root_indices(program: &TassadarSymbolicProgram) -> Vec<usize> {
    let mut roots = program
        .statements
        .iter()
        .enumerate()
        .filter_map(|(statement_index, statement)| match statement {
            TassadarSymbolicStatement::Output { .. } => Some(statement_index),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut last_store_by_slot = BTreeMap::new();
    for (statement_index, statement) in program.statements.iter().enumerate() {
        if let TassadarSymbolicStatement::Store { slot, .. } = statement {
            last_store_by_slot.insert(*slot, statement_index);
        }
    }
    roots.extend(last_store_by_slot.into_values());
    roots.sort_unstable();
    roots.dedup();
    roots
}

fn backward_slice_statement_indices(
    program: &TassadarSymbolicProgram,
    rules: &[TassadarSparseTransitionRule],
    root_indices: Vec<usize>,
) -> BTreeSet<usize> {
    let binding_producers = program
        .statements
        .iter()
        .enumerate()
        .filter_map(|(statement_index, statement)| match statement {
            TassadarSymbolicStatement::Let { name, .. } => Some((name.as_str(), statement_index)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let rules_by_index = rules
        .iter()
        .map(|rule| (rule.statement_index, rule))
        .collect::<BTreeMap<_, _>>();
    let mut covered = BTreeSet::new();
    let mut pending = root_indices;

    while let Some(statement_index) = pending.pop() {
        if !covered.insert(statement_index) {
            continue;
        }
        let Some(rule) = rules_by_index.get(&statement_index) else {
            continue;
        };
        for source_ref in &rule.source_refs {
            if let Some(dependency_index) =
                dependency_statement_index(program, statement_index, source_ref, &binding_producers)
            {
                pending.push(dependency_index);
            }
        }
    }

    covered
}

fn dependency_statement_index(
    program: &TassadarSymbolicProgram,
    current_statement_index: usize,
    source_ref: &TassadarSparseRuleSourceRef,
    binding_producers: &BTreeMap<&str, usize>,
) -> Option<usize> {
    match source_ref.source_kind {
        TassadarSparseRuleSourceKind::Binding => binding_producers
            .get(source_ref.reference.as_str())
            .copied(),
        TassadarSparseRuleSourceKind::MemorySlot => {
            let slot = source_ref.reference.parse::<u8>().ok()?;
            program
                .statements
                .iter()
                .enumerate()
                .take(current_statement_index)
                .rev()
                .find_map(|(statement_index, statement)| match statement {
                    TassadarSymbolicStatement::Store {
                        slot: stored_slot, ..
                    } if *stored_slot == slot => Some(statement_index),
                    _ => None,
                })
        }
        TassadarSparseRuleSourceKind::Input | TassadarSparseRuleSourceKind::Const => None,
    }
}

fn ordered_rule_ids(
    rules: &[TassadarSparseTransitionRule],
    covered_indices: &BTreeSet<usize>,
) -> Vec<String> {
    rules
        .iter()
        .filter(|rule| covered_indices.contains(&rule.statement_index))
        .map(|rule| rule.rule_id.clone())
        .collect()
}

fn duplicate_signature_rule_groups(rules: &[TassadarSparseTransitionRule]) -> Vec<Vec<String>> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for rule in rules {
        grouped
            .entry(rule.signature_digest.clone())
            .or_default()
            .push(rule.rule_id.clone());
    }
    grouped
        .into_values()
        .filter(|rule_ids| rule_ids.len() > 1)
        .collect()
}

fn statement_operation_and_sources(
    program: &TassadarSymbolicProgram,
    statement: &TassadarSymbolicStatement,
) -> (String, Vec<TassadarSparseRuleSourceRef>) {
    match statement {
        TassadarSymbolicStatement::Let { expr, .. } => match expr {
            TassadarSymbolicExpr::Operand { operand } => (
                String::from("operand"),
                vec![operand_source_ref(program, operand)],
            ),
            TassadarSymbolicExpr::Binary { op, left, right } => (
                op.as_str().to_string(),
                vec![
                    operand_source_ref(program, left),
                    operand_source_ref(program, right),
                ],
            ),
        },
        TassadarSymbolicStatement::Store { value, .. } => (
            String::from("store"),
            vec![operand_source_ref(program, value)],
        ),
        TassadarSymbolicStatement::Output { value } => (
            String::from("output"),
            vec![operand_source_ref(program, value)],
        ),
    }
}

fn statement_target(
    statement: &TassadarSymbolicStatement,
) -> (TassadarSparseRuleTargetKind, String) {
    match statement {
        TassadarSymbolicStatement::Let { name, .. } => {
            (TassadarSparseRuleTargetKind::Binding, name.clone())
        }
        TassadarSymbolicStatement::Store { slot, .. } => {
            (TassadarSparseRuleTargetKind::MemorySlot, slot.to_string())
        }
        TassadarSymbolicStatement::Output { .. } => {
            (TassadarSparseRuleTargetKind::Output, String::from("output"))
        }
    }
}

fn operand_source_ref(
    program: &TassadarSymbolicProgram,
    operand: &TassadarSymbolicOperand,
) -> TassadarSparseRuleSourceRef {
    match operand {
        TassadarSymbolicOperand::Name { name } => TassadarSparseRuleSourceRef {
            source_kind: if program.input_slot(name.as_str()).is_some() {
                TassadarSparseRuleSourceKind::Input
            } else {
                TassadarSparseRuleSourceKind::Binding
            },
            reference: name.clone(),
        },
        TassadarSymbolicOperand::Const { value } => TassadarSparseRuleSourceRef {
            source_kind: TassadarSparseRuleSourceKind::Const,
            reference: value.to_string(),
        },
        TassadarSymbolicOperand::MemorySlot { slot } => TassadarSparseRuleSourceRef {
            source_kind: TassadarSparseRuleSourceKind::MemorySlot,
            reference: slot.to_string(),
        },
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
    use psionic_ir::tassadar_sparse_rule_audit_cases;

    use super::build_tassadar_sparse_rule_compiler_audit;

    #[test]
    fn sparse_rule_compiler_audit_covers_kernel_and_scan_style_cases() {
        let audits = tassadar_sparse_rule_audit_cases()
            .into_iter()
            .map(|case| {
                let rule_count = case.program.statements.len();
                let workload_group_id = case.workload_group_id.clone();
                let audit = build_tassadar_sparse_rule_compiler_audit(
                    &case.program,
                    &case.input_assignments,
                )
                .expect("seeded sparse-rule case should compile");
                (workload_group_id, rule_count, audit)
            })
            .collect::<Vec<_>>();

        assert!(audits.iter().any(|(group, _, _)| group == "kernel"));
        assert!(audits.iter().any(|(group, _, _)| group == "scan_style"));
        for (_, expected_rule_count, audit) in audits {
            assert_eq!(audit.rules.len(), expected_rule_count);
            assert_eq!(audit.minimality_audit.total_rule_count, expected_rule_count);
        }
    }

    #[test]
    fn sparse_rule_minimality_audit_detects_dead_rules_in_redundant_scan_style_case() {
        let case = tassadar_sparse_rule_audit_cases()
            .into_iter()
            .find(|case| case.case_id == "scan_style_redundant_memory_tail")
            .expect("redundant scan-style case");
        let audit =
            build_tassadar_sparse_rule_compiler_audit(&case.program, &case.input_assignments)
                .expect("redundant scan-style case should compile");

        assert_eq!(
            audit.minimality_audit.dead_rule_ids,
            vec![String::from("rule_03")]
        );
        assert_eq!(
            audit.minimality_audit.io_only_underconstrained_rule_ids,
            vec![String::from("rule_02")]
        );
    }

    #[test]
    fn sparse_rule_scan_style_compile_size_scales_monotonically() {
        let mut scaling = tassadar_sparse_rule_audit_cases()
            .into_iter()
            .filter(|case| case.workload_group_id == "scan_style")
            .map(|case| {
                let audit = build_tassadar_sparse_rule_compiler_audit(
                    &case.program,
                    &case.input_assignments,
                )
                .expect("scan-style case should compile");
                (case.scaling_step, audit.validated_instruction_count)
            })
            .collect::<Vec<_>>();
        scaling.sort_by_key(|(scaling_step, _)| *scaling_step);

        assert!(scaling.windows(2).all(|window| window[0].1 < window[1].1));
    }
}
