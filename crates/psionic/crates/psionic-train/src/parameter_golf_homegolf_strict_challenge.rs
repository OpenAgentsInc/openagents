use std::{
    env, fs,
    path::{Path, PathBuf},
};

use psionic_core::{PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_parameter_golf_homegolf_track_contract_report,
    parameter_golf_default_validation_batch_sequences, ParameterGolfHomegolfTrackContractReport,
    ParameterGolfPromotedTrainingProfile, ParameterGolfScoreFirstTttConfig,
    ParameterGolfSingleH100ValidationMode, ParameterGolfValidationEvalMode,
};

pub const PARAMETER_GOLF_HOMEGOLF_STRICT_CHALLENGE_LANE_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_strict_challenge_lane.json";
pub const PARAMETER_GOLF_HOMEGOLF_STRICT_CHALLENGE_LANE_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-strict-challenge-lane.sh";
pub const PARAMETER_GOLF_HOMEGOLF_STRICT_CHALLENGE_LANE_AUDIT: &str =
    "docs/audits/2026-03-28-homegolf-strict-preflight-semantics-and-local-entrypoint-audit.md";

const PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
const STRICT_DATASET_ROOT_SHELL_PATH: &str =
    "~/code/parameter-golf/data/datasets/fineweb10B_sp1024";
const STRICT_TOKENIZER_SHELL_PATH: &str =
    "~/code/parameter-golf/data/tokenizers/fineweb_1024_bpe.model";
