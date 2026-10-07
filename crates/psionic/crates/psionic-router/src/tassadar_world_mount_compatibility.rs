use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::TassadarModuleTrustPosture;
use psionic_runtime::seeded_tassadar_module_trust_capability_bundles;

use crate::TassadarPlannerExecutorWasmImportPosture;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";

/// Outcome of one mount-time compatibility negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorldMountNegotiationOutcome {
    Allowed,
    Denied,
    Unresolved,
}

/// Typed reason one executor family stayed incompatible with a world mount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorldMountNegotiationRefusalReason {
    TrustTierInsufficient,
    ImportPostureIncompatible,
    EvidenceRequirementMissing,
    ModuleDependencyMissing,
    ValidatorBindingMissing,
}

/// World-mount compatibility descriptor for one exact-compute lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorldMountCompatibilityDescriptor {
    pub mount_id: String,
    pub required_trust_posture: TassadarModuleTrustPosture,
    pub required_import_posture: TassadarPlannerExecutorWasmImportPosture,
    pub minimum_benchmark_ref_count: u32,
    pub required_module_refs: Vec<String>,
    pub validator_binding_required: bool,
    pub note: String,
}

/// Negotiable router-side surface for one executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorWorldMountSurface {
    pub surface_id: String,
    pub supported_trust_postures: Vec<TassadarModuleTrustPosture>,
    pub import_posture: TassadarPlannerExecutorWasmImportPosture,
    pub minimum_benchmark_ref_count: u32,
    pub available_module_refs: Vec<String>,
    pub validator_binding_available: bool,
    pub claim_boundary: String,
}

/// One mount negotiation case in the committed router report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorldMountNegotiationCaseReport {
    pub descriptor: TassadarWorldMountCompatibilityDescriptor,
    pub outcome: TassadarWorldMountNegotiationOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarWorldMountNegotiationRefusalReason>,
    pub note: String,
}

/// Router-owned world-mount compatibility report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorldMountCompatibilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub surface: TassadarExecutorWorldMountSurface,
    pub allowed_case_count: u32,
    pub denied_case_count: u32,
    pub unresolved_case_count: u32,
    pub case_reports: Vec<TassadarWorldMountNegotiationCaseReport>,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarWorldMountCompatibilityReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Evaluates one world-mount compatibility descriptor.
pub fn negotiate_tassadar_world_mount_compatibility(
    descriptor: &TassadarWorldMountCompatibilityDescriptor,
    surface: &TassadarExecutorWorldMountSurface,
) -> TassadarWorldMountNegotiationCaseReport {
    if descriptor.required_import_posture != surface.import_posture {
        return TassadarWorldMountNegotiationCaseReport {
            descriptor: descriptor.clone(),
            outcome: TassadarWorldMountNegotiationOutcome::Denied,
            refusal_reason: Some(
                TassadarWorldMountNegotiationRefusalReason::ImportPostureIncompatible,
            ),
            note: String::from(
                "the executor lane is capable enough to run, but its import posture is not compatible with this world mount",
            ),
        };
    }
    if descriptor.required_trust_posture
        > *surface
            .supported_trust_postures
            .iter()
            .max()
            .unwrap_or(&TassadarModuleTrustPosture::ResearchOnly)
    {
        return TassadarWorldMountNegotiationCaseReport {
            descriptor: descriptor.clone(),
            outcome: TassadarWorldMountNegotiationOutcome::Denied,
            refusal_reason: Some(TassadarWorldMountNegotiationRefusalReason::TrustTierInsufficient),
            note: String::from(
                "the executor family does not publish a trust posture high enough for the requested world mount",
            ),
        };
    }
    if descriptor.minimum_benchmark_ref_count > surface.minimum_benchmark_ref_count {
        return TassadarWorldMountNegotiationCaseReport {
            descriptor: descriptor.clone(),
            outcome: TassadarWorldMountNegotiationOutcome::Denied,
            refusal_reason: Some(
                TassadarWorldMountNegotiationRefusalReason::EvidenceRequirementMissing,
            ),
            note: String::from(
                "the executor family benchmark lineage is too weak for the world-mount evidence requirement",
            ),
        };
    }
    if descriptor
        .required_module_refs
        .iter()
        .any(|module_ref| !surface.available_module_refs.contains(module_ref))
    {
        return TassadarWorldMountNegotiationCaseReport {
            descriptor: descriptor.clone(),
            outcome: TassadarWorldMountNegotiationOutcome::Unresolved,
            refusal_reason: Some(
                TassadarWorldMountNegotiationRefusalReason::ModuleDependencyMissing,
            ),
            note: String::from(
                "world-mount negotiation stays unresolved when a required module dependency is not currently available on the executor lane",
            ),
        };
    }
    if descriptor.validator_binding_required && !surface.validator_binding_available {
        return TassadarWorldMountNegotiationCaseReport {
            descriptor: descriptor.clone(),
            outcome: TassadarWorldMountNegotiationOutcome::Denied,
            refusal_reason: Some(
                TassadarWorldMountNegotiationRefusalReason::ValidatorBindingMissing,
            ),
            note: String::from(
                "the world mount requires validator binding, but the current executor lane does not publish that binding",
            ),
        };
    }
    TassadarWorldMountNegotiationCaseReport {
        descriptor: descriptor.clone(),
        outcome: TassadarWorldMountNegotiationOutcome::Allowed,
        refusal_reason: None,
        note: String::from(
            "world-mount compatibility stays explicit across trust, imports, evidence, module dependencies, and validator bindings",
        ),
    }
}

