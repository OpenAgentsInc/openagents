use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    train_compiled_agent_grounded_answer_model, train_compiled_agent_route_model,
    CompiledAgentEvidenceClass, CompiledAgentGroundedAnswerModelArtifact,
    CompiledAgentGroundedAnswerTrainingSample, CompiledAgentRouteModelArtifact,
    CompiledAgentRouteTrainingSample,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    apply_external_boundary_posture, build_benchmark_submission_record_with_payload_ref,
    build_compiled_agent_external_runtime_receipt_submission_from_fixture,
    build_compiled_agent_learning_receipt_from_source,
    build_compiled_agent_learning_receipt_ledger_from_receipts,
    build_compiled_agent_replay_bundle_from_ledger,
    build_runtime_submission_record_with_payload_ref, build_shadow_assessments,
    compiled_agent_external_contributor_identity_for_profile, repo_relative_path,
    retained_compiled_agent_external_benchmark_kit, run_compiled_agent_external_benchmark_kit,
    CompiledAgentArtifactContractError, CompiledAgentCorpusSplit,
    CompiledAgentExternalBenchmarkError, CompiledAgentExternalBenchmarkRun,
    CompiledAgentExternalContributorProfile, CompiledAgentExternalIntakeError,
    CompiledAgentExternalQuarantineReport, CompiledAgentExternalReviewState,
    CompiledAgentExternalRuntimeReceiptSubmission, CompiledAgentExternalSubmissionRecord,
    CompiledAgentExternalSubmissionStagingLedger, CompiledAgentLearningReceipt,
    CompiledAgentReceiptError,
};

pub const COMPILED_AGENT_TAILNET_NODE_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.tailnet_node_bundle.v1";
pub const COMPILED_AGENT_TAILNET_GOVERNED_RUN_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.tailnet_governed_run.v1";
pub const COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/tailnet/compiled_agent_tailnet_m5_node_bundle_v1.json";
pub const COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/tailnet/compiled_agent_tailnet_archlinux_node_bundle_v1.json";
pub const COMPILED_AGENT_TAILNET_STAGING_LEDGER_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/tailnet/compiled_agent_tailnet_submission_staging_ledger_v1.json";
pub const COMPILED_AGENT_TAILNET_QUARANTINE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/tailnet/compiled_agent_tailnet_quarantine_report_v1.json";
pub const COMPILED_AGENT_TAILNET_GOVERNED_RUN_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/tailnet/compiled_agent_tailnet_governed_run_v1.json";
pub const COMPILED_AGENT_TAILNET_DOC_PATH: &str = "docs/COMPILED_AGENT_TAILNET_FIRST_PILOT.md";

const COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_ID: &str = "compiled_agent.tailnet_node_bundle.m5.v1";
const COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_ID: &str =
    "compiled_agent.tailnet_node_bundle.archlinux.v1";
const COMPILED_AGENT_TAILNET_STAGING_LEDGER_ID: &str =
    "compiled_agent.tailnet_submission_staging_ledger.v1";
const COMPILED_AGENT_TAILNET_QUARANTINE_REPORT_ID: &str =
    "compiled_agent.tailnet_quarantine_report.v1";
const COMPILED_AGENT_TAILNET_GOVERNED_RUN_ID: &str = "compiled_agent.tailnet_governed_run.v1";

