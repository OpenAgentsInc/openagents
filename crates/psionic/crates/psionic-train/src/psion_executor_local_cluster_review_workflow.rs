use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_local_cluster_autoblocks_report,
    builtin_executor_local_cluster_dashboard_packet, builtin_executor_local_cluster_ledger,
    builtin_executor_mixture_rollback_policy_packet,
    PsionExecutorBaselineTruthError, PsionExecutorLocalClusterAutoblocksError,
    PsionExecutorLocalClusterCandidateStatus, PsionExecutorLocalClusterDashboardError,
    PsionExecutorLocalClusterLedgerError, PsionExecutorMixtureRollbackPolicyError,
    PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
    PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_DOC_PATH,
    PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH,
};

/// Stable schema version for the canonical local-cluster weekly review workflow.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_review_workflow.v1";
/// Canonical fixture path for the local-cluster weekly review workflow packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_review_workflow_v1.json";
/// Canonical doc path for the local-cluster weekly review workflow packet.
pub const PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW.md";

const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_OWNERSHIP_DOC_PATH: &str = "docs/PSION_EXECUTOR_OWNERSHIP.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";
const PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_source_family_contribution_v1.json";
const BASELINE_TEMPLATE_ID: &str = "psion_executor_weekly_baseline_review_template_v1";
const ABLATION_TEMPLATE_ID: &str = "psion_executor_weekly_ablation_review_template_v1";
const MIXTURE_ROLLBACK_TEMPLATE_ID: &str =
    "psion_executor_weekly_mixture_rollback_review_template_v1";
