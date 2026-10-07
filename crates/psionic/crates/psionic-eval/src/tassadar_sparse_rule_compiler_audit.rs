use std::collections::BTreeMap;

use psionic_compiler::{
    build_tassadar_sparse_rule_compiler_audit, TassadarSparseRuleCompilerAuditError,
};
use psionic_ir::tassadar_sparse_rule_audit_cases;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One eval-facing sparse-rule compiler-audit case summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleCompilerAuditCaseReport {
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
    /// Plain-language claim boundary for the case family.
    pub claim_boundary: String,
    /// Monotone size step inside the current workload group.
    pub scaling_step: u32,
    /// Stable symbolic program identifier.
    pub symbolic_program_id: String,
    /// Stable symbolic program digest.
    pub symbolic_program_digest: String,
    /// Sparse-rule count for the case.
    pub rule_count: usize,
    /// Validated runtime instruction count for the lowered artifact.
    pub validated_instruction_count: usize,
    /// Ordered lowering-opcode requirements for the case.
    pub required_lowering_opcodes: Vec<String>,
    /// Dead sparse-rule count.
    pub dead_rule_count: usize,
    /// Final-state-but-not-IO sparse-rule count.
    pub io_only_underconstrained_rule_count: usize,
    /// Full-trace sparse-rule count.
    pub full_trace_rule_count: usize,
    /// Final-state-required sparse-rule count.
    pub final_state_required_rule_count: usize,
    /// IO-only-required sparse-rule count.
    pub io_only_required_rule_count: usize,
    /// Duplicate-signature cluster count.
    pub duplicate_signature_group_count: usize,
    /// Stable digest of the compiler audit backing the case report.
    pub audit_digest: String,
}

/// One workload-group summary over sparse-rule compiler audits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleCompilerAuditGroupReport {
    /// Stable workload-group identifier.
    pub workload_group_id: String,
    /// Total case count inside the group.
    pub total_case_count: u32,
    /// Minimum sparse-rule count inside the group.
    pub min_rule_count: usize,
    /// Maximum sparse-rule count inside the group.
    pub max_rule_count: usize,
    /// Whether validated instruction count grows monotonically by `scaling_step`.
    pub monotonic_compile_size_scaling: bool,
    /// Number of cases in the group with IO-only underconstraint.
    pub io_only_underconstrained_case_count: u32,
    /// Short operator-facing detail summary.
    pub detail: String,
}

