use serde::{Deserialize, Serialize};

use crate::build_tassadar_internal_module_library_publication;

/// Dedicated served product identifier for the staged module-install surface.
pub const EXECUTOR_MODULE_INSTALL_PRODUCT_ID: &str = "psionic.executor_module_install";

/// Explicit bounded install scope for one module update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallScope {
    /// Mount the module into one bounded session only.
    SessionMount,
    /// Mount the module on one trusted worker after challenge gating.
    WorkerMount,
}

/// Install stage for one bounded module update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallStage {
    Staged,
    ChallengeWindow,
    Activated,
    RolledBack,
    Refused,
}

/// Policy posture for one staged module install.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleInstallPolicyPosture {
    TrustedModuleClass,
    ChallengeRequired,
    Refused,
}

/// Served publication for the bounded module-install surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleInstallationPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Backing internal module library identifier.
    pub library_id: String,
    /// Supported install scopes.
    pub supported_scopes: Vec<TassadarModuleInstallScope>,
    /// Trusted module classes admitted by the current surface.
    pub trusted_module_classes: Vec<String>,
    /// Stable benchmark refs gating the surface.
    pub benchmark_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// One served staged-install or update receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStagedModuleInstallReceipt {
    /// Stable install identifier.
    pub install_id: String,
    /// Backing internal module library identifier.
    pub library_id: String,
    /// Stable module identifier.
    pub module_id: String,
    /// Requested version.
    pub requested_version: String,
    /// Resolved version after policy and rollback decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
    /// Install scope.
    pub scope: TassadarModuleInstallScope,
    /// Final stage recorded for the bounded install.
    pub stage: TassadarModuleInstallStage,
    /// Policy posture for the install.
    pub policy_posture: TassadarModuleInstallPolicyPosture,
    /// Challenge ticket when a challenge window is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_ticket: Option<String>,
    /// Rollback receipt ref when the update was rolled back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_receipt_ref: Option<String>,
    /// Stable benchmark refs anchoring the install.
    pub benchmark_refs: Vec<String>,
    /// Plain-language receipt detail.
    pub detail: String,
}

/// Returns the benchmark-gated served module-install surface.
#[must_use]
pub fn tassadar_module_installation_publication() -> TassadarModuleInstallationPublication {
    let library = build_tassadar_internal_module_library_publication().expect("module library");
    TassadarModuleInstallationPublication {
        product_id: String::from(EXECUTOR_MODULE_INSTALL_PRODUCT_ID),
        library_id: library.library_id,
        supported_scopes: vec![
            TassadarModuleInstallScope::SessionMount,
            TassadarModuleInstallScope::WorkerMount,
        ],
        trusted_module_classes: vec![
            String::from("frontier_relax_core"),
            String::from("candidate_select_core"),
            String::from("checkpoint_backtrack_core"),
        ],
        benchmark_refs: library.benchmark_refs,
        claim_boundary: String::from(
            "this install surface is benchmark-gated by the bounded internal module library and keeps session mounts, worker mounts, challenge windows, and rollback posture explicit. It does not claim unrestricted self-modification or autonomous self-installation",
        ),
    }
}