const BASELINE_DECISION_ID: &str = "psion_executor_weekly_baseline_review_2026w14_v1";
const ABLATION_DECISION_ID: &str = "psion_executor_weekly_ablation_review_2026w14_v1";
const MIXTURE_ROLLBACK_DECISION_ID: &str =
    "psion_executor_weekly_mixture_rollback_review_2026w14_v1";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterReviewWorkflowError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
    #[error(transparent)]
    Dashboard(#[from] PsionExecutorLocalClusterDashboardError),
    #[error(transparent)]
    Ledger(#[from] PsionExecutorLocalClusterLedgerError),
    #[error(transparent)]
    Autoblocks(#[from] PsionExecutorLocalClusterAutoblocksError),
    #[error(transparent)]
    RollbackPolicy(#[from] PsionExecutorMixtureRollbackPolicyError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterReviewTemplate {
    pub template_id: String,
    pub review_kind: String,
    pub cadence_id: String,
    pub reviewer_role: String,
    pub frozen_pack_only: bool,
    pub required_refs: Vec<String>,
    pub required_fact_ids: Vec<String>,
    pub decision_rule: String,
    pub detail: String,
    pub template_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterReviewDecision {
    pub review_id: String,
    pub review_kind: String,
    pub cadence_window_id: String,
    pub reviewer_role: String,
    pub reviewer_identity: String,
    pub cited_pack_ids: Vec<String>,
    pub cited_row_ids: Vec<String>,
    pub cited_block_ids: Vec<String>,
    pub decision: String,
    pub status: String,
    pub detail: String,
    pub decision_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterReviewWorkflowPacket {
    pub schema_version: String,
    pub workflow_id: String,
    pub ownership_ref: String,
    pub dashboard_ref: String,
    pub dashboard_digest: String,
    pub ledger_ref: String,
    pub ledger_digest: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_digest: String,
    pub autoblocks_ref: String,
    pub autoblocks_digest: String,
    pub mixture_rollback_policy_ref: String,
    pub mixture_rollback_policy_digest: String,
    pub baseline_review_template: PsionExecutorLocalClusterReviewTemplate,
    pub ablation_review_template: PsionExecutorLocalClusterReviewTemplate,
    pub mixture_rollback_review_template: PsionExecutorLocalClusterReviewTemplate,
    pub current_decisions: Vec<PsionExecutorLocalClusterReviewDecision>,
    pub frozen_pack_only_rule: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub workflow_digest: String,
}

impl PsionExecutorLocalClusterReviewTemplate {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_review_workflow.template.template_id",
                self.template_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.review_kind",
                self.review_kind.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.cadence_id",
                self.cadence_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.reviewer_role",
                self.reviewer_role.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.decision_rule",
                self.decision_rule.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.template.template_digest",
                self.template_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.required_refs.is_empty() || self.required_fact_ids.is_empty() {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::MissingField {
                field: String::from("psion_executor_local_cluster_review_workflow.template.required_refs"),
            });
        }
        if !self.frozen_pack_only {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::InvalidValue {
                field: String::from("psion_executor_local_cluster_review_workflow.template.frozen_pack_only"),
                detail: String::from("weekly review templates must stay frozen-pack only"),
            });
        }
        if stable_template_digest(self) != self.template_digest {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_review_workflow.template.template_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterReviewDecision {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_review_workflow.decision.review_id",
                self.review_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.review_kind",
                self.review_kind.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.cadence_window_id",
                self.cadence_window_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.reviewer_role",
                self.reviewer_role.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.reviewer_identity",
                self.reviewer_identity.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.decision",
                self.decision.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.status",
                self.status.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.decision.decision_digest",
                self.decision_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.cited_pack_ids.is_empty() || self.cited_row_ids.is_empty() {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::MissingField {
                field: String::from("psion_executor_local_cluster_review_workflow.decision.cited_pack_ids"),
            });
        }
        if stable_decision_digest(self) != self.decision_digest {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::DigestMismatch {
                field: String::from(
                    "psion_executor_local_cluster_review_workflow.decision.decision_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterReviewWorkflowPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_review_workflow.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_SCHEMA_VERSION {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_review_workflow.workflow_id",
                self.workflow_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.ownership_ref",
                self.ownership_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.dashboard_ref",
                self.dashboard_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.dashboard_digest",
                self.dashboard_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.ledger_ref",
                self.ledger_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.ledger_digest",
                self.ledger_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.autoblocks_ref",
                self.autoblocks_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.autoblocks_digest",
                self.autoblocks_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.mixture_rollback_policy_ref",
                self.mixture_rollback_policy_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.mixture_rollback_policy_digest",
                self.mixture_rollback_policy_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.frozen_pack_only_rule",
                self.frozen_pack_only_rule.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_review_workflow.workflow_digest",
                self.workflow_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.current_decisions.len() != 3 {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::InvalidValue {
                field: String::from("psion_executor_local_cluster_review_workflow.current_decisions"),
                detail: String::from(
                    "workflow must keep one baseline, one ablation, and one mixture rollback decision",
                ),
            });
        }
        self.baseline_review_template.validate()?;
        self.ablation_review_template.validate()?;
        self.mixture_rollback_review_template.validate()?;
        for decision in &self.current_decisions {
            decision.validate()?;
        }
        for review_kind in ["baseline_review", "ablation_review", "mixture_rollback_review"] {
            if !self
                .current_decisions
                .iter()
                .any(|decision| decision.review_kind == review_kind)
            {
                return Err(PsionExecutorLocalClusterReviewWorkflowError::InvalidValue {
                    field: String::from(
                        "psion_executor_local_cluster_review_workflow.current_decisions[].review_kind",
                    ),
                    detail: format!("missing required review kind `{review_kind}`"),
                });
            }
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::MissingField {
                field: String::from("psion_executor_local_cluster_review_workflow.support_refs"),
            });
        }
        if stable_workflow_digest(self) != self.workflow_digest {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_review_workflow.workflow_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_review_workflow_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterReviewWorkflowPacket, PsionExecutorLocalClusterReviewWorkflowError>
{
    let dashboard = builtin_executor_local_cluster_dashboard_packet(workspace_root)?;
    let ledger = builtin_executor_local_cluster_ledger(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;
    let autoblocks = builtin_executor_local_cluster_autoblocks_report(workspace_root)?;
    let rollback_policy = builtin_executor_mixture_rollback_policy_packet(workspace_root)?;

    let current_best_row = ledger
        .rows_for_candidate_status(PsionExecutorLocalClusterCandidateStatus::CurrentBest)
        .into_iter()
        .next()
        .ok_or_else(|| PsionExecutorLocalClusterReviewWorkflowError::MissingField {
            field: String::from("psion_executor_local_cluster_review_workflow.current_best_row"),
        })?;

    let baseline_template = build_baseline_template();
    let ablation_template = build_ablation_template();
    let mixture_rollback_template = build_mixture_rollback_template();
    let current_decisions = vec![
        build_baseline_decision(current_best_row, &autoblocks)?,
        build_ablation_decision(current_best_row, &autoblocks)?,
        build_mixture_rollback_decision(current_best_row, &autoblocks, &rollback_policy)?,
    ];

    let mut packet = PsionExecutorLocalClusterReviewWorkflowPacket {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_SCHEMA_VERSION),
        workflow_id: String::from("psion_executor_local_cluster_review_workflow_v1"),
        ownership_ref: String::from(PSION_EXECUTOR_OWNERSHIP_DOC_PATH),
        dashboard_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
        dashboard_digest: dashboard.dashboard_digest,
        ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        ledger_digest: ledger.ledger_digest,
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest,
        autoblocks_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH),
        autoblocks_digest: autoblocks.report_digest,
        mixture_rollback_policy_ref: String::from(
            PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH,
        ),
        mixture_rollback_policy_digest: rollback_policy.packet_digest,
        baseline_review_template: baseline_template,
        ablation_review_template: ablation_template,
        mixture_rollback_review_template: mixture_rollback_template,
        current_decisions,
        frozen_pack_only_rule: String::from(
            "Only frozen frequent-pack and promotion-pack results plus the canonical ledger/dashboard/autoblock surfaces count toward weekly review decisions. Partial probes, ad hoc experiments, and convenience subsets do not count as review truth.",
        ),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_OWNERSHIP_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH),
            String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical weekly review workflow. Baseline, ablation, and mixture-rollback decisions all cite frozen-pack ids, retained ledger rows, active auto-block ids, and the canonical rollback policy instead of informal review prose.",
        ),
        workflow_digest: String::new(),
    };
    packet.workflow_digest = stable_workflow_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_local_cluster_review_workflow_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterReviewWorkflowPacket, PsionExecutorLocalClusterReviewWorkflowError>
{
    let packet = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_baseline_template() -> PsionExecutorLocalClusterReviewTemplate {
    let mut template = PsionExecutorLocalClusterReviewTemplate {
        template_id: String::from(BASELINE_TEMPLATE_ID),
        review_kind: String::from("baseline_review"),
        cadence_id: String::from("executor_weekly_baseline_review.v1"),
        reviewer_role: String::from("Weekly baseline review owner"),
        frozen_pack_only: true,
        required_refs: vec![
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH),
        ],
        required_fact_ids: vec![
            String::from("frozen_pack_results_only"),
            String::from("current_best_row_visible"),
            String::from("active_block_ids_visible"),
            String::from("reference_linear_anchor_visible"),
        ],
        decision_rule: String::from(
            "Hold the frozen baseline as the active floor unless the current-best row stays tied to frozen-pack truth and all active promotion block ids clear.",
        ),
        detail: String::from(
            "The weekly baseline review template keeps the baseline decision anchored to frozen-pack truth, the retained current-best row, and the canonical auto-block report.",
        ),
        template_digest: String::new(),
    };
    template.template_digest = stable_template_digest(&template);
    template
}

fn build_ablation_template() -> PsionExecutorLocalClusterReviewTemplate {
    let mut template = PsionExecutorLocalClusterReviewTemplate {
        template_id: String::from(ABLATION_TEMPLATE_ID),
        review_kind: String::from("ablation_review"),
        cadence_id: String::from("executor_weekly_ablation_review.v1"),
        reviewer_role: String::from("Weekly ablation review owner"),
        frozen_pack_only: true,
        required_refs: vec![
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH),
        ],
        required_fact_ids: vec![
            String::from("same_budget_only"),
            String::from("frozen_pack_results_only"),
            String::from("current_best_row_visible"),
            String::from("active_block_ids_visible"),
        ],
        decision_rule: String::from(
            "Authorize no new same-budget ablation win unless the candidate stays inside frozen-pack truth and the current-best row no longer carries active promotion blocks.",
        ),
        detail: String::from(
            "The weekly ablation review template keeps same-budget decisions tied to the retained local-cluster ledger and the canonical auto-block report instead of ad hoc experiment summaries.",
        ),
        template_digest: String::new(),
    };
    template.template_digest = stable_template_digest(&template);
    template
}

fn build_mixture_rollback_template() -> PsionExecutorLocalClusterReviewTemplate {
    let mut template = PsionExecutorLocalClusterReviewTemplate {
        template_id: String::from(MIXTURE_ROLLBACK_TEMPLATE_ID),
        review_kind: String::from("mixture_rollback_review"),
        cadence_id: String::from("executor_weekly_mixture_rollback_review.v1"),
        reviewer_role: String::from("Weekly mixture rollback review owner"),
        frozen_pack_only: true,
        required_refs: vec![
            String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_FIXTURE_PATH),
        ],
        required_fact_ids: vec![
            String::from("held_out_regressions_visible"),
            String::from("adversarial_regressions_visible"),
            String::from("rollback_trigger_visible"),
            String::from("single_lever_retry_rule_visible"),
        ],
        decision_rule: String::from(
            "If a provisional same-budget mixture win posts any negative held-out or adversarial slice delta, roll it back immediately and constrain the next retry to one lever.",
        ),
        detail: String::from(
            "The weekly mixture rollback review template keeps misleading-win rollback tied to the canonical source-family contribution report, the canonical rollback policy packet, and the active auto-block report instead of ad hoc experiment narratives.",
        ),
        template_digest: String::new(),
    };
    template.template_digest = stable_template_digest(&template);
    template
}

fn build_baseline_decision(
    current_best_row: &crate::PsionExecutorLocalClusterLedgerRow,
    autoblocks: &crate::PsionExecutorLocalClusterAutoblocksReport,
) -> Result<PsionExecutorLocalClusterReviewDecision, PsionExecutorLocalClusterReviewWorkflowError> {
    let cited_block_ids = autoblocks.active_promotion_block_ids.clone();
    let blocked = !cited_block_ids.is_empty();
    let mut decision = PsionExecutorLocalClusterReviewDecision {
        review_id: String::from(BASELINE_DECISION_ID),
        review_kind: String::from("baseline_review"),
        cadence_window_id: String::from("2026-W14"),
        reviewer_role: String::from("Weekly baseline review owner"),
        reviewer_identity: String::from("Christopher David"),
        cited_pack_ids: current_best_row.eval_pack_ids.clone(),
        cited_row_ids: vec![current_best_row.row_id.clone()],
        cited_block_ids,
        decision: String::from("hold_frozen_baseline"),
        status: if blocked {
            String::from("promotion_blocked_current_best")
        } else {
            String::from("review_ready_current_best")
        },
        detail: if blocked {
            format!(
                "The weekly baseline review keeps the frozen baseline in place because current-best row `{}` still carries active promotion blocks [{}]. Only the frozen pack ids {:?} were admitted as review truth.",
                current_best_row.row_id,
                autoblocks.active_promotion_block_ids.join(", "),
                current_best_row.eval_pack_ids
            )
        } else {
            format!(
                "The weekly baseline review keeps the frozen baseline as the comparison floor while current-best row `{}` now clears all retained promotion blocks under frozen pack ids {:?}.",
                current_best_row.row_id,
                current_best_row.eval_pack_ids
            )
        },
        decision_digest: String::new(),
    };
    decision.decision_digest = stable_decision_digest(&decision);
    Ok(decision)
}

fn build_ablation_decision(
    current_best_row: &crate::PsionExecutorLocalClusterLedgerRow,
    autoblocks: &crate::PsionExecutorLocalClusterAutoblocksReport,
) -> Result<PsionExecutorLocalClusterReviewDecision, PsionExecutorLocalClusterReviewWorkflowError> {
    let cited_block_ids = autoblocks.active_promotion_block_ids.clone();
    let blocked = !cited_block_ids.is_empty();
    let mut decision = PsionExecutorLocalClusterReviewDecision {
        review_id: String::from(ABLATION_DECISION_ID),
        review_kind: String::from("ablation_review"),
        cadence_window_id: String::from("2026-W14"),
        reviewer_role: String::from("Weekly ablation review owner"),
        reviewer_identity: String::from("Christopher David"),
        cited_pack_ids: current_best_row.eval_pack_ids.clone(),
        cited_row_ids: vec![current_best_row.row_id.clone()],
        cited_block_ids,
        decision: String::from("hold_same_budget_follow_on"),
        status: if blocked {
            String::from("promotion_blocked_current_best")
        } else {
            String::from("review_ready_current_best")
        },
        detail: if blocked {
            format!(
                "The weekly ablation review refuses to authorize a new same-budget winner while current-best row `{}` still carries active promotion blocks [{}]. The cited evidence remains limited to frozen pack ids {:?} plus retained ledger/dashboard/autoblock facts.",
                current_best_row.row_id,
                autoblocks.active_promotion_block_ids.join(", "),
                current_best_row.eval_pack_ids
            )
        } else {
            format!(
                "The weekly ablation review now sees current-best row `{}` as phase-exit green and free of active promotion blocks; same-budget follow-on decisions can proceed from frozen pack ids {:?} instead of export-side blockers.",
                current_best_row.row_id,
                current_best_row.eval_pack_ids
            )
        },
        decision_digest: String::new(),
    };
    decision.decision_digest = stable_decision_digest(&decision);
    Ok(decision)
}

fn build_mixture_rollback_decision(
    current_best_row: &crate::PsionExecutorLocalClusterLedgerRow,
    autoblocks: &crate::PsionExecutorLocalClusterAutoblocksReport,
    rollback_policy: &crate::PsionExecutorMixtureRollbackPolicyPacket,
) -> Result<PsionExecutorLocalClusterReviewDecision, PsionExecutorLocalClusterReviewWorkflowError> {
    let allowed_levers = rollback_policy
        .retry_constraint
        .allowed_lever_classes
        .join(", ");
    let mut decision = PsionExecutorLocalClusterReviewDecision {
        review_id: String::from(MIXTURE_ROLLBACK_DECISION_ID),
        review_kind: String::from("mixture_rollback_review"),
        cadence_window_id: String::from("2026-W14"),
        reviewer_role: String::from("Weekly mixture rollback review owner"),
        reviewer_identity: String::from("Christopher David"),
        cited_pack_ids: current_best_row.eval_pack_ids.clone(),
        cited_row_ids: vec![current_best_row.row_id.clone()],
        cited_block_ids: autoblocks.active_promotion_block_ids.clone(),
        decision: rollback_policy.rollback_decision.clone(),
        status: rollback_policy.rollback_status.clone(),
        detail: if rollback_policy.rollback_triggered {
            format!(
                "The weekly mixture rollback review rolled back the provisional same-budget mixture win on row `{}` because the canonical rollback policy packet recorded active slice regressions. The next admitted retry is capped at one lever with allowed lever classes [{}].",
                current_best_row.row_id, allowed_levers
            )
        } else {
            format!(
                "The weekly mixture rollback review found no retained misleading mixture win on row `{}` this week: the canonical rollback policy packet records no provisional same-budget win claim and no active held-out or adversarial rollback trigger. If a future provisional win regresses those slices, the next admitted retry stays capped at one lever with allowed lever classes [{}].",
                current_best_row.row_id, allowed_levers
            )
        },
        decision_digest: String::new(),
    };
    decision.decision_digest = stable_decision_digest(&decision);
    Ok(decision)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorLocalClusterReviewWorkflowError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorLocalClusterReviewWorkflowError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterReviewWorkflowError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorLocalClusterReviewWorkflowError::Parse {
            path: relative_path.to_string(),
            error,
        }
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterReviewWorkflowError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterReviewWorkflowError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterReviewWorkflowError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_template_digest(template: &PsionExecutorLocalClusterReviewTemplate) -> String {
    let mut clone = template.clone();
    clone.template_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("review template serialization should succeed"),
    ))
}

fn stable_decision_digest(decision: &PsionExecutorLocalClusterReviewDecision) -> String {
    let mut clone = decision.clone();
    clone.decision_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("review decision serialization should succeed"),
    ))
}