/// Builds the committed world-mount compatibility report.
#[must_use]
pub fn build_tassadar_world_mount_compatibility_report() -> TassadarWorldMountCompatibilityReport {
    let bundles = seeded_tassadar_module_trust_capability_bundles();
    let mut supported_trust_postures = bundles
        .iter()
        .map(|bundle| bundle.trust_posture)
        .collect::<Vec<_>>();
    supported_trust_postures.sort();
    supported_trust_postures.dedup();
    let mut available_module_refs = bundles
        .iter()
        .filter(|bundle| bundle.trust_posture != TassadarModuleTrustPosture::ResearchOnly)
        .map(|bundle| bundle.module_ref.clone())
        .collect::<Vec<_>>();
    available_module_refs.sort();
    available_module_refs.dedup();
    let surface = TassadarExecutorWorldMountSurface {
        surface_id: String::from("tassadar.executor_world_mount_surface.v1"),
        supported_trust_postures,
        import_posture: TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
        minimum_benchmark_ref_count: 2,
        available_module_refs,
        validator_binding_available: true,
        claim_boundary: String::from(
            "this router surface keeps executor-family world-mount negotiation explicit and does not treat mount compatibility as accepted-outcome or settlement authority",
        ),
    };
    let case_reports = vec![
        negotiate_tassadar_world_mount_compatibility(
            &TassadarWorldMountCompatibilityDescriptor {
                mount_id: String::from("mount.benchmark_graph"),
                required_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                required_import_posture:
                    TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
                minimum_benchmark_ref_count: 2,
                required_module_refs: vec![
                    String::from("frontier_relax_core@1.0.0"),
                    String::from("checkpoint_backtrack_core@1.0.0"),
                ],
                validator_binding_required: false,
                note: String::from(
                    "benchmark-scoped graph mount with deterministic imports and internal module dependencies only",
                ),
            },
            &surface,
        ),
        negotiate_tassadar_world_mount_compatibility(
            &TassadarWorldMountCompatibilityDescriptor {
                mount_id: String::from("mount.strict_no_imports"),
                required_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                required_import_posture: TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                minimum_benchmark_ref_count: 2,
                required_module_refs: vec![String::from("frontier_relax_core@1.0.0")],
                validator_binding_required: false,
                note: String::from(
                    "strict no-import mount should refuse even a capable lane when import posture is too wide",
                ),
            },
            &surface,
        ),
        negotiate_tassadar_world_mount_compatibility(
            &TassadarWorldMountCompatibilityDescriptor {
                mount_id: String::from("mount.validator_search"),
                required_trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                required_import_posture:
                    TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
                minimum_benchmark_ref_count: 2,
                required_module_refs: vec![String::from("candidate_select_core@1.1.0")],
                validator_binding_required: true,
                note: String::from(
                    "validator-attached search mount keeps trust and validator requirements explicit",
                ),
            },
            &surface,
        ),
        negotiate_tassadar_world_mount_compatibility(
            &TassadarWorldMountCompatibilityDescriptor {
                mount_id: String::from("mount.missing_dependency"),
                required_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
                required_import_posture:
                    TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
                minimum_benchmark_ref_count: 2,
                required_module_refs: vec![String::from("branch_prune_core@0.1.0")],
                validator_binding_required: false,
                note: String::from(
                    "missing-module mount stays unresolved rather than being misreported as compatible",
                ),
            },
            &surface,
        ),
    ];
    let mut report = TassadarWorldMountCompatibilityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.world_mount_compatibility.report.v1"),
        surface,
        allowed_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarWorldMountNegotiationOutcome::Allowed)
            .count() as u32,
        denied_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarWorldMountNegotiationOutcome::Denied)
            .count() as u32,
        unresolved_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarWorldMountNegotiationOutcome::Unresolved)
            .count() as u32,
        case_reports,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical task-scoped resolution outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of authority-facing mount policy outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this router report keeps world-mount compatibility negotiation explicit across trust, imports, evidence, dependencies, and validator policy. It does not imply accepted-outcome or settlement closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "World-mount compatibility report now freezes {} allowed cases, {} denied cases, and {} unresolved cases.",
        report.allowed_case_count, report.denied_case_count, report.unresolved_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_world_mount_compatibility_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_world_mount_compatibility_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_world_mount_compatibility_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWorldMountCompatibilityReport, TassadarWorldMountCompatibilityReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorldMountCompatibilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_world_mount_compatibility_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorldMountCompatibilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_world_mount_compatibility_report(
    path: impl AsRef<Path>,
) -> Result<TassadarWorldMountCompatibilityReport, TassadarWorldMountCompatibilityReportError> {
    read_json(path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarWorldMountCompatibilityReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarWorldMountCompatibilityReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorldMountCompatibilityReportError::Deserialize {
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
    use super::{
        TassadarWorldMountNegotiationOutcome, build_tassadar_world_mount_compatibility_report,
        load_tassadar_world_mount_compatibility_report,
        tassadar_world_mount_compatibility_report_path,
    };

    #[test]
    fn world_mount_compatibility_report_keeps_allow_deny_and_unresolved_explicit() {
        let report = build_tassadar_world_mount_compatibility_report();

        assert_eq!(report.allowed_case_count, 2);
        assert_eq!(report.denied_case_count, 1);
        assert_eq!(report.unresolved_case_count, 1);
        assert!(report.case_reports.iter().any(|case| {
            case.outcome == TassadarWorldMountNegotiationOutcome::Denied
                && case.descriptor.mount_id == "mount.strict_no_imports"
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.outcome == TassadarWorldMountNegotiationOutcome::Unresolved
                && case.descriptor.mount_id == "mount.missing_dependency"
        }));
    }

    #[test]
    fn world_mount_compatibility_report_matches_committed_truth() {
        let expected = build_tassadar_world_mount_compatibility_report();
        let committed = load_tassadar_world_mount_compatibility_report(
            tassadar_world_mount_compatibility_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
