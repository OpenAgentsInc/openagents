use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionBenchmarkCatalog, PsionBenchmarkFamily, PsionBenchmarkGraderInterface,
    PsionBenchmarkPackageFamily, PsionBenchmarkReceiptSet, PsionMetricKind, PsionPhaseGate,
    PsionRouteClass,
};

/// Stable schema version for the first Forge-facing Psionic benchmark-pack
/// manifest.
pub const PSION_FORGE_BENCHMARK_PACK_MANIFEST_SCHEMA_VERSION: &str =
    "psion.forge_benchmark_pack_manifest.v1";
/// Stable schema version for the first Forge-facing Psionic judge-pack manifest.
pub const PSION_FORGE_JUDGE_PACK_MANIFEST_SCHEMA_VERSION: &str =
    "psion.forge_judge_pack_manifest.v1";
/// Canonical benchmark-pack manifest fixture.
pub const PSION_FORGE_BENCHMARK_PACK_FIXTURE_PATH: &str =
    "fixtures/psion/packs/psion_forge_benchmark_pack_manifest_v1.json";
/// Canonical judge-pack manifest fixture.
pub const PSION_FORGE_JUDGE_PACK_FIXTURE_PATH: &str =
    "fixtures/psion/packs/psion_forge_judge_pack_manifest_v1.json";
/// Canonical doc path for the first Forge-facing Psionic pack manifests.
pub const PSION_FORGE_EVAL_PACK_DOC_PATH: &str = "docs/PSION_FORGE_EVAL_PACK_MANIFESTS.md";

pub const PSION_BENCHMARK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json";
pub const PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH: &str =
    "fixtures/psion/benchmarks/psion_benchmark_receipt_set_v1.json";

const BENCHMARK_PACK_DIGEST_PREFIX: &[u8] = b"psion_forge_benchmark_pack_manifest|";
const JUDGE_PACK_DIGEST_PREFIX: &[u8] = b"psion_forge_judge_pack_manifest|";

/// One typed source artifact carried by a published pack manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionForgePackSourceArtifact {
    /// Repo-local path to the source artifact.
    pub path: String,
    /// Stable SHA256 over the source artifact bytes.
    pub sha256: String,
    /// Short explanation of why the artifact is part of the pack.
    pub detail: String,
}

/// One benchmark package entry projected into the benchmark-pack manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionForgeBenchmarkPackEntry {
    /// Stable Psionic package id.
    pub package_id: String,
    /// Benchmark package family.
    pub package_family: PsionBenchmarkPackageFamily,
    /// Acceptance family when the benchmark feeds the acceptance matrix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_family: Option<PsionBenchmarkFamily>,
    /// Stable benchmark reference reused by `psionic-eval`.
    pub benchmark_ref: String,
    /// Benchmark package version.
    pub benchmark_version: String,
    /// Stable digest over the generic benchmark package.
    pub benchmark_digest: String,
    /// Stable receipt id that carries the current bounded review posture.
    pub receipt_id: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
    /// Phase gate the receipt is suitable for.
    pub phase: PsionPhaseGate,
    /// Number of benchmark items in the package.
    pub item_count: u32,
    /// Minimal metric selectors carried by the receipt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metric_kinds: Vec<PsionMetricKind>,
}

/// Judge kind projected for Forge consumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionForgeJudgeKind {
    ExactLabel,
    RubricScore,
    ExactRoute,
    ExactRefusal,
}

/// One judge or verifier entry projected into the judge-pack manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionForgeJudgePackEntry {
    /// Stable judge identifier.
    pub judge_id: String,
    /// Typed judge kind.
    pub judge_kind: PsionForgeJudgeKind,
    /// Benchmark packages that use this judge.
    pub package_ids: Vec<String>,
    /// Benchmark package families that use this judge.
    pub package_families: Vec<PsionBenchmarkPackageFamily>,
    /// Acceptance families reachable through the bound packages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_families: Vec<PsionBenchmarkFamily>,
    /// Rubric ref when this is a rubric-backed judge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubric_ref: Option<String>,
    /// Label namespace when this is an exact-label judge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_namespace: Option<String>,
    /// Accepted exact labels when this is an exact-label judge.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_labels: Vec<String>,
    /// Expected route when this is an exact-route judge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_route: Option<PsionRouteClass>,
    /// Accepted refusal codes when this is an exact-refusal judge.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_reason_codes: Vec<String>,
    /// Short explanation of what the judge enforces.
    pub detail: String,
}

