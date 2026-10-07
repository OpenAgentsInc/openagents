use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PARAMETER_GOLF_HOMEGOLF_ARTIFACT_ACCOUNTING_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_artifact_accounting.json";
pub const PARAMETER_GOLF_HOMEGOLF_ARTIFACT_ACCOUNTING_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-artifact-accounting.sh";
pub const PARAMETER_GOLF_HOMEGOLF_ARTIFACT_ACCOUNTING_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-artifact-accounting-audit.md";

const HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
const HOMEGOLF_CLUSTERED_RUN_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json";
const PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json";
const HOMEGOLF_ARTIFACT_CAP_BYTES: u64 = 16_000_000;

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfArtifactAccountingError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid HOMEGOLF artifact accounting report: {detail}")]
    InvalidReport { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfArtifactBudgetStatus {
    WithinArtifactCap,
    RefusedExceedsArtifactCap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfArtifactAccountingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub source_track_contract_ref: String,
    pub source_clustered_run_surface_ref: String,
    pub source_counted_code_bytes_ref: String,
    pub merged_bundle_descriptor_digest: String,
    pub merged_bundle_tokenizer_digest: String,
    pub counted_code_bytes: u64,
    pub scored_model_artifact_bytes: u64,
    pub total_counted_bytes: u64,
    pub artifact_cap_bytes: u64,
    pub cap_delta_bytes: i64,
    pub budget_status: ParameterGolfHomegolfArtifactBudgetStatus,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Deserialize)]
