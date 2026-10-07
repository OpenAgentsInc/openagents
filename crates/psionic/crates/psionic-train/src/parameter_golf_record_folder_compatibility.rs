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
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
    ParameterGolfReferenceTrainingConfig, ParameterGolfSubmissionFileRole,
};

/// Canonical committed report for Parameter Golf record-folder compatibility.
pub const PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_record_folder_compatibility.json";

const PARAMETER_GOLF_README_REF: &str = "README.md";
const PARAMETER_GOLF_SPEC_REF: &str = "PSIONIC_PARAMETER_GOLF_SPEC.md";
const PARAMETER_GOLF_RECORD_TRACK_RECORDS_RELPATH: &str = "records/track_10min_16mb";
const PARAMETER_GOLF_RECORD_TRACK_EXAMPLE_README_REF: &str =
    "records/track_10min_16mb/2026-03-17_NaiveBaseline/README.md";
const PARAMETER_GOLF_NON_RECORD_TRACK_EXAMPLE_README_REF: &str =
    "records/track_non_record_16mb/2026-03-18_Quasi10Bfrom50B_SP1024_9x512_KV4_4h_pgut3/README.md";

/// Current compatibility disposition for the Psionic Parameter Golf folder contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordFolderCompatibilityStatus {
    Compatible,
    Incompatible,
}

/// Dependency posture required by the public Parameter Golf records contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSubmissionFolderDependencyPosture {
    FolderLocalSelfContained,
}

/// One public records track that the compatibility report understands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionTrackCompatibilityContract {
    /// Stable track identifier used by the verifier.
    pub track_id: String,
    /// Relative records directory inside the public challenge repo.
    pub records_relpath: String,
    /// `submission.json` track field expected by Psionic for this track, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_json_track_value: Option<String>,
    /// Additional `submission.json` keys Psionic requires for this track.
    pub psionic_required_submission_json_keys: Vec<String>,
    /// One current public example README carried by the challenge repo.
    pub current_example_readme_ref: String,
    /// Honest detail about the track contract.
    pub detail: String,
}

/// One required top-level file for a challenge submission folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRequiredTopLevelFileContract {
    /// Stable required file name.
    pub file_name: String,
    /// Stable file role.
    pub role: ParameterGolfSubmissionFileRole,
    /// The file must live at the submission-folder root.
    pub must_live_at_folder_root: bool,
    /// The file must materialize as a regular file.
    pub must_be_regular_file: bool,
    /// Current Psionic file size in bytes.
    pub current_psionic_size_bytes: u64,
    /// Honest detail about why the file matters.
    pub detail: String,
}

