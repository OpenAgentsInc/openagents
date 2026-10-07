use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarInstalledModuleEvidenceBundle;

/// Provider-facing receipt for the installed-module evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInstalledModuleEvidenceReceipt {
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Number of complete records.
    pub complete_record_count: u32,
    /// Number of refused records.
    pub refused_record_count: u32,
    /// Number of revocation-ready records.
    pub revocation_ready_record_count: u32,
    /// Number of records with audit or decompilation artifacts.
    pub audit_receipt_ready_record_count: u32,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarInstalledModuleEvidenceReceipt {
    /// Projects a provider-facing receipt from the shared runtime bundle.
    #[must_use]
    pub fn from_bundle(bundle: &TassadarInstalledModuleEvidenceBundle) -> Self {
        let complete_record_count = bundle
            .records
            .iter()
            .filter(|record| {
                record.status == psionic_runtime::TassadarInstalledModuleEvidenceStatus::Complete
            })
            .count() as u32;
        let audit_receipt_ready_record_count = bundle
            .records
            .iter()
            .filter(|record| !record.audit_artifact_refs.is_empty())
            .count() as u32;
        let revocation_ready_record_count = bundle
            .records
            .iter()
            .filter(|record| !record.revocation_hooks.is_empty())
            .count() as u32;
        Self {
            bundle_id: bundle.bundle_id.clone(),
            complete_record_count,
            refused_record_count: bundle.records.len() as u32 - complete_record_count,
            revocation_ready_record_count,
            audit_receipt_ready_record_count,
            detail: format!(
                "installed-module evidence bundle `{}` currently exposes {} complete records, {} refused records, {} revocation-ready records, and {} audit-ready records",
                bundle.bundle_id,
                complete_record_count,
                bundle.records.len() as u32 - complete_record_count,
                revocation_ready_record_count,
                audit_receipt_ready_record_count,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInstalledModuleEvidenceReceipt;
    use psionic_runtime::build_tassadar_installed_module_evidence_bundle;

    #[test]
    fn installed_module_evidence_receipt_projects_runtime_bundle() {
        let bundle = build_tassadar_installed_module_evidence_bundle().expect("bundle");
        let receipt = TassadarInstalledModuleEvidenceReceipt::from_bundle(&bundle);

        assert_eq!(receipt.complete_record_count, 3);
        assert_eq!(receipt.refused_record_count, 2);
        assert_eq!(receipt.revocation_ready_record_count, 5);
        assert_eq!(receipt.audit_receipt_ready_record_count, 4);
    }
}