#[derive(Debug, Error)]
pub enum CompiledAgentTailnetPilotError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid tailnet contributor profile `{profile_id}`")]
    InvalidContributorProfile { profile_id: String },
    #[error("invalid tailnet node bundle: {detail}")]
    InvalidNodeBundle { detail: String },
    #[error("invalid tailnet governed run: {detail}")]
    InvalidGovernedRun { detail: String },
    #[error(transparent)]
    ExternalBenchmark(#[from] CompiledAgentExternalBenchmarkError),
    #[error(transparent)]
    ExternalIntake(#[from] CompiledAgentExternalIntakeError),
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentTailnetNodeBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub contributor_profile: CompiledAgentExternalContributorProfile,
    pub contributor: crate::CompiledAgentExternalContributorIdentity,
    pub contract_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub benchmark_run: CompiledAgentExternalBenchmarkRun,
    pub runtime_submissions: Vec<CompiledAgentExternalRuntimeReceiptSubmission>,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentTailnetGovernedRun {
    pub schema_version: String,
    pub run_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub local_bundle_digest: String,
    pub remote_bundle_digest: String,
    pub staging_ledger_digest: String,
    pub quarantine_report_digest: String,
    pub tailnet_learning_ledger_digest: String,
    pub tailnet_replay_bundle_digest: String,
    pub route_preview_artifact_digest: String,
    pub grounded_preview_artifact_digest: String,
    pub route_preview_training_accuracy: f32,
    pub grounded_preview_training_accuracy: f32,
    pub contributor_ids: Vec<String>,
    pub summary: String,
    pub run_digest: String,
}

#[must_use]
pub fn compiled_agent_tailnet_m5_node_bundle_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_tailnet_archlinux_node_bundle_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_tailnet_staging_ledger_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_TAILNET_STAGING_LEDGER_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_tailnet_quarantine_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_TAILNET_QUARANTINE_REPORT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_tailnet_governed_run_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_TAILNET_GOVERNED_RUN_FIXTURE_PATH)
}

