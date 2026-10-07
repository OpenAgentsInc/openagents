use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    benchmark_parameter_golf_local_reference, build_parameter_golf_non_record_submission_bundle,
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionBundle,
    ParameterGolfNonRecordSubmissionConfig, ParameterGolfReferenceTrainingConfig,
    ParameterGolfSubmissionFileRole,
};

/// Canonical committed report for the Parameter Golf record-track export surface.
pub const PARAMETER_GOLF_RECORD_EXPORT_SURFACE_CONTRACT_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_record_export_surface_contract.json";

const PARAMETER_GOLF_README_REF: &str = "README.md";
const PARAMETER_GOLF_RUNTIME_PAYLOAD_PATH: &str = "runtime/parameter_golf_submission_runtime";
const PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_PATH: &str = "runtime/parameter_golf_single_h100_train";
const PARAMETER_GOLF_RUNTIME_MANIFEST_PATH: &str = "runtime/parameter_golf_submission_runtime.json";
const PARAMETER_GOLF_REAL_EXECUTION_CONTRACT_PATH: &str =
    "runtime/parameter_golf_real_execution_contract.json";
const PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_PATH: &str =
    "runtime/parameter_golf_google_input_package_descriptor_v1.json";

/// Explicit judgment against the README `train_gpt.py` surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordExportSurfaceJudgment {
    ContestFaithful,
    MaintainerEquivalenceArgument,
    Blocked,
}

/// One shipped artifact that matters to the record-surface judgment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordExportSurfaceArtifact {
    /// Relative path from the exported folder root.
    pub relative_path: String,
    /// Stable file role inside the package.
    pub role: ParameterGolfSubmissionFileRole,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Whether the artifact counts toward the public artifact cap.
    pub counts_toward_artifact_cap: bool,
    /// Honest detail about why the artifact matters to this judgment.
    pub detail: String,
}

/// Machine-readable contract for the current exported record surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordExportSurfaceContractReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public README that owns the evaluated surface.
    pub challenge_repo_readme_ref: String,
    /// Stable package version under review.
    pub package_version: String,
    /// Stable submission identifier under review.
    pub submission_id: String,
    /// Relative folder path where the export lands inside the challenge repo.
    pub record_folder_relpath: String,
    /// Top-level entrypoint path required by the public README.
    pub entrypoint_path: String,
    /// Default execution mode selected by the current launcher.
    pub default_execution_mode: String,
    /// Additional explicit execution modes carried by the folder.
    pub additional_execution_modes: Vec<String>,
    /// Whether the current folder is literally faithful to the README's
    /// `train_gpt.py`-centered counted-code surface.
    pub contest_faithful_to_readme_train_gpt_surface: bool,
    /// Whether all counted code lives only in `train_gpt.py`.
    pub counted_code_lives_only_in_train_gpt_py: bool,
    /// Whether the folder ships additional executable payloads beyond the
    /// top-level launcher.
    pub ships_additional_executable_payloads: bool,
    /// Whether the folder remains fully self-contained at execution time.
    pub folder_local_self_contained_execution: bool,
    /// Current explicit judgment.
    pub judgment: ParameterGolfRecordExportSurfaceJudgment,
    /// Honest explanation of the judgment.
    pub judgment_detail: String,
    /// Exact shipped bytes relevant to the judgment.
    pub relevant_artifacts: Vec<ParameterGolfRecordExportSurfaceArtifact>,
    /// Evidence refs maintainers should review with this report.
    pub evidence_refs: Vec<String>,
    /// Honest claim boundary for the current export surface.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfRecordExportSurfaceContractReport {
    fn new(
        package_version: impl Into<String>,
        submission_id: impl Into<String>,
        record_folder_relpath: impl Into<String>,
        relevant_artifacts: Vec<ParameterGolfRecordExportSurfaceArtifact>,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.record_export_surface_contract.v1"),
            challenge_repo_readme_ref: String::from(PARAMETER_GOLF_README_REF),
            package_version: package_version.into(),
            submission_id: submission_id.into(),
            record_folder_relpath: record_folder_relpath.into(),
            entrypoint_path: String::from("train_gpt.py"),
            default_execution_mode: String::from("local_reference_validation"),
            additional_execution_modes: vec![
                String::from("single_h100_train"),
                String::from("distributed_8xh100_train"),
            ],
            contest_faithful_to_readme_train_gpt_surface: false,
            counted_code_lives_only_in_train_gpt_py: false,
            ships_additional_executable_payloads: true,
            folder_local_self_contained_execution: true,
            judgment: ParameterGolfRecordExportSurfaceJudgment::MaintainerEquivalenceArgument,
            judgment_detail: String::from(
                "The current exported folder is not literally README-faithful because executable counted code spans the top-level train_gpt.py launcher plus two shipped Psionic runtime binaries. The difference is now explicit rather than implied: the report binds the exact launcher, replay runtime, real single-H100 trainer, distributed 8xH100 runtime lane, runtime manifest, real execution contract, and immutable input-package descriptor bytes so maintainers can review one concrete equivalence argument instead of inferring it from scattered artifacts.",
            ),
            relevant_artifacts,
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
                String::from("docs/PARAMETER_GOLF_EXPORTED_SUBMISSION_EVIDENCE.md"),
                String::from("docs/PARAMETER_GOLF_RECORD_TRACK_CONTRACT.md"),
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_submission_run_evidence.json",
                ),
            ],
            claim_boundary: String::from(
                "This report answers only the exported record-surface question. It makes the current train_gpt.py-versus-shipped-runtime difference explicit for maintainer review, but it does not by itself prove record-track acceptance or close the 8xH100 execution lane.",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_parameter_golf_record_export_surface_contract_report|",
            &report,
        );
        report
    }
}

