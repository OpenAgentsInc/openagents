use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    ParameterGolfHomegolfComparisonClass, ParameterGolfHomegolfTrackContractReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_mixed_hardware_manifest.json";
pub const PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-mixed-hardware-manifest.sh";
pub const PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-mixed-hardware-manifest-audit.md";

const HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfMixedHardwareManifestError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid HOMEGOLF mixed hardware manifest report: {detail}")]
    InvalidReport { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfHardwarePresence {
    PresentMeasured,
    OptionalOffline,
    OptionalFuture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfMixedHardwareEntry {
    pub member_id: String,
    pub hardware_class_id: String,
    pub required_for_track: bool,
    pub presence: ParameterGolfHomegolfHardwarePresence,
    pub execution_backend_label: String,
    pub role_detail: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfMixedHardwareManifestReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub mixed_manifest_id: String,
    pub source_track_contract_ref: String,
    pub artifact_cap_bytes: u64,
    pub wallclock_cap_seconds: u64,
    pub exact_public_hardware_equivalence_required: bool,
    pub admitted_comparison_classes: Vec<ParameterGolfHomegolfComparisonClass>,
    pub leaderboard_equivalence_rule: String,
    pub manifest_entries: Vec<ParameterGolfHomegolfMixedHardwareEntry>,
    pub optional_h100_admitted: bool,
    pub score_semantics_preserved: bool,
    pub comparison_policy_preserved: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl ParameterGolfHomegolfMixedHardwareManifestReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_mixed_hardware_manifest|",
            &clone,
        )
    }

    pub fn validate(
        &self,
        track_contract: &ParameterGolfHomegolfTrackContractReport,
    ) -> Result<(), ParameterGolfHomegolfMixedHardwareManifestError> {
        if self.schema_version != 1 {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: format!("schema_version must stay 1 but was {}", self.schema_version),
            });
        }
        if self.track_id != track_contract.track_id {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("track_id drifted from the HOMEGOLF track contract"),
            });
        }
        if self.artifact_cap_bytes != track_contract.artifact_cap_bytes
            || self.wallclock_cap_seconds != track_contract.wallclock_cap_seconds
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("artifact or wallclock cap drifted from HOMEGOLF"),
            });
        }
        if self.admitted_comparison_classes != track_contract.comparison_policy.admitted_classes {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("comparison classes drifted from the HOMEGOLF policy"),
            });
        }
        if self.leaderboard_equivalence_rule
            != track_contract.comparison_policy.leaderboard_equivalence_rule
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("leaderboard equivalence rule drifted"),
            });
        }
        if self.exact_public_hardware_equivalence_required
            != track_contract
                .hardware_manifest_contract
                .exact_public_hardware_equivalence_required
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("exact_public_hardware_equivalence_required drifted"),
            });
        }
        if !self.optional_h100_admitted {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("optional_h100_admitted must stay true"),
            });
        }
        if !self.score_semantics_preserved || !self.comparison_policy_preserved {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from(
                    "score_semantics_preserved and comparison_policy_preserved must stay true",
                ),
            });
        }
        if !self
            .manifest_entries
            .iter()
            .any(|entry| entry.hardware_class_id == "local_apple_silicon_metal"
                && entry.presence == ParameterGolfHomegolfHardwarePresence::PresentMeasured)
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("primary Apple Silicon entry was missing"),
            });
        }
        if !self
            .manifest_entries
            .iter()
            .any(|entry| entry.hardware_class_id == "home_consumer_cuda_node"
                && entry.presence == ParameterGolfHomegolfHardwarePresence::PresentMeasured)
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("consumer CUDA entry was missing"),
            });
        }
        if !self
            .manifest_entries
            .iter()
            .any(|entry| entry.hardware_class_id == "optional_h100_node"
                && entry.presence == ParameterGolfHomegolfHardwarePresence::OptionalFuture)
        {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("optional H100 entry was missing"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
                detail: String::from("report_digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_mixed_hardware_manifest_report(
) -> Result<
    ParameterGolfHomegolfMixedHardwareManifestReport,
    ParameterGolfHomegolfMixedHardwareManifestError,
> {
    let track_contract: ParameterGolfHomegolfTrackContractReport = serde_json::from_slice(
        &fs::read(resolve_repo_path(HOMEGOLF_TRACK_CONTRACT_REF)).map_err(|error| {
            ParameterGolfHomegolfMixedHardwareManifestError::Read {
                path: String::from(HOMEGOLF_TRACK_CONTRACT_REF),
                error,
            }
        })?,
    )?;
    let optional_h100_admitted = track_contract
        .hardware_manifest_contract
        .admitted_hardware_classes
        .iter()
        .any(|class| class.hardware_class_id == "optional_h100_node");
    if !optional_h100_admitted {
        return Err(ParameterGolfHomegolfMixedHardwareManifestError::InvalidReport {
            detail: String::from("HOMEGOLF track contract no longer admits optional_h100_node"),
        });
    }

    let mut report = ParameterGolfHomegolfMixedHardwareManifestReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_mixed_hardware_manifest.v1"),
        track_id: track_contract.track_id.clone(),
        mixed_manifest_id: String::from("home_tailnet_plus_optional_h100.v1"),
        source_track_contract_ref: String::from(HOMEGOLF_TRACK_CONTRACT_REF),
        artifact_cap_bytes: track_contract.artifact_cap_bytes,
        wallclock_cap_seconds: track_contract.wallclock_cap_seconds,
        exact_public_hardware_equivalence_required: track_contract
            .hardware_manifest_contract
            .exact_public_hardware_equivalence_required,
        admitted_comparison_classes: track_contract.comparison_policy.admitted_classes.clone(),
        leaderboard_equivalence_rule: track_contract
            .comparison_policy
            .leaderboard_equivalence_rule
            .clone(),
        manifest_entries: vec![
            ParameterGolfHomegolfMixedHardwareEntry {
                member_id: String::from("local-m5-primary"),
                hardware_class_id: String::from("local_apple_silicon_metal"),
                required_for_track: true,
                presence: ParameterGolfHomegolfHardwarePresence::PresentMeasured,
                execution_backend_label: String::from("mlx"),
                role_detail: String::from("primary retained short-run coordinator/worker"),
                detail: String::from(
                    "This is the currently admitted local Apple Silicon workstation used for HOMEGOLF short runs and retained proofs.",
                ),
            },
            ParameterGolfHomegolfMixedHardwareEntry {
                member_id: String::from("home-rtx4080-node"),
                hardware_class_id: String::from("home_consumer_cuda_node"),
                required_for_track: true,
                presence: ParameterGolfHomegolfHardwarePresence::PresentMeasured,
                execution_backend_label: String::from("cuda"),
                role_detail: String::from("consumer CUDA contributor"),
                detail: String::from(
                    "This is the currently admitted home consumer NVIDIA node that participates in mixed-device HOMEGOLF retained runs.",
                ),
            },
            ParameterGolfHomegolfMixedHardwareEntry {
                member_id: String::from("macbook-m2-peer"),
                hardware_class_id: String::from("secondary_apple_silicon_peer"),
                required_for_track: false,
                presence: ParameterGolfHomegolfHardwarePresence::OptionalOffline,
                execution_backend_label: String::from("mlx"),
                role_detail: String::from("secondary Apple Silicon peer when reachable"),
                detail: String::from(
                    "The secondary Apple Silicon device stays in the manifest even when it is asleep or unreachable during a retained 10-minute window.",
                ),
            },
            ParameterGolfHomegolfMixedHardwareEntry {
                member_id: String::from("future-h100-slot-01"),
                hardware_class_id: String::from("optional_h100_node"),
                required_for_track: false,
                presence: ParameterGolfHomegolfHardwarePresence::OptionalFuture,
                execution_backend_label: String::from("cuda"),
                role_detail: String::from("future dense-trainer accelerator slot"),
                detail: String::from(
                    "One future H100 node may join this same HOMEGOLF track later without changing score semantics, wallclock policy, artifact accounting, or comparison law.",
                ),
            },
        ],
        optional_h100_admitted,
        score_semantics_preserved: true,
        comparison_policy_preserved: true,
        claim_boundary: String::from(
            "This retained manifest proves that HOMEGOLF already admits optional H100 capacity under the same track law. It does not claim that an H100 has already participated in a retained HOMEGOLF run, only that future H100 nodes can join without creating a different benchmark philosophy or score-comparison standard.",
        ),
        summary: String::from(
            "HOMEGOLF now has one explicit mixed-hardware manifest example covering the current home Tailnet devices plus one optional future H100 slot, while keeping the same public-baseline comparison law and the same non-equivalence rule versus the official 8xH100 leaderboard posture.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate(&track_contract)?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_mixed_hardware_manifest_report(
    output_path: &Path,
) -> Result<
    ParameterGolfHomegolfMixedHardwareManifestReport,
    ParameterGolfHomegolfMixedHardwareManifestError,
> {
    let report = build_parameter_golf_homegolf_mixed_hardware_manifest_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfMixedHardwareManifestError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfMixedHardwareManifestError::Write {
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
    hasher.update(
        serde_json::to_vec(value).expect("HOMEGOLF mixed hardware manifest should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_mixed_hardware_manifest_report,
        write_parameter_golf_homegolf_mixed_hardware_manifest_report,
        ParameterGolfHomegolfHardwarePresence,
        PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_FIXTURE_PATH,
    };

    #[test]
    fn mixed_hardware_manifest_admits_optional_h100_without_changing_policy() {
        let report =
            build_parameter_golf_homegolf_mixed_hardware_manifest_report().expect("build report");
        assert!(report.optional_h100_admitted);
        assert!(report.score_semantics_preserved);
        assert!(report.comparison_policy_preserved);
        assert!(report.manifest_entries.iter().any(|entry| {
            entry.hardware_class_id == "optional_h100_node"
                && entry.presence == ParameterGolfHomegolfHardwarePresence::OptionalFuture
        }));
    }

    #[test]
    fn write_mixed_hardware_manifest_report_persists_current_truth() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_mixed_hardware_manifest.json");
        let report = write_parameter_golf_homegolf_mixed_hardware_manifest_report(path.as_path())
            .expect("write report");
        let written = std::fs::read(path).expect("read written report");
        let decoded: serde_json::Value =
            serde_json::from_slice(&written).expect("decode written report");
        assert_eq!(
            decoded["report_digest"].as_str().expect("digest"),
            report.report_digest
        );
    }

    #[test]
    fn committed_mixed_hardware_manifest_fixture_roundtrips() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_MIXED_HARDWARE_MANIFEST_FIXTURE_PATH);
        let bytes = std::fs::read(path).expect("read fixture");
        let decoded: serde_json::Value =
            serde_json::from_slice(&bytes).expect("decode committed fixture");
        let rebuilt =
            serde_json::to_value(build_parameter_golf_homegolf_mixed_hardware_manifest_report().expect("build"))
                .expect("serialize rebuilt report");
        assert_eq!(decoded, rebuilt);
    }
}
