use psionic_eval::{
    ParameterGolfSubmissionPromotionDisposition, ParameterGolfSubmissionPromotionReceipt,
    PARAMETER_GOLF_SOTA_RULE_SNAPSHOT_DATE,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ParameterGolfFinalPrBundleReport, ParameterGolfLocalCloneDryRunReport,
    ParameterGolfLocalCloneDryRunVerdict, ParameterGolfSubmissionRunEvidenceReport,
};

/// One frozen record-candidate family under campaign review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordCandidateFrozenConfig {
    /// Stable submission identifier.
    pub submission_id: String,
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Stable submission track identifier.
    pub track_id: String,
    /// Stable tokenizer or vocabulary reference.
    pub tokenizer_ref: String,
    /// Stable accounting-posture reference.
    pub accounting_posture_ref: String,
    /// Stable run-recipe reference.
    pub run_recipe_ref: String,
}

/// One evidence bundle tied to a frozen record-candidate campaign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRecordCandidateCampaignEvidence {
    /// Stable evidence identifier inside the campaign.
    pub evidence_id: String,
    /// Challenge-facing exported-folder evidence report.
    pub submission_run_evidence: ParameterGolfSubmissionRunEvidenceReport,
    /// Maintainer-facing promotion receipt paired with the same run.
    pub promotion_receipt: ParameterGolfSubmissionPromotionReceipt,
}

/// Summary of one repeated evidence bundle inside the campaign report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRecordCandidateCampaignEvidenceSummary {
    /// Stable evidence identifier.
    pub evidence_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Digest of the exported-folder run-evidence report.
    pub submission_run_evidence_report_digest: String,
    /// Digest of the distributed challenge receipt bound to that folder.
    pub distributed_challenge_receipt_digest: String,
    /// Distributed challenge receipt disposition.
    pub distributed_challenge_disposition: String,
    /// Digest of the promotion receipt.
    pub promotion_receipt_digest: String,
    /// Promotion disposition.
    pub promotion_disposition: ParameterGolfSubmissionPromotionDisposition,
    /// Candidate validation bits per byte.
    pub val_bpb: f64,
}

/// Final integrity status for the frozen candidate campaign.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordCandidateCampaignDisposition {
    /// The campaign is internally coherent enough to feed the later readiness audit.
    ReadyForReadinessAudit,
    /// The campaign is still incomplete or internally inconsistent.
    Blocked,
}

/// Machine-readable report for one frozen record-candidate campaign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRecordCandidateCampaignReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable campaign identifier.
    pub campaign_id: String,
    /// Frozen candidate family for the campaign.
    pub frozen_candidate: ParameterGolfRecordCandidateFrozenConfig,
    /// Ordered repeated evidence summaries.
    pub evidence_summaries: Vec<ParameterGolfRecordCandidateCampaignEvidenceSummary>,
    /// Whether every carried evidence bundle bound one measured `8xH100` receipt.
    pub all_evidence_measured_8xh100: bool,
    /// Ordered blocked reasons when the campaign is not yet coherent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<String>,
    /// Final campaign disposition.
    pub disposition: ParameterGolfRecordCandidateCampaignDisposition,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

/// Current status for one final readiness gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfFinalReadinessGateStatus {
    Satisfied,
    Blocked,
}

/// One explicit readiness gate compared against the current README-facing flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfFinalReadinessGate {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Gate status.
    pub status: ParameterGolfFinalReadinessGateStatus,
    /// Honest detail for the gate.
    pub detail: String,
}

/// Final maintainer-facing disposition for the submission-ready audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfFinalReadinessDisposition {
    ReadyToSubmit,
    Blocked,
}

/// Final readiness audit over the frozen campaign plus the PR and local-clone surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfFinalReadinessAuditReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// README rule snapshot date used for the audit.
    pub challenge_rule_snapshot_date: String,
    /// Stable campaign identifier.
    pub campaign_id: String,
    /// Stable submission identifier.
    pub submission_id: String,
    /// Stable track identifier.
    pub track_id: String,
    /// Digest of the record-candidate campaign report.
    pub campaign_report_digest: String,
    /// Digest of the final PR bundle report.
    pub final_pr_bundle_report_digest: String,
    /// Digest of the local challenge-clone dry-run report.
    pub local_clone_dry_run_report_digest: String,
    /// Ordered readiness gates.
    pub gates: Vec<ParameterGolfFinalReadinessGate>,
    /// Ordered blocked reasons when the audit is not green.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<String>,
    /// Final readiness disposition.
    pub disposition: ParameterGolfFinalReadinessDisposition,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the audit payload.
    pub report_digest: String,
}