struct HomegolfClusteredRunSurfaceSource {
    track_id: String,
    merged_bundle_descriptor_digest: String,
    merged_bundle_tokenizer_digest: String,
    model_artifact_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct ParameterGolfResearchHarnessReport {
    variants: Vec<ParameterGolfResearchHarnessVariant>,
}

#[derive(Debug, Deserialize)]
struct ParameterGolfResearchHarnessVariant {
    variant_id: String,
    measured_metrics: Option<ParameterGolfResearchHarnessMetrics>,
}

#[derive(Debug, Deserialize)]
struct ParameterGolfResearchHarnessMetrics {
    bytes_code: u64,
}

impl ParameterGolfHomegolfArtifactAccountingReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_artifact_accounting|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfArtifactAccountingError> {
        if self.schema_version != 1 {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: format!("schema_version must stay 1 but was {}", self.schema_version),
                },
            );
        }
        if self.artifact_cap_bytes != HOMEGOLF_ARTIFACT_CAP_BYTES {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: String::from("artifact_cap_bytes drifted"),
                },
            );
        }
        if self.total_counted_bytes
            != self
                .counted_code_bytes
                .saturating_add(self.scored_model_artifact_bytes)
        {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: String::from("total_counted_bytes no longer matches code+model bytes"),
                },
            );
        }
        let expected_status = if self.total_counted_bytes <= self.artifact_cap_bytes {
            ParameterGolfHomegolfArtifactBudgetStatus::WithinArtifactCap
        } else {
            ParameterGolfHomegolfArtifactBudgetStatus::RefusedExceedsArtifactCap
        };
        if self.budget_status != expected_status {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: String::from("budget_status drifted from the counted byte totals"),
                },
            );
        }
        if self.cap_delta_bytes != self.total_counted_bytes as i64 - self.artifact_cap_bytes as i64
        {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: String::from("cap_delta_bytes drifted"),
                },
            );
        }
        if self.report_digest != self.stable_digest() {
            return Err(
                ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                    detail: String::from("report_digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_artifact_accounting_report() -> Result<
    ParameterGolfHomegolfArtifactAccountingReport,
    ParameterGolfHomegolfArtifactAccountingError,
> {
    let clustered_surface: HomegolfClusteredRunSurfaceSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF)).map_err(|error| {
            ParameterGolfHomegolfArtifactAccountingError::Read {
                path: String::from(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF),
                error,
            }
        })?,
    )?;
    let research_harness: ParameterGolfResearchHarnessReport = serde_json::from_slice(
        &fs::read(resolve_repo_path(
            PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF,
        ))
        .map_err(|error| ParameterGolfHomegolfArtifactAccountingError::Read {
            path: String::from(PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF),
            error,
        })?,
    )?;
    let counted_code_bytes = research_harness
        .variants
        .iter()
        .find(|variant| variant.variant_id == "baseline_control")
        .and_then(|variant| {
            variant
                .measured_metrics
                .as_ref()
                .map(|metrics| metrics.bytes_code)
        })
        .ok_or_else(
            || ParameterGolfHomegolfArtifactAccountingError::InvalidReport {
                detail: String::from(
                    "baseline_control bytes_code was missing from research harness report",
                ),
            },
        )?;

    let scored_model_artifact_bytes = clustered_surface.model_artifact_bytes;
    let total_counted_bytes = counted_code_bytes.saturating_add(scored_model_artifact_bytes);
    let cap_delta_bytes = total_counted_bytes as i64 - HOMEGOLF_ARTIFACT_CAP_BYTES as i64;
    let budget_status = if total_counted_bytes <= HOMEGOLF_ARTIFACT_CAP_BYTES {
        ParameterGolfHomegolfArtifactBudgetStatus::WithinArtifactCap
    } else {
        ParameterGolfHomegolfArtifactBudgetStatus::RefusedExceedsArtifactCap
    };

    let mut report = ParameterGolfHomegolfArtifactAccountingReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_artifact_accounting.v1"),
        track_id: clustered_surface.track_id,
        source_track_contract_ref: String::from(HOMEGOLF_TRACK_CONTRACT_REF),
        source_clustered_run_surface_ref: String::from(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF),
        source_counted_code_bytes_ref: String::from(PARAMETER_GOLF_RESEARCH_HARNESS_REPORT_REF),
        merged_bundle_descriptor_digest: clustered_surface.merged_bundle_descriptor_digest,
        merged_bundle_tokenizer_digest: clustered_surface.merged_bundle_tokenizer_digest,
        counted_code_bytes,
        scored_model_artifact_bytes,
        total_counted_bytes,
        artifact_cap_bytes: HOMEGOLF_ARTIFACT_CAP_BYTES,
        cap_delta_bytes,
        budget_status,
        claim_boundary: String::from(
            "This accounting report is now bound to the retained H100-backed live dense mixed-device HOMEGOLF surface and the exact dense challenge export bytes rather than the older bounded promoted-bundle surrogate. The counted code bytes still come from Psionic's shipped record-compatible runtime posture, but the scored model bytes now come from the retained int8+zlib dense export that actually fits inside the contest-style cap.",
        ),
        summary: String::from(
            "HOMEGOLF now emits one explicit counted-byte report bound to the retained H100-backed live dense mixed-device surface. The current counted-code posture plus the retained compressed dense export stay inside the 16,000,000-byte contest cap.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_artifact_accounting_report(
    output_path: &Path,
) -> Result<
    ParameterGolfHomegolfArtifactAccountingReport,
    ParameterGolfHomegolfArtifactAccountingError,
> {
    let report = build_parameter_golf_homegolf_artifact_accounting_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfArtifactAccountingError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfArtifactAccountingError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
    hasher.update(serde_json::to_vec(value).expect("HOMEGOLF accounting report should serialize"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_artifact_accounting_report,
        write_parameter_golf_homegolf_artifact_accounting_report,
        ParameterGolfHomegolfArtifactBudgetStatus,
        PARAMETER_GOLF_HOMEGOLF_ARTIFACT_ACCOUNTING_FIXTURE_PATH,
    };

    #[test]
    fn homegolf_artifact_accounting_keeps_within_cap_truth() {
        // Commit e7168d38 upgraded HOMEGOLF to the live dense mixed-device
        // surface whose retained int8+zlib dense export fits inside the
        // 16,000,000-byte contest cap, replacing the earlier refused
        // over-cap posture.
        let report =
            build_parameter_golf_homegolf_artifact_accounting_report().expect("build report");
        assert_eq!(
            report.budget_status,
            ParameterGolfHomegolfArtifactBudgetStatus::WithinArtifactCap
        );
        assert!(report.total_counted_bytes <= report.artifact_cap_bytes);
        assert!(report.cap_delta_bytes <= 0);
    }

    #[test]
    fn write_homegolf_artifact_accounting_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_artifact_accounting.json");
        let written = write_parameter_golf_homegolf_artifact_accounting_report(path.as_path())
            .expect("write report");
        let encoded = std::fs::read(path.as_path()).expect("read report");
        let decoded: super::ParameterGolfHomegolfArtifactAccountingReport =
            serde_json::from_slice(&encoded).expect("decode report");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_homegolf_artifact_accounting_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_ARTIFACT_ACCOUNTING_FIXTURE_PATH);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfArtifactAccountingReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        let rebuilt =
            build_parameter_golf_homegolf_artifact_accounting_report().expect("rebuild report");
        assert_eq!(decoded, rebuilt);
    }
}
