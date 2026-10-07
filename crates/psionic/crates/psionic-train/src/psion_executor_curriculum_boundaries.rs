use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_canonical_mixture_packet, PsionExecutorCanonicalMixtureError,
    PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_CURRICULUM_BOUNDARIES_SCHEMA_VERSION: &str =
    "psion.executor.curriculum_boundaries.v1";
pub const PSION_EXECUTOR_CURRICULUM_BOUNDARIES_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_curriculum_boundaries_v1.json";
pub const PSION_EXECUTOR_CURRICULUM_BOUNDARIES_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_CURRICULUM_BOUNDARIES.md";

const CURRICULUM_ID: &str = "psion_executor_curriculum_boundaries_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_DECISION_THRESHOLDS.md";
const PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_CANONICAL_MIXTURE_V0.md";

#[derive(Debug, Error)]
pub enum PsionExecutorCurriculumBoundariesError {
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorCurriculumStageBoundary {
    pub stage_id: String,
    pub sequence_index: u32,
    pub target_window_tokens: u32,
    pub dominant_source_family_id: String,
    pub certified_pack_ids: Vec<String>,
    pub required_suite_ids: Vec<String>,
    pub transition_rule: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorCurriculumBoundariesPacket {
    pub schema_version: String,
    pub curriculum_id: String,
    pub mixture_ref: String,
    pub mixture_digest: String,
    pub stage_boundaries: Vec<PsionExecutorCurriculumStageBoundary>,
    pub transition_policy: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorCurriculumStageBoundary {
    fn validate(&self) -> Result<(), PsionExecutorCurriculumBoundariesError> {
        for (field, value) in [
            (
                "psion_executor_curriculum_boundaries.stage_boundaries[].stage_id",
                self.stage_id.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.stage_boundaries[].dominant_source_family_id",
                self.dominant_source_family_id.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.stage_boundaries[].transition_rule",
                self.transition_rule.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.stage_boundaries[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.stage_boundaries[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.sequence_index == 0 || self.target_window_tokens == 0 {
            return Err(PsionExecutorCurriculumBoundariesError::InvalidValue {
                field: String::from("psion_executor_curriculum_boundaries.stage_boundaries[].numeric"),
                detail: String::from("stage boundaries must keep positive sequence and target-window values"),
            });
        }
        if self.certified_pack_ids.is_empty() || self.required_suite_ids.is_empty() {
            return Err(PsionExecutorCurriculumBoundariesError::MissingField {
                field: String::from("psion_executor_curriculum_boundaries.stage_boundaries[].required_arrays"),
            });
        }
        if stable_stage_digest(self) != self.row_digest {
            return Err(PsionExecutorCurriculumBoundariesError::DigestMismatch {
                field: String::from("psion_executor_curriculum_boundaries.stage_boundaries[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorCurriculumBoundariesPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorCurriculumBoundariesError> {
        if self.schema_version != PSION_EXECUTOR_CURRICULUM_BOUNDARIES_SCHEMA_VERSION {
            return Err(PsionExecutorCurriculumBoundariesError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_CURRICULUM_BOUNDARIES_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_curriculum_boundaries.curriculum_id",
                self.curriculum_id.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.mixture_ref",
                self.mixture_ref.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.mixture_digest",
                self.mixture_digest.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.transition_policy",
                self.transition_policy.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_curriculum_boundaries.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.stage_boundaries.is_empty() || self.support_refs.is_empty() {
            return Err(PsionExecutorCurriculumBoundariesError::MissingField {
                field: String::from("psion_executor_curriculum_boundaries.required_arrays"),
            });
        }
        let mut expected_sequence = 1u32;
        for row in &self.stage_boundaries {
            row.validate()?;
            if row.sequence_index != expected_sequence {
                return Err(PsionExecutorCurriculumBoundariesError::InvalidValue {
                    field: String::from("psion_executor_curriculum_boundaries.stage_boundaries[].sequence_index"),
                    detail: format!(
                        "expected contiguous stage ordering starting at 1, found {} at expected slot {expected_sequence}",
                        row.sequence_index
                    ),
                });
            }
            expected_sequence += 1;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorCurriculumBoundariesError::DigestMismatch {
                field: String::from("psion_executor_curriculum_boundaries.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn build_executor_curriculum_boundaries_packet(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorCurriculumBoundariesPacket, PsionExecutorCurriculumBoundariesError> {
    let mixture = builtin_executor_canonical_mixture_packet(workspace_root)?;
    let stage_boundaries = vec![
        stage(
            "boundary_anchor_32",
            1,
            32,
            "executor.boundary_prefix_traces",
            vec!["tassadar.eval.frequent.v0"],
            vec![
                "frequent_exactness_cases_v0",
                "frequent_held_out_exclusions_v0",
            ],
            "Advance only after two retained runs keep frequent exactness saturated and frequent held-out exclusions green at the 32-token anchor.",
            "The first stage keeps the curriculum anchored to the same `MAX_TARGET_WINDOW_TOKENS=32` boundary that the canonical mixture manifest fixes as the executor lane's first hard closure target.",
        ),
        stage(
            "frequent_pack_certification",
            2,
            128,
            "executor.article_route_direct_traces",
            vec!["tassadar.eval.frequent.v0"],
            vec![
                "frequent_exactness_cases_v0",
                "frequent_held_out_exclusions_v0",
                "frequent_operator_review_cases_v0",
                "frequent_throughput_blockers_v0",
            ],
            "Advance only after the full frequent pack stays green with no throughput-blocker or operator-review regressions on the admitted local profiles.",
            "The second stage widens beyond the boundary anchor only once the entire frequent pack certifies route, refusal, held-out exclusion, and throughput posture together.",
        ),
        stage(
            "promotion_pack_certification",
            3,
            512,
            "executor.long_loop_kernel_traces",
            vec!["tassadar.eval.frequent.v0", "tassadar.eval.promotion.v0"],
            vec![
                "promotion_exactness_suite_v0",
                "promotion_held_out_suite_v0",
                "promotion_adversarial_suite_v0",
                "promotion_runtime_blockers_v0",
                "promotion_serving_blockers_v0",
                "promotion_reference_linear_anchor_checks_v0",
                "promotion_hull_cache_fast_route_checks_v0",
            ],
            "Stay in the terminal stage until promotion exactness, held-out, and adversarial regressions remain zero, runtime and serving blockers remain green, and the `reference_linear` / `hull_cache` checks remain within the retained decision thresholds.",
            "The final stage is performance-driven by the certified promotion pack instead of intuition. It is the only stage where a same-budget candidate may become promotion-eligible.",
        ),
    ];
    let mut packet = PsionExecutorCurriculumBoundariesPacket {
        schema_version: String::from(PSION_EXECUTOR_CURRICULUM_BOUNDARIES_SCHEMA_VERSION),
        curriculum_id: String::from(CURRICULUM_ID),
        mixture_ref: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH),
        mixture_digest: mixture.packet_digest,
        stage_boundaries,
        transition_policy: String::from(
            "Every stage transition is pack-certified. Train-looking loss or replay gains do not advance the executor curriculum unless the certified required-suite set for the current stage stays green under the frozen frequent/promotion packs.",
        ),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one canonical stagewise curriculum packet. Boundary-anchor, frequent-pack, and promotion-pack transitions are now explicit, performance-driven, and certified against the same frozen pack surfaces that already govern the lane.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_executor_curriculum_boundaries_fixture(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorCurriculumBoundariesPacket, PsionExecutorCurriculumBoundariesError> {
    let workspace_root = workspace_root.as_ref();
    let packet = build_executor_curriculum_boundaries_packet(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_CURRICULUM_BOUNDARIES_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorCurriculumBoundariesError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&packet)?;
    fs::write(&fixture_path, format!("{json}\n")).map_err(|error| {
        PsionExecutorCurriculumBoundariesError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

pub fn builtin_executor_curriculum_boundaries_packet(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorCurriculumBoundariesPacket, PsionExecutorCurriculumBoundariesError> {
    let workspace_root = workspace_root.as_ref();
    let fixture_path = workspace_root.join(PSION_EXECUTOR_CURRICULUM_BOUNDARIES_FIXTURE_PATH);
    let bytes = fs::read(&fixture_path).map_err(|error| {
        PsionExecutorCurriculumBoundariesError::Read {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    let packet: PsionExecutorCurriculumBoundariesPacket = serde_json::from_slice(&bytes)?;
    packet.validate()?;
    if packet != build_executor_curriculum_boundaries_packet(workspace_root)? {
        return Err(PsionExecutorCurriculumBoundariesError::FixtureDrift {
            path: fixture_path.display().to_string(),
        });
    }
    Ok(packet)
}

fn stage(
    stage_id: &str,
    sequence_index: u32,
    target_window_tokens: u32,
    dominant_source_family_id: &str,
    certified_pack_ids: Vec<&str>,
    required_suite_ids: Vec<&str>,
    transition_rule: &str,
    detail: &str,
) -> PsionExecutorCurriculumStageBoundary {
    let mut row = PsionExecutorCurriculumStageBoundary {
        stage_id: String::from(stage_id),
        sequence_index,
        target_window_tokens,
        dominant_source_family_id: String::from(dominant_source_family_id),
        certified_pack_ids: certified_pack_ids.into_iter().map(String::from).collect(),
        required_suite_ids: required_suite_ids.into_iter().map(String::from).collect(),
        transition_rule: String::from(transition_rule),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_stage_digest(&row);
    row
}

fn stable_stage_digest(row: &PsionExecutorCurriculumStageBoundary) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("curriculum stages serialize"),
    ))
}

fn stable_packet_digest(packet: &PsionExecutorCurriculumBoundariesPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("curriculum packet serializes"),
    ))
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorCurriculumBoundariesError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorCurriculumBoundariesError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curriculum_packet_is_machine_legible() -> Result<(), PsionExecutorCurriculumBoundariesError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = build_executor_curriculum_boundaries_packet(&root)?;
        packet.validate()?;
        assert_eq!(packet.stage_boundaries.len(), 3);
        assert_eq!(packet.stage_boundaries[0].target_window_tokens, 32);
        Ok(())
    }

    #[test]
    fn curriculum_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorCurriculumBoundariesError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_curriculum_boundaries_packet(&root)?;
        assert_eq!(packet.curriculum_id, CURRICULUM_ID);
        Ok(())
    }

    #[test]
    fn curriculum_writer_persists_current_truth(
    ) -> Result<(), PsionExecutorCurriculumBoundariesError> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = write_executor_curriculum_boundaries_fixture(&root)?;
        assert_eq!(
            packet.packet_digest,
            build_executor_curriculum_boundaries_packet(&root)?.packet_digest
        );
        Ok(())
    }
}