const TRAINING_REPORT_PLACEHOLDER: &str = "<training_report_path>";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfStrictChallengeLaneError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid strict HOMEGOLF lane report: {detail}")]
    InvalidReport { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfStrictChallengeLaneDisposition {
    PreflightSatisfied,
    RefusedMissingChallengeInputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfStrictChallengeInputStatus {
    NotSupplied,
    PresentExactNamedPath,
    MissingPath,
    WrongPathIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfStrictChallengeInputs {
    pub expected_dataset_root_shell_path: String,
    pub expected_tokenizer_shell_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplied_dataset_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplied_tokenizer_path: Option<String>,
    pub dataset_root_status: ParameterGolfHomegolfStrictChallengeInputStatus,
    pub tokenizer_path_status: ParameterGolfHomegolfStrictChallengeInputStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfStrictChallengeLaneReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub source_track_contract_ref: String,
    pub strict_profile: ParameterGolfPromotedTrainingProfile,
    pub canonical_lane_entrypoint: String,
    pub canonical_dense_trainer_entrypoint: String,
    pub canonical_homegolf_local_cuda_trainer_entrypoint: String,
    pub required_final_validation_mode: ParameterGolfSingleH100ValidationMode,
    pub required_validation_eval_mode: ParameterGolfValidationEvalMode,
    pub required_validation_batch_sequences: usize,
    pub required_score_first_ttt: ParameterGolfScoreFirstTttConfig,
    pub exact_compressed_artifact_cap_bytes: u64,
    pub dense_training_command_template: String,
    pub challenge_inputs: ParameterGolfHomegolfStrictChallengeInputs,
    pub disposition: ParameterGolfHomegolfStrictChallengeLaneDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl ParameterGolfHomegolfStrictChallengeLaneReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_strict_challenge_lane|",
            &clone,
        )
    }

    pub fn validate(
        &self,
        track_contract: &ParameterGolfHomegolfTrackContractReport,
    ) -> Result<(), ParameterGolfHomegolfStrictChallengeLaneError> {
        if self.schema_version != 1 {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: format!("schema_version must stay 1 but was {}", self.schema_version),
                },
            );
        }
        if self.track_id != track_contract.track_id {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: format!(
                        "track_id drifted: expected `{}`, observed `{}`",
                        track_contract.track_id, self.track_id
                    ),
                },
            );
        }
        if self.strict_profile.profile_id != track_contract.strict_profile_id {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: format!(
                        "strict_profile.profile_id drifted: expected `{}`, observed `{}`",
                        track_contract.strict_profile_id, self.strict_profile.profile_id
                    ),
                },
            );
        }
        if !self
            .strict_profile
            .evaluation_policy
            .legal_score_first_ttt_required
            || !self
                .strict_profile
                .evaluation_policy
                .contest_bits_per_byte_accounting_required
            || !self
                .strict_profile
                .artifact_policy
                .exact_compressed_artifact_cap_required
        {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: String::from(
                        "strict challenge lane must keep legal score-first TTT, contest BPB accounting, and exact artifact-cap enforcement enabled",
                    ),
                },
            );
        }
        if self.required_validation_eval_mode
            != (ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 })
        {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: String::from(
                        "required_validation_eval_mode must stay sliding_window:64",
                    ),
                },
            );
        }
        if self.required_score_first_ttt != ParameterGolfScoreFirstTttConfig::leaderboard_defaults()
        {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: String::from(
                        "required_score_first_ttt drifted from leaderboard defaults",
                    ),
                },
            );
        }
        if self.exact_compressed_artifact_cap_bytes != track_contract.artifact_cap_bytes {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: format!(
                        "artifact cap drifted: expected {}, observed {}",
                        track_contract.artifact_cap_bytes, self.exact_compressed_artifact_cap_bytes
                    ),
                },
            );
        }
        match self.disposition {
            ParameterGolfHomegolfStrictChallengeLaneDisposition::PreflightSatisfied => {
                if self.refusal.is_some() {
                    return Err(
                        ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                            detail: String::from(
                                "preflight_satisfied disposition must not carry a refusal",
                            ),
                        },
                    );
                }
                if self.challenge_inputs.dataset_root_status
                    != ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
                    || self.challenge_inputs.tokenizer_path_status
                        != ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
                {
                    return Err(
                        ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                            detail: String::from(
                                "preflight_satisfied requires exact-named dataset and tokenizer inputs",
                            ),
                        },
                    );
                }
            }
            ParameterGolfHomegolfStrictChallengeLaneDisposition::RefusedMissingChallengeInputs => {
                if self.refusal.is_none() {
                    return Err(
                        ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                            detail: String::from(
                                "refused_missing_challenge_inputs must carry a typed refusal",
                            ),
                        },
                    );
                }
            }
        }
        if self.report_digest != self.stable_digest() {
            return Err(
                ParameterGolfHomegolfStrictChallengeLaneError::InvalidReport {
                    detail: String::from("report_digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_strict_challenge_lane_report(
    dataset_root: Option<&Path>,
    tokenizer_path: Option<&Path>,
) -> Result<
    ParameterGolfHomegolfStrictChallengeLaneReport,
    ParameterGolfHomegolfStrictChallengeLaneError,
> {
    let track_contract = build_parameter_golf_homegolf_track_contract_report();
    let strict_profile = ParameterGolfPromotedTrainingProfile::strict_pgolf_challenge();
    let validation_eval_mode = ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 };
    let score_first_ttt = ParameterGolfScoreFirstTttConfig::leaderboard_defaults();
    let challenge_inputs = ParameterGolfHomegolfStrictChallengeInputs {
        expected_dataset_root_shell_path: String::from(STRICT_DATASET_ROOT_SHELL_PATH),
        expected_tokenizer_shell_path: String::from(STRICT_TOKENIZER_SHELL_PATH),
        supplied_dataset_root: dataset_root.map(|path| path.display().to_string()),
        supplied_tokenizer_path: tokenizer_path.map(|path| path.display().to_string()),
        dataset_root_status: classify_dataset_root(dataset_root),
        tokenizer_path_status: classify_tokenizer_path(tokenizer_path),
    };
    let refusal = refusal_for_missing_inputs(&challenge_inputs);
    let disposition = if refusal.is_some() {
        ParameterGolfHomegolfStrictChallengeLaneDisposition::RefusedMissingChallengeInputs
    } else {
        ParameterGolfHomegolfStrictChallengeLaneDisposition::PreflightSatisfied
    };

    let mut report = ParameterGolfHomegolfStrictChallengeLaneReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_strict_challenge_lane.v1"),
        track_id: track_contract.track_id.clone(),
        source_track_contract_ref: String::from(PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REF),
        strict_profile,
        canonical_lane_entrypoint: String::from(
            "crates/psionic-train/src/bin/parameter_golf_homegolf_strict_challenge_lane.rs",
        ),
        canonical_dense_trainer_entrypoint: track_contract
            .canonical_dense_trainer_entrypoint
            .clone(),
        canonical_homegolf_local_cuda_trainer_entrypoint: String::from(
            "crates/psionic-train/src/bin/parameter_golf_homegolf_single_cuda_train.rs",
        ),
        required_final_validation_mode: ParameterGolfSingleH100ValidationMode::Both,
        required_validation_eval_mode: validation_eval_mode.clone(),
        required_validation_batch_sequences: parameter_golf_default_validation_batch_sequences(
            &crate::ParameterGolfBatchGeometry::challenge_single_device_defaults(),
            &validation_eval_mode,
        ),
        required_score_first_ttt: score_first_ttt,
        exact_compressed_artifact_cap_bytes: track_contract.artifact_cap_bytes,
        dense_training_command_template: format!(
            "cargo run -q -p psionic-train --bin parameter_golf_homegolf_single_cuda_train -- <dataset_root> <tokenizer_path> {TRAINING_REPORT_PLACEHOLDER} both sliding_window:64 legal_score_first_ttt"
        ),
        challenge_inputs,
        disposition,
        refusal,
        claim_boundary: String::from(
            "This is the canonical strict HOMEGOLF challenge lane surface. It freezes the exact strict profile, exact challenge-input requirements, sliding-window:64 evaluation posture, legal score-first TTT overlay, and exact 16,000,000-byte artifact-cap law. It proves strict preflight only and now points operators at the actual local HOMEGOLF exact trainer entrypoint. It does not claim the current host can already satisfy the 600-second runtime contract.",
        ),
        summary: String::from(
            "HOMEGOLF now has one canonical strict challenge preflight surface that either satisfies the exact contest-input contract or refuses explicitly when the exact FineWeb/SP1024 inputs are absent. The retained command template now targets the local HOMEGOLF exact trainer path instead of the public single-H100 entrypoint.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate(&track_contract)?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_strict_challenge_lane_report(
    output_path: &Path,
    dataset_root: Option<&Path>,
    tokenizer_path: Option<&Path>,
) -> Result<
    ParameterGolfHomegolfStrictChallengeLaneReport,
    ParameterGolfHomegolfStrictChallengeLaneError,
> {
    let report =
        build_parameter_golf_homegolf_strict_challenge_lane_report(dataset_root, tokenizer_path)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfStrictChallengeLaneError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfStrictChallengeLaneError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn classify_dataset_root(path: Option<&Path>) -> ParameterGolfHomegolfStrictChallengeInputStatus {
    match path {
        None => ParameterGolfHomegolfStrictChallengeInputStatus::NotSupplied,
        Some(path) if expected_dataset_root_path().as_deref() != Some(path) => {
            ParameterGolfHomegolfStrictChallengeInputStatus::WrongPathIdentity
        }
        Some(path) if !path.is_dir() => {
            ParameterGolfHomegolfStrictChallengeInputStatus::MissingPath
        }
        Some(_) => ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath,
    }
}

fn classify_tokenizer_path(path: Option<&Path>) -> ParameterGolfHomegolfStrictChallengeInputStatus {
    match path {
        None => ParameterGolfHomegolfStrictChallengeInputStatus::NotSupplied,
        Some(path) if expected_tokenizer_path().as_deref() != Some(path) => {
            ParameterGolfHomegolfStrictChallengeInputStatus::WrongPathIdentity
        }
        Some(path) if !path.is_file() => {
            ParameterGolfHomegolfStrictChallengeInputStatus::MissingPath
        }
        Some(_) => ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath,
    }
}

fn refusal_for_missing_inputs(
    inputs: &ParameterGolfHomegolfStrictChallengeInputs,
) -> Option<PsionicRefusal> {
    let mut missing = Vec::new();
    if inputs.dataset_root_status
        != ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
    {
        missing.push(format!(
            "dataset_root must be supplied as the exact FineWeb SP1024 lane `{}`",
            inputs.expected_dataset_root_shell_path
        ));
    }
    if inputs.tokenizer_path_status
        != ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
    {
        missing.push(format!(
            "tokenizer_path must be supplied as the exact SP1024 tokenizer `{}`",
            inputs.expected_tokenizer_shell_path
        ));
    }
    if missing.is_empty() {
        None
    } else {
        Some(
            PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Runtime,
                format!(
                    "{}; local-reference fallback is denied for the strict HOMEGOLF lane",
                    missing.join("; ")
                ),
            )
            .with_subject("parameter_golf_homegolf_strict_challenge_inputs"),
        )
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("HOMEGOLF strict challenge lane report should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

fn strict_shell_path_to_pathbuf(shell_path: &str) -> Option<PathBuf> {
    shell_path.strip_prefix("~/").map_or_else(
        || Some(PathBuf::from(shell_path)),
        |suffix| env::var_os("HOME").map(|home| PathBuf::from(home).join(suffix)),
    )
}

fn expected_dataset_root_path() -> Option<PathBuf> {
    strict_shell_path_to_pathbuf(STRICT_DATASET_ROOT_SHELL_PATH)
}

fn expected_tokenizer_path() -> Option<PathBuf> {
    strict_shell_path_to_pathbuf(STRICT_TOKENIZER_SHELL_PATH)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_strict_challenge_lane_report, expected_dataset_root_path,
        expected_tokenizer_path, write_parameter_golf_homegolf_strict_challenge_lane_report,
        ParameterGolfHomegolfStrictChallengeInputStatus,
        ParameterGolfHomegolfStrictChallengeLaneDisposition,
        PARAMETER_GOLF_HOMEGOLF_STRICT_CHALLENGE_LANE_FIXTURE_PATH,
    };

    #[test]
    fn strict_challenge_lane_refuses_when_exact_inputs_are_omitted() {
        let report =
            build_parameter_golf_homegolf_strict_challenge_lane_report(None, None).expect("build");
        assert_eq!(
            report.disposition,
            ParameterGolfHomegolfStrictChallengeLaneDisposition::RefusedMissingChallengeInputs
        );
        assert_eq!(
            report.challenge_inputs.dataset_root_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::NotSupplied
        );
        assert_eq!(
            report.challenge_inputs.tokenizer_path_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::NotSupplied
        );
        assert!(report.refusal.is_some());
        assert!(report
            .refusal
            .as_ref()
            .expect("refusal")
            .detail
            .contains("local-reference fallback is denied"));
    }

    #[test]
    fn strict_challenge_lane_rejects_fake_same_basename_inputs() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let dataset_root = tempdir.path().join("fineweb10B_sp1024");
        let tokenizer_path = tempdir.path().join("fineweb_1024_bpe.model");
        std::fs::create_dir_all(&dataset_root).expect("create dataset root");
        std::fs::write(&tokenizer_path, b"placeholder").expect("write tokenizer");
        let report = build_parameter_golf_homegolf_strict_challenge_lane_report(
            Some(dataset_root.as_path()),
            Some(tokenizer_path.as_path()),
        )
        .expect("build");
        assert_eq!(
            report.disposition,
            ParameterGolfHomegolfStrictChallengeLaneDisposition::RefusedMissingChallengeInputs
        );
        assert_eq!(
            report.challenge_inputs.dataset_root_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::WrongPathIdentity
        );
        assert_eq!(
            report.challenge_inputs.tokenizer_path_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::WrongPathIdentity
        );
        assert!(report.refusal.is_some());
    }

    #[test]
    fn strict_challenge_lane_marks_real_expected_inputs_ready_when_present() {
        let Some(dataset_root) = expected_dataset_root_path() else {
            return;
        };
        let Some(tokenizer_path) = expected_tokenizer_path() else {
            return;
        };
        if !dataset_root.is_dir() || !tokenizer_path.is_file() {
            return;
        }
        let report = build_parameter_golf_homegolf_strict_challenge_lane_report(
            Some(dataset_root.as_path()),
            Some(tokenizer_path.as_path()),
        )
        .expect("build");
        assert_eq!(
            report.disposition,
            ParameterGolfHomegolfStrictChallengeLaneDisposition::PreflightSatisfied
        );
        assert_eq!(
            report.challenge_inputs.dataset_root_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
        );
        assert_eq!(
            report.challenge_inputs.tokenizer_path_status,
            ParameterGolfHomegolfStrictChallengeInputStatus::PresentExactNamedPath
        );
        assert!(report.refusal.is_none());
    }

    #[test]
    fn write_strict_challenge_lane_report_roundtrips() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("parameter_golf_homegolf_strict_challenge_lane.json");
        let report = write_parameter_golf_homegolf_strict_challenge_lane_report(
            output_path.as_path(),
            None,
            None,
        )
        .expect("write");
        let decoded: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output_path).expect("read written report"))
                .expect("decode");
        assert_eq!(
            decoded["report_digest"].as_str().expect("digest"),
            report.report_digest
        );
    }

    #[test]
    fn committed_strict_challenge_lane_fixture_roundtrips() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_STRICT_CHALLENGE_LANE_FIXTURE_PATH);
        let committed: super::ParameterGolfHomegolfStrictChallengeLaneReport =
            serde_json::from_slice(&std::fs::read(fixture_path).expect("read fixture"))
                .expect("decode fixture");
        let rebuilt = build_parameter_golf_homegolf_strict_challenge_lane_report(None, None)
            .expect("rebuild report");
        assert_eq!(committed, rebuilt);
    }
}
