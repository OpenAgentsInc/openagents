use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PARAMETER_GOLF_HOMEGOLF_MULTI_SEED_PACKAGE_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_multiseed_package.json";
pub const PARAMETER_GOLF_HOMEGOLF_MULTI_SEED_PACKAGE_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-multiseed-package.sh";
pub const PARAMETER_GOLF_HOMEGOLF_MULTI_SEED_PACKAGE_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-multiseed-package-audit.md";

const HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
const HOMEGOLF_PUBLIC_COMPARISON_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_public_comparison.json";
const HOMEGOLF_ARTIFACT_ACCOUNTING_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_artifact_accounting.json";
const HOMEGOLF_MULTI_SEED_RUN_REFS: [&str; 3] = [
    "fixtures/parameter_golf/reports/homegolf_multiseed/parameter_golf_homegolf_dense_bundle_proof_seed_000.json",
    "fixtures/parameter_golf/reports/homegolf_multiseed/parameter_golf_homegolf_dense_bundle_proof_seed_001.json",
    "fixtures/parameter_golf/reports/homegolf_multiseed/parameter_golf_homegolf_dense_bundle_proof_seed_002.json",
];
const HOMEGOLF_TRACK_ID: &str = "parameter_golf.home_cluster_compatible_10min.v1";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfMultiSeedPackageError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid HOMEGOLF multi-seed package: {detail}")]
    InvalidReport { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfPackageClaimClass {
    PublicBaselineComparableOnly,
    BeatClaimUnsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfEvidenceBar {
    pub minimum_repeated_run_count: usize,
    pub current_claim_class: ParameterGolfHomegolfPackageClaimClass,
    pub stronger_claim_requirement: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfMultiSeedRunReceipt {
    pub seed_slot: u32,
    pub source_run_report_ref: String,
    pub source_bundle_proof_ref: String,
    pub run_id: String,
    pub descriptor_digest: String,
    pub tokenizer_digest: String,
    pub model_artifact_bytes: u64,
    pub final_validation_mean_loss: f64,
    pub final_validation_bits_per_byte: f64,
    pub direct_and_served_match: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfMultiSeedPackageReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub source_track_contract_ref: String,
    pub source_public_comparison_ref: String,
    pub source_artifact_accounting_ref: String,
    pub evidence_bar: ParameterGolfHomegolfEvidenceBar,
    pub seed_runs: Vec<ParameterGolfHomegolfMultiSeedRunReceipt>,
    pub mean_validation_bits_per_byte: f64,
    pub stddev_validation_bits_per_byte: f64,
    pub min_validation_bits_per_byte: f64,
    pub max_validation_bits_per_byte: f64,
    pub mean_delta_vs_public_naive_baseline: f64,
    pub mean_delta_vs_current_public_best: f64,
    pub artifact_budget_status: String,
    pub comparison_language: Vec<String>,
    pub beat_public_naive_baseline_claim_supported: bool,
    pub beat_current_public_best_claim_supported: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Deserialize)]
struct HomegolfDenseBundleProofSource {
    track_id: String,
    evidence_seed_slot: Option<u32>,
    run_id: String,
    descriptor_digest: String,
    tokenizer_digest: String,
    model_artifact_bytes: u64,
    final_validation_mean_loss: f64,
    final_validation_bits_per_byte: f64,
    direct_and_served_match: bool,
}

#[derive(Debug, Deserialize)]
struct HomegolfPublicComparisonSource {
    public_naive_baseline: HomegolfPublicComparisonPoint,
    current_public_leaderboard_best: HomegolfPublicComparisonPoint,
}

#[derive(Debug, Deserialize)]
struct HomegolfPublicComparisonPoint {
    val_bpb: f64,
}

#[derive(Debug, Deserialize)]
struct HomegolfArtifactAccountingSource {
    budget_status: String,
}

impl ParameterGolfHomegolfMultiSeedPackageReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_multiseed_package|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfMultiSeedPackageError> {
        if self.schema_version != 1 {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: format!("schema_version must stay 1 but was {}", self.schema_version),
            });
        }
        if self.track_id != HOMEGOLF_TRACK_ID {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from("track_id drifted"),
            });
        }
        if self.seed_runs.len() < self.evidence_bar.minimum_repeated_run_count {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from("seed_runs no longer satisfy the minimum repeated-run bar"),
            });
        }
        let mut observed_slots = self
            .seed_runs
            .iter()
            .map(|run| run.seed_slot)
            .collect::<Vec<_>>();
        observed_slots.sort_unstable();
        observed_slots.dedup();
        if observed_slots.len() != self.seed_runs.len() {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from("seed_slots must remain unique"),
            });
        }
        if self
            .seed_runs
            .iter()
            .any(|run| !run.direct_and_served_match)
        {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from(
                    "all repeated HOMEGOLF runs must preserve direct/served parity",
                ),
            });
        }
        let computed_mean = mean(
            self.seed_runs
                .iter()
                .map(|run| run.final_validation_bits_per_byte)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let computed_stddev = population_stddev(
            self.seed_runs
                .iter()
                .map(|run| run.final_validation_bits_per_byte)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        if !approx_eq(self.mean_validation_bits_per_byte, computed_mean)
            || !approx_eq(self.stddev_validation_bits_per_byte, computed_stddev)
        {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from("mean/stddev drifted from seed run receipts"),
            });
        }
        if self.mean_delta_vs_public_naive_baseline <= 0.0
            && self.beat_public_naive_baseline_claim_supported
        {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from(
                    "beat_public_naive_baseline_claim_supported cannot be true when the mean delta is not better",
                ),
            });
        }
        if self.mean_delta_vs_current_public_best <= 0.0
            && self.beat_current_public_best_claim_supported
        {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from(
                    "beat_current_public_best_claim_supported cannot be true when the mean delta is not better",
                ),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
                detail: String::from("report_digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_multiseed_package_report(
) -> Result<ParameterGolfHomegolfMultiSeedPackageReport, ParameterGolfHomegolfMultiSeedPackageError>
{
    let seed_runs = HOMEGOLF_MULTI_SEED_RUN_REFS
        .iter()
        .map(|report_ref| load_seed_run(report_ref))
        .collect::<Result<Vec<_>, _>>()?;
    if seed_runs.is_empty() {
        return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
            detail: String::from("seed_runs must not be empty"),
        });
    }

    let public_comparison: HomegolfPublicComparisonSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(HOMEGOLF_PUBLIC_COMPARISON_REF)).map_err(|error| {
            ParameterGolfHomegolfMultiSeedPackageError::Read {
                path: String::from(HOMEGOLF_PUBLIC_COMPARISON_REF),
                error,
            }
        })?,
    )?;
    let artifact_accounting: HomegolfArtifactAccountingSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(HOMEGOLF_ARTIFACT_ACCOUNTING_REF)).map_err(|error| {
            ParameterGolfHomegolfMultiSeedPackageError::Read {
                path: String::from(HOMEGOLF_ARTIFACT_ACCOUNTING_REF),
                error,
            }
        })?,
    )?;

    let values = seed_runs
        .iter()
        .map(|run| run.final_validation_bits_per_byte)
        .collect::<Vec<_>>();
    let mean_validation_bits_per_byte = mean(values.as_slice());
    let stddev_validation_bits_per_byte = population_stddev(values.as_slice());
    let min_validation_bits_per_byte = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_validation_bits_per_byte = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let mean_delta_vs_public_naive_baseline =
        mean_validation_bits_per_byte - public_comparison.public_naive_baseline.val_bpb;
    let mean_delta_vs_current_public_best =
        mean_validation_bits_per_byte - public_comparison.current_public_leaderboard_best.val_bpb;

    let mut report = ParameterGolfHomegolfMultiSeedPackageReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_multiseed_package.v1"),
        track_id: String::from(HOMEGOLF_TRACK_ID),
        source_track_contract_ref: String::from(HOMEGOLF_TRACK_CONTRACT_REF),
        source_public_comparison_ref: String::from(HOMEGOLF_PUBLIC_COMPARISON_REF),
        source_artifact_accounting_ref: String::from(HOMEGOLF_ARTIFACT_ACCOUNTING_REF),
        evidence_bar: ParameterGolfHomegolfEvidenceBar {
            minimum_repeated_run_count: HOMEGOLF_MULTI_SEED_RUN_REFS.len(),
            current_claim_class: ParameterGolfHomegolfPackageClaimClass::PublicBaselineComparableOnly,
            stronger_claim_requirement: String::from(
                "If HOMEGOLF ever claims to beat the public naive baseline or current public best, the retained package must move beyond this reproducibility-grade bar and include enough repeated exact-lane runs to support the stronger claim honestly.",
            ),
            detail: String::from(
                "The current HOMEGOLF package is a reproducibility-grade repeated-run package for a deterministic exact-family proof lane. It is enough for honest public-baseline comparison language, but not enough for stronger win-style rhetoric.",
            ),
        },
        seed_runs,
        mean_validation_bits_per_byte,
        stddev_validation_bits_per_byte,
        min_validation_bits_per_byte,
        max_validation_bits_per_byte,
        mean_delta_vs_public_naive_baseline,
        mean_delta_vs_current_public_best,
        artifact_budget_status: artifact_accounting.budget_status,
        comparison_language: vec![
            String::from("public-baseline comparable"),
            String::from("not public-leaderboard equivalent"),
        ],
        beat_public_naive_baseline_claim_supported: false,
        beat_current_public_best_claim_supported: false,
        claim_boundary: String::from(
            "This package records repeated exact HOMEGOLF reruns strongly enough to prove reproducibility and honest public-comparison posture for the current deterministic proof lane. It does not claim baseline competitiveness or support beat claims against the public naive baseline or current public best.",
        ),
        summary: String::from(
            "HOMEGOLF now has a retained multi-run package over the exact-family proof lane. The current repeated runs reproduce the same val_bpb and the same train-to-infer/serve closure, so the spread is zero. That is enough for reproducibility-grade public comparison, but not enough for stronger contest rhetoric.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_multiseed_package_report(
    output_path: &Path,
) -> Result<ParameterGolfHomegolfMultiSeedPackageReport, ParameterGolfHomegolfMultiSeedPackageError>
{
    let report = build_parameter_golf_homegolf_multiseed_package_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfMultiSeedPackageError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfMultiSeedPackageError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn load_seed_run(
    report_ref: &str,
) -> Result<ParameterGolfHomegolfMultiSeedRunReceipt, ParameterGolfHomegolfMultiSeedPackageError> {
    let source: HomegolfDenseBundleProofSource =
        serde_json::from_slice(&fs::read(resolve_repo_path(report_ref)).map_err(|error| {
            ParameterGolfHomegolfMultiSeedPackageError::Read {
                path: String::from(report_ref),
                error,
            }
        })?)?;
    if source.track_id != HOMEGOLF_TRACK_ID {
        return Err(ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
            detail: format!("seed report `{report_ref}` drifted off the HOMEGOLF track"),
        });
    }
    let seed_slot = source.evidence_seed_slot.ok_or_else(|| {
        ParameterGolfHomegolfMultiSeedPackageError::InvalidReport {
            detail: format!("seed report `{report_ref}` is missing evidence_seed_slot"),
        }
    })?;
    Ok(ParameterGolfHomegolfMultiSeedRunReceipt {
        seed_slot,
        source_run_report_ref: String::from(report_ref),
        source_bundle_proof_ref: String::from(report_ref),
        run_id: source.run_id,
        descriptor_digest: source.descriptor_digest,
        tokenizer_digest: source.tokenizer_digest,
        model_artifact_bytes: source.model_artifact_bytes,
        final_validation_mean_loss: source.final_validation_mean_loss,
        final_validation_bits_per_byte: source.final_validation_bits_per_byte,
        direct_and_served_match: source.direct_and_served_match,
    })
}