/// Published benchmark-pack manifest for Forge consumption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionForgeBenchmarkPackManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable manifest identifier.
    pub pack_id: String,
    /// Human-readable pack name.
    pub display_name: String,
    /// Source benchmark catalog id.
    pub catalog_id: String,
    /// Source benchmark catalog digest.
    pub catalog_digest: String,
    /// Source benchmark receipt-set id.
    pub receipt_set_id: String,
    /// Source benchmark receipt-set digest.
    pub receipt_set_digest: String,
    /// Source artifacts that define the pack.
    pub source_artifacts: Vec<PsionForgePackSourceArtifact>,
    /// Typed benchmark package entries included in the pack.
    pub packages: Vec<PsionForgeBenchmarkPackEntry>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the manifest payload.
    pub pack_digest: String,
}

impl PsionForgeBenchmarkPackManifest {
    /// Returns the stable digest over the benchmark-pack payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.pack_digest.clear();
        stable_digest(BENCHMARK_PACK_DIGEST_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.pack_digest = self.stable_digest();
        self
    }

    /// Validates the manifest shape and stable digest.
    pub fn validate(&self) -> Result<(), PsionForgeEvalPackManifestError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_forge_benchmark_pack_manifest.schema_version",
        )?;
        if self.schema_version != PSION_FORGE_BENCHMARK_PACK_MANIFEST_SCHEMA_VERSION {
            return Err(PsionForgeEvalPackManifestError::SchemaVersionMismatch {
                expected: String::from(PSION_FORGE_BENCHMARK_PACK_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.pack_id.as_str(),
            "psion_forge_benchmark_pack_manifest.pack_id",
        )?;
        ensure_nonempty(
            self.display_name.as_str(),
            "psion_forge_benchmark_pack_manifest.display_name",
        )?;
        ensure_nonempty(
            self.catalog_id.as_str(),
            "psion_forge_benchmark_pack_manifest.catalog_id",
        )?;
        ensure_nonempty(
            self.catalog_digest.as_str(),
            "psion_forge_benchmark_pack_manifest.catalog_digest",
        )?;
        ensure_nonempty(
            self.receipt_set_id.as_str(),
            "psion_forge_benchmark_pack_manifest.receipt_set_id",
        )?;
        ensure_nonempty(
            self.receipt_set_digest.as_str(),
            "psion_forge_benchmark_pack_manifest.receipt_set_digest",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_forge_benchmark_pack_manifest.claim_boundary",
        )?;
        if self.source_artifacts.is_empty() {
            return Err(PsionForgeEvalPackManifestError::MissingField {
                field: String::from("psion_forge_benchmark_pack_manifest.source_artifacts"),
            });
        }
        let mut seen_sources = BTreeSet::new();
        for artifact in &self.source_artifacts {
            ensure_nonempty(
                artifact.path.as_str(),
                "psion_forge_benchmark_pack_manifest.source_artifacts[].path",
            )?;
            ensure_nonempty(
                artifact.sha256.as_str(),
                "psion_forge_benchmark_pack_manifest.source_artifacts[].sha256",
            )?;
            ensure_nonempty(
                artifact.detail.as_str(),
                "psion_forge_benchmark_pack_manifest.source_artifacts[].detail",
            )?;
            if !seen_sources.insert(artifact.path.as_str()) {
                return Err(PsionForgeEvalPackManifestError::DuplicateSourceArtifact {
                    path: artifact.path.clone(),
                });
            }
        }
        if self.packages.is_empty() {
            return Err(PsionForgeEvalPackManifestError::MissingField {
                field: String::from("psion_forge_benchmark_pack_manifest.packages"),
            });
        }
        let mut seen_packages = BTreeSet::new();
        for package in &self.packages {
            ensure_nonempty(
                package.package_id.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].package_id",
            )?;
            ensure_nonempty(
                package.benchmark_ref.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].benchmark_ref",
            )?;
            ensure_nonempty(
                package.benchmark_version.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].benchmark_version",
            )?;
            ensure_nonempty(
                package.benchmark_digest.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].benchmark_digest",
            )?;
            ensure_nonempty(
                package.receipt_id.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].receipt_id",
            )?;
            ensure_nonempty(
                package.receipt_digest.as_str(),
                "psion_forge_benchmark_pack_manifest.packages[].receipt_digest",
            )?;
            if !seen_packages.insert(package.package_id.as_str()) {
                return Err(PsionForgeEvalPackManifestError::DuplicatePackage {
                    package_id: package.package_id.clone(),
                });
            }
        }
        if self.pack_digest != self.stable_digest() {
            return Err(PsionForgeEvalPackManifestError::DigestMismatch {
                kind: String::from("psion_forge_benchmark_pack_manifest"),
            });
        }
        Ok(())
    }

    /// Writes the manifest as pretty JSON.
    pub fn write_json(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionForgeEvalPackManifestError> {
        write_json(output_path, self)
    }

    /// Loads the manifest from JSON.
    pub fn read_json(
        input_path: impl AsRef<Path>,
    ) -> Result<Self, PsionForgeEvalPackManifestError> {
        read_json(input_path)
    }
}