/// Machine-readable compatibility contract between Psionic and the public Parameter Golf repo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordFolderCompatibilityReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Current compatibility status.
    pub compatibility_status: ParameterGolfRecordFolderCompatibilityStatus,
    /// Public challenge README that owns the folder contract.
    pub challenge_repo_readme_ref: String,
    /// Psionic-owned spec that defines the lane posture.
    pub psionic_parameter_golf_spec_ref: String,
    /// Stable non-record package version used as the baseline export.
    pub baseline_submission_package_version: String,
    /// Stable baseline submission identifier.
    pub baseline_submission_id: String,
    /// Relative folder path where the current Psionic bundle would land in the challenge repo.
    pub baseline_record_folder_relpath: String,
    /// Minimal `submission.json` keys Psionic treats as part of the compatibility gate.
    pub required_submission_json_core_keys: Vec<String>,
    /// Supported public records tracks.
    pub track_contracts: Vec<ParameterGolfSubmissionTrackCompatibilityContract>,
    /// Required top-level files shared across tracks.
    pub required_top_level_files: Vec<ParameterGolfRequiredTopLevelFileContract>,
    /// Folder-local dependency posture required by the verifier.
    pub dependency_posture: ParameterGolfSubmissionFolderDependencyPosture,
    /// Whether the public repo allows extra files inside the submission folder.
    pub public_repo_allows_additional_folder_contents: bool,
    /// Honest detail about extra files and self-contained execution.
    pub dependency_detail: String,
    /// Current non-required files Psionic preserves in the export folder.
    pub current_psionic_extra_submission_paths: Vec<String>,
    /// Canonical verifier command for this contract.
    pub verifier_runner: String,
    /// Example verifier invocation against the local public repo clone.
    pub verifier_example_command: String,
    /// Honest compatibility boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfRecordFolderCompatibilityReport {
    fn new(
        baseline_submission_package_version: impl Into<String>,
        baseline_submission_id: impl Into<String>,
        baseline_record_folder_relpath: impl Into<String>,
        track_contracts: Vec<ParameterGolfSubmissionTrackCompatibilityContract>,
        required_top_level_files: Vec<ParameterGolfRequiredTopLevelFileContract>,
        current_psionic_extra_submission_paths: Vec<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.record_folder_compatibility.v1"),
            compatibility_status: ParameterGolfRecordFolderCompatibilityStatus::Compatible,
            challenge_repo_readme_ref: String::from(PARAMETER_GOLF_README_REF),
            psionic_parameter_golf_spec_ref: String::from(PARAMETER_GOLF_SPEC_REF),
            baseline_submission_package_version: baseline_submission_package_version.into(),
            baseline_submission_id: baseline_submission_id.into(),
            baseline_record_folder_relpath: baseline_record_folder_relpath.into(),
            required_submission_json_core_keys: vec![
                String::from("author"),
                String::from("github_id"),
                String::from("name"),
                String::from("blurb"),
                String::from("date"),
                String::from("val_loss"),
                String::from("val_bpb"),
                String::from("bytes_total"),
                String::from("bytes_code"),
            ],
            track_contracts,
            required_top_level_files,
            dependency_posture:
                ParameterGolfSubmissionFolderDependencyPosture::FolderLocalSelfContained,
            public_repo_allows_additional_folder_contents: true,
            dependency_detail: String::from(
                "The public README requires a runnable folder rooted at README.md, submission.json, train.log, and train_gpt.py, while also allowing extra shipped dependencies so long as they remain inside the folder and the entrypoint runs without hidden repo, network, or external-download dependencies. Psionic's current non-record bundle therefore remains compatible even though it preserves extra nested benchmark and accounting JSON artifacts alongside the required top-level files.",
            ),
            current_psionic_extra_submission_paths,
            verifier_runner: String::from(
                "scripts/check-parameter-golf-record-folder-compatibility.sh",
            ),
            verifier_example_command: String::from(
                "scripts/check-parameter-golf-record-folder-compatibility.sh --parameter-golf-root ~/code/parameter-golf --submission-dir /tmp/records/track_non_record_16mb/<submission_id>",
            ),
            claim_boundary: String::from(
                "This report closes challenge-repo folder compatibility only. It confirms that the current non-record folder carries a real folder-local train_gpt.py launcher plus shipped Psionic runtime payloads, but it does not by itself promote the bounded local-reference replay path into a record-track runtime claim or prove reproducible 8xH100 record-track execution from the exported folder.",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_parameter_golf_record_folder_compatibility_report|",
            &report,
        );
        report
    }
}

