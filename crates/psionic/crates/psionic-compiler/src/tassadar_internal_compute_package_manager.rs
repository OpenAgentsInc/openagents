use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarModuleCatalogLookupError, TassadarModuleCatalogLookupRequest,
    build_tassadar_module_catalog_report, lookup_tassadar_module_catalog_entry,
};
use psionic_ir::TassadarModuleTrustPosture;

pub const TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_report.json";
pub const TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID: &str =
    "cpu_reference_current_host";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputePackageSolverStatus {
    Exact,
    Refusal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputePackageSolverRefusalReason {
    AmbiguousDependencySolver,
    InsufficientBenchmarkLineage,
    PortabilityEnvelopeMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageEntry {
    pub package_id: String,
    pub module_refs: Vec<String>,
    pub workload_family: String,
    pub required_profile_ids: Vec<String>,
    pub portability_envelope_id: String,
    pub benchmark_refs: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageSolverCase {
    pub case_id: String,
    pub requested_capability_labels: Vec<String>,
    pub workload_family: String,
    pub requested_portability_envelope_id: String,
    pub status: TassadarInternalComputePackageSolverStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_package_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarInternalComputePackageSolverRefusalReason>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageManagerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub package_entries: Vec<TassadarInternalComputePackageEntry>,
    pub solver_cases: Vec<TassadarInternalComputePackageSolverCase>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub public_package_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalComputePackageManagerReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_internal_compute_package_manager_report(
) -> TassadarInternalComputePackageManagerReport {
    let package_entries = vec![
        clrs_shortest_path_package(),
        hungarian_matching_package(),
        verifier_search_package(),
    ];
    let solver_cases = vec![
        exact_case(
            "solver.clrs_shortest_path_stack.v1",
            &["frontier_relaxation"],
            "clrs_shortest_path",
            "package.clrs_shortest_path_stack.v1",
            "the bounded package manager resolves the shortest-path stack to one benchmarked frontier-relax package instead of leaving catalog selection implicit",
        ),
        exact_case(
            "solver.hungarian_matching_stack.v1",
            &["matching_primitive"],
            "hungarian_matching",
            "package.hungarian_matching_stack.v1",
            "the bounded package manager resolves the Hungarian matching stack to one benchmarked candidate-selection package rather than one-off manual bundle curation",
        ),
        exact_case(
            "solver.verifier_search_stack.v1",
            &["candidate_selection", "checkpointing"],
            "verifier_search",
            "package.verifier_search_stack.v1",
            "the bounded package manager resolves verifier search to one explicit multi-module package with typed dependency order instead of hidden orchestration",
        ),
        ambiguous_refusal_case(),
        insufficient_evidence_refusal_case(),
        portability_mismatch_refusal_case(),
    ];
    let mut report = TassadarInternalComputePackageManagerReport {
        schema_version: 1,
        report_id: String::from("tassadar.internal_compute_package_manager.report.v1"),
        package_entries,
        solver_cases,
        exact_case_count: 0,
        refusal_case_count: 0,
        public_package_ids: vec![
            String::from("package.clrs_shortest_path_stack.v1"),
            String::from("package.hungarian_matching_stack.v1"),
            String::from("package.verifier_search_stack.v1"),
        ],
        portability_envelope_ids: vec![String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        )],
        claim_boundary: String::from(
            "this compiler-owned report freezes one bounded internal-compute package manager over named packages, package-local dependency order, trust posture, benchmark lineage, and portability envelopes. It does not imply arbitrary package discovery, arbitrary dependency solving, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.exact_case_count = report
        .solver_cases
        .iter()
        .filter(|case| case.status == TassadarInternalComputePackageSolverStatus::Exact)
        .count() as u32;
    report.refusal_case_count = report
        .solver_cases
        .iter()
        .filter(|case| case.status == TassadarInternalComputePackageSolverStatus::Refusal)
        .count() as u32;
    report.summary = format!(
        "Internal compute package manager report freezes {} package entries across {} exact solver cases and {} refusal cases.",
        report.package_entries.len(),
        report.exact_case_count,
        report.refusal_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_internal_compute_package_manager_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_internal_compute_package_manager_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_REPORT_REF)
}

pub fn write_tassadar_internal_compute_package_manager_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalComputePackageManagerReport, TassadarInternalComputePackageManagerReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComputePackageManagerReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_compute_package_manager_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComputePackageManagerReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn clrs_shortest_path_package() -> TassadarInternalComputePackageEntry {
    let entry = lookup_tassadar_module_catalog_entry(
        &build_tassadar_module_catalog_report(),
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("frontier_relaxation"),
            workload_family: String::from("clrs_shortest_path"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 2,
        },
    )
    .expect("shortest-path catalog entry");
    TassadarInternalComputePackageEntry {
        package_id: String::from("package.clrs_shortest_path_stack.v1"),
        module_refs: vec![entry.module_ref],
        workload_family: String::from("clrs_shortest_path"),
        required_profile_ids: vec![String::from(
            "tassadar.internal_compute.component_model_abi.v1",
        )],
        portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        benchmark_refs: entry.benchmark_refs,
        claim_boundary: String::from(
            "this bounded package remains one shortest-path stack rooted in frontier-relaxation only; it is not a generic graph-package manager row",
        ),
    }
}

fn hungarian_matching_package() -> TassadarInternalComputePackageEntry {
    let entry = lookup_tassadar_module_catalog_entry(
        &build_tassadar_module_catalog_report(),
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("matching_primitive"),
            workload_family: String::from("hungarian_matching"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 2,
        },
    )
    .expect("hungarian catalog entry");
    TassadarInternalComputePackageEntry {
        package_id: String::from("package.hungarian_matching_stack.v1"),
        module_refs: vec![entry.module_ref],
        workload_family: String::from("hungarian_matching"),
        required_profile_ids: vec![String::from(
            "tassadar.internal_compute.component_model_abi.v1",
        )],
        portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        benchmark_refs: entry.benchmark_refs,
        claim_boundary: String::from(
            "this bounded package remains one matching stack rooted in candidate selection only; it is not a generic combinatorial-optimization package row",
        ),
    }
}

fn verifier_search_package() -> TassadarInternalComputePackageEntry {
    let report = build_tassadar_module_catalog_report();
    let candidate_entry = lookup_tassadar_module_catalog_entry(
        &report,
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("candidate_selection"),
            workload_family: String::from("verifier_search"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 2,
        },
    )
    .expect("candidate selection entry");
    let checkpoint_entry = lookup_tassadar_module_catalog_entry(
        &report,
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("checkpointing"),
            workload_family: String::from("verifier_search"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 2,
        },
    )
    .expect("checkpoint entry");
    let benchmark_refs = candidate_entry
        .benchmark_refs
        .into_iter()
        .chain(checkpoint_entry.benchmark_refs)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    TassadarInternalComputePackageEntry {
        package_id: String::from("package.verifier_search_stack.v1"),
        module_refs: vec![candidate_entry.module_ref, checkpoint_entry.module_ref],
        workload_family: String::from("verifier_search"),
        required_profile_ids: vec![String::from(
            "tassadar.internal_compute.component_model_abi.v1",
        )],
        portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        benchmark_refs,
        claim_boundary: String::from(
            "this bounded package remains one verifier-search stack rooted in explicit candidate-selection plus checkpoint primitives; it is not arbitrary bounded-search auto-composition",
        ),
    }
}

fn exact_case(
    case_id: &str,
    requested_capability_labels: &[&str],
    workload_family: &str,
    selected_package_id: &str,
    note: &str,
) -> TassadarInternalComputePackageSolverCase {
    TassadarInternalComputePackageSolverCase {
        case_id: String::from(case_id),
        requested_capability_labels: requested_capability_labels
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        workload_family: String::from(workload_family),
        requested_portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarInternalComputePackageSolverStatus::Exact,
        selected_package_id: Some(String::from(selected_package_id)),
        refusal_reason: None,
        note: String::from(note),
    }
}

fn ambiguous_refusal_case() -> TassadarInternalComputePackageSolverCase {
    let refusal_reason = match lookup_tassadar_module_catalog_entry(
        &build_tassadar_module_catalog_report(),
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("bounded_search"),
            workload_family: String::from("verifier_search"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 1,
        },
    )
    .expect_err("ambiguous bounded-search lookup")
    {
        TassadarModuleCatalogLookupError::AmbiguousCatalogMatch { .. } => {
            TassadarInternalComputePackageSolverRefusalReason::AmbiguousDependencySolver
        }
        _ => panic!("unexpected refusal"),
    };
    TassadarInternalComputePackageSolverCase {
        case_id: String::from("solver.ambiguous_bounded_search_auto.v1"),
        requested_capability_labels: vec![String::from("bounded_search")],
        workload_family: String::from("verifier_search"),
        requested_portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarInternalComputePackageSolverStatus::Refusal,
        selected_package_id: None,
        refusal_reason: Some(refusal_reason),
        note: String::from(
            "auto-resolution for generic bounded_search stays refused when candidate-selection and checkpoint primitives both match verifier_search under the same trust posture",
        ),
    }
}

fn insufficient_evidence_refusal_case() -> TassadarInternalComputePackageSolverCase {
    let refusal_reason = match lookup_tassadar_module_catalog_entry(
        &build_tassadar_module_catalog_report(),
        &TassadarModuleCatalogLookupRequest {
            capability_label: String::from("matching_primitive"),
            workload_family: String::from("hungarian_matching"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 9,
        },
    )
    .expect_err("insufficient evidence lookup")
    {
        TassadarModuleCatalogLookupError::InsufficientEvidence { .. } => {
            TassadarInternalComputePackageSolverRefusalReason::InsufficientBenchmarkLineage
        }
        _ => panic!("unexpected refusal"),
    };
    TassadarInternalComputePackageSolverCase {
        case_id: String::from("solver.insufficient_evidence_matching.v1"),
        requested_capability_labels: vec![String::from("matching_primitive")],
        workload_family: String::from("hungarian_matching"),
        requested_portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPUTE_PACKAGE_MANAGER_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarInternalComputePackageSolverStatus::Refusal,
        selected_package_id: None,
        refusal_reason: Some(refusal_reason),
        note: String::from(
            "package resolution stays refused when requested benchmark lineage exceeds the bounded catalog evidence carried by the matching stack",
        ),
    }
}

fn portability_mismatch_refusal_case() -> TassadarInternalComputePackageSolverCase {
    TassadarInternalComputePackageSolverCase {
        case_id: String::from("solver.portability_mismatch_gpu_portable.v1"),
        requested_capability_labels: vec![
            String::from("candidate_selection"),
            String::from("checkpointing"),
        ],
        workload_family: String::from("verifier_search"),
        requested_portability_envelope_id: String::from("gpu_portable_cross_host"),
        status: TassadarInternalComputePackageSolverStatus::Refusal,
        selected_package_id: None,
        refusal_reason: Some(
            TassadarInternalComputePackageSolverRefusalReason::PortabilityEnvelopeMismatch,
        ),
        note: String::from(
            "package resolution stays refused when the caller requests a portability envelope broader than the current host-cpu package line actually supports",
        ),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarInternalComputePackageManagerReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarInternalComputePackageManagerReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalComputePackageManagerReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        TassadarInternalComputePackageSolverRefusalReason,
        build_tassadar_internal_compute_package_manager_report, read_json,
        tassadar_internal_compute_package_manager_report_path,
        write_tassadar_internal_compute_package_manager_report,
    };

    #[test]
    fn internal_compute_package_manager_report_keeps_solver_and_refusals_explicit() {
        let report = build_tassadar_internal_compute_package_manager_report();

        assert_eq!(report.package_entries.len(), 3);
        assert_eq!(report.exact_case_count, 3);
        assert_eq!(report.refusal_case_count, 3);
        assert!(report.public_package_ids.contains(&String::from(
            "package.verifier_search_stack.v1"
        )));
        assert!(report.solver_cases.iter().any(|case| {
            case.refusal_reason
                == Some(TassadarInternalComputePackageSolverRefusalReason::AmbiguousDependencySolver)
        }));
    }

    #[test]
    fn internal_compute_package_manager_report_matches_committed_truth() {
        let generated = build_tassadar_internal_compute_package_manager_report();
        let committed = read_json(tassadar_internal_compute_package_manager_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_compute_package_manager_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join(format!(
            "tassadar_internal_compute_package_manager_report_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        let report = write_tassadar_internal_compute_package_manager_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");
        std::fs::remove_file(&output_path).expect("cleanup");

        assert_eq!(report, persisted);
    }
}