/// Published judge-pack manifest for Forge consumption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionForgeJudgePackManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable manifest identifier.
    pub pack_id: String,
    /// Human-readable pack name.
    pub display_name: String,
    /// Source benchmark catalog id.
    pub catalog_id: String,
    /// Source benchmark catalog digest.
    pub catalog_digest: String,
    /// Source artifacts that define the pack.
    pub source_artifacts: Vec<PsionForgePackSourceArtifact>,
    /// Typed judge entries included in the pack.
    pub judges: Vec<PsionForgeJudgePackEntry>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the manifest payload.
    pub pack_digest: String,
}

impl PsionForgeJudgePackManifest {
    /// Returns the stable digest over the judge-pack payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.pack_digest.clear();
        stable_digest(JUDGE_PACK_DIGEST_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.pack_digest = self.stable_digest();
        self
    }

    /// Validates the manifest shape and stable digest.
    pub fn validate(&self) -> Result<(), PsionForgeEvalPackManifestError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_forge_judge_pack_manifest.schema_version",
        )?;
        if self.schema_version != PSION_FORGE_JUDGE_PACK_MANIFEST_SCHEMA_VERSION {
            return Err(PsionForgeEvalPackManifestError::SchemaVersionMismatch {
                expected: String::from(PSION_FORGE_JUDGE_PACK_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.pack_id.as_str(),
            "psion_forge_judge_pack_manifest.pack_id",
        )?;
        ensure_nonempty(
            self.display_name.as_str(),
            "psion_forge_judge_pack_manifest.display_name",
        )?;
        ensure_nonempty(
            self.catalog_id.as_str(),
            "psion_forge_judge_pack_manifest.catalog_id",
        )?;
        ensure_nonempty(
            self.catalog_digest.as_str(),
            "psion_forge_judge_pack_manifest.catalog_digest",
        )?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_forge_judge_pack_manifest.claim_boundary",
        )?;
        if self.source_artifacts.is_empty() {
            return Err(PsionForgeEvalPackManifestError::MissingField {
                field: String::from("psion_forge_judge_pack_manifest.source_artifacts"),
            });
        }
        let mut seen_sources = BTreeSet::new();
        for artifact in &self.source_artifacts {
            ensure_nonempty(
                artifact.path.as_str(),
                "psion_forge_judge_pack_manifest.source_artifacts[].path",
            )?;
            ensure_nonempty(
                artifact.sha256.as_str(),
                "psion_forge_judge_pack_manifest.source_artifacts[].sha256",
            )?;
            ensure_nonempty(
                artifact.detail.as_str(),
                "psion_forge_judge_pack_manifest.source_artifacts[].detail",
            )?;
            if !seen_sources.insert(artifact.path.as_str()) {
                return Err(PsionForgeEvalPackManifestError::DuplicateSourceArtifact {
                    path: artifact.path.clone(),
                });
            }
        }
        if self.judges.is_empty() {
            return Err(PsionForgeEvalPackManifestError::MissingField {
                field: String::from("psion_forge_judge_pack_manifest.judges"),
            });
        }
        let mut seen_judges = BTreeSet::new();
        for judge in &self.judges {
            ensure_nonempty(
                judge.judge_id.as_str(),
                "psion_forge_judge_pack_manifest.judges[].judge_id",
            )?;
            ensure_nonempty(
                judge.detail.as_str(),
                "psion_forge_judge_pack_manifest.judges[].detail",
            )?;
            if judge.package_ids.is_empty() {
                return Err(PsionForgeEvalPackManifestError::MissingField {
                    field: format!(
                        "psion_forge_judge_pack_manifest.judges[{}].package_ids",
                        judge.judge_id
                    ),
                });
            }
            if judge.package_families.is_empty() {
                return Err(PsionForgeEvalPackManifestError::MissingField {
                    field: format!(
                        "psion_forge_judge_pack_manifest.judges[{}].package_families",
                        judge.judge_id
                    ),
                });
            }
            if !seen_judges.insert(judge.judge_id.as_str()) {
                return Err(PsionForgeEvalPackManifestError::DuplicateJudge {
                    judge_id: judge.judge_id.clone(),
                });
            }
        }
        if self.pack_digest != self.stable_digest() {
            return Err(PsionForgeEvalPackManifestError::DigestMismatch {
                kind: String::from("psion_forge_judge_pack_manifest"),
            });
        }
        Ok(())
    }

    /// Writes the manifest as pretty JSON.
    pub fn write_json(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionForgeEvalPackManifestError> {
        write_json(output_path, self)
    }

    /// Loads the manifest from JSON.
    pub fn read_json(
        input_path: impl AsRef<Path>,
    ) -> Result<Self, PsionForgeEvalPackManifestError> {
        read_json(input_path)
    }
}

/// Error while publishing Forge-facing Psionic eval pack manifests.
#[derive(Debug, Error)]
pub enum PsionForgeEvalPackManifestError {
    #[error("io error for `{path}`: {error}")]
    Io {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("json error for `{path}`: {error}")]
    Json {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("duplicate source artifact `{path}`")]
    DuplicateSourceArtifact { path: String },
    #[error("duplicate package `{package_id}`")]
    DuplicatePackage { package_id: String },
    #[error("duplicate judge `{judge_id}`")]
    DuplicateJudge { judge_id: String },
    #[error(
        "benchmark receipt-set `{receipt_set_id}` does not match benchmark catalog `{catalog_id}`"
    )]
    CatalogDigestMismatch {
        catalog_id: String,
        receipt_set_id: String,
    },
    #[error("benchmark package `{package_id}` is missing a matching receipt")]
    MissingPackageReceipt { package_id: String },
    #[error("judge `{judge_id}` is reused with incompatible definitions")]
    InconsistentJudgeReuse { judge_id: String },
    #[error("stable digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
}

