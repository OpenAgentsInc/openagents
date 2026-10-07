use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_COMPUTATIONAL_MODULE_MANIFEST_SCHEMA_VERSION: u16 = 1;

/// Trust posture published by one computational module manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleTrustPosture {
    /// Bounded research-only artifact with no install or served widening.
    ResearchOnly,
    /// Internal benchmark-gated artifact with explicit install policy.
    BenchmarkGatedInternal,
    /// Installable artifact gated by challenge and evidence posture.
    ChallengeGatedInstall,
}

/// Import family carried by one computational module symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleImportClass {
    /// Another internal computational module ABI.
    InternalModuleAbi,
    /// Deterministic runtime state projection.
    RuntimeStateView,
    /// Deterministic benchmark or validator feed.
    EvidenceFeed,
}

/// State field family carried by one computational module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleStateFieldKind {
    /// Frontier or queue state.
    FrontierState,
    /// Candidate or ranking state.
    CandidateState,
    /// Checkpoint or backtrack state.
    CheckpointState,
}

/// One imported symbol contract in the computational module ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComputationalModuleImport {
    /// Stable symbol identifier.
    pub symbol: String,
    /// Typed import class.
    pub import_class: TassadarModuleImportClass,
    /// Whether this import must be resolved to load the module.
    pub required: bool,
    /// Plain-language import boundary.
    pub claim_boundary: String,
}

/// One exported symbol contract in the computational module ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComputationalModuleExport {
    /// Stable export symbol.
    pub symbol: String,
    /// Stable ABI version for the symbol family.
    pub abi_version: u16,
    /// Machine-legible input channel labels.
    pub input_channels: Vec<String>,
    /// Machine-legible output channel labels.
    pub output_channels: Vec<String>,
    /// Plain-language export boundary.
    pub claim_boundary: String,
}

/// One state field carried by the computational module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComputationalModuleStateField {
    /// Stable field identifier.
    pub field_id: String,
    /// Typed state field family.
    pub field_kind: TassadarModuleStateFieldKind,
    /// Stable shape label for the field.
    pub shape: String,
    /// Whether the field is mutable during execution.
    pub mutable: bool,
}

/// Capability summary published by one computational module manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComputationalModuleCapabilitySummary {
    /// Stable capability labels.
    pub capability_labels: Vec<String>,
    /// Stable workload families that may consume the module.
    pub supported_workload_families: Vec<String>,
    /// Explicit refusal boundaries that remain outside the manifest claim.
    pub refusal_boundaries: Vec<String>,
}

/// Typed ABI and evidence manifest for one computational module artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComputationalModuleManifest {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Stable module reference with semantic version.
    pub module_ref: String,
    /// Stable ABI family identifier.
    pub abi_family: String,
    /// Explicit claim class.
    pub claim_class: String,
    /// Typed trust posture.
    pub trust_posture: TassadarModuleTrustPosture,
    /// Declared imports for the module ABI.
    pub imports: Vec<TassadarComputationalModuleImport>,
    /// Declared exports for the module ABI.
    pub exports: Vec<TassadarComputationalModuleExport>,
    /// Declared state shape for the module ABI.
    pub state_fields: Vec<TassadarComputationalModuleStateField>,
    /// Declared capability summary.
    pub capability_summary: TassadarComputationalModuleCapabilitySummary,
    /// Stable benchmark refs that justify the module.
    pub benchmark_lineage_refs: Vec<String>,
    /// Stable evidence refs required before install or promotion.
    pub required_evidence_refs: Vec<String>,
    /// Stable compatibility digest across the ABI-relevant subset.
    pub compatibility_digest: String,
    /// Stable digest across the full manifest.
    pub manifest_digest: String,
}

impl TassadarComputationalModuleManifest {
    /// Creates a computational module manifest and computes stable digests.
    #[must_use]
    pub fn new(
        manifest_id: impl Into<String>,
        module_ref: impl Into<String>,
        abi_family: impl Into<String>,
        claim_class: impl Into<String>,
        trust_posture: TassadarModuleTrustPosture,
        mut imports: Vec<TassadarComputationalModuleImport>,
        mut exports: Vec<TassadarComputationalModuleExport>,
        mut state_fields: Vec<TassadarComputationalModuleStateField>,
        mut capability_summary: TassadarComputationalModuleCapabilitySummary,
        mut benchmark_lineage_refs: Vec<String>,
        mut required_evidence_refs: Vec<String>,
    ) -> Self {
        imports.sort_by(|left, right| left.symbol.cmp(&right.symbol));
        exports.sort_by(|left, right| left.symbol.cmp(&right.symbol));
        state_fields.sort_by(|left, right| left.field_id.cmp(&right.field_id));
        capability_summary.capability_labels.sort();
        capability_summary.capability_labels.dedup();
        capability_summary.supported_workload_families.sort();
        capability_summary.supported_workload_families.dedup();
        capability_summary.refusal_boundaries.sort();
        capability_summary.refusal_boundaries.dedup();
        benchmark_lineage_refs.sort();
        benchmark_lineage_refs.dedup();
        required_evidence_refs.sort();
        required_evidence_refs.dedup();
        let mut manifest = Self {
            schema_version: TASSADAR_COMPUTATIONAL_MODULE_MANIFEST_SCHEMA_VERSION,
            manifest_id: manifest_id.into(),
            module_ref: module_ref.into(),
            abi_family: abi_family.into(),
            claim_class: claim_class.into(),
            trust_posture,
            imports,
            exports,
            state_fields,
            capability_summary,
            benchmark_lineage_refs,
            required_evidence_refs,
            compatibility_digest: String::new(),
            manifest_digest: String::new(),
        };
        manifest.compatibility_digest = manifest.stable_compatibility_digest();
        manifest.manifest_digest = stable_digest(b"psionic_tassadar_module_manifest|", &manifest);
        manifest
    }

