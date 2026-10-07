use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarModuleCompatibilityError, TassadarModuleCompatibilityRequest,
    check_tassadar_module_manifest_compatibility,
};
use psionic_ir::{
    TassadarComputationalModuleManifest, TassadarComputationalModuleManifestError,
    TassadarModuleImportClass, TassadarModuleTrustPosture,
};

/// One dependency-graph node selected during bounded module linking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleDependencyNode {
    /// Stable module reference.
    pub module_ref: String,
    /// Stable module identifier without version.
    pub module_id: String,
    /// Typed trust posture.
    pub trust_posture: TassadarModuleTrustPosture,
    /// Explicit claim class.
    pub claim_class: String,
    /// Stable compatibility digest.
    pub compatibility_digest: String,
}

/// One dependency edge between linked computational modules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleDependencyEdge {
    /// Importing module reference.
    pub importer_module_ref: String,
    /// Imported symbol.
    pub import_symbol: String,
    /// Provider module reference.
    pub provider_module_ref: String,
    /// Provider export symbol.
    pub provider_export_symbol: String,
}

/// Dependency graph realized for one bounded module link request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleDependencyGraph {
    /// Stable consumer family.
    pub consumer_family: String,
    /// Ordered dependency nodes.
    pub nodes: Vec<TassadarModuleDependencyNode>,
    /// Ordered dependency edges.
    pub edges: Vec<TassadarModuleDependencyEdge>,
    /// Stable digest over the graph.
    pub graph_digest: String,
}

impl TassadarModuleDependencyGraph {
    fn new(
        consumer_family: impl Into<String>,
        mut nodes: Vec<TassadarModuleDependencyNode>,
        mut edges: Vec<TassadarModuleDependencyEdge>,
    ) -> Self {
        nodes.sort_by(|left, right| left.module_ref.cmp(&right.module_ref));
        edges.sort_by(|left, right| {
            (
                left.importer_module_ref.as_str(),
                left.import_symbol.as_str(),
                left.provider_module_ref.as_str(),
            )
                .cmp(&(
                    right.importer_module_ref.as_str(),
                    right.import_symbol.as_str(),
                    right.provider_module_ref.as_str(),
                ))
        });
        let mut graph = Self {
            consumer_family: consumer_family.into(),
            nodes,
            edges,
            graph_digest: String::new(),
        };
        graph.graph_digest = stable_digest(b"psionic_tassadar_module_dependency_graph|", &graph);
        graph
    }
}

/// Link posture for one bounded module link request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleLinkPosture {
    /// Requested modules linked exactly as requested.
    Exact,
    /// One requested module rolled back to an explicit fallback target.
    RolledBack,
}

/// One bounded module link request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkRequest {
    /// Stable consumer family requesting the link.
    pub consumer_family: String,
    /// Ordered preferred module refs.
    pub requested_module_refs: Vec<String>,
    /// Ordered rollback module refs available to the linker.
    pub rollback_module_refs: Vec<String>,
    /// Minimum trust posture accepted by the consumer.
    pub minimum_trust_posture: TassadarModuleTrustPosture,
    /// Allowed claim classes for the consumer.
    pub allowed_claim_classes: Vec<String>,
}

/// Deterministic result of one bounded module link request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkResolution {
    /// Stable consumer family.
    pub consumer_family: String,
    /// Ordered requested module refs.
    pub requested_module_refs: Vec<String>,
    /// Ordered selected module refs after rollback resolution.
    pub selected_module_refs: Vec<String>,
    /// Final link posture.
    pub posture: TassadarModuleLinkPosture,
    /// Dependency graph realized for the selected modules.
    pub dependency_graph: TassadarModuleDependencyGraph,
    /// Explicit rollback detail when posture is rolled back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_detail: Option<String>,
    /// Stable digest over the resolution.
    pub resolution_digest: String,
}

impl TassadarModuleLinkResolution {
    fn new(
        consumer_family: impl Into<String>,
        requested_module_refs: Vec<String>,
        selected_module_refs: Vec<String>,
        posture: TassadarModuleLinkPosture,
        dependency_graph: TassadarModuleDependencyGraph,
        rollback_detail: Option<String>,
    ) -> Self {
        let mut resolution = Self {
            consumer_family: consumer_family.into(),
            requested_module_refs,
            selected_module_refs,
            posture,
            dependency_graph,
            rollback_detail,
            resolution_digest: String::new(),
        };
        resolution.resolution_digest =
            stable_digest(b"psionic_tassadar_module_link_resolution|", &resolution);
        resolution
    }
}