/// Sparse-rule eval summary failure.
#[derive(Debug, Error)]
pub enum TassadarSparseRuleCompilerAuditEvalError {
    /// Compiler-side sparse-rule audit construction failed.
    #[error(transparent)]
    Compiler(#[from] TassadarSparseRuleCompilerAuditError),
}

/// Builds all eval-facing sparse-rule compiler-audit case reports.
pub fn build_tassadar_sparse_rule_compiler_audit_case_reports(
) -> Result<Vec<TassadarSparseRuleCompilerAuditCaseReport>, TassadarSparseRuleCompilerAuditEvalError>
{
    let mut reports = Vec::new();
    for case in tassadar_sparse_rule_audit_cases() {
        let audit =
            build_tassadar_sparse_rule_compiler_audit(&case.program, &case.input_assignments)?;
        reports.push(TassadarSparseRuleCompilerAuditCaseReport {
            case_id: case.case_id,
            workload_group_id: case.workload_group_id,
            workload_family_id: case.workload_family_id,
            summary: case.summary,
            claim_class: case.claim_class,
            claim_boundary: case.claim_boundary,
            scaling_step: case.scaling_step,
            symbolic_program_id: audit.symbolic_program_id,
            symbolic_program_digest: audit.symbolic_program_digest,
            rule_count: audit.rules.len(),
            validated_instruction_count: audit.validated_instruction_count,
            required_lowering_opcodes: audit
                .required_lowering_opcodes
                .iter()
                .map(|opcode| opcode.mnemonic().to_string())
                .collect(),
            dead_rule_count: audit.minimality_audit.dead_rule_ids.len(),
            io_only_underconstrained_rule_count: audit
                .minimality_audit
                .io_only_underconstrained_rule_ids
                .len(),
            full_trace_rule_count: audit.minimality_audit.full_trace_rule_ids.len(),
            final_state_required_rule_count: audit
                .minimality_audit
                .final_state_required_rule_ids
                .len(),
            io_only_required_rule_count: audit.minimality_audit.io_only_required_rule_ids.len(),
            duplicate_signature_group_count: audit
                .minimality_audit
                .duplicate_signature_rule_groups
                .len(),
            audit_digest: audit.audit_digest,
        });
    }
    Ok(reports)
}

/// Builds grouped sparse-rule compiler-audit summaries from case reports.
#[must_use]
pub fn build_tassadar_sparse_rule_compiler_audit_group_reports(
    case_reports: &[TassadarSparseRuleCompilerAuditCaseReport],
) -> Vec<TassadarSparseRuleCompilerAuditGroupReport> {
    let grouped = case_reports.iter().fold(
        BTreeMap::<String, Vec<&TassadarSparseRuleCompilerAuditCaseReport>>::new(),
        |mut grouped, report| {
            grouped
                .entry(report.workload_group_id.clone())
                .or_default()
                .push(report);
            grouped
        },
    );

    grouped
        .into_iter()
        .map(|(workload_group_id, mut reports)| {
            reports.sort_by(|left, right| {
                left.scaling_step
                    .cmp(&right.scaling_step)
                    .then_with(|| left.case_id.cmp(&right.case_id))
            });
            let rule_counts = reports.iter().map(|report| report.rule_count).collect::<Vec<_>>();
            let monotonic_compile_size_scaling = reports.windows(2).all(|window| {
                window[0].validated_instruction_count <= window[1].validated_instruction_count
            });
            let io_only_underconstrained_case_count = reports
                .iter()
                .filter(|report| report.io_only_underconstrained_rule_count > 0)
                .count() as u32;
            TassadarSparseRuleCompilerAuditGroupReport {
                workload_group_id: workload_group_id.clone(),
                total_case_count: reports.len() as u32,
                min_rule_count: rule_counts.iter().copied().min().unwrap_or_default(),
                max_rule_count: rule_counts.iter().copied().max().unwrap_or_default(),
                monotonic_compile_size_scaling,
                io_only_underconstrained_case_count,
                detail: format!(
                    "{workload_group_id} spans {} cases, {}..{} sparse rules, monotonic_compile_size_scaling={}, io_only_underconstrained_cases={}",
                    reports.len(),
                    rule_counts.iter().copied().min().unwrap_or_default(),
                    rule_counts.iter().copied().max().unwrap_or_default(),
                    monotonic_compile_size_scaling,
                    io_only_underconstrained_case_count,
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        build_tassadar_sparse_rule_compiler_audit_case_reports,
        build_tassadar_sparse_rule_compiler_audit_group_reports,
    };

    #[test]
    fn sparse_rule_compiler_audit_groups_cover_kernel_and_scan_style() {
        let case_reports = build_tassadar_sparse_rule_compiler_audit_case_reports()
            .expect("seeded sparse-rule case reports should build");
        let group_reports =
            build_tassadar_sparse_rule_compiler_audit_group_reports(case_reports.as_slice());
        let groups = group_reports
            .iter()
            .map(|report| report.workload_group_id.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            groups,
            BTreeSet::from([String::from("kernel"), String::from("scan_style")])
        );
    }

    #[test]
    fn sparse_rule_scan_style_group_scales_monotonically() {
        let case_reports = build_tassadar_sparse_rule_compiler_audit_case_reports()
            .expect("seeded sparse-rule case reports should build");
        let group_report =
            build_tassadar_sparse_rule_compiler_audit_group_reports(case_reports.as_slice())
                .into_iter()
                .find(|report| report.workload_group_id == "scan_style")
                .expect("scan_style group report");

        assert!(group_report.monotonic_compile_size_scaling);
    }

    #[test]
    fn sparse_rule_redundant_case_surfaces_dead_and_io_only_gaps() {
        let case_report = build_tassadar_sparse_rule_compiler_audit_case_reports()
            .expect("seeded sparse-rule case reports should build")
            .into_iter()
            .find(|report| report.case_id == "scan_style_redundant_memory_tail")
            .expect("redundant scan-style case report");

        assert!(case_report.dead_rule_count > 0);
        assert!(case_report.io_only_underconstrained_rule_count > 0);
    }
}