/// Failure while building or persisting the record-folder compatibility report.
#[derive(Debug, Error)]
pub enum ParameterGolfRecordFolderCompatibilityError {
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

/// Builds the committed Parameter Golf record-folder compatibility report.
pub fn build_parameter_golf_record_folder_compatibility_report(
) -> Result<ParameterGolfRecordFolderCompatibilityReport, ParameterGolfRecordFolderCompatibilityError>
{
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &training_config)?;
    let submission_bundle = build_parameter_golf_non_record_submission_bundle(
        &benchmark_bundle,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;

    let required_top_level_files = vec![
        required_top_level_file(&submission_bundle, "README.md", ParameterGolfSubmissionFileRole::Readme, "human-readable folder overview required by the public README")?,
        required_top_level_file(
            &submission_bundle,
            "submission.json",
            ParameterGolfSubmissionFileRole::SubmissionManifest,
            "machine-readable submission metadata required by the public README",
        )?,
        required_top_level_file(
            &submission_bundle,
            "train.log",
            ParameterGolfSubmissionFileRole::TrainLog,
            "preserved train log required by the public README",
        )?,
        required_top_level_file(
            &submission_bundle,
            "train_gpt.py",
            ParameterGolfSubmissionFileRole::Entrypoint,
            "top-level entrypoint required by the public README and used by the compatibility verifier dry-run",
        )?,
    ];

    let required_names = required_top_level_files
        .iter()
        .map(|file| file.file_name.as_str())
        .collect::<Vec<_>>();
    let mut extra_paths = submission_bundle
        .package
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .filter(|path| !required_names.iter().any(|required| path == required))
        .collect::<Vec<_>>();
    extra_paths.sort();

    let track_contracts = vec![
        ParameterGolfSubmissionTrackCompatibilityContract {
            track_id: String::from("record_10min_16mb"),
            records_relpath: String::from(PARAMETER_GOLF_RECORD_TRACK_RECORDS_RELPATH),
            submission_json_track_value: None,
            psionic_required_submission_json_keys: Vec::new(),
            current_example_readme_ref: String::from(
                PARAMETER_GOLF_RECORD_TRACK_EXAMPLE_README_REF,
            ),
            detail: String::from(
                "Record-track submissions land under records/track_10min_16mb and currently use the shared top-level file contract without a required submission.json track field in the public example folder.",
            ),
        },
        ParameterGolfSubmissionTrackCompatibilityContract {
            track_id: String::from("non_record_16mb"),
            records_relpath: String::from("records/track_non_record_16mb"),
            submission_json_track_value: Some(String::from(
                crate::PARAMETER_GOLF_NON_RECORD_TRACK_ID,
            )),
            psionic_required_submission_json_keys: vec![String::from("track")],
            current_example_readme_ref: String::from(
                PARAMETER_GOLF_NON_RECORD_TRACK_EXAMPLE_README_REF,
            ),
            detail: String::from(
                "Non-record submissions land under records/track_non_record_16mb. Psionic requires the explicit non-record track field so exported folders stay machine-legible about their unlimited-compute posture.",
            ),
        },
    ];

    Ok(ParameterGolfRecordFolderCompatibilityReport::new(
        submission_bundle.package.package_version.clone(),
        submission_bundle.package.submission_id.clone(),
        submission_bundle.package.record_folder_relpath.clone(),
        track_contracts,
        required_top_level_files,
        extra_paths,
    ))
}

/// Returns the canonical absolute path for the committed record-folder compatibility report.
#[must_use]
pub fn parameter_golf_record_folder_compatibility_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_REPORT_REF)
}