/// Failure while building the record-candidate campaign or final readiness audit.
#[derive(Debug, Error)]
pub enum ParameterGolfCampaignError {
    #[error("campaign `{campaign_id}` must be non-empty")]
    MissingCampaignId { campaign_id: String },
    #[error("campaign `{campaign_id}` does not carry any evidence bundles")]
    MissingEvidence { campaign_id: String },
    #[error("campaign evidence `{evidence_id}` is invalid: {message}")]
    InvalidEvidence {
        evidence_id: String,
        message: String,
    },
    #[error("readiness audit input is inconsistent: {message}")]
    InvalidReadinessInput { message: String },
}

/// Builds one frozen record-candidate campaign report from repeated evidence bundles.
pub fn build_parameter_golf_record_candidate_campaign_report(
    campaign_id: impl Into<String>,
    frozen_candidate: ParameterGolfRecordCandidateFrozenConfig,
    evidence: &[ParameterGolfRecordCandidateCampaignEvidence],
) -> Result<ParameterGolfRecordCandidateCampaignReport, ParameterGolfCampaignError> {
    let campaign_id = campaign_id.into();
    if campaign_id.trim().is_empty() {
        return Err(ParameterGolfCampaignError::MissingCampaignId { campaign_id });
    }
    if evidence.is_empty() {
        return Err(ParameterGolfCampaignError::MissingEvidence { campaign_id });
    }

    let mut blocked_reasons = Vec::new();
    if evidence.len() < 2 {
        blocked_reasons.push(String::from(
            "record-candidate campaign requires repeated evidence bundles; found fewer than two runs",
        ));
    }

    let mut summaries = Vec::with_capacity(evidence.len());
    for entry in evidence {
        if entry.evidence_id.trim().is_empty() {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: String::from("evidence_id must be non-empty"),
            });
        }
        let run_evidence = &entry.submission_run_evidence;
        let promotion_receipt = &entry.promotion_receipt;
        if run_evidence.submission_id != frozen_candidate.submission_id {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: format!(
                    "submission run evidence submission_id `{}` drifted from frozen candidate `{}`",
                    run_evidence.submission_id, frozen_candidate.submission_id
                ),
            });
        }
        if promotion_receipt.candidate.submission_id != frozen_candidate.submission_id {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: format!(
                    "promotion receipt submission_id `{}` drifted from frozen candidate `{}`",
                    promotion_receipt.candidate.submission_id, frozen_candidate.submission_id
                ),
            });
        }
        if run_evidence.track_id != frozen_candidate.track_id
            || promotion_receipt.candidate.track_id != frozen_candidate.track_id
        {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: String::from(
                    "submission run evidence or promotion receipt drifted from the frozen track_id",
                ),
            });
        }
        if promotion_receipt.candidate.benchmark_ref != frozen_candidate.benchmark_ref {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: format!(
                    "promotion receipt benchmark_ref `{}` drifted from frozen candidate `{}`",
                    promotion_receipt.candidate.benchmark_ref, frozen_candidate.benchmark_ref
                ),
            });
        }
        if run_evidence.run_id != promotion_receipt.candidate.run_id {
            return Err(ParameterGolfCampaignError::InvalidEvidence {
                evidence_id: entry.evidence_id.clone(),
                message: format!(
                    "submission run evidence run_id `{}` drifted from promotion receipt run_id `{}`",
                    run_evidence.run_id, promotion_receipt.candidate.run_id
                ),
            });
        }
        if !promotion_receipt.candidate.record_track_candidate {
            blocked_reasons.push(format!(
                "evidence `{}` does not target the record track in its promotion receipt",
                entry.evidence_id
            ));
        }
        if run_evidence.distributed_challenge_receipt.refusal.is_some()
            || run_evidence.distributed_challenge_receipt.disposition
                != psionic_eval::ParameterGolfDistributedLaneDisposition::Measured
        {
            blocked_reasons.push(format!(
                "evidence `{}` does not carry a measured 8xH100 distributed receipt",
                entry.evidence_id
            ));
        }
        summaries.push(ParameterGolfRecordCandidateCampaignEvidenceSummary {
            evidence_id: entry.evidence_id.clone(),
            run_id: run_evidence.run_id.clone(),
            submission_run_evidence_report_digest: run_evidence.report_digest.clone(),
            distributed_challenge_receipt_digest: run_evidence
                .distributed_challenge_receipt
                .receipt_digest
                .clone(),
            distributed_challenge_disposition: format!(
                "{:?}",
                run_evidence.distributed_challenge_receipt.disposition
            )
            .to_lowercase(),
            promotion_receipt_digest: promotion_receipt.receipt_digest.clone(),
            promotion_disposition: promotion_receipt.disposition,
            val_bpb: promotion_receipt.candidate.val_bpb,
        });
    }

    let all_evidence_measured_8xh100 = evidence.iter().all(|entry| {
        entry
            .submission_run_evidence
            .distributed_challenge_receipt
            .refusal
            .is_none()
            && entry
                .submission_run_evidence
                .distributed_challenge_receipt
                .disposition
                == psionic_eval::ParameterGolfDistributedLaneDisposition::Measured
    });
    let disposition = if blocked_reasons.is_empty() {
        ParameterGolfRecordCandidateCampaignDisposition::ReadyForReadinessAudit
    } else {
        ParameterGolfRecordCandidateCampaignDisposition::Blocked
    };
    let mut report = ParameterGolfRecordCandidateCampaignReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.record_candidate_campaign.v1"),
        campaign_id,
        frozen_candidate,
        evidence_summaries: summaries,
        all_evidence_measured_8xh100,
        blocked_reasons,
        disposition,
        claim_boundary: String::from(
            "This report freezes one exact Parameter Golf candidate family and binds repeated evidence bundles plus promotion receipts to that candidate. It does not by itself claim submission readiness or external submission approval.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_record_candidate_campaign_report|",
        &report,
    );
    Ok(report)
}

