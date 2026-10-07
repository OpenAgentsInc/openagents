use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterContributionArtifactDisposition, AdapterContributionSecurityDisposition,
    DecentralizedAdapterReferenceFamily, DecentralizedAdapterReferenceProgramReport,
    PsionPhaseGate,
};

/// Stable schema version for the first Psion decentralized-contribution bundle.
pub const PSION_DECENTRALIZED_CONTRIBUTION_BUNDLE_SCHEMA_VERSION: &str =
    "psion.decentralized_contribution_bundle.v1";
/// Stable rollback schema version required by contributed outputs before serving publication.
pub const PSION_CAPABILITY_WITHDRAWAL_SCHEMA_VERSION: &str =
    "psion.capability_withdrawal_receipt.v1";

/// First bounded decentralized-contribution mode admitted into the Psion lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionDecentralizedContributionMode {
    /// One adapter-delta window above the bounded cluster-backed control plane.
    AdapterDeltaWindow,
}

/// Generic artifact reference carried by the Psion decentralized-contribution bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContributionArtifactReference {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl PsionContributionArtifactReference {
    fn validate(&self, field: &str) -> Result<(), PsionDecentralizedContributionError> {
        ensure_nonempty(
            self.artifact_id.as_str(),
            format!("{field}.artifact_id").as_str(),
        )?;
        ensure_nonempty(
            self.artifact_digest.as_str(),
            format!("{field}.artifact_digest").as_str(),
        )?;
        Ok(())
    }
}

/// Acceptance-matrix discipline that contributed outputs must still satisfy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContributionAcceptanceBinding {
    /// Stable acceptance-matrix identifier.
    pub acceptance_matrix_id: String,
    /// Stable acceptance-matrix version.
    pub acceptance_matrix_version: String,
    /// Current promoted Psion decision that contributed outputs build on top of.
    pub current_promoted_decision_ref: String,
    /// Current promoted phase carried by that decision.
    pub current_promoted_phase: PsionPhaseGate,
    /// Phase gate contributed outputs must still clear before serving or publication.
    pub required_publication_phase: PsionPhaseGate,
    /// Plain-language note.
    pub detail: String,
}

impl PsionContributionAcceptanceBinding {
    fn validate(&self) -> Result<(), PsionDecentralizedContributionError> {
        ensure_nonempty(
            self.acceptance_matrix_id.as_str(),
            "psion_decentralized_contribution.acceptance_binding.acceptance_matrix_id",
        )?;
        ensure_nonempty(
            self.acceptance_matrix_version.as_str(),
            "psion_decentralized_contribution.acceptance_binding.acceptance_matrix_version",
        )?;
        ensure_nonempty(
            self.current_promoted_decision_ref.as_str(),
            "psion_decentralized_contribution.acceptance_binding.current_promoted_decision_ref",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_decentralized_contribution.acceptance_binding.detail",
        )?;
        if self.required_publication_phase == PsionPhaseGate::Pilot {
            return Err(
                PsionDecentralizedContributionError::InvalidAcceptanceBinding {
                    detail: String::from(
                        "contributed outputs may not claim pilot-only publication as the final gate",
                    ),
                },
            );
        }
        Ok(())
    }
}

/// Capability and rollback discipline that contributed outputs must still satisfy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContributionCapabilityBinding {
    /// Stable capability-matrix identifier.
    pub capability_matrix_id: String,
    /// Stable capability-matrix version.
    pub capability_matrix_version: String,
    /// Required rollback schema version for contributed outputs.
    pub rollback_receipt_schema_version: String,
    /// One reference rollback receipt proving the lane does not bypass downgrade discipline.
    pub rollback_reference_receipt: PsionContributionArtifactReference,
    /// Plain-language note.
    pub detail: String,
}

