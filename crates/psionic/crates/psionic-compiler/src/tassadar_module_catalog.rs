use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{TassadarModuleTrustPosture, seeded_tassadar_computational_module_manifests};

pub const TASSADAR_MODULE_CATALOG_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_catalog_report.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One reusable catalog entry in the bounded module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogEntry {
    /// Stable entry identifier.
    pub entry_id: String,
    /// Stable module ref.
    pub module_ref: String,
    /// Capability labels advertised by the module.
    pub capability_labels: Vec<String>,
    /// Supported workload families.
    pub workload_families: Vec<String>,
    /// Typed trust posture.
    pub trust_posture: TassadarModuleTrustPosture,
    /// Stable benchmark refs gating the entry.
    pub benchmark_refs: Vec<String>,
    /// Reuse-rate metric in basis points across seeded consumer families.
    pub reuse_rate_bps: u16,
    /// Held-out-program lift in basis points from selecting the module.
    pub held_out_program_lift_bps: i16,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Compiler-owned report for the bounded module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Ordered catalog entries.
    pub entries: Vec<TassadarModuleCatalogEntry>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// One machine-legible lookup request against the module catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCatalogLookupRequest {
    /// Requested capability label.
    pub capability_label: String,
    /// Requested workload family.
    pub workload_family: String,
    /// Minimum trust posture accepted by the caller.
    pub minimum_trust_posture: TassadarModuleTrustPosture,
    /// Minimum benchmark-ref count accepted by the caller.
    pub minimum_benchmark_ref_count: u32,
}

/// Failure while resolving the bounded module catalog.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleCatalogLookupError {
    /// No entry matched the requested capability and workload family.
    #[error(
        "no module catalog entry matched capability `{capability_label}` for workload `{workload_family}`"
    )]
    NoCatalogMatch {
        capability_label: String,
        workload_family: String,
    },
    /// More than one entry matched the request and the result would be ambiguous.
    #[error(
        "module catalog lookup for capability `{capability_label}` and workload `{workload_family}` was ambiguous across {entry_ids:?}"
    )]
    AmbiguousCatalogMatch {
        capability_label: String,
        workload_family: String,
        entry_ids: Vec<String>,
    },
    /// The best-matching entry did not carry enough benchmark evidence.
    #[error(
        "module catalog entry `{entry_id}` only carried {actual} benchmark refs, below required {required}"
    )]
    InsufficientEvidence {
        entry_id: String,
        actual: u32,
        required: u32,
    },
}

/// Builds the machine-legible bounded module catalog report.
#[must_use]
pub fn build_tassadar_module_catalog_report() -> TassadarModuleCatalogReport {
    let manifests = seeded_tassadar_computational_module_manifests()
        .into_iter()
        .map(|manifest| (manifest.module_ref.clone(), manifest))
        .collect::<std::collections::BTreeMap<_, _>>();
    let frontier = manifests
        .get("frontier_relax_core@1.0.0")
        .expect("frontier manifest");
    let candidate = manifests
        .get("candidate_select_core@1.1.0")
        .expect("candidate manifest");
    let checkpoint = manifests
        .get("checkpoint_backtrack_core@1.0.0")
        .expect("checkpoint manifest");
    let entries = vec![
        TassadarModuleCatalogEntry {
            entry_id: String::from("catalog.frontier_relax_core.v1"),
            module_ref: frontier.module_ref.clone(),
            capability_labels: vec![
                String::from("frontier_relaxation"),
                String::from("shortest_path_primitive"),
            ],
            workload_families: vec![
                String::from("clrs_shortest_path"),
                String::from("clrs_wasm_shortest_path"),
            ],
            trust_posture: frontier.trust_posture,
            benchmark_refs: frontier.benchmark_lineage_refs.clone(),
            reuse_rate_bps: 10000,
            held_out_program_lift_bps: 2400,
            claim_boundary: String::from(
                "frontier_relax_core remains bounded to frontier-relaxation and shortest-path family reuse; it is not a generic graph-program catalog row",
            ),
        },
        TassadarModuleCatalogEntry {
            entry_id: String::from("catalog.candidate_select_core.v1"),
            module_ref: candidate.module_ref.clone(),
            capability_labels: vec![
                String::from("candidate_selection"),
                String::from("bounded_search"),
                String::from("matching_primitive"),
            ],
            workload_families: vec![
                String::from("hungarian_matching"),
                String::from("verifier_search"),
            ],
            trust_posture: candidate.trust_posture,
            benchmark_refs: candidate.benchmark_lineage_refs.clone(),
            reuse_rate_bps: 6666,
            held_out_program_lift_bps: 1800,
            claim_boundary: String::from(
                "candidate_select_core is discoverable as a bounded reusable primitive for matching and verifier-search families only",
            ),
        },
        TassadarModuleCatalogEntry {
            entry_id: String::from("catalog.checkpoint_backtrack_core.v1"),
            module_ref: checkpoint.module_ref.clone(),
            capability_labels: vec![
                String::from("checkpointing"),
                String::from("bounded_search"),
                String::from("rollback_primitive"),
            ],
            workload_families: vec![
                String::from("verifier_search"),
                String::from("sudoku_v0_search"),
            ],
            trust_posture: checkpoint.trust_posture,
            benchmark_refs: checkpoint.benchmark_lineage_refs.clone(),
            reuse_rate_bps: 5000,
            held_out_program_lift_bps: 900,
            claim_boundary: String::from(
                "checkpoint_backtrack_core is discoverable as a bounded checkpoint and rollback primitive rather than a generic solver module",
            ),
        },
    ];
    let mut report = TassadarModuleCatalogReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_catalog.report.v1"),
        entries,
        claim_boundary: String::from(
            "this compiler-owned catalog report freezes reusable module primitives keyed by capability, workload family, trust posture, and benchmark lineage. It does not make every installed module automatically reusable and it does not widen served capability by implication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module catalog report now freezes {} reusable entries with max reuse_rate={}bps and max held_out_program_lift={}bps.",
        report.entries.len(),
        report
            .entries
            .iter()
            .map(|entry| entry.reuse_rate_bps)
            .max()
            .unwrap_or(0),
        report
            .entries
            .iter()
            .map(|entry| entry.held_out_program_lift_bps)
            .max()
            .unwrap_or(0),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_module_catalog_report|", &report);
    report
}

/// Resolves one deterministic catalog lookup.
pub fn lookup_tassadar_module_catalog_entry(
    report: &TassadarModuleCatalogReport,
    request: &TassadarModuleCatalogLookupRequest,
) -> Result<TassadarModuleCatalogEntry, TassadarModuleCatalogLookupError> {
    let matches = report
        .entries
        .iter()
        .filter(|entry| {
            entry.trust_posture >= request.minimum_trust_posture
                && entry
                    .capability_labels
                    .iter()
                    .any(|label| label == &request.capability_label)
                && entry
                    .workload_families
                    .iter()
                    .any(|family| family == &request.workload_family)
        })
        .cloned()
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(TassadarModuleCatalogLookupError::NoCatalogMatch {
            capability_label: request.capability_label.clone(),
            workload_family: request.workload_family.clone(),
        });
    }
    if matches.len() > 1 {
        return Err(TassadarModuleCatalogLookupError::AmbiguousCatalogMatch {
            capability_label: request.capability_label.clone(),
            workload_family: request.workload_family.clone(),
            entry_ids: matches.into_iter().map(|entry| entry.entry_id).collect(),
        });
    }
    let entry = matches.into_iter().next().expect("one match");
    if entry.benchmark_refs.len() < request.minimum_benchmark_ref_count as usize {
        return Err(TassadarModuleCatalogLookupError::InsufficientEvidence {
            entry_id: entry.entry_id,
            actual: entry.benchmark_refs.len() as u32,
            required: request.minimum_benchmark_ref_count,
        });
    }
    Ok(entry)
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_module_catalog_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_CATALOG_REPORT_REF)
}

