use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    tassadar_symbolic_program_examples, TassadarSymbolicProgram, TassadarSymbolicProgramExample,
    TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS,
};

/// Stable schema version for sparse-rule compiler audits over the bounded
/// symbolic lane.
pub const TASSADAR_SPARSE_RULE_AUDIT_SCHEMA_VERSION: u16 = 1;
/// Coarse claim class for sparse-rule audits over the bounded symbolic lane.
pub const TASSADAR_SPARSE_RULE_AUDIT_CLAIM_CLASS: &str = TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS;

/// One dependency-source family in the sparse-rule audit view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSparseRuleSourceKind {
    /// One declared program input.
    Input,
    /// One prior symbolic binding.
    Binding,
    /// One direct memory-slot read.
    MemorySlot,
    /// One inline constant payload.
    Const,
}

impl TassadarSparseRuleSourceKind {
    /// Returns the stable source-kind label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Binding => "binding",
            Self::MemorySlot => "memory_slot",
            Self::Const => "const",
        }
    }
}

/// One sparse dependency edge for one symbolic statement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleSourceRef {
    /// Stable source family.
    pub source_kind: TassadarSparseRuleSourceKind,
    /// Stable source identifier within the current family.
    pub reference: String,
}

/// One target family in the sparse-rule audit view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSparseRuleTargetKind {
    /// One symbolic binding.
    Binding,
    /// One runtime memory slot.
    MemorySlot,
    /// One executor output.
    Output,
}

impl TassadarSparseRuleTargetKind {
    /// Returns the stable target-kind label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Binding => "binding",
            Self::MemorySlot => "memory_slot",
            Self::Output => "output",
        }
    }
}

/// One statement-projected sparse transition rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseTransitionRule {
    /// Stable rule identifier within the current program.
    pub rule_id: String,
    /// Zero-based statement index from the symbolic source program.
    pub statement_index: usize,
    /// Stable operation label for the statement family.
    pub operation: String,
    /// Target family written by the statement.
    pub target_kind: TassadarSparseRuleTargetKind,
    /// Stable target identifier inside the target family.
    pub target_label: String,
    /// Ordered sparse dependencies consumed by the statement.
    pub source_refs: Vec<TassadarSparseRuleSourceRef>,
    /// Stable label-neutral signature digest for duplicate-rule clustering.
    pub signature_digest: String,
}

impl TassadarSparseTransitionRule {
    /// Creates one sparse transition rule with a stable label-neutral signature.
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        statement_index: usize,
        operation: impl Into<String>,
        target_kind: TassadarSparseRuleTargetKind,
        target_label: impl Into<String>,
        source_refs: Vec<TassadarSparseRuleSourceRef>,
    ) -> Self {
        let operation = operation.into();
        let target_label = target_label.into();
        let signature_digest =
            stable_signature_digest(operation.as_str(), target_kind, source_refs.as_slice());
        Self {
            rule_id: rule_id.into(),
            statement_index,
            operation,
            target_kind,
            target_label,
            source_refs,
            signature_digest,
        }
    }
}

/// One supervision posture used by the sparse-rule audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSparseRuleSupervisionMode {
    /// Every statement must remain visible.
    FullTrace,
    /// Only final outputs plus final memory state remain visible.
    FinalState,
    /// Only final outputs remain visible.
    IoOnly,
}

/// One seeded audit case over the bounded symbolic compiler lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleAuditCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable workload-group identifier.
    pub workload_group_id: String,
    /// Stable workload-family identifier.
    pub workload_family_id: String,
    /// Plain-language case summary.
    pub summary: String,
    /// Coarse claim class for the case family.
    pub claim_class: String,
    /// Boundary statement for the case family.
    pub claim_boundary: String,
    /// Monotone size step inside the current workload group.
    pub scaling_step: u32,
    /// Symbolic program audited by this case.
    pub program: TassadarSymbolicProgram,
    /// Concrete input assignments for the case.
    pub input_assignments: BTreeMap<String, i32>,
}

impl TassadarSparseRuleAuditCase {
    fn from_symbolic_example(
        example: TassadarSymbolicProgramExample,
        workload_family_id: impl Into<String>,
        scaling_step: u32,
    ) -> Self {
        Self {
            case_id: example.case_id,
            workload_group_id: String::from("kernel"),
            workload_family_id: workload_family_id.into(),
            summary: example.summary,
            claim_class: example.claim_class,
            claim_boundary: example.claim_boundary,
            scaling_step,
            program: example.program,
            input_assignments: example.input_assignments,
        }
    }

    fn new_scan_style_case(
        case_id: impl Into<String>,
        summary: impl Into<String>,
        scaling_step: u32,
        source: &str,
        input_assignments: BTreeMap<String, i32>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let program = TassadarSymbolicProgram::parse(source)
            .expect("seeded scan-style audit case should parse");
        Self {
            case_id: case_id.into(),
            workload_group_id: String::from("scan_style"),
            workload_family_id: String::from("scan_style_composition"),
            summary: summary.into(),
            claim_class: String::from(TASSADAR_SPARSE_RULE_AUDIT_CLAIM_CLASS),
            claim_boundary: claim_boundary.into(),
            scaling_step,
            program,
            input_assignments,
        }
    }
}

