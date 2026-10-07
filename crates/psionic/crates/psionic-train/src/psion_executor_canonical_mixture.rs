use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PSION_EXECUTOR_CANONICAL_MIXTURE_SCHEMA_VERSION: &str =
    "psion.executor.canonical_mixture.v0";
pub const PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_canonical_mixture_v0.json";
pub const PSION_EXECUTOR_CANONICAL_MIXTURE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_CANONICAL_MIXTURE_V0.md";

const MIXTURE_ID: &str = "psion_executor_canonical_mixture_v0";
const CORPUS_ID: &str = "tassadar.executor.local_cluster.canonical_mixture_v0";
const MODEL_ID: &str = "tassadar-article-transformer-trace-bound-trained-v0";
const TASK_FAMILY_ID: &str = "tassadar.executor.admitted_workload_family.v0";
const STAGE_ANCHOR_ID: &str = "executor_boundary_anchor_32.v0";
const MAX_TARGET_WINDOW_TOKENS: u32 = 32;
const FREQUENT_PACK_ID: &str = "tassadar.eval.frequent.v0";
const PROMOTION_PACK_ID: &str = "tassadar.eval.promotion.v0";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_BASELINE_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE_TRUTH.md";

#[derive(Debug, Error)]
pub enum PsionExecutorCanonicalMixtureError {
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorCanonicalMixtureSourceFamily {
    pub source_family_id: String,
    pub role: String,
    pub initial_weight_bps: u32,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorCanonicalMixtureSeedRow {
    pub seed_id: String,
    pub workload_family_id: String,
    pub usage_role: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorCanonicalMixturePacket {
    pub schema_version: String,
    pub mixture_id: String,
    pub corpus_id: String,
    pub model_id: String,
    pub task_family_id: String,
    pub stage_anchor_id: String,
    pub max_target_window_tokens: u32,
    pub frozen_pack_ids: Vec<String>,
    pub source_families: Vec<PsionExecutorCanonicalMixtureSourceFamily>,
    pub seed_suite: Vec<PsionExecutorCanonicalMixtureSeedRow>,
    pub held_out_exclusion_ids: Vec<String>,
    pub evaluation_exclusion_ids: Vec<String>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorCanonicalMixtureSourceFamily {
    fn validate(&self) -> Result<(), PsionExecutorCanonicalMixtureError> {
        for (field, value) in [
            (
                "psion_executor_canonical_mixture.source_families[].source_family_id",
                self.source_family_id.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.source_families[].role",
                self.role.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.source_families[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.source_families[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.initial_weight_bps == 0 {
            return Err(PsionExecutorCanonicalMixtureError::InvalidValue {
                field: String::from(
                    "psion_executor_canonical_mixture.source_families[].initial_weight_bps",
                ),
                detail: String::from("source-family weights must stay positive"),
            });
        }
        if stable_source_family_digest(self) != self.row_digest {
            return Err(PsionExecutorCanonicalMixtureError::DigestMismatch {
                field: String::from(
                    "psion_executor_canonical_mixture.source_families[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorCanonicalMixtureSeedRow {
    fn validate(&self) -> Result<(), PsionExecutorCanonicalMixtureError> {
        for (field, value) in [
            (
                "psion_executor_canonical_mixture.seed_suite[].seed_id",
                self.seed_id.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.seed_suite[].workload_family_id",
                self.workload_family_id.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.seed_suite[].usage_role",
                self.usage_role.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.seed_suite[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.seed_suite[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_seed_row_digest(self) != self.row_digest {
            return Err(PsionExecutorCanonicalMixtureError::DigestMismatch {
                field: String::from("psion_executor_canonical_mixture.seed_suite[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorCanonicalMixturePacket {
    pub fn validate(&self) -> Result<(), PsionExecutorCanonicalMixtureError> {
        if self.schema_version != PSION_EXECUTOR_CANONICAL_MIXTURE_SCHEMA_VERSION {
            return Err(PsionExecutorCanonicalMixtureError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            ("psion_executor_canonical_mixture.mixture_id", self.mixture_id.as_str()),
            ("psion_executor_canonical_mixture.corpus_id", self.corpus_id.as_str()),
            ("psion_executor_canonical_mixture.model_id", self.model_id.as_str()),
            (
                "psion_executor_canonical_mixture.task_family_id",
                self.task_family_id.as_str(),
            ),
            (
                "psion_executor_canonical_mixture.stage_anchor_id",
                self.stage_anchor_id.as_str(),
            ),
            ("psion_executor_canonical_mixture.summary", self.summary.as_str()),
            (
                "psion_executor_canonical_mixture.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.max_target_window_tokens != MAX_TARGET_WINDOW_TOKENS {
            return Err(PsionExecutorCanonicalMixtureError::InvalidValue {
                field: String::from("psion_executor_canonical_mixture.max_target_window_tokens"),
                detail: String::from("canonical executor mixture stays anchored at 32 target tokens"),
            });
        }
        if self.frozen_pack_ids != vec![String::from(FREQUENT_PACK_ID), String::from(PROMOTION_PACK_ID)]
        {
            return Err(PsionExecutorCanonicalMixtureError::InvalidValue {
                field: String::from("psion_executor_canonical_mixture.frozen_pack_ids"),
                detail: String::from("canonical executor mixture must stay tied to the frozen frequent and promotion packs"),
            });
        }
        if self.source_families.is_empty()
            || self.seed_suite.is_empty()
            || self.held_out_exclusion_ids.is_empty()
            || self.evaluation_exclusion_ids.is_empty()
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorCanonicalMixtureError::MissingField {
                field: String::from("psion_executor_canonical_mixture.required_arrays"),
            });
        }
        let mut total_weight_bps = 0u32;
        for row in &self.source_families {
            row.validate()?;
            total_weight_bps = total_weight_bps.saturating_add(row.initial_weight_bps);
        }
        if total_weight_bps != 10_000 {
            return Err(PsionExecutorCanonicalMixtureError::InvalidValue {
                field: String::from("psion_executor_canonical_mixture.source_families"),
                detail: format!("source-family weights must sum to 10000 bps, found {total_weight_bps}"),
            });
        }
        for row in &self.seed_suite {
            row.validate()?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorCanonicalMixtureError::DigestMismatch {
                field: String::from("psion_executor_canonical_mixture.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn build_executor_canonical_mixture_packet() -> PsionExecutorCanonicalMixturePacket {
    let source_families = vec![
        source_family(
            "executor.boundary_prefix_traces",
            "stage_anchor",
            2200,
            "Boundary-prefix traces stay overweighted at the start because the executor lane is still anchored to the first `MAX_TARGET_WINDOW_TOKENS=32` target-window closure rather than broad long-trace fluency.",
        ),
        source_family(
            "executor.article_route_direct_traces",
            "route_exactness",
            1800,
            "Direct article-route traces keep the current bounded executor route visible inside the same mixture instead of treating route behavior as an external prompt-only artifact.",
        ),
        source_family(
            "executor.long_loop_kernel_traces",
            "admitted_workload_exactness",
            2000,
            "Long-loop kernel traces preserve the admitted exactness family that currently anchors the executor lane baseline.",
        ),
        source_family(
            "executor.sudoku_v0_traces",
            "admitted_workload_exactness",
            1600,
            "Sudoku traces keep one combinatorial solver family inside the canonical mixture so later same-budget mixture changes remain tied to certified packs.",
        ),
        source_family(
            "executor.hungarian_matching_traces",
            "admitted_workload_exactness",
            1600,
            "Hungarian matching traces preserve the second structured solver family used by the frozen promotion surface.",
        ),
        source_family(
            "executor.refusal_negative_traces",
            "bounded_refusal",
            800,
            "Negative and refusal traces remain explicit so route-local train wins do not silently degrade refusal posture on unsupported executor asks.",
        ),
    ];
    let seed_suite = vec![
        seed_row(
            "long_loop_kernel",
            "admitted_exactness_family",
            "seed_exactness_anchor",
            "Keeps the long-loop executor family inside the first canonical mixture seed suite.",
        ),
        seed_row(
            "sudoku_v0_test_a",
            "admitted_exactness_family",
            "seed_exactness_anchor",
            "Keeps the bounded Sudoku solver family visible inside the same canonical seed suite.",
        ),
        seed_row(
            "hungarian_matching",
            "admitted_exactness_family",
            "seed_exactness_anchor",
            "Keeps the bounded Hungarian matching family visible inside the same canonical seed suite.",
        ),
        seed_row(
            "article_route_direct_refusal_cluster",
            "bounded_route_and_refusal_family",
            "seed_route_anchor",
            "Keeps direct route and bounded refusal traces visible so the executor mixture does not drift away from the current served route envelope.",
        ),
    ];
    let mut packet = PsionExecutorCanonicalMixturePacket {
        schema_version: String::from(PSION_EXECUTOR_CANONICAL_MIXTURE_SCHEMA_VERSION),
        mixture_id: String::from(MIXTURE_ID),
        corpus_id: String::from(CORPUS_ID),
        model_id: String::from(MODEL_ID),
        task_family_id: String::from(TASK_FAMILY_ID),
        stage_anchor_id: String::from(STAGE_ANCHOR_ID),
        max_target_window_tokens: MAX_TARGET_WINDOW_TOKENS,
        frozen_pack_ids: vec![String::from(FREQUENT_PACK_ID), String::from(PROMOTION_PACK_ID)],
        source_families,
        seed_suite,
        held_out_exclusion_ids: vec![
            String::from("frequent_held_out_exclusions_v0"),
            String::from("promotion_held_out_suite_v0"),
            String::from("promotion_adversarial_suite_v0"),
        ],
        evaluation_exclusion_ids: vec![
            String::from("frequent_exactness_cases_v0"),
            String::from("frequent_held_out_exclusions_v0"),
            String::from("frequent_operator_review_cases_v0"),
            String::from("frequent_throughput_blockers_v0"),
            String::from("promotion_exactness_suite_v0"),
            String::from("promotion_held_out_suite_v0"),
            String::from("promotion_adversarial_suite_v0"),
            String::from("promotion_runtime_blockers_v0"),
            String::from("promotion_serving_blockers_v0"),
            String::from("promotion_reference_linear_anchor_checks_v0"),
            String::from("promotion_hull_cache_fast_route_checks_v0"),
        ],
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one canonical mixture manifest. It keeps the admitted executor family anchored to `MAX_TARGET_WINDOW_TOKENS=32`, explicit route/refusal traces, explicit solver-family weights, and explicit held-out/evaluation exclusions before any mixture-search or curriculum change is reviewed.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet
}

pub fn write_executor_canonical_mixture_fixture(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorCanonicalMixturePacket, PsionExecutorCanonicalMixtureError> {
    let workspace_root = workspace_root.as_ref();
    let packet = build_executor_canonical_mixture_packet();
    packet.validate()?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionExecutorCanonicalMixtureError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(&packet)?;
    fs::write(&fixture_path, format!("{json}\n")).map_err(|error| {
        PsionExecutorCanonicalMixtureError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

pub fn builtin_executor_canonical_mixture_packet(
    workspace_root: impl AsRef<Path>,
) -> Result<PsionExecutorCanonicalMixturePacket, PsionExecutorCanonicalMixtureError> {
    let workspace_root = workspace_root.as_ref();
    let fixture_path = workspace_root.join(PSION_EXECUTOR_CANONICAL_MIXTURE_FIXTURE_PATH);
    let bytes = read_bytes(&fixture_path)?;
    let packet: PsionExecutorCanonicalMixturePacket = serde_json::from_slice(&bytes)?;
    packet.validate()?;
    if packet != build_executor_canonical_mixture_packet() {
        return Err(PsionExecutorCanonicalMixtureError::FixtureDrift {
            path: fixture_path.display().to_string(),
        });
    }
    Ok(packet)
}

fn source_family(
    source_family_id: &str,
    role: &str,
    initial_weight_bps: u32,
    detail: &str,
) -> PsionExecutorCanonicalMixtureSourceFamily {
    let mut row = PsionExecutorCanonicalMixtureSourceFamily {
        source_family_id: String::from(source_family_id),
        role: String::from(role),
        initial_weight_bps,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_source_family_digest(&row);
    row
}

fn seed_row(
    seed_id: &str,
    workload_family_id: &str,
    usage_role: &str,
    detail: &str,
) -> PsionExecutorCanonicalMixtureSeedRow {
    let mut row = PsionExecutorCanonicalMixtureSeedRow {
        seed_id: String::from(seed_id),
        workload_family_id: String::from(workload_family_id),
        usage_role: String::from(usage_role),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_seed_row_digest(&row);
    row
}

fn stable_source_family_digest(row: &PsionExecutorCanonicalMixtureSourceFamily) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    let json = serde_json::to_vec(&clone).expect("source-family rows serialize");
    stable_digest(b"psion_executor_canonical_mixture_source_family|", &json)
}

fn stable_seed_row_digest(row: &PsionExecutorCanonicalMixtureSeedRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    let json = serde_json::to_vec(&clone).expect("seed rows serialize");
    stable_digest(b"psion_executor_canonical_mixture_seed_row|", &json)
}

fn stable_packet_digest(packet: &PsionExecutorCanonicalMixturePacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    let json = serde_json::to_vec(&clone).expect("canonical mixture packet serializes");
    stable_digest(b"psion_executor_canonical_mixture_packet|", &json)
}

fn stable_digest(prefix: &[u8], json: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(json);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorCanonicalMixtureError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorCanonicalMixtureError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, PsionExecutorCanonicalMixtureError> {
    fs::read(path).map_err(|error| PsionExecutorCanonicalMixtureError::Read {
        path: path.display().to_string(),
        error,
    })
}

#[allow(dead_code)]
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, PsionExecutorCanonicalMixtureError> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorCanonicalMixtureError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_mixture_packet_is_machine_legible() {
        let packet = build_executor_canonical_mixture_packet();
        packet.validate().expect("packet should validate");
        assert_eq!(packet.max_target_window_tokens, 32);
        assert_eq!(
            packet
                .source_families
                .iter()
                .map(|row| row.initial_weight_bps)
                .sum::<u32>(),
            10_000
        );
    }

    #[test]
    fn canonical_mixture_fixture_matches_committed_truth() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = builtin_executor_canonical_mixture_packet(&root).expect("fixture should load");
        assert_eq!(packet.mixture_id, MIXTURE_ID);
        assert_eq!(packet.held_out_exclusion_ids.len(), 3);
    }

    #[test]
    fn canonical_mixture_writer_persists_current_truth() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet = write_executor_canonical_mixture_fixture(&root).expect("fixture should write");
        assert_eq!(packet.packet_digest, build_executor_canonical_mixture_packet().packet_digest);
    }
}
