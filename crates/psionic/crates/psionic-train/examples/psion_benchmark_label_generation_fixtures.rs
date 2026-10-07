use std::{error::Error, fs, path::PathBuf};

use psionic_data::{
    PsionArtifactLineageManifest, PsionExclusionManifest, PsionSourceLifecycleManifest,
};
use psionic_train::{
    record_psion_benchmark_label_generation_receipt,
    record_psion_benchmark_label_generation_receipt_set, PsionBenchmarkCatalog,
    PsionBenchmarkDerivedDataLineage, PsionBenchmarkExactTruthBinding,
    PsionBenchmarkGraderInterface, PsionBenchmarkItemLabelGenerationReceipt,
    PsionBenchmarkLabelGenerationMode, PsionBenchmarkLabelLogicBinding,
    PsionBenchmarkPackageContract, PsionBenchmarkRubricVersionBinding,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/benchmarks");
    fs::create_dir_all(&fixtures_dir)?;

    let lifecycle: PsionSourceLifecycleManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"),
    )?)?;
    let exclusion: PsionExclusionManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/isolation/psion_exclusion_manifest_v1.json"),
    )?)?;
    let artifact_lineage: PsionArtifactLineageManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
        )?)?;
    let catalog: PsionBenchmarkCatalog = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"),
    )?)?;

    let receipts = catalog
        .packages
        .iter()
        .map(|package| build_package_receipt(package, &artifact_lineage, &lifecycle, &exclusion))
        .collect::<Result<Vec<_>, _>>()?;
    let receipt_set = record_psion_benchmark_label_generation_receipt_set(
        "psion-benchmark-label-generation-receipt-set-v1",
        &catalog,
        &artifact_lineage,
        &lifecycle,
        &exclusion,
        receipts,
        "Canonical receipt set proving the main Psion benchmark families record exact-truth bindings, rubric versions, and derived-data lineage back to reviewed parent sources or generators instead of treating benchmark labels as opaque hand-maintained state.",
    )?;

    fs::write(
        fixtures_dir.join("psion_benchmark_label_generation_receipt_set_v1.json"),
        serde_json::to_vec_pretty(&receipt_set)?,
    )?;
    Ok(())
}