/// Writes the committed module-catalog report.
pub fn write_tassadar_module_catalog_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleCatalogReport, TassadarModuleCatalogReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleCatalogReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_catalog_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleCatalogReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

/// Report persistence failure.
#[derive(Debug, Error)]
pub enum TassadarModuleCatalogReportError {
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

#[cfg(test)]
pub fn load_tassadar_module_catalog_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModuleCatalogReport, TassadarModuleCatalogReportError> {
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
) -> Result<T, TassadarModuleCatalogReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarModuleCatalogReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarModuleCatalogReportError::Deserialize {
        path: path.display().to_string(),
        error,
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
        TassadarModuleCatalogLookupError, TassadarModuleCatalogLookupRequest,
        build_tassadar_module_catalog_report, load_tassadar_module_catalog_report,
        lookup_tassadar_module_catalog_entry, tassadar_module_catalog_report_path,
    };
    use psionic_ir::TassadarModuleTrustPosture;

    #[test]
    fn module_catalog_resolves_frontier_relax_core_deterministically() {
        let report = build_tassadar_module_catalog_report();
        let request = TassadarModuleCatalogLookupRequest {
            capability_label: String::from("frontier_relaxation"),
            workload_family: String::from("clrs_shortest_path"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 2,
        };

        let entry = lookup_tassadar_module_catalog_entry(&report, &request).expect("entry");

        assert_eq!(entry.module_ref, "frontier_relax_core@1.0.0");
        assert_eq!(entry.reuse_rate_bps, 10000);
    }

    #[test]
    fn module_catalog_refuses_ambiguous_bounded_search_lookup() {
        let report = build_tassadar_module_catalog_report();
        let request = TassadarModuleCatalogLookupRequest {
            capability_label: String::from("bounded_search"),
            workload_family: String::from("verifier_search"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 1,
        };

        let error = lookup_tassadar_module_catalog_entry(&report, &request).expect_err("error");

        assert_eq!(
            error,
            TassadarModuleCatalogLookupError::AmbiguousCatalogMatch {
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                entry_ids: vec![
                    String::from("catalog.candidate_select_core.v1"),
                    String::from("catalog.checkpoint_backtrack_core.v1"),
                ],
            }
        );
    }

    #[test]
    fn module_catalog_refuses_insufficient_evidence() {
        let report = build_tassadar_module_catalog_report();
        let request = TassadarModuleCatalogLookupRequest {
            capability_label: String::from("checkpointing"),
            workload_family: String::from("sudoku_v0_search"),
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            minimum_benchmark_ref_count: 3,
        };

        let error = lookup_tassadar_module_catalog_entry(&report, &request).expect_err("error");

        assert_eq!(
            error,
            TassadarModuleCatalogLookupError::InsufficientEvidence {
                entry_id: String::from("catalog.checkpoint_backtrack_core.v1"),
                actual: 2,
                required: 3,
            }
        );
    }

    #[test]
    fn module_catalog_report_matches_committed_truth() {
        let expected = build_tassadar_module_catalog_report();
        let committed = load_tassadar_module_catalog_report(tassadar_module_catalog_report_path())
            .expect("committed report");

        assert_eq!(committed, expected);
    }
}
