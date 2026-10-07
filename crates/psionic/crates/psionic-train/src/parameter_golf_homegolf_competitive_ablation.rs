use std::{fs, path::Path};

use psionic_models::ParameterGolfConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_parameter_golf_homegolf_track_contract_report,
    parameter_golf_default_validation_batch_sequences, ParameterGolfBatchGeometry,
    ParameterGolfEmaConfig, ParameterGolfFinalArtifactConfig, ParameterGolfFinalModelSurface,
    ParameterGolfScoreFirstTttConfig, ParameterGolfSingleH100ModelVariant,
    ParameterGolfSingleH100TrainingConfig, ParameterGolfSwaConfig, ParameterGolfValidationEvalMode,
};

pub const PARAMETER_GOLF_HOMEGOLF_COMPETITIVE_ABLATION_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_competitive_ablation.json";
pub const PARAMETER_GOLF_HOMEGOLF_COMPETITIVE_ABLATION_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-competitive-ablation.sh";
pub const PARAMETER_GOLF_HOMEGOLF_COMPETITIVE_ABLATION_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-competitive-ablation-audit.md";

const HOMEGOLF_TRACK_ID: &str = "parameter_golf.home_cluster_compatible_10min.v1";
const HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
const HOMEGOLF_STRICT_CHALLENGE_LANE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_strict_challenge_lane.json";
const UPSTREAM_OPPORTUNITY_AUDIT_REF: &str =
    "docs/audits/2026-03-22-parameter-golf-upstream-leaderboard-and-pr-opportunity-audit.md";
const STRICT_DATASET_ROOT_SHELL_PATH: &str =
    "~/code/parameter-golf/data/datasets/fineweb10B_sp1024";