pub fn build_compiled_agent_tailnet_node_bundle(
    profile: CompiledAgentExternalContributorProfile,
) -> Result<CompiledAgentTailnetNodeBundle, CompiledAgentTailnetPilotError> {
    let contributor = compiled_agent_external_contributor_identity_for_profile(profile);
    let contract = retained_compiled_agent_external_benchmark_kit()?;
    let benchmark_run = run_compiled_agent_external_benchmark_kit(&contract, &contributor)?;
    let runtime_submissions = tailnet_runtime_fixture_names(profile)
        .iter()
        .enumerate()
        .map(|(index, fixture_name)| {
            build_compiled_agent_external_runtime_receipt_submission_from_fixture(
                format!(
                    "tailnet://{}/runtime_submission/{}",
                    contributor.source_machine_id,
                    index + 1
                )
                .as_str(),
                format!(
                    "submission.compiled_agent.tailnet.{}.runtime_{}.v1",
                    profile.profile_id(),
                    index + 1
                )
                .as_str(),
                fixture_name,
                &contributor,
                tailnet_runtime_tags(profile, fixture_name),
                format!(
                    "Tailnet-first runtime disagreement receipt from `{}` on fixture `{}`.",
                    contributor.source_machine_id, fixture_name
                ),
                format!(
                    "This Tailnet-first runtime receipt preserves contributor identity `{}` and machine class `{}` while keeping disagreement evidence in the governed external contract.",
                    contributor.contributor_id, contributor.machine_class
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut bundle = CompiledAgentTailnetNodeBundle {
        schema_version: String::from(COMPILED_AGENT_TAILNET_NODE_BUNDLE_SCHEMA_VERSION),
        bundle_id: match profile {
            CompiledAgentExternalContributorProfile::TailnetM5Mlx => {
                String::from(COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_ID)
            }
            CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda => {
                String::from(COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_ID)
            }
            CompiledAgentExternalContributorProfile::ExternalAlpha => {
                String::from("compiled_agent.tailnet_node_bundle.external_alpha.v1")
            }
        },
        contributor_profile: profile,
        contributor,
        contract_digest: contract.contract_digest,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        benchmark_run,
        runtime_submissions,
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Tailnet node bundle for contributor `{}` retains one governed benchmark run and {} runtime disagreement submissions on the admitted compiled-agent family.",
        bundle.contributor.contributor_id,
        bundle.runtime_submissions.len()
    );
    bundle.bundle_digest = stable_digest(
        b"compiled_agent_tailnet_node_bundle|",
        &bundle_without_digest(&bundle),
    );
    bundle.validate()?;
    Ok(bundle)
}

pub fn write_compiled_agent_tailnet_node_bundle(
    output_path: impl AsRef<Path>,
    profile: CompiledAgentExternalContributorProfile,
) -> Result<CompiledAgentTailnetNodeBundle, CompiledAgentTailnetPilotError> {
    let output_path = output_path.as_ref();
    write_pretty_json(output_path, || {
        build_compiled_agent_tailnet_node_bundle(profile)
    })
}

pub fn load_compiled_agent_tailnet_node_bundle(
    path: impl AsRef<Path>,
) -> Result<CompiledAgentTailnetNodeBundle, CompiledAgentTailnetPilotError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| CompiledAgentTailnetPilotError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let bundle: CompiledAgentTailnetNodeBundle = serde_json::from_slice(&bytes)?;
    bundle.validate()?;
    Ok(bundle)
}

pub fn build_compiled_agent_tailnet_governed_run(
    local_bundle: &CompiledAgentTailnetNodeBundle,
    remote_bundle: &CompiledAgentTailnetNodeBundle,
) -> Result<
    (
        CompiledAgentExternalSubmissionStagingLedger,
        CompiledAgentExternalQuarantineReport,
        CompiledAgentTailnetGovernedRun,
    ),
    CompiledAgentTailnetPilotError,
> {
    let mut submissions = Vec::<CompiledAgentExternalSubmissionRecord>::new();
    let mut shadow_assessments = Vec::new();
    let promoted_contract = crate::canonical_compiled_agent_promoted_artifact_contract()?;

    let local_benchmark_payload = format!(
        "{}#benchmark_run",
        COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_FIXTURE_PATH
    );
    let remote_benchmark_payload = format!(
        "{}#benchmark_run",
        COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_FIXTURE_PATH
    );
    let (local_benchmark_record, local_review_receipt, _) =
        build_benchmark_submission_record_with_payload_ref(
            String::from("submission.compiled_agent.tailnet.m5.benchmark.v1"),
            local_benchmark_payload,
            &local_bundle.benchmark_run,
        )?;
    let (remote_benchmark_record, remote_review_receipt, _) =
        build_benchmark_submission_record_with_payload_ref(
            String::from("submission.compiled_agent.tailnet.archlinux.benchmark.v1"),
            remote_benchmark_payload,
            &remote_bundle.benchmark_run,
        )?;
    shadow_assessments.extend(build_shadow_assessments(
        local_benchmark_record.submission_id.as_str(),
        &local_review_receipt,
        local_bundle
            .benchmark_run
            .row_runs
            .iter()
            .find(|row| row.row_id == "external.negated_wallet.v1")
            .map(|row| row.source_receipt.clone()),
    )?);
    shadow_assessments.extend(build_shadow_assessments(
        remote_benchmark_record.submission_id.as_str(),
        &remote_review_receipt,
        remote_bundle
            .benchmark_run
            .row_runs
            .iter()
            .find(|row| row.row_id == "external.negated_wallet.v1")
            .map(|row| row.source_receipt.clone()),
    )?);
    submissions.push(local_benchmark_record);
    submissions.push(remote_benchmark_record);

    for (bundle, node_label, bundle_fixture) in [
        (
            local_bundle,
            "m5",
            COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_FIXTURE_PATH,
        ),
        (
            remote_bundle,
            "archlinux",
            COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_FIXTURE_PATH,
        ),
    ] {
        for submission in &bundle.runtime_submissions {
            let payload_ref = format!("{bundle_fixture}#{}", submission.submission_id);
            let (record, learning_receipt, _) =
                build_runtime_submission_record_with_payload_ref(payload_ref, submission)?;
            shadow_assessments.extend(build_shadow_assessments(
                record.submission_id.as_str(),
                &learning_receipt,
                Some(submission.source_receipt.clone()),
            )?);
            let _ = node_label;
            submissions.push(record);
        }
    }

    shadow_assessments.sort_by(|left, right| left.assessment_id.cmp(&right.assessment_id));
    let contributor_trust_histories = apply_external_boundary_posture(&mut submissions);
    let accepted_submission_count = submissions
        .iter()
        .filter(|record| record.review_state != CompiledAgentExternalReviewState::Rejected)
        .count() as u32;
    let rejected_submission_count = submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::Rejected)
        .count() as u32;
    let review_required_submission_count = submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .count() as u32;
    let replay_candidate_submission_count = submissions
        .iter()
        .filter(|record| !record.replay_candidate_receipt_ids.is_empty())
        .count() as u32;
    let anomaly_submission_count = submissions
        .iter()
        .filter(|record| !record.anomaly_flags.is_empty())
        .count() as u32;
    let mut staging_ledger = CompiledAgentExternalSubmissionStagingLedger {
        schema_version: String::from(
            "psionic.compiled_agent.external_submission_staging_ledger.v1",
        ),
        ledger_id: String::from(COMPILED_AGENT_TAILNET_STAGING_LEDGER_ID),
        row_id: String::from("compiled_agent.tailnet_external_beta.v1"),
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        contributor_contract_version: local_bundle.contributor.contract_version_accepted.clone(),
        promoted_artifact_contract_digest: promoted_contract.contract_digest.clone(),
        total_submission_count: submissions.len() as u32,
        accepted_submission_count,
        rejected_submission_count,
        review_required_submission_count,
        replay_candidate_submission_count,
        anomaly_submission_count,
        submissions,
        contributor_trust_histories,
        summary: String::new(),
        ledger_digest: String::new(),
    };
    staging_ledger.summary = format!(
        "Tailnet-first staging ledger retained {} submissions across the M5 and archlinux RTX 4080 nodes: {} accepted into quarantine, {} rejected, {} review-required, {} anomaly-flagged, and {} replay-candidate submission records.",
        staging_ledger.total_submission_count,
        staging_ledger.accepted_submission_count,
        staging_ledger.rejected_submission_count,
        staging_ledger.review_required_submission_count,
        staging_ledger.anomaly_submission_count,
        staging_ledger.replay_candidate_submission_count
    );
    staging_ledger.ledger_digest = staging_ledger.stable_digest();
    staging_ledger.validate()?;

    let accepted_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state != CompiledAgentExternalReviewState::Rejected)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let rejected_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::Rejected)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let review_required_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let replay_candidate_receipt_ids = staging_ledger
        .submissions
        .iter()
        .flat_map(|record| record.replay_candidate_receipt_ids.clone())
        .collect::<Vec<_>>();
    let proposed_replay_sample_ids = staging_ledger
        .submissions
        .iter()
        .flat_map(|record| record.proposed_replay_sample_ids.clone())
        .collect::<Vec<_>>();
    let anomaly_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| !record.anomaly_flags.is_empty())
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let trusted_signal_contributor_ids = staging_ledger
        .contributor_trust_histories
        .iter()
        .filter(|history| {
            history.trust_posture == crate::CompiledAgentExternalTrustPosture::TrustedSignal
        })
        .map(|history| history.contributor_id.clone())
        .collect::<Vec<_>>();
    let watch_contributor_ids = staging_ledger
        .contributor_trust_histories
        .iter()
        .filter(|history| history.trust_posture == crate::CompiledAgentExternalTrustPosture::Watch)
        .map(|history| history.contributor_id.clone())
        .collect::<Vec<_>>();
    let mut quarantine_report = CompiledAgentExternalQuarantineReport {
        schema_version: String::from("psionic.compiled_agent.external_quarantine_report.v1"),
        report_id: String::from(COMPILED_AGENT_TAILNET_QUARANTINE_REPORT_ID),
        row_id: staging_ledger.row_id.clone(),
        evidence_class: staging_ledger.evidence_class,
        staging_ledger_digest: staging_ledger.ledger_digest.clone(),
        promoted_artifact_contract_digest: promoted_contract.contract_digest.clone(),
        accepted_submission_ids,
        rejected_submission_ids,
        review_required_submission_ids,
        replay_candidate_receipt_ids,
        proposed_replay_sample_ids,
        anomaly_submission_ids,
        trusted_signal_contributor_ids,
        watch_contributor_ids,
        shadow_assessments,
        summary: String::new(),
        report_digest: String::new(),
    };
    quarantine_report.summary = format!(
        "Tailnet-first quarantine retained {} accepted submissions, {} rejected submissions, {} review-required submissions, {} anomaly-flagged submissions, and {} shadow assessments while preserving separate contributor identity for the M5 and archlinux nodes.",
        quarantine_report.accepted_submission_ids.len(),
        quarantine_report.rejected_submission_ids.len(),
        quarantine_report.review_required_submission_ids.len(),
        quarantine_report.anomaly_submission_ids.len(),
        quarantine_report.shadow_assessments.len()
    );
    quarantine_report.report_digest = quarantine_report.stable_digest();
    quarantine_report.validate(&staging_ledger)?;

    let accepted_learning_receipts = accepted_learning_receipts_from_bundle(
        COMPILED_AGENT_TAILNET_M5_NODE_BUNDLE_FIXTURE_PATH,
        local_bundle,
    )?
    .into_iter()
    .chain(accepted_learning_receipts_from_bundle(
        COMPILED_AGENT_TAILNET_ARCHLINUX_NODE_BUNDLE_FIXTURE_PATH,
        remote_bundle,
    )?)
    .collect::<Vec<_>>();
    let tailnet_learning_ledger = build_compiled_agent_learning_receipt_ledger_from_receipts(
        accepted_learning_receipts.clone(),
        "compiled_agent.tailnet_external_beta.v1",
    )?;
    let tailnet_replay_bundle =
        build_compiled_agent_replay_bundle_from_ledger(&tailnet_learning_ledger)?;
    let route_preview = build_route_preview_artifact(
        &accepted_learning_receipts,
        &tailnet_replay_bundle.bundle_digest,
    );
    let grounded_preview = build_grounded_preview_artifact(
        &accepted_learning_receipts,
        &tailnet_replay_bundle.bundle_digest,
    );

    let mut run = CompiledAgentTailnetGovernedRun {
        schema_version: String::from(COMPILED_AGENT_TAILNET_GOVERNED_RUN_SCHEMA_VERSION),
        run_id: String::from(COMPILED_AGENT_TAILNET_GOVERNED_RUN_ID),
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        local_bundle_digest: local_bundle.bundle_digest.clone(),
        remote_bundle_digest: remote_bundle.bundle_digest.clone(),
        staging_ledger_digest: staging_ledger.ledger_digest.clone(),
        quarantine_report_digest: quarantine_report.report_digest.clone(),
        tailnet_learning_ledger_digest: tailnet_learning_ledger.ledger_digest,
        tailnet_replay_bundle_digest: tailnet_replay_bundle.bundle_digest,
        route_preview_artifact_digest: route_preview.artifact_digest.clone(),
        grounded_preview_artifact_digest: grounded_preview.artifact_digest.clone(),
        route_preview_training_accuracy: route_preview.training_accuracy,
        grounded_preview_training_accuracy: grounded_preview.training_accuracy,
        contributor_ids: vec![
            local_bundle.contributor.contributor_id.clone(),
            remote_bundle.contributor.contributor_id.clone(),
        ],
        summary: String::new(),
        run_digest: String::new(),
    };
    run.summary = format!(
        "Tailnet-first governed run retained one shared admitted-family training window across `{}` and `{}` with {} accepted learning receipts, route preview accuracy {:.2}, and grounded preview accuracy {:.2}.",
        run.contributor_ids[0],
        run.contributor_ids[1],
        accepted_learning_receipts.len(),
        run.route_preview_training_accuracy,
        run.grounded_preview_training_accuracy
    );
    run.run_digest = stable_digest(
        b"compiled_agent_tailnet_governed_run|",
        &run_without_digest(&run),
    );
    run.validate()?;
    Ok((staging_ledger, quarantine_report, run))
}

