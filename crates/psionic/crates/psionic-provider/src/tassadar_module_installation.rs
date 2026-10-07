use serde::{Deserialize, Serialize};

use psionic_serve::TassadarStagedModuleInstallReceipt;

/// Provider-facing receipt for one bounded staged module install.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallationReceipt {
    /// Stable install identifier.
    pub install_id: String,
    /// Stable module identifier.
    pub module_id: String,
    /// Final install stage.
    pub stage: psionic_serve::TassadarModuleInstallStage,
    /// Policy posture for the install.
    pub policy_posture: psionic_serve::TassadarModuleInstallPolicyPosture,
    /// Whether the receipt still requires or references a challenge ticket.
    pub challenge_ticket_present: bool,
    /// Whether the receipt carries explicit rollback lineage.
    pub rollback_ready: bool,
    /// Count of benchmark refs backing the install.
    pub benchmark_ref_count: u32,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarModuleInstallationReceipt {
    /// Builds a provider-facing receipt from one served install receipt.
    #[must_use]
    pub fn from_served_receipt(receipt: &TassadarStagedModuleInstallReceipt) -> Self {
        Self {
            install_id: receipt.install_id.clone(),
            module_id: receipt.module_id.clone(),
            stage: receipt.stage,
            policy_posture: receipt.policy_posture,
            challenge_ticket_present: receipt.challenge_ticket.is_some(),
            rollback_ready: receipt.rollback_receipt_ref.is_some(),
            benchmark_ref_count: receipt.benchmark_refs.len() as u32,
            detail: receipt.detail.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarModuleInstallationReceipt;
    use psionic_serve::{seeded_tassadar_module_install_receipts, TassadarModuleInstallStage};

    #[test]
    fn module_installation_receipt_projects_served_install_receipts() {
        let receipts = seeded_tassadar_module_install_receipts();
        let rollback = receipts
            .iter()
            .find(|receipt| receipt.stage == TassadarModuleInstallStage::RolledBack)
            .expect("rollback receipt");
        let projected = TassadarModuleInstallationReceipt::from_served_receipt(rollback);

        assert_eq!(projected.module_id, "candidate_select_core");
        assert!(projected.challenge_ticket_present);
        assert!(projected.rollback_ready);
        assert!(projected.benchmark_ref_count >= 1);
    }
}