    /// Returns a stable digest over the ABI-relevant compatibility subset.
    #[must_use]
    pub fn stable_compatibility_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_module_manifest_compatibility|",
            &(
                self.schema_version,
                &self.module_ref,
                &self.abi_family,
                &self.claim_class,
                self.trust_posture,
                &self.imports,
                &self.exports,
                &self.state_fields,
                &self.capability_summary.capability_labels,
                &self.required_evidence_refs,
            ),
        )
    }

    /// Validates the manifest without linking or install flow state.
    pub fn validate(&self) -> Result<(), TassadarComputationalModuleManifestError> {
        if self.manifest_id.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingManifestId);
        }
        if self.module_ref.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingModuleRef);
        }
        if self.abi_family.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingAbiFamily);
        }
        if self.claim_class.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingClaimClass);
        }
        if self.exports.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingExports);
        }
        if self.capability_summary.capability_labels.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingCapabilitySummary);
        }
        if self.benchmark_lineage_refs.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingBenchmarkLineage);
        }
        if self.required_evidence_refs.is_empty() {
            return Err(TassadarComputationalModuleManifestError::MissingRequiredEvidence);
        }
        if self
            .capability_summary
            .capability_labels
            .iter()
            .any(|label| label == "generic_module")
        {
            return Err(
                TassadarComputationalModuleManifestError::GenericCapabilityWideningForbidden,
            );
        }
        validate_unique_import_symbols(self.imports.iter().map(|import| import.symbol.as_str()))?;
        validate_unique_export_symbols(self.exports.iter().map(|export| export.symbol.as_str()))?;
        validate_unique_state_fields(
            self.state_fields
                .iter()
                .map(|field| field.field_id.as_str()),
        )?;
        if self.compatibility_digest != self.stable_compatibility_digest() {
            return Err(TassadarComputationalModuleManifestError::CompatibilityDigestDrift);
        }
        Ok(())
    }
}

/// Validation failure for one computational module manifest.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarComputationalModuleManifestError {
    /// The manifest omitted its identifier.
    #[error("computational module manifest omitted its manifest_id")]
    MissingManifestId,
    /// The manifest omitted its module ref.
    #[error("computational module manifest omitted its module_ref")]
    MissingModuleRef,
    /// The manifest omitted its ABI family.
    #[error("computational module manifest omitted its abi_family")]
    MissingAbiFamily,
    /// The manifest omitted its claim class.
    #[error("computational module manifest omitted its claim_class")]
    MissingClaimClass,
    /// The manifest omitted its exported symbol surface.
    #[error("computational module manifest omitted its exports")]
    MissingExports,
    /// The manifest omitted its capability summary.
    #[error("computational module manifest omitted its capability summary")]
    MissingCapabilitySummary,
    /// The manifest omitted benchmark lineage.
    #[error("computational module manifest omitted benchmark lineage refs")]
    MissingBenchmarkLineage,
    /// The manifest omitted required evidence refs.
    #[error("computational module manifest omitted required evidence refs")]
    MissingRequiredEvidence,
    /// The manifest attempted to widen capability through a generic label.
    #[error("computational module manifest cannot claim generic capability widening")]
    GenericCapabilityWideningForbidden,
    /// The manifest contained duplicate imported symbols.
    #[error("computational module manifest contains duplicate import symbol `{symbol}`")]
    DuplicateImportSymbol {
        /// Duplicated symbol.
        symbol: String,
    },
    /// The manifest contained duplicate exported symbols.
    #[error("computational module manifest contains duplicate export symbol `{symbol}`")]
    DuplicateExportSymbol {
        /// Duplicated symbol.
        symbol: String,
    },
    /// The manifest contained duplicate state fields.
    #[error("computational module manifest contains duplicate state field `{field_id}`")]
    DuplicateStateField {
        /// Duplicated field identifier.
        field_id: String,
    },
    /// The stored compatibility digest drifted from the manifest contents.
    #[error("computational module manifest compatibility digest drifted from ABI contents")]
    CompatibilityDigestDrift,
}