fn build_package_receipt(
    package: &PsionBenchmarkPackageContract,
    artifact_lineage: &PsionArtifactLineageManifest,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
) -> Result<psionic_train::PsionBenchmarkLabelGenerationReceipt, Box<dyn Error>> {
    let item_receipts = package
        .items
        .iter()
        .map(|item| build_item_receipt(package, item.item_id.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(record_psion_benchmark_label_generation_receipt(
        format!("{}-label-generation-receipt-v1", package.package_id),
        package,
        item_receipts,
        format!(
            "Label-generation receipt for `{}` proving benchmark labels stay bound to exact-truth or rubric-version contracts with explicit derived-data lineage.",
            package.package_id
        ),
        artifact_lineage,
        lifecycle,
        exclusion,
    )?)
}

fn build_item_receipt(
    package: &PsionBenchmarkPackageContract,
    item_id: &str,
) -> Result<PsionBenchmarkItemLabelGenerationReceipt, Box<dyn Error>> {
    let item = package
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .ok_or_else(|| format!("missing package item `{item_id}`"))?;
    let grader = package
        .grader_interfaces
        .iter()
        .find(|grader| grader_id(grader) == item.grader_id)
        .ok_or_else(|| format!("missing grader `{}`", item.grader_id))?;
    let item_receipt = match grader {
        PsionBenchmarkGraderInterface::RubricScore(rubric) => {
            PsionBenchmarkItemLabelGenerationReceipt {
                item_id: item.item_id.clone(),
                grader_id: item.grader_id.clone(),
                generation_mode: PsionBenchmarkLabelGenerationMode::RubricBacked,
                label_logic: PsionBenchmarkLabelLogicBinding {
                    logic_id: format!("{}-labelgen", item.item_id),
                    logic_version: String::from("v1"),
                    generator_ref: String::from(
                        "generator://psion/benchmark/rubric-label-assembly-v1",
                    ),
                    detail: String::from(
                        "Rubric-backed items pin the label-generation logic version used to assemble reasoning labels.",
                    ),
                },
                exact_truth: None,
                rubric_binding: Some(PsionBenchmarkRubricVersionBinding {
                    rubric_ref: rubric.rubric_ref.clone(),
                    rubric_version: String::from("2026.03.22"),
                    reviewer_guidance_ref: String::from(
                        "guidance://psion/benchmark/rubric/reasoning-v1",
                    ),
                    detail: String::from(
                        "Rubric-backed labels pin the rubric reference, rubric version, and reviewer guidance used to score the item.",
                    ),
                }),
                derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                    generated_item_digest: format!("derived-item-{}", item.item_id),
                    generated_label_digest: format!("derived-label-{}", item.item_id),
                    parent_source_ids: item.source_ids.clone(),
                    parent_artifact_refs: vec![String::from(
                        "artifact://psion/benchmark/source-pack-v1",
                    )],
                    generator_refs: vec![String::from(
                        "generator://psion/benchmark/rubric-label-assembly-v1",
                    )],
                    derived_from_parent_sources: true,
                    contamination_review_bound: true,
                    detail: String::from(
                        "Derived-data lineage preserves the parent reviewed sources, parent artifact reference, and rubric generator reference for the item and its label.",
                    ),
                },
                detail: String::from(
                    "Rubric-backed item records rubric versioning and derived-data lineage.",
                ),
            }
        }
        PsionBenchmarkGraderInterface::ExactLabel(grader) => {
            let exact_truth = if item.item_id == "eng-case-2" {
                PsionBenchmarkExactTruthBinding::CpuReferenceLabel {
                    runtime_ref: String::from("runtime://psion/cpu_reference_queue_model_v1"),
                    truth_artifact_ref: String::from(
                        "artifact://psion/benchmark/queue_depth_limit_cpu_reference_v1",
                    ),
                    truth_artifact_digest: String::from(
                        "sha256:psion_queue_depth_limit_cpu_reference_v1",
                    ),
                    label_namespace: grader.label_namespace.clone(),
                    accepted_labels: grader.accepted_labels.clone(),
                    detail: String::from(
                        "This exact-label item binds to a CPU-reference queue model so exact execution truth is explicit on the label-generation side.",
                    ),
                }
            } else {
                PsionBenchmarkExactTruthBinding::EquivalentExactLabel {
                    truth_ref: String::from("truth://psion/benchmark/spec-extract-v1"),
                    truth_artifact_digest: format!("sha256:truth_{}", item.item_id),
                    label_namespace: grader.label_namespace.clone(),
                    accepted_labels: grader.accepted_labels.clone(),
                    detail: String::from(
                        "Equivalent exact truth keeps the deterministic label extraction source versioned when CPU-reference execution is not the right anchor.",
                    ),
                }
            };
            PsionBenchmarkItemLabelGenerationReceipt {
                item_id: item.item_id.clone(),
                grader_id: item.grader_id.clone(),
                generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                label_logic: PsionBenchmarkLabelLogicBinding {
                    logic_id: format!("{}-labelgen", item.item_id),
                    logic_version: String::from("v1"),
                    generator_ref: String::from(
                        "generator://psion/benchmark/exact-label-projection-v1",
                    ),
                    detail: String::from(
                        "Exact-label items pin the deterministic projection logic used to materialize labels.",
                    ),
                },
                exact_truth: Some(exact_truth),
                rubric_binding: None,
                derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                    generated_item_digest: format!("derived-item-{}", item.item_id),
                    generated_label_digest: format!("derived-label-{}", item.item_id),
                    parent_source_ids: item.source_ids.clone(),
                    parent_artifact_refs: vec![String::from(
                        "artifact://psion/benchmark/source-pack-v1",
                    )],
                    generator_refs: vec![String::from(
                        "generator://psion/benchmark/exact-label-projection-v1",
                    )],
                    derived_from_parent_sources: true,
                    contamination_review_bound: true,
                    detail: String::from(
                        "Derived-data lineage preserves the parent sources and deterministic generator for the exact label.",
                    ),
                },
                detail: String::from(
                    "Exact-label item records CPU-reference or equivalent exact truth plus derived-data lineage.",
                ),
            }
        }
        PsionBenchmarkGraderInterface::ExactRoute(grader) => {
            PsionBenchmarkItemLabelGenerationReceipt {
                item_id: item.item_id.clone(),
                grader_id: item.grader_id.clone(),
                generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                label_logic: PsionBenchmarkLabelLogicBinding {
                    logic_id: format!("{}-labelgen", item.item_id),
                    logic_version: String::from("v1"),
                    generator_ref: String::from("generator://psion/benchmark/route-policy-v1"),
                    detail: String::from(
                        "Route items pin the exact policy procedure used to derive route labels.",
                    ),
                },
                exact_truth: Some(PsionBenchmarkExactTruthBinding::RoutePolicy {
                    truth_ref: String::from("route://psion/exactness_boundary"),
                    truth_artifact_digest: format!("sha256:truth_{}", item.item_id),
                    expected_route: grader.expected_route,
                    detail: String::from(
                        "Route-selection labels bind to an exact route boundary instead of a freeform annotation pass.",
                    ),
                }),
                rubric_binding: None,
                derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                    generated_item_digest: format!("derived-item-{}", item.item_id),
                    generated_label_digest: format!("derived-label-{}", item.item_id),
                    parent_source_ids: item.source_ids.clone(),
                    parent_artifact_refs: vec![String::from(
                        "artifact://psion/benchmark/source-pack-v1",
                    )],
                    generator_refs: vec![String::from(
                        "generator://psion/benchmark/route-policy-v1",
                    )],
                    derived_from_parent_sources: true,
                    contamination_review_bound: true,
                    detail: String::from(
                        "Derived-data lineage preserves the parent sources and exact route-policy generator used for the label.",
                    ),
                },
                detail: String::from(
                    "Exact route item records route-policy truth and derived-data lineage.",
                ),
            }
        }
        PsionBenchmarkGraderInterface::ExactRefusal(grader) => {
            PsionBenchmarkItemLabelGenerationReceipt {
                item_id: item.item_id.clone(),
                grader_id: item.grader_id.clone(),
                generation_mode: PsionBenchmarkLabelGenerationMode::Exact,
                label_logic: PsionBenchmarkLabelLogicBinding {
                    logic_id: format!("{}-labelgen", item.item_id),
                    logic_version: String::from("v1"),
                    generator_ref: String::from("generator://psion/benchmark/refusal-policy-v1"),
                    detail: String::from(
                        "Refusal items pin the exact refusal-policy procedure used to derive labels.",
                    ),
                },
                exact_truth: Some(PsionBenchmarkExactTruthBinding::RefusalPolicy {
                    truth_ref: String::from("refusal://psion/benchmark/boundary-v1"),
                    truth_artifact_digest: format!("sha256:truth_{}", item.item_id),
                    accepted_reason_codes: grader.accepted_reason_codes.clone(),
                    detail: String::from(
                        "Refusal labels bind to an exact refusal boundary instead of a freeform review note.",
                    ),
                }),
                rubric_binding: None,
                derived_data_lineage: PsionBenchmarkDerivedDataLineage {
                    generated_item_digest: format!("derived-item-{}", item.item_id),
                    generated_label_digest: format!("derived-label-{}", item.item_id),
                    parent_source_ids: item.source_ids.clone(),
                    parent_artifact_refs: vec![String::from(
                        "artifact://psion/benchmark/source-pack-v1",
                    )],
                    generator_refs: vec![String::from(
                        "generator://psion/benchmark/refusal-policy-v1",
                    )],
                    derived_from_parent_sources: true,
                    contamination_review_bound: true,
                    detail: String::from(
                        "Derived-data lineage preserves the parent sources and exact refusal generator used for the label.",
                    ),
                },
                detail: String::from(
                    "Exact refusal item records refusal-policy truth and derived-data lineage.",
                ),
            }
        }
    };
    Ok(item_receipt)
}

fn grader_id(grader: &PsionBenchmarkGraderInterface) -> &str {
    match grader {
        PsionBenchmarkGraderInterface::ExactLabel(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::RubricScore(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::ExactRoute(grader) => grader.grader_id.as_str(),
        PsionBenchmarkGraderInterface::ExactRefusal(grader) => grader.grader_id.as_str(),
    }
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| String::from("workspace root not found").into())
}
