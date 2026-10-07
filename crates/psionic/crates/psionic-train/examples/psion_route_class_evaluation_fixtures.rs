use std::{error::Error, fs, path::PathBuf};

use psionic_data::PsionArtifactLineageManifest;
use psionic_train::{
    record_psion_route_class_evaluation_receipt, PsionBenchmarkCatalog, PsionRouteClass,
    PsionRouteClassEvaluationRow,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/route");
    fs::create_dir_all(&fixtures_dir)?;

    let catalog: PsionBenchmarkCatalog = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json"),
    )?)?;
    let artifact_lineage: PsionArtifactLineageManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
        )?)?;
    let route_package = catalog
        .packages
        .iter()
        .find(|package| package.package_id == "psion_route_benchmark_v1")
        .ok_or("route package missing from benchmark catalog")?;

    let receipt = record_psion_route_class_evaluation_receipt(
        "psion-route-class-evaluation-receipt-v1",
        route_package,
        vec![
            PsionRouteClassEvaluationRow {
                item_id: String::from("route-case-answer"),
                route_class: PsionRouteClass::AnswerInLanguage,
                observed_route_accuracy_bps: 9790,
                false_positive_delegation_bps: 280,
                false_negative_delegation_bps: 0,
                detail: String::from(
                    "Language-answer route row shows the model can answer directly without spuriously delegating the prompt.",
                ),
            },
            PsionRouteClassEvaluationRow {
                item_id: String::from("route-case-uncertainty"),
                route_class: PsionRouteClass::AnswerWithUncertainty,
                observed_route_accuracy_bps: 9650,
                false_positive_delegation_bps: 160,
                false_negative_delegation_bps: 0,
                detail: String::from(
                    "Uncertainty row shows the model can stay in language while marking uncertainty instead of escalating too early.",
                ),
            },
            PsionRouteClassEvaluationRow {
                item_id: String::from("route-case-structure"),
                route_class: PsionRouteClass::RequestStructuredInputs,
                observed_route_accuracy_bps: 9580,
                false_positive_delegation_bps: 220,
                false_negative_delegation_bps: 0,
                detail: String::from(
                    "Structured-input row shows the lane asks for the missing fields instead of delegating or hallucinating them.",
                ),
            },
            PsionRouteClassEvaluationRow {
                item_id: String::from("route-case-delegate"),
                route_class: PsionRouteClass::DelegateToExactExecutor,
                observed_route_accuracy_bps: 9880,
                false_positive_delegation_bps: 0,
                false_negative_delegation_bps: 140,
                detail: String::from(
                    "Delegation row tracks exact-executor handoff without claiming the learned lane executed the workload itself.",
                ),
            },
        ],
        "Canonical route-class receipt proving answer, uncertainty, structured-input, and exact-delegation classes stay distinguishable with measurable delegation error rates.",
        &artifact_lineage,
    )?;

    fs::write(
        fixtures_dir.join("psion_route_class_evaluation_receipt_v1.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "failed to locate workspace root".into())
}