pub fn write_compiled_agent_tailnet_governed_run(
    local_bundle: &CompiledAgentTailnetNodeBundle,
    remote_bundle: &CompiledAgentTailnetNodeBundle,
    staging_output_path: impl AsRef<Path>,
    quarantine_output_path: impl AsRef<Path>,
    run_output_path: impl AsRef<Path>,
) -> Result<CompiledAgentTailnetGovernedRun, CompiledAgentTailnetPilotError> {
    let (staging, quarantine, run) =
        build_compiled_agent_tailnet_governed_run(local_bundle, remote_bundle)?;
    write_json_value(staging_output_path.as_ref(), &staging)?;
    write_json_value(quarantine_output_path.as_ref(), &quarantine)?;
    write_json_value(run_output_path.as_ref(), &run)?;
    Ok(run)
}

impl CompiledAgentTailnetNodeBundle {
    fn validate(&self) -> Result<(), CompiledAgentTailnetPilotError> {
        if self.schema_version != COMPILED_AGENT_TAILNET_NODE_BUNDLE_SCHEMA_VERSION {
            return Err(CompiledAgentTailnetPilotError::InvalidNodeBundle {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.evidence_class != CompiledAgentEvidenceClass::LearnedLane {
            return Err(CompiledAgentTailnetPilotError::InvalidNodeBundle {
                detail: String::from("tailnet node bundle drifted evidence class"),
            });
        }
        if self.benchmark_run.contributor != self.contributor {
            return Err(CompiledAgentTailnetPilotError::InvalidNodeBundle {
                detail: String::from("benchmark run lost contributor linkage"),
            });
        }
        if self.runtime_submissions.is_empty() {
            return Err(CompiledAgentTailnetPilotError::InvalidNodeBundle {
                detail: String::from(
                    "tailnet node bundle requires at least one runtime submission",
                ),
            });
        }
        if self.bundle_digest
            != stable_digest(
                b"compiled_agent_tailnet_node_bundle|",
                &bundle_without_digest(self),
            )
        {
            return Err(CompiledAgentTailnetPilotError::InvalidNodeBundle {
                detail: String::from("bundle digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentTailnetGovernedRun {
    fn validate(&self) -> Result<(), CompiledAgentTailnetPilotError> {
        if self.schema_version != COMPILED_AGENT_TAILNET_GOVERNED_RUN_SCHEMA_VERSION {
            return Err(CompiledAgentTailnetPilotError::InvalidGovernedRun {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.contributor_ids.len() != 2 {
            return Err(CompiledAgentTailnetPilotError::InvalidGovernedRun {
                detail: String::from("governed run must retain exactly two contributor ids"),
            });
        }
        if self.route_preview_training_accuracy <= 0.0
            || self.grounded_preview_training_accuracy <= 0.0
        {
            return Err(CompiledAgentTailnetPilotError::InvalidGovernedRun {
                detail: String::from("preview training accuracy must be positive"),
            });
        }
        if self.run_digest
            != stable_digest(
                b"compiled_agent_tailnet_governed_run|",
                &run_without_digest(self),
            )
        {
            return Err(CompiledAgentTailnetPilotError::InvalidGovernedRun {
                detail: String::from("run digest drifted"),
            });
        }
        Ok(())
    }
}

fn tailnet_runtime_fixture_names(
    profile: CompiledAgentExternalContributorProfile,
) -> &'static [&'static str] {
    match profile {
        CompiledAgentExternalContributorProfile::TailnetM5Mlx => &[
            "openagents_wallet_provider_compare_heldout_receipt_v1.json",
            "openagents_wallet_recent_earnings_short_heldout_receipt_v1.json",
        ],
        CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda => &[
            "openagents_negated_provider_wallet_heldout_receipt_v1.json",
            "openagents_wallet_address_heldout_receipt_v1.json",
        ],
        CompiledAgentExternalContributorProfile::ExternalAlpha => {
            &["openagents_negated_wallet_receipt_v1.json"]
        }
    }
}

fn tailnet_runtime_tags(
    profile: CompiledAgentExternalContributorProfile,
    fixture_name: &str,
) -> Vec<String> {
    let mut tags = vec![
        String::from("external"),
        String::from("tailnet"),
        String::from("runtime"),
        profile.profile_id().to_string(),
    ];
    if fixture_name.contains("negated") {
        tags.push(String::from("negated"));
    }
    if fixture_name.contains("compare") {
        tags.push(String::from("route_ambiguity"));
    }
    if fixture_name.contains("wallet_recent_earnings") {
        tags.push(String::from("grounded_synthesis_drift"));
    }
    if fixture_name.contains("wallet_address") {
        tags.push(String::from("unsupported_wallet_question"));
    }
    tags
}

fn accepted_learning_receipts_from_bundle(
    bundle_fixture_path: &str,
    bundle: &CompiledAgentTailnetNodeBundle,
) -> Result<Vec<CompiledAgentLearningReceipt>, CompiledAgentTailnetPilotError> {
    bundle
        .benchmark_run
        .row_runs
        .iter()
        .filter(|row| {
            row.validator_outcome == crate::CompiledAgentExternalValidatorOutcome::Accepted
        })
        .map(|row| {
            build_compiled_agent_learning_receipt_from_source(
                format!("{bundle_fixture_path}#benchmark_run/{}", row.row_id).as_str(),
                &row.source_receipt,
                &crate::CompiledAgentReceiptSupervisionLabel {
                    expected_route: row.expected_route,
                    expected_public_response: row.expected_public_response.clone(),
                    corpus_split: row.corpus_split,
                    tags: row.tags.clone(),
                    operator_note: row.operator_note.clone(),
                },
            )
            .map_err(CompiledAgentTailnetPilotError::from)
        })
        .collect()
}

fn build_route_preview_artifact(
    receipts: &[CompiledAgentLearningReceipt],
    replay_bundle_digest: &str,
) -> CompiledAgentRouteModelArtifact {
    let samples = receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::Training)
        .map(|receipt| CompiledAgentRouteTrainingSample {
            sample_id: receipt.receipt_id.clone(),
            user_request: receipt.user_request.clone(),
            expected_route: receipt.expected_route,
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    train_compiled_agent_route_model(
        "compiled_agent.route.tailnet_dual_node_preview_v1",
        "compiled_agent.tailnet_external_beta.training_window.v1",
        replay_bundle_digest.to_string(),
        &samples,
        &[],
    )
}

fn build_grounded_preview_artifact(
    receipts: &[CompiledAgentLearningReceipt],
    replay_bundle_digest: &str,
) -> CompiledAgentGroundedAnswerModelArtifact {
    let samples = receipts
        .iter()
        .filter(|receipt| {
            receipt.corpus_split == CompiledAgentCorpusSplit::Training
                && receipt.expected_public_response.kind
                    != psionic_eval::CompiledAgentPublicOutcomeKind::ConfidenceFallback
        })
        .map(|receipt| CompiledAgentGroundedAnswerTrainingSample {
            sample_id: receipt.receipt_id.clone(),
            route: receipt.expected_route,
            tool_results: receipt.observed_tool_results.clone(),
            expected_kind: receipt.expected_public_response.kind,
            expected_response: receipt.expected_public_response.response.clone(),
            tags: receipt.tags.clone(),
        })
        .collect::<Vec<_>>();
    train_compiled_agent_grounded_answer_model(
        "compiled_agent.grounded_answer.tailnet_dual_node_preview_v1",
        "compiled_agent.tailnet_external_beta.training_window.v1",
        replay_bundle_digest.to_string(),
        &samples,
        &[],
    )
}

fn bundle_without_digest(
    bundle: &CompiledAgentTailnetNodeBundle,
) -> CompiledAgentTailnetNodeBundle {
    let mut clone = bundle.clone();
    clone.bundle_digest.clear();
    clone
}

fn run_without_digest(run: &CompiledAgentTailnetGovernedRun) -> CompiledAgentTailnetGovernedRun {
    let mut clone = run.clone();
    clone.run_digest.clear();
    clone
}

fn write_pretty_json<T, F>(
    output_path: &Path,
    build: F,
) -> Result<T, CompiledAgentTailnetPilotError>
where
    T: Serialize,
    F: FnOnce() -> Result<T, CompiledAgentTailnetPilotError>,
{
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentTailnetPilotError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let value = build()?;
    write_json_value(output_path, &value)?;
    Ok(value)
}

fn write_json_value<T: Serialize>(
    output_path: &Path,
    value: &T,
) -> Result<(), CompiledAgentTailnetPilotError> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentTailnetPilotError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentTailnetPilotError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization must succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}