impl PsionContributionCapabilityBinding {
    fn validate(&self) -> Result<(), PsionDecentralizedContributionError> {
        ensure_nonempty(
            self.capability_matrix_id.as_str(),
            "psion_decentralized_contribution.capability_binding.capability_matrix_id",
        )?;
        ensure_nonempty(
            self.capability_matrix_version.as_str(),
            "psion_decentralized_contribution.capability_binding.capability_matrix_version",
        )?;
        ensure_nonempty(
            self.rollback_receipt_schema_version.as_str(),
            "psion_decentralized_contribution.capability_binding.rollback_receipt_schema_version",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_decentralized_contribution.capability_binding.detail",
        )?;
        if self.rollback_receipt_schema_version != PSION_CAPABILITY_WITHDRAWAL_SCHEMA_VERSION {
            return Err(
                PsionDecentralizedContributionError::InvalidCapabilityBinding {
                    detail: format!(
                        "expected rollback schema `{PSION_CAPABILITY_WITHDRAWAL_SCHEMA_VERSION}`, found `{}`",
                        self.rollback_receipt_schema_version
                    ),
                },
            );
        }
        self.rollback_reference_receipt.validate(
            "psion_decentralized_contribution.capability_binding.rollback_reference_receipt",
        )?;
        Ok(())
    }
}

/// Contributor-level artifact and security summary preserved for one bounded window lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionContributorReceiptSummary {
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable staged artifact identifier.
    pub artifact_id: String,
    /// Stable artifact receipt digest.
    pub artifact_receipt_digest: String,
    /// Stable security receipt identifier.
    pub security_receipt_id: String,
    /// Stable security receipt digest.
    pub security_receipt_digest: String,
    /// Current artifact retention disposition.
    pub artifact_disposition: AdapterContributionArtifactDisposition,
    /// Final provenance-security disposition.
    pub security_disposition: AdapterContributionSecurityDisposition,
    /// Whether the contribution actually entered one sealed-window aggregation.
    pub accepted_for_window_aggregation: bool,
    /// Whether the contribution's window carried replay-checked work.
    pub window_replay_checked: bool,
    /// Plain-language note.
    pub detail: String,
}

impl PsionContributorReceiptSummary {
    fn validate(&self) -> Result<(), PsionDecentralizedContributionError> {
        ensure_nonempty(
            self.contribution_id.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].contribution_id",
        )?;
        ensure_nonempty(
            self.window_id.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].window_id",
        )?;
        ensure_nonempty(
            self.worker_id.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].worker_id",
        )?;
        ensure_nonempty(
            self.artifact_id.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].artifact_id",
        )?;
        ensure_nonempty(
            self.artifact_receipt_digest.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].artifact_receipt_digest",
        )?;
        ensure_nonempty(
            self.security_receipt_id.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].security_receipt_id",
        )?;
        ensure_nonempty(
            self.security_receipt_digest.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].security_receipt_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_decentralized_contribution.contributor_receipts[].detail",
        )?;
        Ok(())
    }
}