/// Writes the committed record-folder compatibility report.
pub fn write_parameter_golf_record_folder_compatibility_report(
    output_path: impl AsRef<Path>,
) -> Result<ParameterGolfRecordFolderCompatibilityReport, ParameterGolfRecordFolderCompatibilityError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfRecordFolderCompatibilityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_record_folder_compatibility_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfRecordFolderCompatibilityError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn required_top_level_file(
    submission_bundle: &crate::ParameterGolfNonRecordSubmissionBundle,
    relative_path: &str,
    role: ParameterGolfSubmissionFileRole,
    detail: &str,
) -> Result<ParameterGolfRequiredTopLevelFileContract, ParameterGolfRecordFolderCompatibilityError>
{
    let artifact = submission_bundle.artifact(relative_path).ok_or_else(|| {
        ParameterGolfRecordFolderCompatibilityError::MissingArtifact {
            path: String::from(relative_path),
        }
    })?;
    Ok(ParameterGolfRequiredTopLevelFileContract {
        file_name: String::from(relative_path),
        role,
        must_live_at_folder_root: true,
        must_be_regular_file: true,
        current_psionic_size_bytes: artifact.bytes.len() as u64,
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
) -> Result<T, ParameterGolfRecordFolderCompatibilityError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| ParameterGolfRecordFolderCompatibilityError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfRecordFolderCompatibilityError::Deserialize {
            artifact_kind: String::from("parameter_golf_record_folder_compatibility_report"),
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
    use std::{error::Error, fs, path::PathBuf, process::Command};

    use serde_json::Value;

    use super::{
        build_parameter_golf_record_folder_compatibility_report, repo_root,
        write_parameter_golf_record_folder_compatibility_report,
        ParameterGolfRecordFolderCompatibilityReport,
        PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_REPORT_REF,
    };
    use crate::{
        benchmark_parameter_golf_local_reference,
        build_parameter_golf_non_record_submission_bundle,
        parameter_golf_record_folder_compatibility_report_path,
        write_parameter_golf_non_record_submission_bundle, ParameterGolfLocalReferenceFixture,
        ParameterGolfNonRecordSubmissionConfig, ParameterGolfReferenceTrainingConfig,
    };

    #[test]
    fn parameter_golf_record_folder_compatibility_report_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        let generated = build_parameter_golf_record_folder_compatibility_report()?;
        let committed: ParameterGolfRecordFolderCompatibilityReport =
            super::read_repo_json(PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_parameter_golf_record_folder_compatibility_report_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("parameter_golf_record_folder_compatibility.json");
        let written = write_parameter_golf_record_folder_compatibility_report(&output_path)?;
        let persisted: ParameterGolfRecordFolderCompatibilityReport =
            serde_json::from_slice(&fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_record_folder_compatibility_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("parameter_golf_record_folder_compatibility.json")
        );
        Ok(())
    }

    #[test]
    fn parameter_golf_record_folder_verifier_dry_runs_exported_bundle() -> Result<(), Box<dyn Error>>
    {
        if !crate::parameter_golf_submission_runtime_payload_matches_host() {
            eprintln!(
                "skipping parameter_golf_record_folder_verifier_dry_runs_exported_bundle: \
                 committed parameter_golf_submission_runtime payload targets \
                 x86_64-unknown-linux-gnu and cannot execute on this {}-{} host",
                std::env::consts::ARCH,
                std::env::consts::OS,
            );
            return Ok(());
        }
        let report = build_parameter_golf_record_folder_compatibility_report()?;
        let submission_dir = build_submission_bundle_dir()?;
        let parameter_golf_root = build_mock_parameter_golf_root(&report)?;
        let verification_dir = tempfile::tempdir()?;
        let report_path = verification_dir.path().join("verification_report.json");
        let output = Command::new("bash")
            .arg(repo_root().join("scripts/check-parameter-golf-record-folder-compatibility.sh"))
            .arg("--parameter-golf-root")
            .arg(&parameter_golf_root)
            .arg("--submission-dir")
            .arg(&submission_dir)
            .arg("--report")
            .arg(&report_path)
            .output()?;
        assert!(
            output.status.success(),
            "compatibility verifier failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let verification: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
        assert_eq!(verification["verdict"], "compatible");
        assert_eq!(verification["track_id"], "non_record_16mb");
        Ok(())
    }

    #[test]
    fn parameter_golf_record_folder_verifier_refuses_missing_required_file(
    ) -> Result<(), Box<dyn Error>> {
        let report = build_parameter_golf_record_folder_compatibility_report()?;
        let submission_dir = build_submission_bundle_dir()?;
        fs::remove_file(submission_dir.join("train.log"))?;
        let parameter_golf_root = build_mock_parameter_golf_root(&report)?;
        let output = Command::new("bash")
            .arg(repo_root().join("scripts/check-parameter-golf-record-folder-compatibility.sh"))
            .arg("--parameter-golf-root")
            .arg(&parameter_golf_root)
            .arg("--submission-dir")
            .arg(&submission_dir)
            .output()?;
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing required top-level file `train.log`"));
        Ok(())
    }

    fn build_submission_bundle_dir() -> Result<PathBuf, Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let training = ParameterGolfReferenceTrainingConfig::local_reference();
        let benchmark = benchmark_parameter_golf_local_reference(&fixture, &training)?;
        let submission = build_parameter_golf_non_record_submission_bundle(
            &benchmark,
            &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
        )?;
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.keep();
        write_parameter_golf_non_record_submission_bundle(&submission, &path)?;
        Ok(path)
    }

    fn build_mock_parameter_golf_root(
        report: &ParameterGolfRecordFolderCompatibilityReport,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = temp_dir.keep();
        fs::write(root.join("README.md"), "# mock parameter golf\n")?;
        for track in &report.track_contracts {
            fs::create_dir_all(root.join(&track.records_relpath))?;
            let example = root.join(&track.current_example_readme_ref);
            if let Some(parent) = example.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(example, "example\n")?;
        }
        Ok(root)
    }
}