/// Returns the seeded bounded computational module manifests.
#[must_use]
pub fn seeded_tassadar_computational_module_manifests() -> Vec<TassadarComputationalModuleManifest>
{
    vec![
        TassadarComputationalModuleManifest::new(
            "tassadar.module.frontier_relax_core.manifest.v1",
            "frontier_relax_core@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![TassadarComputationalModuleImport {
                symbol: String::from("runtime.frontier_state.read"),
                import_class: TassadarModuleImportClass::RuntimeStateView,
                required: true,
                claim_boundary: String::from(
                    "frontier_relax_core only reads the bounded frontier-state view declared by the internal runtime lane",
                ),
            }],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("frontier_relax_step"),
                abi_version: 1,
                input_channels: vec![
                    String::from("node_id"),
                    String::from("frontier_weight"),
                    String::from("adjacency_window"),
                ],
                output_channels: vec![
                    String::from("candidate_updates"),
                    String::from("frontier_delta"),
                ],
                claim_boundary: String::from(
                    "frontier_relax_core exports one bounded frontier-relax step and does not claim arbitrary graph execution",
                ),
            }],
            vec![TassadarComputationalModuleStateField {
                field_id: String::from("frontier_slots"),
                field_kind: TassadarModuleStateFieldKind::FrontierState,
                shape: String::from("slots[16]"),
                mutable: true,
            }],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![
                    String::from("frontier_relaxation"),
                    String::from("graph_state_delta"),
                ],
                supported_workload_families: vec![
                    String::from("clrs_shortest_path"),
                    String::from("clrs_wasm_shortest_path"),
                ],
                refusal_boundaries: vec![
                    String::from("no arbitrary graph topology widening"),
                    String::from("no host-import execution"),
                ],
            },
            vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
            ],
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
        ),
        TassadarComputationalModuleManifest::new(
            "tassadar.module.candidate_select_core.manifest.v1",
            "candidate_select_core@1.1.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::ChallengeGatedInstall,
            vec![TassadarComputationalModuleImport {
                symbol: String::from("runtime.candidate_state.read"),
                import_class: TassadarModuleImportClass::RuntimeStateView,
                required: true,
                claim_boundary: String::from(
                    "candidate_select_core only reads bounded candidate-state windows and does not claim arbitrary memory inspection",
                ),
            }],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("candidate_select_step"),
                abi_version: 1,
                input_channels: vec![
                    String::from("candidate_window"),
                    String::from("score_vector"),
                ],
                output_channels: vec![
                    String::from("selected_candidate"),
                    String::from("selection_confidence"),
                ],
                claim_boundary: String::from(
                    "candidate_select_core exports a bounded candidate-selection step and does not widen into generic planner control",
                ),
            }],
            vec![TassadarComputationalModuleStateField {
                field_id: String::from("candidate_window"),
                field_kind: TassadarModuleStateFieldKind::CandidateState,
                shape: String::from("window[8]"),
                mutable: true,
            }],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![
                    String::from("candidate_selection"),
                    String::from("bounded_ranking"),
                ],
                supported_workload_families: vec![
                    String::from("hungarian_matching"),
                    String::from("verifier_search"),
                ],
                refusal_boundaries: vec![
                    String::from("no arbitrary search policy widening"),
                    String::from("no cross-tier import widening"),
                ],
            },
            vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
            ],
            vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_module_installation_staging_report.json",
                ),
            ],
        ),
        TassadarComputationalModuleManifest::new(
            "tassadar.module.checkpoint_backtrack_core.manifest.v1",
            "checkpoint_backtrack_core@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![
                TassadarComputationalModuleImport {
                    symbol: String::from("candidate_select_step"),
                    import_class: TassadarModuleImportClass::InternalModuleAbi,
                    required: true,
                    claim_boundary: String::from(
                        "checkpoint_backtrack_core only composes through the bounded candidate-select ABI and does not assume hidden planner control",
                    ),
                },
                TassadarComputationalModuleImport {
                    symbol: String::from("evidence.checkpoint_budget"),
                    import_class: TassadarModuleImportClass::EvidenceFeed,
                    required: true,
                    claim_boundary: String::from(
                        "checkpoint_backtrack_core requires explicit checkpoint-budget evidence and does not assume unbounded search allowance",
                    ),
                },
            ],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("checkpoint_push"),
                abi_version: 1,
                input_channels: vec![String::from("search_state")],
                output_channels: vec![String::from("checkpoint_token")],
                claim_boundary: String::from(
                    "checkpoint_backtrack_core only exports bounded checkpoint issuance",
                ),
            }],
            vec![TassadarComputationalModuleStateField {
                field_id: String::from("checkpoint_stack"),
                field_kind: TassadarModuleStateFieldKind::CheckpointState,
                shape: String::from("stack[4]"),
                mutable: true,
            }],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![
                    String::from("checkpointing"),
                    String::from("bounded_backtrack"),
                ],
                supported_workload_families: vec![
                    String::from("verifier_search"),
                    String::from("sudoku_v0_search"),
                ],
                refusal_boundaries: vec![
                    String::from("no unbounded backtracking"),
                    String::from("no opaque external verifier tool calls"),
                ],
            },
            vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
            ],
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_internal_module_library_report.json",
            )],
        ),
    ]
}