/// Records the first published benchmark-pack manifest above the canonical
/// benchmark catalog and receipt set.
pub fn record_psion_forge_benchmark_pack_manifest(
    repo_root: &Path,
    catalog: &PsionBenchmarkCatalog,
    receipt_set: &PsionBenchmarkReceiptSet,
) -> Result<PsionForgeBenchmarkPackManifest, PsionForgeEvalPackManifestError> {
    if receipt_set.catalog_digest != catalog.catalog_digest {
        return Err(PsionForgeEvalPackManifestError::CatalogDigestMismatch {
            catalog_id: catalog.catalog_id.clone(),
            receipt_set_id: receipt_set.receipt_set_id.clone(),
        });
    }

    let packages = catalog
        .packages
        .iter()
        .map(|package| {
            let receipt = receipt_set
                .receipts
                .iter()
                .find(|receipt| receipt.package_id == package.package_id)
                .ok_or_else(|| PsionForgeEvalPackManifestError::MissingPackageReceipt {
                    package_id: package.package_id.clone(),
                })?;
            Ok(PsionForgeBenchmarkPackEntry {
                package_id: package.package_id.clone(),
                package_family: package.package_family,
                acceptance_family: package.acceptance_family,
                benchmark_ref: package.benchmark_package.key.benchmark_ref.clone(),
                benchmark_version: package.benchmark_package.key.version.clone(),
                benchmark_digest: package.benchmark_package.stable_digest(),
                receipt_id: receipt.receipt_id.clone(),
                receipt_digest: receipt.receipt_digest.clone(),
                phase: receipt.phase,
                item_count: receipt.item_count,
                metric_kinds: receipt
                    .observed_metrics
                    .iter()
                    .map(|metric| metric.metric_kind)
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let manifest = PsionForgeBenchmarkPackManifest {
        schema_version: String::from(PSION_FORGE_BENCHMARK_PACK_MANIFEST_SCHEMA_VERSION),
        pack_id: String::from("psion.pack.benchmark.core.v1"),
        display_name: String::from("Psion core benchmark pack"),
        catalog_id: catalog.catalog_id.clone(),
        catalog_digest: catalog.catalog_digest.clone(),
        receipt_set_id: receipt_set.receipt_set_id.clone(),
        receipt_set_digest: receipt_set.receipt_set_digest.clone(),
        source_artifacts: vec![
            record_source_artifact(
                repo_root,
                PSION_BENCHMARK_CATALOG_FIXTURE_PATH,
                "Canonical benchmark catalog defining the benchmark package set projected into the first Forge-facing benchmark pack.",
            )?,
            record_source_artifact(
                repo_root,
                PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH,
                "Canonical benchmark receipt set carrying the bounded review posture and metric selectors for the published benchmark-pack entries.",
            )?,
        ],
        packages,
        claim_boundary: String::from(
            "This manifest publishes the current bounded Psion benchmark package set and receipt-backed selectors for Forge consumption. It does not claim campaign orchestration, payout authority, or automatic promotion beyond the cited benchmark and receipt artifacts.",
        ),
        pack_digest: String::new(),
    }
    .with_stable_digest();
    manifest.validate()?;
    Ok(manifest)
}

/// Records the first published judge-pack manifest above the canonical
/// benchmark catalog.
pub fn record_psion_forge_judge_pack_manifest(
    repo_root: &Path,
    catalog: &PsionBenchmarkCatalog,
) -> Result<PsionForgeJudgePackManifest, PsionForgeEvalPackManifestError> {
    let mut judges = BTreeMap::<String, PsionForgeJudgePackEntry>::new();
    for package in &catalog.packages {
        for grader in &package.grader_interfaces {
            let candidate = judge_entry_from_grader(grader);
            let entry = judges
                .entry(candidate.judge_id.clone())
                .or_insert_with(|| candidate.clone());
            if !judge_entries_compatible(entry, &candidate) {
                return Err(PsionForgeEvalPackManifestError::InconsistentJudgeReuse {
                    judge_id: candidate.judge_id,
                });
            }
            if !entry.package_ids.contains(&package.package_id) {
                entry.package_ids.push(package.package_id.clone());
            }
            if !entry.package_families.contains(&package.package_family) {
                entry.package_families.push(package.package_family);
                entry.package_families.sort();
            }
            if let Some(acceptance_family) = package.acceptance_family
                && !entry.acceptance_families.contains(&acceptance_family)
            {
                entry.acceptance_families.push(acceptance_family);
                entry.acceptance_families.sort();
            }
        }
    }

    let manifest = PsionForgeJudgePackManifest {
        schema_version: String::from(PSION_FORGE_JUDGE_PACK_MANIFEST_SCHEMA_VERSION),
        pack_id: String::from("psion.pack.judge.core.v1"),
        display_name: String::from("Psion core judge pack"),
        catalog_id: catalog.catalog_id.clone(),
        catalog_digest: catalog.catalog_digest.clone(),
        source_artifacts: vec![record_source_artifact(
            repo_root,
            PSION_BENCHMARK_CATALOG_FIXTURE_PATH,
            "Canonical benchmark catalog carrying the grader interfaces that the first Forge-facing judge pack reuses directly.",
        )?],
        judges: judges.into_values().collect(),
        claim_boundary: String::from(
            "This manifest publishes the current bounded Psion grader and verifier selectors already attached to the canonical benchmark packages. It does not claim generic rubric hosting, campaign control, or arbitrary judge execution outside the cited benchmark catalog.",
        ),
        pack_digest: String::new(),
    }
    .with_stable_digest();
    manifest.validate()?;
    Ok(manifest)
}

fn judge_entry_from_grader(grader: &PsionBenchmarkGraderInterface) -> PsionForgeJudgePackEntry {
    match grader {
        PsionBenchmarkGraderInterface::ExactLabel(grader) => PsionForgeJudgePackEntry {
            judge_id: grader.grader_id.clone(),
            judge_kind: PsionForgeJudgeKind::ExactLabel,
            package_ids: Vec::new(),
            package_families: Vec::new(),
            acceptance_families: Vec::new(),
            rubric_ref: None,
            label_namespace: Some(grader.label_namespace.clone()),
            accepted_labels: grader.accepted_labels.clone(),
            expected_route: None,
            accepted_reason_codes: Vec::new(),
            detail: grader.detail.clone(),
        },
        PsionBenchmarkGraderInterface::RubricScore(grader) => PsionForgeJudgePackEntry {
            judge_id: grader.grader_id.clone(),
            judge_kind: PsionForgeJudgeKind::RubricScore,
            package_ids: Vec::new(),
            package_families: Vec::new(),
            acceptance_families: Vec::new(),
            rubric_ref: Some(grader.rubric_ref.clone()),
            label_namespace: None,
            accepted_labels: Vec::new(),
            expected_route: None,
            accepted_reason_codes: Vec::new(),
            detail: grader.detail.clone(),
        },
        PsionBenchmarkGraderInterface::ExactRoute(grader) => PsionForgeJudgePackEntry {
            judge_id: grader.grader_id.clone(),
            judge_kind: PsionForgeJudgeKind::ExactRoute,
            package_ids: Vec::new(),
            package_families: Vec::new(),
            acceptance_families: Vec::new(),
            rubric_ref: None,
            label_namespace: None,
            accepted_labels: Vec::new(),
            expected_route: Some(grader.expected_route),
            accepted_reason_codes: Vec::new(),
            detail: grader.detail.clone(),
        },
        PsionBenchmarkGraderInterface::ExactRefusal(grader) => PsionForgeJudgePackEntry {
            judge_id: grader.grader_id.clone(),
            judge_kind: PsionForgeJudgeKind::ExactRefusal,
            package_ids: Vec::new(),
            package_families: Vec::new(),
            acceptance_families: Vec::new(),
            rubric_ref: None,
            label_namespace: None,
            accepted_labels: Vec::new(),
            expected_route: None,
            accepted_reason_codes: grader.accepted_reason_codes.clone(),
            detail: grader.detail.clone(),
        },
    }
}

fn judge_entries_compatible(
    existing: &PsionForgeJudgePackEntry,
    candidate: &PsionForgeJudgePackEntry,
) -> bool {
    existing.judge_kind == candidate.judge_kind
        && existing.rubric_ref == candidate.rubric_ref
        && existing.label_namespace == candidate.label_namespace
        && existing.accepted_labels == candidate.accepted_labels
        && existing.expected_route == candidate.expected_route
        && existing.accepted_reason_codes == candidate.accepted_reason_codes
        && existing.detail == candidate.detail
}

fn record_source_artifact(
    repo_root: &Path,
    relative_path: &str,
    detail: &str,
) -> Result<PsionForgePackSourceArtifact, PsionForgeEvalPackManifestError> {
    let absolute_path = repo_root.join(relative_path);
    let bytes = fs::read(&absolute_path).map_err(|error| PsionForgeEvalPackManifestError::Io {
        path: absolute_path.display().to_string(),
        error,
    })?;
    Ok(PsionForgePackSourceArtifact {
        path: String::from(relative_path),
        sha256: hex::encode(Sha256::digest(&bytes)),
        detail: String::from(detail),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("pack manifest should serialize"));
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionForgeEvalPackManifestError> {
    if value.trim().is_empty() {
        return Err(PsionForgeEvalPackManifestError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json<T: Serialize>(
    output_path: impl AsRef<Path>,
    value: &T,
) -> Result<(), PsionForgeEvalPackManifestError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionForgeEvalPackManifestError::Io {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        PsionForgeEvalPackManifestError::Json {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        PsionForgeEvalPackManifestError::Io {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(
    input_path: impl AsRef<Path>,
) -> Result<T, PsionForgeEvalPackManifestError> {
    let input_path = input_path.as_ref();
    let body =
        fs::read_to_string(input_path).map_err(|error| PsionForgeEvalPackManifestError::Io {
            path: input_path.display().to_string(),
            error,
        })?;
    serde_json::from_str(&body).map_err(|error| PsionForgeEvalPackManifestError::Json {
        path: input_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{
        PSION_BENCHMARK_CATALOG_FIXTURE_PATH, PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH,
        PSION_FORGE_BENCHMARK_PACK_FIXTURE_PATH, PSION_FORGE_JUDGE_PACK_FIXTURE_PATH,
        PsionForgeBenchmarkPackManifest, PsionForgeJudgeKind, PsionForgeJudgePackManifest,
        record_psion_forge_benchmark_pack_manifest, record_psion_forge_judge_pack_manifest,
    };
    use crate::{PsionBenchmarkCatalog, PsionBenchmarkReceiptSet};

    #[test]
    fn benchmark_and_judge_pack_manifests_build_from_canonical_benchmark_artifacts() {
        let repo_root = repo_root();
        let catalog: PsionBenchmarkCatalog = serde_json::from_str(
            &fs::read_to_string(repo_root.join(PSION_BENCHMARK_CATALOG_FIXTURE_PATH))
                .expect("read canonical benchmark catalog"),
        )
        .expect("decode canonical benchmark catalog");
        let receipt_set: PsionBenchmarkReceiptSet = serde_json::from_str(
            &fs::read_to_string(repo_root.join(PSION_BENCHMARK_RECEIPT_SET_FIXTURE_PATH))
                .expect("read canonical benchmark receipt set"),
        )
        .expect("decode canonical benchmark receipt set");

        let benchmark_manifest =
            record_psion_forge_benchmark_pack_manifest(repo_root.as_path(), &catalog, &receipt_set)
                .expect("build benchmark pack manifest");
        benchmark_manifest
            .validate()
            .expect("validate benchmark pack");
        assert_eq!(benchmark_manifest.packages.len(), catalog.packages.len());

        let judge_manifest = record_psion_forge_judge_pack_manifest(repo_root.as_path(), &catalog)
            .expect("build judge pack manifest");
        judge_manifest.validate().expect("validate judge pack");
        assert!(
            judge_manifest
                .judges
                .iter()
                .any(|judge| judge.judge_kind == PsionForgeJudgeKind::RubricScore)
        );
    }

    #[test]
    fn committed_forge_pack_fixtures_round_trip_and_validate() {
        let repo_root = repo_root();
        let benchmark_manifest = PsionForgeBenchmarkPackManifest::read_json(
            repo_root.join(PSION_FORGE_BENCHMARK_PACK_FIXTURE_PATH),
        )
        .expect("read committed benchmark pack fixture");
        benchmark_manifest
            .validate()
            .expect("validate committed benchmark pack fixture");

        let judge_manifest = PsionForgeJudgePackManifest::read_json(
            repo_root.join(PSION_FORGE_JUDGE_PACK_FIXTURE_PATH),
        )
        .expect("read committed judge pack fixture");
        judge_manifest
            .validate()
            .expect("validate committed judge pack fixture");
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir should have parent")
            .parent()
            .expect("workspace root should exist")
            .to_path_buf()
    }
}