/// Failure returned by the bounded module linker.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarModuleLinkError {
    /// One manifest failed schema validation.
    #[error(transparent)]
    InvalidManifest(#[from] TassadarComputationalModuleManifestError),
    /// One selected module failed compatibility checks.
    #[error(transparent)]
    Compatibility(#[from] TassadarModuleCompatibilityError),
    /// The request tried to link conflicting versions of the same module family.
    #[error(
        "consumer `{consumer_family}` requested conflicting versions `{first_module_ref}` and `{second_module_ref}` for module `{module_id}`"
    )]
    ConflictingVersion {
        /// Consumer family.
        consumer_family: String,
        /// Stable module identifier without version.
        module_id: String,
        /// First requested module ref.
        first_module_ref: String,
        /// Second requested module ref.
        second_module_ref: String,
    },
    /// One requested module and rollback target were both unavailable.
    #[error("consumer `{consumer_family}` could not resolve requested module `{module_ref}`")]
    MissingRequestedModule {
        /// Consumer family.
        consumer_family: String,
        /// Missing module ref.
        module_ref: String,
    },
    /// One required internal module dependency was missing.
    #[error(
        "module `{module_ref}` requires internal import `{import_symbol}` but no linked provider exported it"
    )]
    MissingInternalDependency {
        /// Importing module ref.
        module_ref: String,
        /// Missing internal import symbol.
        import_symbol: String,
    },
}

/// Resolves one deterministic bounded dependency graph over computational modules.
pub fn link_tassadar_module_dependency_graph(
    manifests: &[TassadarComputationalModuleManifest],
    request: &TassadarModuleLinkRequest,
) -> Result<TassadarModuleLinkResolution, TassadarModuleLinkError> {
    for manifest in manifests {
        manifest.validate()?;
    }
    refuse_conflicting_versions(request)?;
    let manifest_by_ref = manifests
        .iter()
        .map(|manifest| (manifest.module_ref.as_str(), manifest))
        .collect::<BTreeMap<_, _>>();
    let rollback_by_module_id = request
        .rollback_module_refs
        .iter()
        .map(|module_ref| (module_id_from_ref(module_ref), module_ref))
        .collect::<BTreeMap<_, _>>();
    let mut selected_manifests = Vec::with_capacity(request.requested_module_refs.len());
    let mut selected_module_refs = Vec::with_capacity(request.requested_module_refs.len());
    let mut posture = TassadarModuleLinkPosture::Exact;
    let mut rollback_notes = Vec::new();
    for requested_module_ref in &request.requested_module_refs {
        let module_id = module_id_from_ref(requested_module_ref);
        let manifest = if let Some(manifest) = manifest_by_ref.get(requested_module_ref.as_str()) {
            validate_manifest_against_request(manifest, request)?;
            *manifest
        } else if let Some(rollback_module_ref) = rollback_by_module_id.get(module_id.as_str()) {
            let rollback_manifest = manifest_by_ref
                .get(rollback_module_ref.as_str())
                .ok_or_else(|| TassadarModuleLinkError::MissingRequestedModule {
                    consumer_family: request.consumer_family.clone(),
                    module_ref: requested_module_ref.clone(),
                })?;
            validate_manifest_against_request(rollback_manifest, request)?;
            posture = TassadarModuleLinkPosture::RolledBack;
            rollback_notes.push(format!(
                "{requested_module_ref} rolled back to {} under the explicit rollback set",
                rollback_manifest.module_ref
            ));
            *rollback_manifest
        } else {
            return Err(TassadarModuleLinkError::MissingRequestedModule {
                consumer_family: request.consumer_family.clone(),
                module_ref: requested_module_ref.clone(),
            });
        };
        selected_manifests.push(manifest.clone());
        selected_module_refs.push(manifest.module_ref.clone());
    }
    let dependency_graph = build_tassadar_module_dependency_graph(
        request.consumer_family.as_str(),
        &selected_manifests,
    )?;
    let rollback_detail = if rollback_notes.is_empty() {
        None
    } else {
        Some(rollback_notes.join("; "))
    };
    Ok(TassadarModuleLinkResolution::new(
        request.consumer_family.clone(),
        request.requested_module_refs.clone(),
        selected_module_refs,
        posture,
        dependency_graph,
        rollback_detail,
    ))
}

fn validate_manifest_against_request(
    manifest: &TassadarComputationalModuleManifest,
    request: &TassadarModuleLinkRequest,
) -> Result<(), TassadarModuleLinkError> {
    let compatibility_request = TassadarModuleCompatibilityRequest {
        consumer_family: request.consumer_family.clone(),
        required_exports: vec![],
        required_benchmark_refs: vec![],
        minimum_trust_posture: request.minimum_trust_posture,
        allowed_claim_classes: request.allowed_claim_classes.clone(),
    };
    check_tassadar_module_manifest_compatibility(manifest, &compatibility_request)?;
    Ok(())
}