/// First bounded decentralized-contribution bundle bound back to the Psion lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionDecentralizedContributionBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Bounded contribution mode.
    pub contribution_mode: PsionDecentralizedContributionMode,
    /// Trusted-cluster run artifact this contribution lane sits above.
    pub trusted_cluster_run_artifact: PsionContributionArtifactReference,
    /// Reasoning-SFT run artifact this contribution lane sits above.
    pub reasoning_sft_run_artifact: PsionContributionArtifactReference,
    /// Main-lane acceptance discipline that contributed outputs must still satisfy.
    pub acceptance_binding: PsionContributionAcceptanceBinding,
    /// Main-lane capability and rollback discipline that contributed outputs must still satisfy.
    pub capability_binding: PsionContributionCapabilityBinding,
    /// Reused bounded adapter-window reference program proving contributor and window artifacts.
    pub reference_program: DecentralizedAdapterReferenceProgramReport,
    /// Contributor-level artifact and security summaries.
    pub contributor_receipts: Vec<PsionContributorReceiptSummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionDecentralizedContributionBundle {
    /// Validates the bundle shape and the bounded contribution posture.
    pub fn validate(&self) -> Result<(), PsionDecentralizedContributionError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_decentralized_contribution.schema_version",
        )?;
        if self.schema_version != PSION_DECENTRALIZED_CONTRIBUTION_BUNDLE_SCHEMA_VERSION {
            return Err(PsionDecentralizedContributionError::SchemaVersionMismatch {
                expected: String::from(PSION_DECENTRALIZED_CONTRIBUTION_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.lane_id.as_str(),
            "psion_decentralized_contribution.lane_id",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_decentralized_contribution.claim_boundary",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_decentralized_contribution.summary",
        )?;
        self.trusted_cluster_run_artifact
            .validate("psion_decentralized_contribution.trusted_cluster_run_artifact")?;
        self.reasoning_sft_run_artifact
            .validate("psion_decentralized_contribution.reasoning_sft_run_artifact")?;
        self.acceptance_binding.validate()?;
        self.capability_binding.validate()?;
        self.validate_reference_program()?;
        self.validate_contributor_receipts()?;
        if self.bundle_digest != stable_psion_decentralized_contribution_digest(self) {
            return Err(PsionDecentralizedContributionError::DigestMismatch);
        }
        Ok(())
    }

    fn validate_reference_program(&self) -> Result<(), PsionDecentralizedContributionError> {
        if self
            .reference_program
            .operator_view
            .accepted_contribution_count
            == 0
        {
            return Err(
                PsionDecentralizedContributionError::InvalidReferenceProgram {
                    detail: String::from(
                        "reference program must carry at least one accepted contribution",
                    ),
                },
            );
        }
        if self
            .reference_program
            .operator_view
            .replay_checked_contribution_count
            == 0
        {
            return Err(
                PsionDecentralizedContributionError::InvalidReferenceProgram {
                    detail: String::from(
                        "reference program must carry at least one replay-checked contribution",
                    ),
                },
            );
        }
        if self
            .reference_program
            .operator_view
            .promoted_policy_revision_ids
            .is_empty()
        {
            return Err(
                PsionDecentralizedContributionError::InvalidReferenceProgram {
                    detail: String::from(
                        "reference program must promote at least one bounded policy revision",
                    ),
                },
            );
        }
        if !matches!(
            self.reference_program.spec.family,
            DecentralizedAdapterReferenceFamily::OpenGptOssLmHead
                | DecentralizedAdapterReferenceFamily::AppleFoundationModels
        ) {
            return Err(
                PsionDecentralizedContributionError::InvalidReferenceProgram {
                    detail: String::from("reference program used an unsupported family"),
                },
            );
        }
        if self.reference_program.first_promotion.promotion_disposition
            != crate::AdapterPolicyPromotionDisposition::Promoted
            && self
                .reference_program
                .second_promotion
                .promotion_disposition
                != crate::AdapterPolicyPromotionDisposition::Promoted
        {
            return Err(
                PsionDecentralizedContributionError::InvalidReferenceProgram {
                    detail: String::from(
                        "reference program must promote at least one sealed bounded window",
                    ),
                },
            );
        }
        Ok(())
    }

    fn validate_contributor_receipts(&self) -> Result<(), PsionDecentralizedContributionError> {
        if self.contributor_receipts.is_empty() {
            return Err(PsionDecentralizedContributionError::MissingField {
                field: String::from("psion_decentralized_contribution.contributor_receipts"),
            });
        }
        if self.contributor_receipts.len() != self.reference_program.artifact_receipts.len()
            || self.contributor_receipts.len() != self.reference_program.security_receipts.len()
        {
            return Err(PsionDecentralizedContributionError::ContributorMismatch {
                detail: String::from(
                    "contributor receipt summaries must cover every artifact and security receipt in the reference program",
                ),
            });
        }
        let security_by_contribution = self
            .reference_program
            .security_receipts
            .iter()
            .map(|receipt| (receipt.contribution_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let accepted_contributions = self
            .reference_program
            .first_promotion
            .accepted_contributions
            .iter()
            .chain(
                self.reference_program
                    .second_promotion
                    .accepted_contributions
                    .iter(),
            )
            .map(|lineage| lineage.contribution_id.as_str())
            .collect::<BTreeSet<_>>();
        let window_replay_checked = BTreeMap::from([
            (
                self.reference_program
                    .first_window_summary
                    .window_id
                    .as_str(),
                self.reference_program
                    .first_window_summary
                    .replay_checked_contributions
                    > 0,
            ),
            (
                self.reference_program
                    .second_window_summary
                    .window_id
                    .as_str(),
                self.reference_program
                    .second_window_summary
                    .replay_checked_contributions
                    > 0,
            ),
        ]);
        let mut seen = BTreeSet::new();
        for summary in &self.contributor_receipts {
            summary.validate()?;
            if !seen.insert(summary.contribution_id.as_str()) {
                return Err(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "contributor receipt summary repeated contribution `{}`",
                        summary.contribution_id
                    ),
                });
            }
            let artifact = self
                .reference_program
                .artifact_receipts
                .iter()
                .find(|receipt| receipt.contribution_id == summary.contribution_id)
                .ok_or(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "missing artifact receipt for contribution `{}`",
                        summary.contribution_id
                    ),
                })?;
            let security = security_by_contribution
                .get(summary.contribution_id.as_str())
                .ok_or(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "missing security receipt for contribution `{}`",
                        summary.contribution_id
                    ),
                })?;
            if summary.window_id != artifact.window_id
                || summary.worker_id != artifact.worker_id
                || summary.artifact_id != artifact.artifact_id
                || summary.artifact_receipt_digest != artifact.receipt_digest
                || summary.security_receipt_id != security.receipt_id
                || summary.security_receipt_digest != security.receipt_digest
                || summary.artifact_disposition != artifact.disposition
                || summary.security_disposition != security.disposition
            {
                return Err(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "contributor receipt summary drifted from stored artifact or security receipts for `{}`",
                        summary.contribution_id
                    ),
                });
            }
            if summary.accepted_for_window_aggregation
                != accepted_contributions.contains(summary.contribution_id.as_str())
            {
                return Err(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "aggregation-acceptance summary drifted for contribution `{}`",
                        summary.contribution_id
                    ),
                });
            }
            if summary.window_replay_checked
                != *window_replay_checked
                    .get(summary.window_id.as_str())
                    .unwrap_or(&false)
            {
                return Err(PsionDecentralizedContributionError::ContributorMismatch {
                    detail: format!(
                        "window replay-check summary drifted for contribution `{}`",
                        summary.contribution_id
                    ),
                });
            }
        }
        Ok(())
    }
}