fn stable_workflow_digest(packet: &PsionExecutorLocalClusterReviewWorkflowPacket) -> String {
    let mut clone = packet.clone();
    clone.workflow_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("review workflow serialization should succeed"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_local_cluster_review_workflow_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_review_workflow_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_local_cluster_review_workflow_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterReviewWorkflowPacket =
            read_json(root.as_path(), PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH)?;
        let actual = builtin_executor_local_cluster_review_workflow_packet(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterReviewWorkflowError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn executor_local_cluster_review_workflow_stays_frozen_pack_only(
    ) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        let root = workspace_root();
        let packet = builtin_executor_local_cluster_review_workflow_packet(root.as_path())?;
        assert!(packet.baseline_review_template.frozen_pack_only);
        assert!(packet.ablation_review_template.frozen_pack_only);
        assert!(packet.mixture_rollback_review_template.frozen_pack_only);
        for decision in &packet.current_decisions {
            assert_eq!(decision.cited_pack_ids.len(), 2);
        }
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_review_workflow_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterReviewWorkflowError> {
        let root = workspace_root();
        let packet = write_builtin_executor_local_cluster_review_workflow_packet(root.as_path())?;
        let committed: PsionExecutorLocalClusterReviewWorkflowPacket =
            read_json(root.as_path(), PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH)?;
        assert_eq!(packet, committed);
        Ok(())
    }
}