const STRICT_TOKENIZER_SHELL_PATH: &str =
    "~/code/parameter-golf/data/tokenizers/fineweb_1024_bpe.model";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfCompetitiveAblationError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid HOMEGOLF competitive ablation report: {detail}")]
    InvalidReport { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfCompetitiveTechniqueStatus {
    WiredInBestKnownVariant,
    AvailableButNotSelected,
    NotYetAdmitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfCompetitiveSignal {
    PositivePublicSignal,
    MixedPublicSignal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfCompetitiveTechniqueRow {
    pub technique_id: String,
    pub status: ParameterGolfHomegolfCompetitiveTechniqueStatus,
    pub public_signal: ParameterGolfHomegolfCompetitiveSignal,
    pub detail: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfCompetitiveLane {
    pub lane_id: String,
    pub model_variant: ParameterGolfSingleH100ModelVariant,
    pub model_config: ParameterGolfConfig,
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    pub validation_batch_sequences: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_first_ttt: Option<ParameterGolfScoreFirstTttConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ema: Option<ParameterGolfEmaConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swa: Option<ParameterGolfSwaConfig>,
    pub final_model_surface: ParameterGolfFinalModelSurface,
    pub final_artifact_config: ParameterGolfFinalArtifactConfig,
    pub dense_training_command_template: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfCompetitiveAblationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub source_track_contract_ref: String,
    pub source_strict_challenge_lane_ref: String,
    pub source_upstream_opportunity_audit_ref: String,
    pub baseline_lane: ParameterGolfHomegolfCompetitiveLane,
    pub best_known_lane: ParameterGolfHomegolfCompetitiveLane,
    pub technique_rows: Vec<ParameterGolfHomegolfCompetitiveTechniqueRow>,
    pub best_known_variant_replaces_bounded_proof_baseline: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl ParameterGolfHomegolfCompetitiveAblationReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_competitive_ablation|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfCompetitiveAblationError> {
        if self.schema_version != 1 {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: format!("schema_version must stay 1 but was {}", self.schema_version),
                },
            );
        }
        if self.track_id != HOMEGOLF_TRACK_ID {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from("track_id drifted"),
                },
            );
        }
        if self.best_known_lane.model_variant
            != ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1
        {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from(
                        "best_known_lane must remain the competitive HOMEGOLF variant",
                    ),
                },
            );
        }
        if self.best_known_lane.score_first_ttt.is_none()
            || self.best_known_lane.ema.is_none()
            || self.best_known_lane.swa.is_none()
        {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from(
                        "best_known_lane must keep score-first TTT, EMA, and SWA enabled",
                    ),
                },
            );
        }
        if self.best_known_lane.final_artifact_config
            != ParameterGolfFinalArtifactConfig::competitive_defaults()
        {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from("best_known_lane final_artifact_config drifted"),
                },
            );
        }
        if !self.technique_rows.iter().any(|row| {
            row.technique_id == "bigram_hash"
                && row.status
                    == ParameterGolfHomegolfCompetitiveTechniqueStatus::WiredInBestKnownVariant
        }) {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from("bigram_hash must remain wired in the best-known lane"),
                },
            );
        }
        if self.report_digest != self.stable_digest() {
            return Err(
                ParameterGolfHomegolfCompetitiveAblationError::InvalidReport {
                    detail: String::from("report_digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_competitive_ablation_report() -> Result<
    ParameterGolfHomegolfCompetitiveAblationReport,
    ParameterGolfHomegolfCompetitiveAblationError,
> {
    let track_contract = build_parameter_golf_homegolf_track_contract_report();
    let baseline_config = ParameterGolfSingleH100TrainingConfig::challenge_defaults(
        STRICT_DATASET_ROOT_SHELL_PATH,
        STRICT_TOKENIZER_SHELL_PATH,
    );
    let competitive_config =
        ParameterGolfSingleH100TrainingConfig::challenge_competitive_homegolf_v1_defaults(
            STRICT_DATASET_ROOT_SHELL_PATH,
            STRICT_TOKENIZER_SHELL_PATH,
        );

    let baseline_lane = lane_from_config("baseline_exact_challenge", &baseline_config);
    let best_known_lane = lane_from_config("competitive_homegolf_v1", &competitive_config);

    let technique_rows = vec![
        wired_row(
            "bigram_hash",
            "The exact HOMEGOLF lane now selects hashed bigram context features directly in the best-known competitive variant.",
            &[
                "crates/psionic-models/src/parameter_golf.rs",
                "crates/psionic-train/src/parameter_golf_single_h100_training.rs",
            ],
        ),
        wired_row(
            "deep_layer_xsa",
            "The best-known competitive HOMEGOLF variant now keeps deep-layer XSA on the exact trainer config instead of only inside general family capability docs.",
            &[
                "crates/psionic-models/src/parameter_golf.rs",
                "fixtures/parameter_golf/reports/parameter_golf_homegolf_competitive_ablation.json",
            ],
        ),
        wired_row(
            "partial_rope",
            "The best-known competitive HOMEGOLF variant now uses an explicit partial RoPE rotary sub-dimension.",
            &["crates/psionic-models/src/parameter_golf.rs"],
        ),
        wired_row(
            "leaky_relu_squared_point_five",
            "The exact HOMEGOLF lane now selects LeakyReLU(0.5)^2 in the best-known competitive variant.",
            &["crates/psionic-models/src/parameter_golf.rs"],
        ),
        wired_row(
            "late_layer_value_embeddings",
            "Late-layer value embeddings are now part of the best-known exact HOMEGOLF competitive lane.",
            &["crates/psionic-models/src/parameter_golf.rs"],
        ),
        wired_row(
            "ema",
            "EMA is now explicitly enabled on the best-known exact HOMEGOLF competitive lane.",
            &[
                "crates/psionic-train/src/parameter_golf_single_h100_training.rs",
                "docs/audits/2026-03-22-parameter-golf-upstream-leaderboard-and-pr-opportunity-audit.md",
            ],
        ),
        wired_row(
            "swa",
            "SWA is now explicitly enabled on the best-known exact HOMEGOLF competitive lane and consumes EMA snapshots.",
            &["crates/psionic-train/src/parameter_golf_single_h100_training.rs"],
        ),
        wired_row(
            "parameter_banking_plus_muon",
            "The competitive HOMEGOLF lane reuses the existing Parameter Banking plus Muon optimizer grouping already owned by the exact trainer surface.",
            &[
                "crates/psionic-train/src/parameter_golf.rs",
                "docs/ROADMAP_PARAMETERGOLF.md",
            ],
        ),
        wired_row(
            "legal_score_first_ttt",
            "The best-known exact HOMEGOLF competitive lane now keeps legal score-first TTT enabled under the strict challenge overlay.",
            &[
                "crates/psionic-train/src/parameter_golf_single_h100_training.rs",
                "crates/psionic-train/src/parameter_golf_homegolf_strict_challenge.rs",
            ],
        ),
        wired_row(
            "int6_gptq_lite_export",
            "The best-known competitive lane now selects the existing int6 GPTQ-lite plus zstd export surface instead of the baseline int8-zlib export.",
            &[
                "crates/psionic-train/src/parameter_golf_reference.rs",
                "crates/psionic-train/src/parameter_golf_single_h100_training.rs",
            ],
        ),
        refused_row(
            "smeargate",
            "SmearGate is still not admitted into the exact HOMEGOLF lane because the current promoted PGOLF family contract does not expose that residual-control surface yet.",
        ),
        refused_row(
            "mixed_qat_train_time",
            "Mixed-bit QAT remains out of the exact HOMEGOLF lane for now. The current exact trainer owns competitive roundtrip export, but not yet one train-time mixed-bit QAT contract.",
        ),
    ];

    let mut report = ParameterGolfHomegolfCompetitiveAblationReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_competitive_ablation.v1"),
        track_id: track_contract.track_id,
        source_track_contract_ref: String::from(HOMEGOLF_TRACK_CONTRACT_REF),
        source_strict_challenge_lane_ref: String::from(HOMEGOLF_STRICT_CHALLENGE_LANE_REF),
        source_upstream_opportunity_audit_ref: String::from(UPSTREAM_OPPORTUNITY_AUDIT_REF),
        baseline_lane,
        best_known_lane,
        technique_rows,
        best_known_variant_replaces_bounded_proof_baseline: true,
        claim_boundary: String::from(
            "This report closes the technique-porting and exact-lane wiring gap for HOMEGOLF. It freezes one real competitive exact-lane variant built only from already-admitted family and trainer surfaces, and it records which public-winning techniques are now wired versus still explicitly out of scope. It does not yet claim that this best-known variant has already posted a retained exact-challenge score on local hardware.",
        ),
        summary: String::from(
            "The best-known exact HOMEGOLF configuration is no longer the naive baseline proof shape. The exact trainer now admits one competitive HOMEGOLF variant with BigramHash, partial RoPE, deep-layer XSA, LeakyReLU(0.5)^2, VE, EMA, SWA, legal score-first TTT, and competitive export defaults, while explicitly refusing the remaining not-yet-admitted frontier tricks.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_competitive_ablation_report(
    output_path: &Path,
) -> Result<
    ParameterGolfHomegolfCompetitiveAblationReport,
    ParameterGolfHomegolfCompetitiveAblationError,
> {
    let report = build_parameter_golf_homegolf_competitive_ablation_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfCompetitiveAblationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfCompetitiveAblationError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn lane_from_config(
    lane_id: &str,
    config: &ParameterGolfSingleH100TrainingConfig,
) -> ParameterGolfHomegolfCompetitiveLane {
    let validation_batch_sequences = parameter_golf_default_validation_batch_sequences(
        &ParameterGolfBatchGeometry::challenge_single_device_defaults(),
        &config.validation_eval_mode,
    );
    let model_variant_label = config.model_variant.as_str();
    ParameterGolfHomegolfCompetitiveLane {
        lane_id: String::from(lane_id),
        model_variant: config.model_variant,
        model_config: config.model_config.clone(),
        validation_eval_mode: config.validation_eval_mode.clone(),
        validation_batch_sequences,
        score_first_ttt: config.score_first_ttt.clone(),
        ema: config.ema.clone(),
        swa: config.swa.clone(),
        final_model_surface: config.final_model_surface,
        final_artifact_config: config.final_artifact_config.clone(),
        dense_training_command_template: format!(
            "PSIONIC_PARAMETER_GOLF_MODEL_VARIANT={model_variant_label} cargo run -q -p psionic-train --bin parameter_golf_single_h100_train -- {STRICT_DATASET_ROOT_SHELL_PATH} {STRICT_TOKENIZER_SHELL_PATH} <training_report_path> both sliding_window:64 score_first_ttt"
        ),
    }
}

fn wired_row(
    technique_id: &str,
    detail: &str,
    evidence_refs: &[&str],
) -> ParameterGolfHomegolfCompetitiveTechniqueRow {
    ParameterGolfHomegolfCompetitiveTechniqueRow {
        technique_id: String::from(technique_id),
        status: ParameterGolfHomegolfCompetitiveTechniqueStatus::WiredInBestKnownVariant,
        public_signal: ParameterGolfHomegolfCompetitiveSignal::PositivePublicSignal,
        detail: String::from(detail),
        evidence_refs: evidence_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
    }
}

fn refused_row(technique_id: &str, detail: &str) -> ParameterGolfHomegolfCompetitiveTechniqueRow {
    ParameterGolfHomegolfCompetitiveTechniqueRow {
        technique_id: String::from(technique_id),
        status: ParameterGolfHomegolfCompetitiveTechniqueStatus::NotYetAdmitted,
        public_signal: ParameterGolfHomegolfCompetitiveSignal::MixedPublicSignal,
        detail: String::from(detail),
        evidence_refs: vec![String::from(UPSTREAM_OPPORTUNITY_AUDIT_REF)],
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect("HOMEGOLF competitive ablation should serialize"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_competitive_ablation_report,
        write_parameter_golf_homegolf_competitive_ablation_report,
        ParameterGolfHomegolfCompetitiveTechniqueStatus,
        PARAMETER_GOLF_HOMEGOLF_COMPETITIVE_ABLATION_REPORT_REF,
    };
    use crate::ParameterGolfSingleH100ModelVariant;

    #[test]
    fn competitive_ablation_keeps_best_known_competitive_variant() {
        let report =
            build_parameter_golf_homegolf_competitive_ablation_report().expect("build report");
        assert_eq!(
            report.best_known_lane.model_variant,
            ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1
        );
        assert!(report.best_known_lane.score_first_ttt.is_some());
        assert!(report.best_known_lane.ema.is_some());
        assert!(report.best_known_lane.swa.is_some());
        assert!(report.technique_rows.iter().any(|row| {
            row.technique_id == "bigram_hash"
                && row.status
                    == ParameterGolfHomegolfCompetitiveTechniqueStatus::WiredInBestKnownVariant
        }));
    }

    #[test]
    fn write_competitive_ablation_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_competitive_ablation.json");
        let written = write_parameter_golf_homegolf_competitive_ablation_report(path.as_path())
            .expect("write report");
        let encoded = std::fs::read(path.as_path()).expect("read report");
        let decoded: super::ParameterGolfHomegolfCompetitiveAblationReport =
            serde_json::from_slice(&encoded).expect("decode report");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_competitive_ablation_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_COMPETITIVE_ABLATION_REPORT_REF);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfCompetitiveAblationReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        decoded.validate().expect("validate fixture");
    }
}