fn mean(values: &[f64]) -> f64 {
    values.iter().copied().sum::<f64>() / values.len() as f64
}

fn population_stddev(values: &[f64]) -> f64 {
    let mean_value = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean_value;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn resolve_repo_path(relpath: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relpath)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("HOMEGOLF multi-seed package should serialize"));
    format!("{:x}", hasher.finalize())
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-12
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_multiseed_package_report,
        write_parameter_golf_homegolf_multiseed_package_report,
        ParameterGolfHomegolfPackageClaimClass,
        PARAMETER_GOLF_HOMEGOLF_MULTI_SEED_PACKAGE_FIXTURE_PATH,
    };

    #[test]
    fn multiseed_package_stays_public_baseline_comparable_only() {
        let report = build_parameter_golf_homegolf_multiseed_package_report().expect("build");
        assert_eq!(
            report.evidence_bar.current_claim_class,
            ParameterGolfHomegolfPackageClaimClass::PublicBaselineComparableOnly
        );
        assert_eq!(report.seed_runs.len(), 3);
        assert!(report.stddev_validation_bits_per_byte >= 0.0);
        assert!(!report.beat_public_naive_baseline_claim_supported);
        assert!(!report.beat_current_public_best_claim_supported);
    }

    #[test]
    fn write_multiseed_package_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_multiseed_package.json");
        let written =
            write_parameter_golf_homegolf_multiseed_package_report(path.as_path()).expect("write");
        let encoded = std::fs::read(path.as_path()).expect("read");
        let decoded: super::ParameterGolfHomegolfMultiSeedPackageReport =
            serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_multiseed_package_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_MULTI_SEED_PACKAGE_FIXTURE_PATH);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfMultiSeedPackageReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        decoded.validate().expect("validate fixture");
    }
}
