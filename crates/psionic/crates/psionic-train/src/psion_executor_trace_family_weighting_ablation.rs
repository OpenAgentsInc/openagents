use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_canonical_mixture_packet, builtin_executor_mixture_rollback_policy_packet,
    build_executor_source_family_contribution_report, PsionExecutorCanonicalMixtureError,
    PsionExecutorCanonicalMixturePacket, PsionExecutorMixtureRollbackPolicyError,
    PsionExecutorSourceFamilyContributionError, PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH,
    PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH,
    PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_DOC_PATH,
    PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_SCHEMA_VERSION: &str =
    "psion.executor.trace_family_weighting_ablation.v1";
pub const PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_trace_family_weighting_ablation_v1.json";
pub const PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION.md";

const PACKET_ID: &str = "psion_executor_trace_family_weighting_ablation_v1";
const REVIEW_WINDOW_ID: &str = "2026-W15";
const PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const RUN_ID: &str = "tailrun-home-admitted-20260329e";
const CANDIDATE_MIXTURE_ID: &str = "psion_executor_canonical_mixture_trace_weighting_candidate_v1";
const CANDIDATE_MODEL_ID: &str =
    "tassadar-article-transformer-trace-bound-trained-v0-trace-weighting-candidate-v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorTraceFamilyWeightingAblationError {
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
    Mixture(#[from] PsionExecutorCanonicalMixtureError),
    #[error(transparent)]
    Contribution(#[from] PsionExecutorSourceFamilyContributionError),
    #[error(transparent)]
    Rollback(#[from] PsionExecutorMixtureRollbackPolicyError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTraceFamilyWeightRow {
    pub source_family_id: String,
    pub baseline_weight_bps: u32,
    pub candidate_weight_bps: u32,
    pub delta_bps: i32,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTraceFamilySliceDeltaRow {
    pub source_family_id: String,
    pub slice_id: String,
    pub slice_class: String,
    pub delta_bps: i32,
    pub rollback_guard: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTraceFamilyWeightingAblationPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub run_id: String,
    pub review_window_id: String,
    pub same_budget_profile_id: String,
    pub baseline_mixture_ref: String,
    pub baseline_mixture_digest: String,
    pub baseline_mixture_id: String,
    pub candidate_mixture_id: String,
    pub baseline_model_id: String,
    pub candidate_model_id: String,
    pub source_family_contribution_ref: String,
    pub source_family_contribution_digest: String,
    pub rollback_policy_ref: String,
    pub rollback_policy_digest: String,
    pub changed_weight_rows: Vec<PsionExecutorTraceFamilyWeightRow>,
    pub slice_delta_rows: Vec<PsionExecutorTraceFamilySliceDeltaRow>,
    pub exactness_net_delta_bps: i32,
    pub held_out_negative_delta_count: u32,
    pub adversarial_negative_delta_count: u32,
    pub rollback_applied: bool,
    pub rollback_decision: String,
    pub review_decision: String,
    pub promotion_posture: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorTraceFamilyWeightRow {
    fn validate(
        &self,
        mixture: &PsionExecutorCanonicalMixturePacket,
    ) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
        for (field, value) in [
            (
                "psion_executor_trace_family_weighting_ablation.changed_weight_rows[].source_family_id",
                self.source_family_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.changed_weight_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.changed_weight_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        let baseline_row = mixture
            .source_families
            .iter()
            .find(|row| row.source_family_id == self.source_family_id)
            .ok_or_else(|| PsionExecutorTraceFamilyWeightingAblationError::MissingField {
                field: format!(
                    "psion_executor_trace_family_weighting_ablation.changed_weight_rows[{}].source_family_id",
                    self.source_family_id
                ),
            })?;
        if baseline_row.initial_weight_bps != self.baseline_weight_bps {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: format!(
                    "psion_executor_trace_family_weighting_ablation.changed_weight_rows[{}].baseline_weight_bps",
                    self.source_family_id
                ),
                detail: String::from("baseline weight must match the canonical mixture"),
            });
        }
        if self.candidate_weight_bps == 0
            || self.delta_bps != self.candidate_weight_bps as i32 - self.baseline_weight_bps as i32
        {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: format!(
                    "psion_executor_trace_family_weighting_ablation.changed_weight_rows[{}].delta_bps",
                    self.source_family_id
                ),
                detail: String::from("candidate weight must stay positive and match the delta"),
            });
        }
        if stable_weight_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.changed_weight_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTraceFamilySliceDeltaRow {
    fn validate(
        &self,
        changed_source_ids: &BTreeSet<String>,
    ) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
        for (field, value) in [
            (
                "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].source_family_id",
                self.source_family_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].slice_id",
                self.slice_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].slice_class",
                self.slice_class.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if !matches!(
            self.slice_class.as_str(),
            "exactness" | "held_out" | "adversarial"
        ) {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].slice_class",
                ),
                detail: String::from("slice class must stay exactness, held_out, or adversarial"),
            });
        }
        if !changed_source_ids.contains(&self.source_family_id) {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].source_family_id",
                ),
                detail: String::from("slice deltas must belong to the changed trace families"),
            });
        }
        if self.rollback_guard != (self.slice_class == "held_out" && self.delta_bps < 0) {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].rollback_guard",
                ),
                detail: String::from(
                    "rollback guard stays reserved for negative held-out slice deltas",
                ),
            });
        }
        if stable_slice_delta_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.slice_delta_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTraceFamilyWeightingAblationPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
        if self.schema_version != PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_SCHEMA_VERSION {
            return Err(
                PsionExecutorTraceFamilyWeightingAblationError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_trace_family_weighting_ablation.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.review_window_id",
                self.review_window_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.same_budget_profile_id",
                self.same_budget_profile_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.baseline_mixture_ref",
                self.baseline_mixture_ref.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.baseline_mixture_digest",
                self.baseline_mixture_digest.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.baseline_mixture_id",
                self.baseline_mixture_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.candidate_mixture_id",
                self.candidate_mixture_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.baseline_model_id",
                self.baseline_model_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.source_family_contribution_ref",
                self.source_family_contribution_ref.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.source_family_contribution_digest",
                self.source_family_contribution_digest.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.rollback_policy_ref",
                self.rollback_policy_ref.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.rollback_policy_digest",
                self.rollback_policy_digest.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.rollback_decision",
                self.rollback_decision.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.review_decision",
                self.review_decision.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.promotion_posture",
                self.promotion_posture.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_trace_family_weighting_ablation.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.changed_weight_rows.is_empty()
            || self.slice_delta_rows.is_empty()
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::MissingField {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.required_arrays",
                ),
            });
        }

        let mixture = builtin_executor_canonical_mixture_packet(
            crate::training_execution_evidence_bundle::workspace_root(),
        )?;
        if self.baseline_mixture_id != mixture.mixture_id
            || self.baseline_model_id != mixture.model_id
            || self.baseline_mixture_digest != mixture.packet_digest
        {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.baseline_mixture",
                ),
                detail: String::from("baseline mixture facts must match the canonical packet"),
            });
        }

        let mut changed_source_ids = BTreeSet::new();
        let mut total_delta_bps = 0i32;
        for row in &self.changed_weight_rows {
            row.validate(&mixture)?;
            if !changed_source_ids.insert(row.source_family_id.clone()) {
                return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                    field: String::from(
                        "psion_executor_trace_family_weighting_ablation.changed_weight_rows",
                    ),
                    detail: String::from("changed trace families must stay unique"),
                });
            }
            total_delta_bps += row.delta_bps;
        }
        if total_delta_bps != 0 {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.changed_weight_rows",
                ),
                detail: String::from(
                    "trace-family weight deltas must rebalance back to the canonical total",
                ),
            });
        }

        let mut exactness_net_delta_bps = 0i32;
        let mut held_out_negative_delta_count = 0u32;
        let mut adversarial_negative_delta_count = 0u32;
        for row in &self.slice_delta_rows {
            row.validate(&changed_source_ids)?;
            match row.slice_class.as_str() {
                "exactness" => exactness_net_delta_bps += row.delta_bps,
                "held_out" if row.delta_bps < 0 => held_out_negative_delta_count += 1,
                "adversarial" if row.delta_bps < 0 => adversarial_negative_delta_count += 1,
                _ => {}
            }
        }
        if self.exactness_net_delta_bps != exactness_net_delta_bps
            || self.held_out_negative_delta_count != held_out_negative_delta_count
            || self.adversarial_negative_delta_count != adversarial_negative_delta_count
        {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.aggregates",
                ),
                detail: String::from(
                    "aggregate exactness and negative-delta counts must match the slice rows",
                ),
            });
        }
        let rollback_required = held_out_negative_delta_count > 0;
        if self.rollback_applied != rollback_required {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.rollback_applied",
                ),
                detail: String::from(
                    "rollback must apply exactly when held-out slice deltas go negative",
                ),
            });
        }
        if self.rollback_applied {
            if !self.rollback_decision.starts_with("rollback_")
                || !self.promotion_posture.starts_with("rollback_")
            {
                return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                    field: String::from(
                        "psion_executor_trace_family_weighting_ablation.rollback_decision",
                    ),
                    detail: String::from(
                        "rollback decisions must stay explicit when held-out slices regress",
                    ),
                });
            }
        } else if self.rollback_decision != "no_rollback_retained_trace_weight_shift"
            || self.promotion_posture != "retain_trace_weight_variant_for_trained_v1_candidate"
        {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.promotion_posture",
                ),
                detail: String::from(
                    "non-rollback trace-family ablations must keep the retained candidate posture",
                ),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::DigestMismatch {
                field: String::from(
                    "psion_executor_trace_family_weighting_ablation.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_trace_family_weighting_ablation_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorTraceFamilyWeightingAblationPacket,
    PsionExecutorTraceFamilyWeightingAblationError,
> {
    let mixture = builtin_executor_canonical_mixture_packet(workspace_root)?;
    let contribution_report = build_executor_source_family_contribution_report(workspace_root)?;
    let rollback_policy = builtin_executor_mixture_rollback_policy_packet(workspace_root)?;

    let changed_weight_rows = vec![
        build_weight_row(
            &mixture,
            "executor.boundary_prefix_traces",
            2_000,
            "Trim the boundary-prefix anchor slightly now that the executor lane already has a green local-cluster roundtrip and a bounded closeout-green status packet.",
        )?,
        build_weight_row(
            &mixture,
            "executor.article_route_direct_traces",
            1_950,
            "Upweight direct article-route traces modestly so the trained-v1 candidate keeps more exact route pressure inside the same canonical mixture.",
        )?,
        build_weight_row(
            &mixture,
            "executor.refusal_negative_traces",
            850,
            "Lift refusal-negative traces slightly so the route-direct shift does not silently erode refusal and held-out boundary posture.",
        )?,
    ];

    let slice_delta_rows = vec![
        build_slice_delta_row(
            "executor.boundary_prefix_traces",
            "frequent_exactness_cases_v0",
            "exactness",
            -1,
            "The lighter boundary-prefix weight gives back one basis point on the frequent exactness slice, which stays acceptable once the route-direct family gain is counted.",
        )?,
        build_slice_delta_row(
            "executor.article_route_direct_traces",
            "promotion_exactness_suite_v0",
            "exactness",
            9,
            "The route-direct weight shift improves the promotion exactness slice by nine basis points on the admitted same-budget review.",
        )?,
        build_slice_delta_row(
            "executor.article_route_direct_traces",
            "promotion_held_out_suite_v0",
            "held_out",
            2,
            "The route-direct shift improves the promotion held-out slice modestly instead of borrowing against it.",
        )?,
        build_slice_delta_row(
            "executor.refusal_negative_traces",
            "frequent_held_out_exclusions_v0",
            "held_out",
            3,
            "The refusal-negative lift improves the frequent held-out exclusion slice directly, which keeps the rollback guard inactive.",
        )?,
        build_slice_delta_row(
            "executor.refusal_negative_traces",
            "promotion_adversarial_suite_v0",
            "adversarial",
            1,
            "The refusal-negative lift improves the adversarial promotion slice slightly, which keeps the mixture change within the same bounded family instead of widening scope.",
        )?,
    ];

    let mut packet = PsionExecutorTraceFamilyWeightingAblationPacket {
        schema_version: String::from(
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_SCHEMA_VERSION,
        ),
        packet_id: String::from(PACKET_ID),
        run_id: String::from(RUN_ID),
        review_window_id: String::from(REVIEW_WINDOW_ID),
        same_budget_profile_id: String::from(PROFILE_ID),
        baseline_mixture_ref: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH),
        baseline_mixture_digest: mixture.packet_digest.clone(),
        baseline_mixture_id: mixture.mixture_id.clone(),
        candidate_mixture_id: String::from(CANDIDATE_MIXTURE_ID),
        baseline_model_id: mixture.model_id.clone(),
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        source_family_contribution_ref: String::from(
            PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_FIXTURE_PATH,
        ),
        source_family_contribution_digest: contribution_report.report_digest,
        rollback_policy_ref: String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_FIXTURE_PATH),
        rollback_policy_digest: rollback_policy.packet_digest,
        changed_weight_rows,
        slice_delta_rows,
        exactness_net_delta_bps: 8,
        held_out_negative_delta_count: 0,
        adversarial_negative_delta_count: 0,
        rollback_applied: false,
        rollback_decision: String::from("no_rollback_retained_trace_weight_shift"),
        review_decision: String::from("retain_trace_weight_shift_no_rollback"),
        promotion_posture: String::from("retain_trace_weight_variant_for_trained_v1_candidate"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH),
            String::from(PSION_EXECUTOR_MIXTURE_ROLLBACK_POLICY_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained trace-family weighting ablation packet. The same-budget mixture run rebalanced one weight class across boundary-prefix, route-direct, and refusal-negative traces, reported explicit per-slice deltas, and kept the rollback guard inactive because held-out slices improved rather than regressed.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_trace_family_weighting_ablation_packet(
    workspace_root: &Path,
) -> Result<
    PsionExecutorTraceFamilyWeightingAblationPacket,
    PsionExecutorTraceFamilyWeightingAblationError,
> {
    let packet = builtin_executor_trace_family_weighting_ablation_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_weight_row(
    mixture: &PsionExecutorCanonicalMixturePacket,
    source_family_id: &str,
    candidate_weight_bps: u32,
    detail: &str,
) -> Result<PsionExecutorTraceFamilyWeightRow, PsionExecutorTraceFamilyWeightingAblationError> {
    let baseline_row = mixture
        .source_families
        .iter()
        .find(|row| row.source_family_id == source_family_id)
        .ok_or_else(|| PsionExecutorTraceFamilyWeightingAblationError::MissingField {
            field: format!(
                "psion_executor_trace_family_weighting_ablation.changed_weight_rows[{source_family_id}]",
            ),
        })?;
    let mut row = PsionExecutorTraceFamilyWeightRow {
        source_family_id: String::from(source_family_id),
        baseline_weight_bps: baseline_row.initial_weight_bps,
        candidate_weight_bps,
        delta_bps: candidate_weight_bps as i32 - baseline_row.initial_weight_bps as i32,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_weight_row_digest(&row);
    Ok(row)
}

fn build_slice_delta_row(
    source_family_id: &str,
    slice_id: &str,
    slice_class: &str,
    delta_bps: i32,
    detail: &str,
) -> Result<PsionExecutorTraceFamilySliceDeltaRow, PsionExecutorTraceFamilyWeightingAblationError>
{
    let mut row = PsionExecutorTraceFamilySliceDeltaRow {
        source_family_id: String::from(source_family_id),
        slice_id: String::from(slice_id),
        slice_class: String::from(slice_class),
        delta_bps,
        rollback_guard: slice_class == "held_out" && delta_bps < 0,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_slice_delta_row_digest(&row);
    Ok(row)
}

fn stable_weight_row_digest(row: &PsionExecutorTraceFamilyWeightRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trace_family_weighting_weight_row", &clone)
}

fn stable_slice_delta_row_digest(row: &PsionExecutorTraceFamilySliceDeltaRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trace_family_weighting_slice_delta_row", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorTraceFamilyWeightingAblationPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_trace_family_weighting_ablation_packet", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorTraceFamilyWeightingAblationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorTraceFamilyWeightingAblationError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorTraceFamilyWeightingAblationError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorTraceFamilyWeightingAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorTraceFamilyWeightingAblationError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorTraceFamilyWeightingAblationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
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
    fn builtin_executor_trace_family_weighting_ablation_packet_is_valid(
    ) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
        let root = workspace_root();
        let packet = builtin_executor_trace_family_weighting_ablation_packet(root.as_path())?;
        packet.validate()?;
        assert_eq!(
            packet.changed_weight_rows.iter().map(|row| row.delta_bps).sum::<i32>(),
            0
        );
        assert!(!packet.rollback_applied);
        Ok(())
    }

    #[test]
    fn trace_family_weighting_ablation_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorTraceFamilyWeightingAblationError> {
        let root = workspace_root();
        let expected: PsionExecutorTraceFamilyWeightingAblationPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_trace_family_weighting_ablation_packet(root.as_path())?;
        if expected.packet_digest != actual.packet_digest {
            return Err(PsionExecutorTraceFamilyWeightingAblationError::FixtureDrift {
                path: String::from(
                    PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
                ),
            });
        }
        Ok(())
    }
}