/// Records one bounded decentralized-contribution bundle and validates it.
pub fn record_psion_decentralized_contribution_bundle(
    lane_id: impl Into<String>,
    trusted_cluster_run_artifact: PsionContributionArtifactReference,
    reasoning_sft_run_artifact: PsionContributionArtifactReference,
    acceptance_binding: PsionContributionAcceptanceBinding,
    capability_binding: PsionContributionCapabilityBinding,
    reference_program: DecentralizedAdapterReferenceProgramReport,
    contributor_receipts: Vec<PsionContributorReceiptSummary>,
    claim_boundary: impl Into<String>,
    summary: impl Into<String>,
) -> Result<PsionDecentralizedContributionBundle, PsionDecentralizedContributionError> {
    let mut bundle = PsionDecentralizedContributionBundle {
        schema_version: String::from(PSION_DECENTRALIZED_CONTRIBUTION_BUNDLE_SCHEMA_VERSION),
        lane_id: lane_id.into(),
        contribution_mode: PsionDecentralizedContributionMode::AdapterDeltaWindow,
        trusted_cluster_run_artifact,
        reasoning_sft_run_artifact,
        acceptance_binding,
        capability_binding,
        reference_program,
        contributor_receipts,
        claim_boundary: claim_boundary.into(),
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_psion_decentralized_contribution_digest(&bundle);
    bundle.validate()?;
    Ok(bundle)
}

/// Builds contributor summaries directly from the bounded adapter reference program report.
#[must_use]
pub fn psion_contributor_receipt_summaries(
    report: &DecentralizedAdapterReferenceProgramReport,
) -> Vec<PsionContributorReceiptSummary> {
    let security_by_contribution = report
        .security_receipts
        .iter()
        .map(|receipt| (receipt.contribution_id.as_str(), receipt))
        .collect::<BTreeMap<_, _>>();
    let accepted_contributions = report
        .first_promotion
        .accepted_contributions
        .iter()
        .chain(report.second_promotion.accepted_contributions.iter())
        .map(|lineage| lineage.contribution_id.as_str())
        .collect::<BTreeSet<_>>();
    let window_replay_checked = BTreeMap::from([
        (
            report.first_window_summary.window_id.as_str(),
            report.first_window_summary.replay_checked_contributions > 0,
        ),
        (
            report.second_window_summary.window_id.as_str(),
            report.second_window_summary.replay_checked_contributions > 0,
        ),
    ]);

    report
        .artifact_receipts
        .iter()
        .filter_map(|artifact| {
            let security = security_by_contribution.get(artifact.contribution_id.as_str())?;
            Some(PsionContributorReceiptSummary {
                contribution_id: artifact.contribution_id.clone(),
                window_id: artifact.window_id.clone(),
                worker_id: artifact.worker_id.clone(),
                artifact_id: artifact.artifact_id.clone(),
                artifact_receipt_digest: artifact.receipt_digest.clone(),
                security_receipt_id: security.receipt_id.clone(),
                security_receipt_digest: security.receipt_digest.clone(),
                artifact_disposition: artifact.disposition,
                security_disposition: security.disposition,
                accepted_for_window_aggregation: accepted_contributions
                    .contains(artifact.contribution_id.as_str()),
                window_replay_checked: *window_replay_checked
                    .get(artifact.window_id.as_str())
                    .unwrap_or(&false),
                detail: format!(
                    "Contribution `{}` remains bounded to adapter-window artifact `{}` plus security receipt `{}`; no full-model synchronous all-reduce claim is implied.",
                    artifact.contribution_id, artifact.artifact_id, security.receipt_id
                ),
            })
        })
        .collect()
}

/// Validation failures for Psion decentralized contribution bundles.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionDecentralizedContributionError {
    /// One required field was missing.
    #[error("Psion decentralized-contribution field `{field}` is missing")]
    MissingField {
        /// Missing field label.
        field: String,
    },
    /// The schema version drifted from the current contract.
    #[error(
        "Psion decentralized contribution expected schema version `{expected}`, found `{actual}`"
    )]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The bound acceptance discipline was malformed.
    #[error("Psion decentralized contribution acceptance binding is invalid: {detail}")]
    InvalidAcceptanceBinding {
        /// Machine-readable detail.
        detail: String,
    },
    /// The bound capability or rollback discipline was malformed.
    #[error("Psion decentralized contribution capability binding is invalid: {detail}")]
    InvalidCapabilityBinding {
        /// Machine-readable detail.
        detail: String,
    },
    /// The embedded adapter reference program did not satisfy the bounded lane contract.
    #[error("Psion decentralized contribution reference program is invalid: {detail}")]
    InvalidReferenceProgram {
        /// Machine-readable detail.
        detail: String,
    },
    /// One contributor summary drifted from the embedded bounded receipts.
    #[error("Psion decentralized contribution contributor receipts are invalid: {detail}")]
    ContributorMismatch {
        /// Machine-readable detail.
        detail: String,
    },
    /// The stable digest drifted from the bundle contents.
    #[error("Psion decentralized contribution bundle digest mismatch")]
    DigestMismatch,
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionDecentralizedContributionError> {
    if value.trim().is_empty() {
        return Err(PsionDecentralizedContributionError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_psion_decentralized_contribution_digest(
    bundle: &PsionDecentralizedContributionBundle,
) -> String {
    let mut stripped = bundle.clone();
    stripped.bundle_digest.clear();
    let mut hasher = Sha256::new();
    hasher.update(b"psion_decentralized_contribution_bundle|");
    hasher.update(
        serde_json::to_vec(&stripped).expect("psion decentralized contribution should serialize"),
    );
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> PsionDecentralizedContributionBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/decentralized/psion_decentralized_contribution_bundle_v1.json"
        ))
        .expect("decentralized contribution fixture should parse")
    }

    #[test]
    fn decentralized_contribution_fixture_validates() {
        bundle()
            .validate()
            .expect("decentralized contribution fixture should validate");
    }

    #[test]
    fn decentralized_contribution_rejects_wrong_rollback_schema() {
        let mut bundle = bundle();
        bundle.capability_binding.rollback_receipt_schema_version =
            String::from("psion.capability_withdrawal_receipt.v0");
        bundle.bundle_digest = stable_psion_decentralized_contribution_digest(&bundle);
        let error = bundle
            .validate()
            .expect_err("wrong rollback schema should be rejected");
        assert!(matches!(
            error,
            PsionDecentralizedContributionError::InvalidCapabilityBinding { .. }
        ));
    }

    #[test]
    fn decentralized_contribution_rejects_missing_contributor_security_summary() {
        let mut bundle = bundle();
        bundle.contributor_receipts.pop();
        bundle.bundle_digest = stable_psion_decentralized_contribution_digest(&bundle);
        let error = bundle
            .validate()
            .expect_err("missing contributor summary should be rejected");
        assert!(matches!(
            error,
            PsionDecentralizedContributionError::ContributorMismatch { .. }
        ));
    }
}