/// Failure while building or persisting the export-surface contract report.
#[derive(Debug, Error)]
pub enum ParameterGolfRecordExportSurfaceContractError {
    #[error(transparent)]
    ReferenceTraining(#[from] crate::ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Training(#[from] crate::ParameterGolfBenchmarkBundleError),
    #[error(transparent)]
    Submission(#[from] crate::ParameterGolfSubmissionError),
    #[error("missing generated submission artifact `{path}`")]
    MissingArtifact { path: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed Parameter Golf record export-surface contract report.
pub fn build_parameter_golf_record_export_surface_contract_report() -> Result<
    ParameterGolfRecordExportSurfaceContractReport,
    ParameterGolfRecordExportSurfaceContractError,
> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &training_config)?;
    let submission_bundle = build_parameter_golf_non_record_submission_bundle(
        &benchmark_bundle,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;
    let relevant_artifacts = vec![
        relevant_artifact(
            &submission_bundle,
            "train_gpt.py",
            ParameterGolfSubmissionFileRole::Entrypoint,
            "required top-level entrypoint named by the public README; all later execution modes still flow through this launcher",
        )?,
        relevant_artifact(
            &submission_bundle,
            PARAMETER_GOLF_RUNTIME_PAYLOAD_PATH,
            ParameterGolfSubmissionFileRole::RuntimePayload,
            "default exported-folder executable payload for bounded local-reference replay",
        )?,
        relevant_artifact(
            &submission_bundle,
            PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_PATH,
            ParameterGolfSubmissionFileRole::RealRuntimePayload,
            "additional exported-folder executable payload for explicit single-H100 trainer dispatch",
        )?,
        relevant_artifact(
            &submission_bundle,
            PARAMETER_GOLF_RUNTIME_MANIFEST_PATH,
            ParameterGolfSubmissionFileRole::RuntimeManifest,
            "machine-readable manifest that binds train_gpt.py to the shipped default runtime surface",
        )?,
        relevant_artifact(
            &submission_bundle,
            PARAMETER_GOLF_REAL_EXECUTION_CONTRACT_PATH,
            ParameterGolfSubmissionFileRole::RealExecutionContract,
            "machine-readable contract that widens the entrypoint into explicit single-H100 trainer execution",
        )?,
        relevant_artifact(
            &submission_bundle,
            PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_PATH,
            ParameterGolfSubmissionFileRole::RuntimeInputPackageDescriptor,
            "immutable remote-input descriptor carried forward into the explicit real-execution path",
        )?,
    ];
    Ok(ParameterGolfRecordExportSurfaceContractReport::new(
        submission_bundle.package.package_version.clone(),
        submission_bundle.package.submission_id.clone(),
        submission_bundle.package.record_folder_relpath.clone(),
        relevant_artifacts,
    ))
}

/// Returns the canonical absolute path for the committed export-surface contract report.
#[must_use]
pub fn parameter_golf_record_export_surface_contract_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RECORD_EXPORT_SURFACE_CONTRACT_REPORT_REF)
}

/// Writes the committed export-surface contract report.
pub fn write_parameter_golf_record_export_surface_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    ParameterGolfRecordExportSurfaceContractReport,
    ParameterGolfRecordExportSurfaceContractError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfRecordExportSurfaceContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_record_export_surface_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfRecordExportSurfaceContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn relevant_artifact(
    submission_bundle: &ParameterGolfNonRecordSubmissionBundle,
    relative_path: &str,
    role: ParameterGolfSubmissionFileRole,
    detail: &str,
) -> Result<ParameterGolfRecordExportSurfaceArtifact, ParameterGolfRecordExportSurfaceContractError>
{
    let artifact = submission_bundle.artifact(relative_path).ok_or_else(|| {
        ParameterGolfRecordExportSurfaceContractError::MissingArtifact {
            path: String::from(relative_path),
        }
    })?;
    let file = submission_bundle
        .package
        .files
        .iter()
        .find(|file| file.relative_path == relative_path)
        .ok_or_else(
            || ParameterGolfRecordExportSurfaceContractError::MissingArtifact {
                path: String::from(relative_path),
            },
        )?;
    Ok(ParameterGolfRecordExportSurfaceArtifact {
        relative_path: artifact.artifact_ref.clone(),
        role,
        artifact_digest: artifact.artifact_digest.clone(),
        size_bytes: artifact.bytes.len() as u64,
        counts_toward_artifact_cap: file.counts_toward_artifact_cap,
        detail: String::from(detail),
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, ParameterGolfRecordExportSurfaceContractError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| ParameterGolfRecordExportSurfaceContractError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfRecordExportSurfaceContractError::Deserialize {
            artifact_kind: String::from("parameter_golf_record_export_surface_contract_report"),
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use super::{
        build_parameter_golf_record_export_surface_contract_report,
        parameter_golf_record_export_surface_contract_report_path, read_repo_json,
        write_parameter_golf_record_export_surface_contract_report,
        ParameterGolfRecordExportSurfaceContractReport, ParameterGolfRecordExportSurfaceJudgment,
        PARAMETER_GOLF_RECORD_EXPORT_SURFACE_CONTRACT_REPORT_REF,
    };

    #[test]
    fn parameter_golf_record_export_surface_contract_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        let generated = build_parameter_golf_record_export_surface_contract_report()?;
        let committed: ParameterGolfRecordExportSurfaceContractReport =
            read_repo_json(PARAMETER_GOLF_RECORD_EXPORT_SURFACE_CONTRACT_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn parameter_golf_record_export_surface_contract_stays_explicit_about_equivalence(
    ) -> Result<(), Box<dyn Error>> {
        let report = build_parameter_golf_record_export_surface_contract_report()?;
        assert_eq!(
            report.judgment,
            ParameterGolfRecordExportSurfaceJudgment::MaintainerEquivalenceArgument
        );
        assert!(!report.contest_faithful_to_readme_train_gpt_surface);
        assert!(!report.counted_code_lives_only_in_train_gpt_py);
        assert!(report.ships_additional_executable_payloads);
        assert!(report.folder_local_self_contained_execution);
        assert_eq!(report.relevant_artifacts.len(), 6);
        Ok(())
    }

    #[test]
    fn write_parameter_golf_record_export_surface_contract_report_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("parameter_golf_record_export_surface_contract.json");
        let written = write_parameter_golf_record_export_surface_contract_report(&output_path)?;
        let persisted: ParameterGolfRecordExportSurfaceContractReport =
            serde_json::from_slice(&fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        Ok(())
    }

    #[test]
    fn canonical_record_export_surface_contract_path_lives_under_fixtures() {
        let path = parameter_golf_record_export_surface_contract_report_path();
        assert!(path.ends_with(PARAMETER_GOLF_RECORD_EXPORT_SURFACE_CONTRACT_REPORT_REF));
    }
}
