use serde::{Deserialize, Serialize};

use psionic_sandbox::TassadarImportPolicyMatrixReport;

/// Provider-facing receipt for the sandbox-owned import policy matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarImportPolicyMatrixReceipt {
    pub report_id: String,
    pub entry_count: u32,
    pub allowed_internal_case_count: u32,
    pub delegated_case_count: u32,
    pub refused_case_count: u32,
    pub deterministic_stub_entry_count: u32,
    pub delegated_entry_count: u32,
    pub refused_entry_count: u32,
    pub detail: String,
}

impl TassadarImportPolicyMatrixReceipt {
    /// Builds a provider-facing receipt from the committed sandbox report.
    #[must_use]
    pub fn from_report(report: &TassadarImportPolicyMatrixReport) -> Self {
        let deterministic_stub_entry_count = report
            .policy_matrix
            .entries
            .iter()
            .filter(|entry| {
                entry.import_class == psionic_sandbox::TassadarImportClass::DeterministicStub
            })
            .count() as u32;
        let delegated_entry_count = report
            .policy_matrix
            .entries
            .iter()
            .filter(|entry| {
                entry.import_class
                    == psionic_sandbox::TassadarImportClass::ExternalSandboxDelegation
            })
            .count() as u32;
        let refused_entry_count = report
            .policy_matrix
            .entries
            .iter()
            .filter(|entry| {
                entry.import_class == psionic_sandbox::TassadarImportClass::UnsafeSideEffect
            })
            .count() as u32;
        Self {
            report_id: report.report_id.clone(),
            entry_count: report.policy_matrix.entries.len() as u32,
            allowed_internal_case_count: report.allowed_internal_case_count,
            delegated_case_count: report.delegated_case_count,
            refused_case_count: report.refused_case_count,
            deterministic_stub_entry_count,
            delegated_entry_count,
            refused_entry_count,
            detail: format!(
                "import-policy report `{}` exposes {} entries with {} deterministic stubs, {} delegated imports, and {} refused imports across {} internal, {} delegated, and {} refused policy cases",
                report.report_id,
                report.policy_matrix.entries.len(),
                deterministic_stub_entry_count,
                delegated_entry_count,
                refused_entry_count,
                report.allowed_internal_case_count,
                report.delegated_case_count,
                report.refused_case_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarImportPolicyMatrixReceipt;
    use psionic_sandbox::build_tassadar_import_policy_matrix_report;

    #[test]
    fn import_policy_matrix_receipt_projects_sandbox_report() {
        let report = build_tassadar_import_policy_matrix_report();
        let receipt = TassadarImportPolicyMatrixReceipt::from_report(&report);

        assert_eq!(receipt.entry_count, 3);
        assert_eq!(receipt.allowed_internal_case_count, 1);
        assert_eq!(receipt.delegated_case_count, 1);
        assert_eq!(receipt.refused_case_count, 2);
    }
}