/// Returns seeded served install receipts for the bounded install lane.
#[must_use]
pub fn seeded_tassadar_module_install_receipts() -> Vec<TassadarStagedModuleInstallReceipt> {
    let publication = tassadar_module_installation_publication();
    vec![
        TassadarStagedModuleInstallReceipt {
            install_id: String::from("install.frontier_relax_core.session.v1"),
            library_id: publication.library_id.clone(),
            module_id: String::from("frontier_relax_core"),
            requested_version: String::from("1.0.0"),
            resolved_version: Some(String::from("1.0.0")),
            scope: TassadarModuleInstallScope::SessionMount,
            stage: TassadarModuleInstallStage::Activated,
            policy_posture: TassadarModuleInstallPolicyPosture::TrustedModuleClass,
            challenge_ticket: None,
            rollback_receipt_ref: None,
            benchmark_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
            detail: String::from(
                "frontier_relax_core was mounted into one bounded session after direct compatibility and benchmark checks passed",
            ),
        },
        TassadarStagedModuleInstallReceipt {
            install_id: String::from("install.candidate_select_core.challenge.v1"),
            library_id: publication.library_id.clone(),
            module_id: String::from("candidate_select_core"),
            requested_version: String::from("1.2.0"),
            resolved_version: None,
            scope: TassadarModuleInstallScope::WorkerMount,
            stage: TassadarModuleInstallStage::ChallengeWindow,
            policy_posture: TassadarModuleInstallPolicyPosture::ChallengeRequired,
            challenge_ticket: Some(String::from("challenge.candidate_select_core.1_2_0")),
            rollback_receipt_ref: None,
            benchmark_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
            detail: String::from(
                "candidate_select_core@1.2.0 entered a bounded worker-mount challenge window before activation because rollback posture remains explicit",
            ),
        },
        TassadarStagedModuleInstallReceipt {
            install_id: String::from("install.candidate_select_core.rollback.v1"),
            library_id: publication.library_id.clone(),
            module_id: String::from("candidate_select_core"),
            requested_version: String::from("1.2.0"),
            resolved_version: Some(String::from("1.1.0")),
            scope: TassadarModuleInstallScope::WorkerMount,
            stage: TassadarModuleInstallStage::RolledBack,
            policy_posture: TassadarModuleInstallPolicyPosture::ChallengeRequired,
            challenge_ticket: Some(String::from("challenge.candidate_select_core.1_2_0")),
            rollback_receipt_ref: Some(String::from(
                "tassadar://receipt/module_install/candidate_select_core/rollback/1.1.0",
            )),
            benchmark_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
            detail: String::from(
                "candidate_select_core@1.2.0 failed the challenge witness and rolled back to 1.1.0 under the published rollback plan",
            ),
        },
        TassadarStagedModuleInstallReceipt {
            install_id: String::from("install.branch_prune_core.refused.v1"),
            library_id: publication.library_id,
            module_id: String::from("branch_prune_core"),
            requested_version: String::from("0.1.0"),
            resolved_version: None,
            scope: TassadarModuleInstallScope::SessionMount,
            stage: TassadarModuleInstallStage::Refused,
            policy_posture: TassadarModuleInstallPolicyPosture::Refused,
            challenge_ticket: None,
            rollback_receipt_ref: None,
            benchmark_refs: Vec::new(),
            detail: String::from(
                "branch_prune_core was refused because it is not a trusted module class with benchmark lineage inside the current bounded install surface",
            ),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        seeded_tassadar_module_install_receipts, tassadar_module_installation_publication,
        EXECUTOR_MODULE_INSTALL_PRODUCT_ID, TassadarModuleInstallStage,
    };

    #[test]
    fn module_installation_publication_is_machine_legible() {
        let publication = tassadar_module_installation_publication();

        assert_eq!(publication.product_id, EXECUTOR_MODULE_INSTALL_PRODUCT_ID);
        assert!(publication
            .trusted_module_classes
            .contains(&String::from("candidate_select_core")));
        assert_eq!(publication.supported_scopes.len(), 2);
        assert!(!publication.benchmark_refs.is_empty());
    }

    #[test]
    fn seeded_module_install_receipts_keep_challenge_and_rollback_explicit() {
        let receipts = seeded_tassadar_module_install_receipts();

        assert!(receipts.iter().any(|receipt| {
            receipt.stage == TassadarModuleInstallStage::ChallengeWindow
                && receipt.challenge_ticket.is_some()
        }));
        assert!(receipts.iter().any(|receipt| {
            receipt.stage == TassadarModuleInstallStage::RolledBack
                && receipt.rollback_receipt_ref.is_some()
        }));
        assert!(receipts.iter().any(|receipt| {
            receipt.stage == TassadarModuleInstallStage::Refused
                && receipt.benchmark_refs.is_empty()
        }));
    }
}