fn refuse_conflicting_versions(
    request: &TassadarModuleLinkRequest,
) -> Result<(), TassadarModuleLinkError> {
    let mut module_versions = BTreeMap::<String, String>::new();
    for module_ref in &request.requested_module_refs {
        let module_id = module_id_from_ref(module_ref);
        if let Some(existing) = module_versions.get(module_id.as_str()) {
            if existing != module_ref {
                return Err(TassadarModuleLinkError::ConflictingVersion {
                    consumer_family: request.consumer_family.clone(),
                    module_id,
                    first_module_ref: existing.clone(),
                    second_module_ref: module_ref.clone(),
                });
            }
        } else {
            module_versions.insert(module_id, module_ref.clone());
        }
    }
    Ok(())
}

fn build_tassadar_module_dependency_graph(
    consumer_family: &str,
    selected_manifests: &[TassadarComputationalModuleManifest],
) -> Result<TassadarModuleDependencyGraph, TassadarModuleLinkError> {
    let nodes = selected_manifests
        .iter()
        .map(|manifest| TassadarModuleDependencyNode {
            module_ref: manifest.module_ref.clone(),
            module_id: module_id_from_ref(&manifest.module_ref),
            trust_posture: manifest.trust_posture,
            claim_class: manifest.claim_class.clone(),
            compatibility_digest: manifest.compatibility_digest.clone(),
        })
        .collect::<Vec<_>>();
    let export_map = selected_manifests
        .iter()
        .flat_map(|manifest| {
            manifest
                .exports
                .iter()
                .map(|export| (export.symbol.as_str(), manifest.module_ref.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::new();
    let mut edge_keys = BTreeSet::new();
    for manifest in selected_manifests {
        for import in &manifest.imports {
            if import.import_class != TassadarModuleImportClass::InternalModuleAbi {
                continue;
            }
            let provider_module_ref = export_map.get(import.symbol.as_str()).ok_or_else(|| {
                TassadarModuleLinkError::MissingInternalDependency {
                    module_ref: manifest.module_ref.clone(),
                    import_symbol: import.symbol.clone(),
                }
            })?;
            let edge = TassadarModuleDependencyEdge {
                importer_module_ref: manifest.module_ref.clone(),
                import_symbol: import.symbol.clone(),
                provider_module_ref: String::from(*provider_module_ref),
                provider_export_symbol: import.symbol.clone(),
            };
            let edge_key = (
                edge.importer_module_ref.clone(),
                edge.import_symbol.clone(),
                edge.provider_module_ref.clone(),
            );
            if edge_keys.insert(edge_key) {
                edges.push(edge);
            }
        }
    }
    Ok(TassadarModuleDependencyGraph::new(
        consumer_family,
        nodes,
        edges,
    ))
}

fn module_id_from_ref(module_ref: &str) -> String {
    module_ref.split_once('@').map_or_else(
        || String::from(module_ref),
        |(module_id, _)| String::from(module_id),
    )
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
        TassadarModuleLinkError, TassadarModuleLinkPosture, TassadarModuleLinkRequest,
        link_tassadar_module_dependency_graph,
    };
    use psionic_ir::{
        TassadarComputationalModuleCapabilitySummary, TassadarComputationalModuleExport,
        TassadarComputationalModuleImport, TassadarComputationalModuleManifest,
        TassadarComputationalModuleStateField, TassadarModuleImportClass,
        TassadarModuleStateFieldKind, TassadarModuleTrustPosture,
        seeded_tassadar_computational_module_manifests,
    };

    #[test]
    fn module_linker_builds_dependency_graph_for_internal_module_imports() {
        let manifests = seeded_tassadar_computational_module_manifests();
        let request = TassadarModuleLinkRequest {
            consumer_family: String::from("verifier_search"),
            requested_module_refs: vec![
                String::from("candidate_select_core@1.1.0"),
                String::from("checkpoint_backtrack_core@1.0.0"),
            ],
            rollback_module_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let resolution =
            link_tassadar_module_dependency_graph(manifests.as_slice(), &request).expect("link");

        assert_eq!(resolution.posture, TassadarModuleLinkPosture::Exact);
        assert_eq!(resolution.dependency_graph.nodes.len(), 2);
        assert_eq!(resolution.dependency_graph.edges.len(), 1);
        assert_eq!(
            resolution.dependency_graph.edges[0].provider_module_ref,
            "candidate_select_core@1.1.0"
        );
    }

    #[test]
    fn module_linker_refuses_conflicting_versions() {
        let mut manifests = seeded_tassadar_computational_module_manifests();
        let candidate = manifests
            .iter()
            .find(|manifest| manifest.module_ref == "candidate_select_core@1.1.0")
            .expect("candidate manifest")
            .clone();
        manifests.push(TassadarComputationalModuleManifest::new(
            "tassadar.module.candidate_select_core.manifest.v2",
            "candidate_select_core@1.2.0",
            candidate.abi_family,
            candidate.claim_class,
            candidate.trust_posture,
            candidate.imports,
            candidate.exports,
            candidate.state_fields,
            candidate.capability_summary,
            candidate.benchmark_lineage_refs,
            candidate.required_evidence_refs,
        ));
        let request = TassadarModuleLinkRequest {
            consumer_family: String::from("hungarian_matching"),
            requested_module_refs: vec![
                String::from("candidate_select_core@1.1.0"),
                String::from("candidate_select_core@1.2.0"),
            ],
            rollback_module_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let error = link_tassadar_module_dependency_graph(manifests.as_slice(), &request)
            .expect_err("error");

        assert_eq!(
            error,
            TassadarModuleLinkError::ConflictingVersion {
                consumer_family: String::from("hungarian_matching"),
                module_id: String::from("candidate_select_core"),
                first_module_ref: String::from("candidate_select_core@1.1.0"),
                second_module_ref: String::from("candidate_select_core@1.2.0"),
            }
        );
    }

    #[test]
    fn module_linker_rolls_back_when_requested_version_is_unavailable() {
        let manifests = seeded_tassadar_computational_module_manifests();
        let request = TassadarModuleLinkRequest {
            consumer_family: String::from("hungarian_matching"),
            requested_module_refs: vec![String::from("candidate_select_core@1.2.0")],
            rollback_module_refs: vec![String::from("candidate_select_core@1.1.0")],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let resolution =
            link_tassadar_module_dependency_graph(manifests.as_slice(), &request).expect("link");

        assert_eq!(resolution.posture, TassadarModuleLinkPosture::RolledBack);
        assert_eq!(
            resolution.selected_module_refs,
            vec![String::from("candidate_select_core@1.1.0")]
        );
        assert!(resolution.rollback_detail.is_some());
    }

    #[test]
    fn module_linker_refuses_missing_internal_dependency() {
        let provider_manifest = TassadarComputationalModuleManifest::new(
            "manifest.provider",
            "provider_core@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("provider_step"),
                abi_version: 1,
                input_channels: vec![String::from("input")],
                output_channels: vec![String::from("output")],
                claim_boundary: String::from("bounded provider"),
            }],
            vec![],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![String::from("provider")],
                supported_workload_families: vec![String::from("test")],
                refusal_boundaries: vec![String::from("no widening")],
            },
            vec![String::from("benchmark.json")],
            vec![String::from("evidence.json")],
        );
        let dependent_manifest = TassadarComputationalModuleManifest::new(
            "manifest.dependent",
            "dependent_core@1.0.0",
            "tassadar.module.abi.v1",
            "compiled bounded exactness / promotion discipline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![TassadarComputationalModuleImport {
                symbol: String::from("missing_step"),
                import_class: TassadarModuleImportClass::InternalModuleAbi,
                required: true,
                claim_boundary: String::from("requires bounded provider"),
            }],
            vec![TassadarComputationalModuleExport {
                symbol: String::from("dependent_step"),
                abi_version: 1,
                input_channels: vec![String::from("input")],
                output_channels: vec![String::from("output")],
                claim_boundary: String::from("bounded dependent"),
            }],
            vec![TassadarComputationalModuleStateField {
                field_id: String::from("dependent_state"),
                field_kind: TassadarModuleStateFieldKind::CheckpointState,
                shape: String::from("stack[1]"),
                mutable: true,
            }],
            TassadarComputationalModuleCapabilitySummary {
                capability_labels: vec![String::from("dependent")],
                supported_workload_families: vec![String::from("test")],
                refusal_boundaries: vec![String::from("no widening")],
            },
            vec![String::from("benchmark.json")],
            vec![String::from("evidence.json")],
        );
        let request = TassadarModuleLinkRequest {
            consumer_family: String::from("test_consumer"),
            requested_module_refs: vec![String::from("dependent_core@1.0.0")],
            rollback_module_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let error = link_tassadar_module_dependency_graph(
            &[provider_manifest, dependent_manifest],
            &request,
        )
        .expect_err("error");

        assert_eq!(
            error,
            TassadarModuleLinkError::MissingInternalDependency {
                module_ref: String::from("dependent_core@1.0.0"),
                import_symbol: String::from("missing_step"),
            }
        );
    }
}