/// Builds one final readiness audit from the frozen campaign plus the PR and
/// local challenge-clone dry-run reports.
pub fn build_parameter_golf_final_readiness_audit_report(
    campaign: &ParameterGolfRecordCandidateCampaignReport,
    final_pr_bundle: &ParameterGolfFinalPrBundleReport,
    local_clone_dry_run: &ParameterGolfLocalCloneDryRunReport,
) -> Result<ParameterGolfFinalReadinessAuditReport, ParameterGolfCampaignError> {
    if campaign.frozen_candidate.submission_id != final_pr_bundle.submission_id
        || campaign.frozen_candidate.submission_id != local_clone_dry_run.submission_id
    {
        return Err(ParameterGolfCampaignError::InvalidReadinessInput {
            message: String::from(
                "campaign, final PR bundle, and local clone dry run must target the same submission_id",
            ),
        });
    }
    if campaign.frozen_candidate.track_id != final_pr_bundle.track_id
        || campaign.frozen_candidate.track_id != local_clone_dry_run.track_id
    {
        return Err(ParameterGolfCampaignError::InvalidReadinessInput {
            message: String::from(
                "campaign, final PR bundle, and local clone dry run must target the same track_id",
            ),
        });
    }

    let latest_promotion = campaign
        .evidence_summaries
        .last()
        .map(|entry| entry.promotion_disposition)
        .unwrap_or(ParameterGolfSubmissionPromotionDisposition::Refused);
    let mut gates = Vec::new();
    gates.push(ParameterGolfFinalReadinessGate {
        gate_id: String::from("measured_8xh100_execution"),
        status: if campaign.all_evidence_measured_8xh100 {
            ParameterGolfFinalReadinessGateStatus::Satisfied
        } else {
            ParameterGolfFinalReadinessGateStatus::Blocked
        },
        detail: String::from(
            "final readiness requires measured 8xH100 execution evidence instead of posture-only or refused distributed receipts",
        ),
    });
    gates.push(ParameterGolfFinalReadinessGate {
        gate_id: String::from("repeated_record_candidate_campaign"),
        status: if campaign.disposition
            == ParameterGolfRecordCandidateCampaignDisposition::ReadyForReadinessAudit
        {
            ParameterGolfFinalReadinessGateStatus::Satisfied
        } else {
            ParameterGolfFinalReadinessGateStatus::Blocked
        },
        detail: String::from(
            "final readiness requires one frozen candidate family with repeated evidence bundles and tied promotion receipts",
        ),
    });
    gates.push(ParameterGolfFinalReadinessGate {
        gate_id: String::from("promotion_posture"),
        status: if latest_promotion == ParameterGolfSubmissionPromotionDisposition::Promotable {
            ParameterGolfFinalReadinessGateStatus::Satisfied
        } else {
            ParameterGolfFinalReadinessGateStatus::Blocked
        },
        detail: String::from(
            "final readiness requires the latest promotion receipt to be promotable instead of refused",
        ),
    });
    gates.push(ParameterGolfFinalReadinessGate {
        gate_id: String::from("final_pr_bundle_present"),
        status: if !final_pr_bundle.record_folder_relpath.trim().is_empty()
            && !final_pr_bundle.promotion_receipt_path.trim().is_empty()
        {
            ParameterGolfFinalReadinessGateStatus::Satisfied
        } else {
            ParameterGolfFinalReadinessGateStatus::Blocked
        },
        detail: String::from(
            "final readiness requires one final PR bundle with the record folder and promotion receipt paths sealed",
        ),
    });
    gates.push(ParameterGolfFinalReadinessGate {
        gate_id: String::from("local_clone_dry_run"),
        status: if local_clone_dry_run.verdict == ParameterGolfLocalCloneDryRunVerdict::CleanPass {
            ParameterGolfFinalReadinessGateStatus::Satisfied
        } else {
            ParameterGolfFinalReadinessGateStatus::Blocked
        },
        detail: String::from(
            "final readiness requires the local parameter-golf clone dry run to pass cleanly and leave the clone state restored",
        ),
    });

    let blocked_reasons = gates
        .iter()
        .filter(|gate| gate.status == ParameterGolfFinalReadinessGateStatus::Blocked)
        .map(|gate| format!("readiness gate `{}` is still blocked", gate.gate_id))
        .collect::<Vec<_>>();
    let disposition = if blocked_reasons.is_empty() {
        ParameterGolfFinalReadinessDisposition::ReadyToSubmit
    } else {
        ParameterGolfFinalReadinessDisposition::Blocked
    };

    let mut report = ParameterGolfFinalReadinessAuditReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.final_readiness_audit.v1"),
        challenge_rule_snapshot_date: String::from(PARAMETER_GOLF_SOTA_RULE_SNAPSHOT_DATE),
        campaign_id: campaign.campaign_id.clone(),
        submission_id: campaign.frozen_candidate.submission_id.clone(),
        track_id: campaign.frozen_candidate.track_id.clone(),
        campaign_report_digest: campaign.report_digest.clone(),
        final_pr_bundle_report_digest: final_pr_bundle.report_digest.clone(),
        local_clone_dry_run_report_digest: local_clone_dry_run.report_digest.clone(),
        gates,
        blocked_reasons,
        disposition,
        claim_boundary: String::from(
            "This readiness audit compares the frozen record-candidate campaign, final PR bundle, and local challenge-clone dry run against the current README-facing promotion and execution gates. It does not perform the external submission itself.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_final_readiness_audit_report|",
        &report,
    );
    Ok(report)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("serialize campaign report"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_parameter_golf_final_readiness_audit_report,
        build_parameter_golf_record_candidate_campaign_report,
        ParameterGolfFinalReadinessDisposition, ParameterGolfRecordCandidateCampaignDisposition,
        ParameterGolfRecordCandidateCampaignEvidence, ParameterGolfRecordCandidateFrozenConfig,
    };
    use psionic_eval::{
        build_parameter_golf_submission_promotion_receipt,
        ParameterGolfSubmissionPromotionCandidate, ParameterGolfSubmissionPromotionDisposition,
    };

    use crate::{
        ParameterGolfFinalPrBundleReport, ParameterGolfLocalCloneDryRunReport,
        ParameterGolfSubmissionRunEvidenceReport,
    };

    fn sample_run_evidence() -> ParameterGolfSubmissionRunEvidenceReport {
        let mut report: ParameterGolfSubmissionRunEvidenceReport = serde_json::from_str(
            include_str!(
                "../../../fixtures/parameter_golf/reports/parameter_golf_submission_run_evidence.json"
            ),
        )
        .expect("submission run evidence fixture should decode");
        report.submission_id = String::from("2026-03-24_psionic_record_candidate");
        report.track_id = String::from("track_record_10min_16mb");
        report.run_id = String::from("record-candidate-run-1");
        report.distributed_challenge_receipt.disposition =
            psionic_eval::ParameterGolfDistributedLaneDisposition::Measured;
        report.distributed_challenge_receipt.refusal = None;
        report.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-1");
        report.report_digest = String::from("run-evidence-1");
        report
    }

    fn sample_promotion_receipt(
        run_evidence: &ParameterGolfSubmissionRunEvidenceReport,
        run_id: &str,
        val_bpb: f64,
    ) -> psionic_eval::ParameterGolfSubmissionPromotionReceipt {
        build_parameter_golf_submission_promotion_receipt(
            ParameterGolfSubmissionPromotionCandidate {
                submission_id: run_evidence.submission_id.clone(),
                run_id: String::from(run_id),
                benchmark_ref: run_evidence
                    .distributed_challenge_receipt
                    .benchmark_ref
                    .clone(),
                track_id: run_evidence.track_id.clone(),
                record_track_candidate: true,
                val_bpb,
                systems_only_waiver_claimed: false,
                systems_only_waiver_supported: false,
                significance_p_value: Some(0.001),
                significance_evidence_refs: vec![String::from("campaign-significance")],
                evidence_refs: vec![String::from("submission-run-evidence")],
            },
        )
    }

    fn sample_final_pr_bundle() -> ParameterGolfFinalPrBundleReport {
        serde_json::from_str(include_str!(
            "../../../fixtures/parameter_golf/reports/parameter_golf_final_pr_bundle.json"
        ))
        .expect("final pr bundle fixture should decode")
    }

    fn sample_local_clone_dry_run() -> ParameterGolfLocalCloneDryRunReport {
        serde_json::from_str(include_str!(
            "../../../fixtures/parameter_golf/reports/parameter_golf_local_clone_dry_run.json"
        ))
        .expect("local clone dry run fixture should decode")
    }

    #[test]
    fn record_candidate_campaign_report_requires_repeated_measured_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut run1 = sample_run_evidence();
        run1.run_id = String::from("record-candidate-run-1");
        run1.report_digest = String::from("run-evidence-1");
        run1.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-1");
        let mut run2 = run1.clone();
        run2.run_id = String::from("record-candidate-run-2");
        run2.report_digest = String::from("run-evidence-2");
        run2.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-2");
        let frozen = ParameterGolfRecordCandidateFrozenConfig {
            submission_id: run1.submission_id.clone(),
            benchmark_ref: run1.distributed_challenge_receipt.benchmark_ref.clone(),
            track_id: run1.track_id.clone(),
            tokenizer_ref: String::from("fineweb_1024_bpe.model"),
            accounting_posture_ref: String::from("parameter_golf_accounting.md@2026-03-24"),
            run_recipe_ref: String::from("runpod_8xh100_parameter_golf"),
        };
        let report = build_parameter_golf_record_candidate_campaign_report(
            "campaign.parameter_golf.record_candidate.v1",
            frozen,
            &[
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-1"),
                    submission_run_evidence: run1.clone(),
                    promotion_receipt: sample_promotion_receipt(&run1, &run1.run_id, 1.20),
                },
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-2"),
                    submission_run_evidence: run2.clone(),
                    promotion_receipt: sample_promotion_receipt(&run2, &run2.run_id, 1.19),
                },
            ],
        )?;
        assert_eq!(
            report.disposition,
            ParameterGolfRecordCandidateCampaignDisposition::ReadyForReadinessAudit
        );
        assert!(report.all_evidence_measured_8xh100);
        assert_eq!(report.evidence_summaries.len(), 2);
        Ok(())
    }

    #[test]
    fn final_readiness_audit_turns_green_when_campaign_and_review_surfaces_align(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut run1 = sample_run_evidence();
        run1.run_id = String::from("record-candidate-run-1");
        run1.report_digest = String::from("run-evidence-1");
        run1.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-1");
        let mut run2 = run1.clone();
        run2.run_id = String::from("record-candidate-run-2");
        run2.report_digest = String::from("run-evidence-2");
        run2.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-2");
        let frozen = ParameterGolfRecordCandidateFrozenConfig {
            submission_id: run1.submission_id.clone(),
            benchmark_ref: run1.distributed_challenge_receipt.benchmark_ref.clone(),
            track_id: run1.track_id.clone(),
            tokenizer_ref: String::from("fineweb_1024_bpe.model"),
            accounting_posture_ref: String::from("parameter_golf_accounting.md@2026-03-24"),
            run_recipe_ref: String::from("runpod_8xh100_parameter_golf"),
        };
        let campaign = build_parameter_golf_record_candidate_campaign_report(
            "campaign.parameter_golf.record_candidate.v1",
            frozen,
            &[
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-1"),
                    submission_run_evidence: run1.clone(),
                    promotion_receipt: sample_promotion_receipt(&run1, &run1.run_id, 1.20),
                },
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-2"),
                    submission_run_evidence: run2.clone(),
                    promotion_receipt: sample_promotion_receipt(&run2, &run2.run_id, 1.19),
                },
            ],
        )?;

        let mut final_pr_bundle = sample_final_pr_bundle();
        final_pr_bundle.submission_id = run1.submission_id.clone();
        final_pr_bundle.track_id = run1.track_id.clone();
        let mut local_clone_dry_run = sample_local_clone_dry_run();
        local_clone_dry_run.submission_id = run1.submission_id.clone();
        local_clone_dry_run.track_id = run1.track_id.clone();
        local_clone_dry_run.verdict = crate::ParameterGolfLocalCloneDryRunVerdict::CleanPass;

        let audit = build_parameter_golf_final_readiness_audit_report(
            &campaign,
            &final_pr_bundle,
            &local_clone_dry_run,
        )?;
        assert_eq!(
            audit.disposition,
            ParameterGolfFinalReadinessDisposition::ReadyToSubmit
        );
        assert!(audit.blocked_reasons.is_empty());
        Ok(())
    }

    #[test]
    fn final_readiness_audit_blocks_when_latest_promotion_is_refused(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut run1 = sample_run_evidence();
        run1.run_id = String::from("record-candidate-run-1");
        run1.report_digest = String::from("run-evidence-1");
        run1.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-1");
        let mut run2 = run1.clone();
        run2.run_id = String::from("record-candidate-run-2");
        run2.report_digest = String::from("run-evidence-2");
        run2.distributed_challenge_receipt.receipt_digest = String::from("measured-receipt-2");
        let frozen = ParameterGolfRecordCandidateFrozenConfig {
            submission_id: run1.submission_id.clone(),
            benchmark_ref: run1.distributed_challenge_receipt.benchmark_ref.clone(),
            track_id: run1.track_id.clone(),
            tokenizer_ref: String::from("fineweb_1024_bpe.model"),
            accounting_posture_ref: String::from("parameter_golf_accounting.md@2026-03-24"),
            run_recipe_ref: String::from("runpod_8xh100_parameter_golf"),
        };
        let campaign = build_parameter_golf_record_candidate_campaign_report(
            "campaign.parameter_golf.record_candidate.v1",
            frozen,
            &[
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-1"),
                    submission_run_evidence: run1.clone(),
                    promotion_receipt: sample_promotion_receipt(&run1, &run1.run_id, 1.20),
                },
                ParameterGolfRecordCandidateCampaignEvidence {
                    evidence_id: String::from("run-2"),
                    submission_run_evidence: run2.clone(),
                    promotion_receipt: sample_promotion_receipt(&run2, &run2.run_id, 1.223),
                },
            ],
        )?;

        let mut final_pr_bundle = sample_final_pr_bundle();
        final_pr_bundle.submission_id = run1.submission_id.clone();
        final_pr_bundle.track_id = run1.track_id.clone();
        let mut local_clone_dry_run = sample_local_clone_dry_run();
        local_clone_dry_run.submission_id = run1.submission_id.clone();
        local_clone_dry_run.track_id = run1.track_id.clone();
        local_clone_dry_run.verdict = crate::ParameterGolfLocalCloneDryRunVerdict::CleanPass;

        let audit = build_parameter_golf_final_readiness_audit_report(
            &campaign,
            &final_pr_bundle,
            &local_clone_dry_run,
        )?;
        assert_eq!(
            campaign
                .evidence_summaries
                .last()
                .map(|entry| entry.promotion_disposition),
            Some(ParameterGolfSubmissionPromotionDisposition::Refused)
        );
        assert_eq!(
            audit.disposition,
            ParameterGolfFinalReadinessDisposition::Blocked
        );
        assert!(!audit.blocked_reasons.is_empty());
        Ok(())
    }
}