fn validate_unique_import_symbols<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Result<(), TassadarComputationalModuleManifestError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(
                TassadarComputationalModuleManifestError::DuplicateImportSymbol {
                    symbol: String::from(value),
                },
            );
        }
    }
    Ok(())
}

fn validate_unique_export_symbols<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Result<(), TassadarComputationalModuleManifestError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(
                TassadarComputationalModuleManifestError::DuplicateExportSymbol {
                    symbol: String::from(value),
                },
            );
        }
    }
    Ok(())
}

fn validate_unique_state_fields<'a>(
    values: impl Iterator<Item = &'a str>,
) -> Result<(), TassadarComputationalModuleManifestError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(
                TassadarComputationalModuleManifestError::DuplicateStateField {
                    field_id: String::from(value),
                },
            );
        }
    }
    Ok(())
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
        TassadarComputationalModuleCapabilitySummary, TassadarComputationalModuleExport,
        TassadarComputationalModuleManifest, TassadarComputationalModuleManifestError,
        TassadarComputationalModuleStateField, TassadarModuleStateFieldKind,
        TassadarModuleTrustPosture, seeded_tassadar_computational_module_manifests,
    };

    #[test]
    fn seeded_module_manifests_are_machine_legible() {
        let manifests = seeded_tassadar_computational_module_manifests();

        assert_eq!(manifests.len(), 3);
        assert!(manifests.iter().any(|manifest| manifest.module_ref
            == "candidate_select_core@1.1.0"
            && manifest.trust_posture == TassadarModuleTrustPosture::ChallengeGatedInstall));
        for manifest in manifests {
            manifest
                .validate()
                .expect("seeded manifest should validate");
            assert!(!manifest.compatibility_digest.is_empty());
            assert!(!manifest.manifest_digest.is_empty());
        }
    }

    #[test]
    fn module_manifest_refuses_missing_evidence_and_duplicate_state_fields() {
        let mut manifest = TassadarComputationalModuleManifest::new(
            "manifest",
            "module@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("export"),
                abi_version: 1,
                input_channels: vec![String::from("input")],
                output_channels: vec![String::from("output")],
                claim_boundary: String::from("bounded export"),
            }],
            vec![
                TassadarComputationalModuleStateField {
                    field_id: String::from("stack[4]"),
                    field_kind: TassadarModuleStateFieldKind::CheckpointState,
                    shape: String::from("stack[4]"),
                    mutable: true,
                },
                TassadarComputationalModuleStateField {
                    field_id: String::from("stack[4]"),
                    field_kind: TassadarModuleStateFieldKind::CheckpointState,
                    shape: String::from("stack[4]"),
                    mutable: true,
                },
            ],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![String::from("checkpointing")],
                supported_workload_families: vec![String::from("search")],
                refusal_boundaries: vec![String::from("no widening")],
            },
            vec![String::from("benchmark.json")],
            vec![],
        );

        assert_eq!(
            manifest.validate(),
            Err(TassadarComputationalModuleManifestError::MissingRequiredEvidence)
        );
        manifest.required_evidence_refs = vec![String::from("evidence.json")];
        assert_eq!(
            manifest.validate(),
            Err(
                TassadarComputationalModuleManifestError::DuplicateStateField {
                    field_id: String::from("stack[4]")
                }
            )
        );
    }

    #[test]
    fn module_manifest_refuses_generic_capability_widening() {
        let manifest = TassadarComputationalModuleManifest::new(
            "manifest",
            "module@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("export"),
                abi_version: 1,
                input_channels: vec![String::from("input")],
                output_channels: vec![String::from("output")],
                claim_boundary: String::from("bounded export"),
            }],
            vec![],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![String::from("generic_module")],
                supported_workload_families: vec![String::from("search")],
                refusal_boundaries: vec![String::from("no widening")],
            },
            vec![String::from("benchmark.json")],
            vec![String::from("evidence.json")],
        );

        assert_eq!(
            manifest.validate(),
            Err(TassadarComputationalModuleManifestError::GenericCapabilityWideningForbidden)
        );
    }
}