/// Returns the seeded sparse-rule audit cases for the bounded symbolic lane.
#[must_use]
pub fn tassadar_sparse_rule_audit_cases() -> Vec<TassadarSparseRuleAuditCase> {
    let kernel_cases = tassadar_symbolic_program_examples()
        .into_iter()
        .map(|example| {
            let scaling_step = example.program.statements.len() as u32;
            let workload_family_id = kernel_workload_family_id(example.case_id.as_str());
            TassadarSparseRuleAuditCase::from_symbolic_example(
                example,
                workload_family_id,
                scaling_step,
            )
        })
        .collect::<Vec<_>>();
    let mut cases = kernel_cases;
    cases.extend([
        TassadarSparseRuleAuditCase::new_scan_style_case(
            "scan_style_single_step",
            "single primitive command compilation over one bounded displacement",
            1,
            SCAN_STYLE_SINGLE_STEP_PROGRAM,
            scan_input_assignments(&[("start", 3), ("delta", 1)]),
            "this sparse-rule audit case covers one fixed-width compositional command primitive in the bounded symbolic lane only; it does not imply broad SCAN, seq2seq, or learned compositional closure",
        ),
        TassadarSparseRuleAuditCase::new_scan_style_case(
            "scan_style_two_step_composition",
            "two-step compositional command chain with one intermediate symbolic binding",
            2,
            SCAN_STYLE_TWO_STEP_PROGRAM,
            scan_input_assignments(&[("start", 3), ("delta", 1), ("boost", 2)]),
            "this sparse-rule audit case covers one two-step compositional command chain in the bounded symbolic lane only; it does not imply broad language-command grounding or arbitrary sequence generalization",
        ),
        TassadarSparseRuleAuditCase::new_scan_style_case(
            "scan_style_redundant_memory_tail",
            "composed command chain with one final-state store and one intentionally dead symbolic rule",
            3,
            SCAN_STYLE_REDUNDANT_MEMORY_PROGRAM,
            scan_input_assignments(&[("start", 3), ("delta", 1), ("boost", 2)]),
            "this sparse-rule audit case is designed to separate final-state-required sparse rules from dead or IO-only-underconstrained rules in the bounded symbolic lane; it is an audit harness, not a broad compositionality claim",
        ),
    ]);
    cases
}

fn kernel_workload_family_id(case_id: &str) -> String {
    match case_id {
        "addition_pair" => String::from("addition"),
        "parity_two_bits" => String::from("parity"),
        "memory_accumulator" => String::from("memory"),
        "finite_state_counter" => String::from("finite_state"),
        "stack_machine_add_step" => String::from("simple_stack_machine"),
        _ => String::from("bounded_symbolic"),
    }
}

fn scan_input_assignments(pairs: &[(&str, i32)]) -> BTreeMap<String, i32> {
    pairs
        .iter()
        .map(|(name, value)| (String::from(*name), *value))
        .collect()
}

fn stable_signature_digest(
    operation: &str,
    target_kind: TassadarSparseRuleTargetKind,
    source_refs: &[TassadarSparseRuleSourceRef],
) -> String {
    let signature_lines = source_refs
        .iter()
        .map(|source_ref| match source_ref.source_kind {
            TassadarSparseRuleSourceKind::Const => {
                format!(
                    "{}:{}",
                    source_ref.source_kind.as_str(),
                    source_ref.reference
                )
            }
            _ => source_ref.source_kind.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    stable_digest(
        b"tassadar_sparse_transition_rule_signature|",
        &(
            String::from(operation),
            String::from(target_kind.as_str()),
            signature_lines,
        ),
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

const SCAN_STYLE_SINGLE_STEP_PROGRAM: &str = r#"
program tassadar.symbolic.scan_style_single_step.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input start = slot(0)
input delta = slot(1)
let step_1 = add(start, delta)
output step_1
"#;

const SCAN_STYLE_TWO_STEP_PROGRAM: &str = r#"
program tassadar.symbolic.scan_style_two_step.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input start = slot(0)
input delta = slot(1)
input boost = slot(2)
let step_1 = add(start, delta)
let step_2 = add(step_1, boost)
output step_2
"#;

const SCAN_STYLE_REDUNDANT_MEMORY_PROGRAM: &str = r#"
program tassadar.symbolic.scan_style_redundant_memory_tail.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 4
input start = slot(0)
input delta = slot(1)
input boost = slot(2)
init slot(3) = 0
let step_1 = add(start, delta)
let step_2 = add(step_1, boost)
store slot(3) = step_2
let dead_probe = add(step_2, const(99))
output step_2
"#;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::tassadar_sparse_rule_audit_cases;

    #[test]
    fn sparse_rule_audit_cases_cover_kernel_and_scan_style_groups() {
        let cases = tassadar_sparse_rule_audit_cases();
        let groups = cases
            .iter()
            .map(|case| case.workload_group_id.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            groups,
            BTreeSet::from([String::from("kernel"), String::from("scan_style")])
        );
        assert!(cases.iter().all(|case| case.program.validate().is_ok()));
    }

    #[test]
    fn sparse_rule_scan_style_cases_scale_by_statement_count() {
        let mut scaling = tassadar_sparse_rule_audit_cases()
            .into_iter()
            .filter(|case| case.workload_group_id == "scan_style")
            .map(|case| (case.scaling_step, case.program.statements.len()))
            .collect::<Vec<_>>();
        scaling.sort_by_key(|(scaling_step, _)| *scaling_step);

        assert_eq!(scaling, vec![(1, 2), (2, 3), (3, 5)]);
    }
}
