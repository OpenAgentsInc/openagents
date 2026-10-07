use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_models::{
    TassadarArchitectureBakeoffFamily, tassadar_architecture_bakeoff_publication,
};

const BUNDLE_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_ARCHITECTURE_BAKEOFF_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_architecture_bakeoff_v1/architecture_bakeoff_budget_bundle.json";

/// Same-task same-budget row for one architecture family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBakeoffBudgetRow {
    pub architecture_family: TassadarArchitectureBakeoffFamily,
    pub train_budget_tokens: u32,
    pub eval_case_budget: u32,
    pub cost_cap_bps: u32,
    pub stability_replay_count: u32,
    pub source_refs: Vec<String>,
    pub note: String,
}

/// Train-side same-budget bundle for the architecture bakeoff lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBakeoffBudgetBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub publication_id: String,
    pub workload_families: Vec<String>,
    pub budget_rows: Vec<TassadarArchitectureBakeoffBudgetRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Builds the canonical architecture-bakeoff budget bundle.
#[must_use]
pub fn build_tassadar_architecture_bakeoff_budget_bundle() -> TassadarArchitectureBakeoffBudgetBundle
{
    let publication = tassadar_architecture_bakeoff_publication();
    let budget_rows = vec![
        row(
            TassadarArchitectureBakeoffFamily::CompiledExactExecutor,
            &[
                "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json",
            ],
            "compiled exact executors stay on the shared budget while carrying the exact checkpoint, effect-taxonomy, and long-horizon receipts that anchor the broadened program families",
        ),
        row(
            TassadarArchitectureBakeoffFamily::FlatDecoderTraceModel,
            &[
                "fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v11/architecture_comparison_report.json",
                "fixtures/tassadar/reports/tassadar_trace_state_ablation_report.json",
            ],
            "flat decoder trace models share the same budget as the other families and stay explicit as one baseline rather than the default winner",
        ),
        row(
            TassadarArchitectureBakeoffFamily::SharedDepthRecurrentRefinement,
            &["fixtures/tassadar/reports/tassadar_shared_depth_architecture_report.json"],
            "shared-depth recurrent refinement uses the same train and eval budget so loop-heavy wins stay comparable rather than narrative-driven",
        ),
        row(
            TassadarArchitectureBakeoffFamily::LinearRecurrentizedAttentionExecutor,
            &[
                "fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v11/architecture_comparison_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
            ],
            "linear or recurrentized attention executors use the same budget so cheaper decode paths do not get extra budget advantages in the bakeoff",
        ),
        row(
            TassadarArchitectureBakeoffFamily::MemoryAugmentedExecutor,
            &[
                "fixtures/tassadar/reports/tassadar_working_memory_tier_summary.json",
                "fixtures/tassadar/reports/tassadar_module_state_architecture_report.json",
            ],
            "memory-augmented executors keep the same budget while carrying explicit state-publication and module-state evidence refs",
        ),
        row(
            TassadarArchitectureBakeoffFamily::PointerExecutor,
            &["fixtures/tassadar/reports/tassadar_conditional_masking_report.json"],
            "pointer executors stay on the shared budget and keep pointer-head wins tied to the same workload and cost cap",
        ),
        row(
            TassadarArchitectureBakeoffFamily::SearchNativeExecutor,
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_architecture_report.json"],
            "search-native executors stay on the shared budget instead of getting a search-only evaluation universe",
        ),
        row(
            TassadarArchitectureBakeoffFamily::HybridPlannerExecutor,
            &[
                "fixtures/tassadar/reports/tassadar_composite_routing_report.json",
                "fixtures/tassadar/reports/tassadar_counterfactual_route_quality_report.json",
                "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json",
            ],
            "hybrid planner executors stay on the same budget and keep routing plus effect-policy evidence explicit instead of inheriting a separate orchestration-only scorecard",
        ),
    ];
    let mut bundle = TassadarArchitectureBakeoffBudgetBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_id: String::from("tassadar.architecture_bakeoff.budget_bundle.v1"),
        publication_id: publication.publication_id,
        workload_families: publication.workload_families,
        budget_rows,
        claim_boundary: String::from(
            "this bundle freezes one same-task same-budget architecture matrix over shared workload families. It does not promote any architecture family by itself, and it does not allow earlier-landed lanes to quietly inherit extra budget or a separate evaluation universe",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Architecture bakeoff budget bundle freezes {} architecture rows over {} shared workload families at one common budget.",
        bundle.budget_rows.len(),
        bundle.workload_families.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_architecture_bakeoff_budget_bundle|",
        &bundle,
    );
    bundle
}

/// Returns the canonical absolute path for the committed budget bundle.
#[must_use]
pub fn tassadar_architecture_bakeoff_budget_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_ARCHITECTURE_BAKEOFF_BUNDLE_REF)
}

/// Writes the committed architecture-bakeoff budget bundle.
pub fn write_tassadar_architecture_bakeoff_budget_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArchitectureBakeoffBudgetBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_architecture_bakeoff_budget_bundle();
    let json = serde_json::to_string_pretty(&bundle)
        .expect("architecture bakeoff budget bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_architecture_bakeoff_budget_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarArchitectureBakeoffBudgetBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn row(
    architecture_family: TassadarArchitectureBakeoffFamily,
    source_refs: &[&str],
    note: &str,
) -> TassadarArchitectureBakeoffBudgetRow {
    TassadarArchitectureBakeoffBudgetRow {
        architecture_family,
        train_budget_tokens: 1_200_000,
        eval_case_budget: 32,
        cost_cap_bps: 8_000,
        stability_replay_count: 4,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        build_tassadar_architecture_bakeoff_budget_bundle,
        load_tassadar_architecture_bakeoff_budget_bundle,
        tassadar_architecture_bakeoff_budget_bundle_path,
    };
    use psionic_models::TassadarArchitectureBakeoffFamily;

    #[test]
    fn architecture_bakeoff_budget_bundle_is_machine_legible() {
        let bundle = build_tassadar_architecture_bakeoff_budget_bundle();

        assert_eq!(bundle.budget_rows.len(), 8);
        assert!(
            bundle
                .budget_rows
                .iter()
                .all(|row| row.eval_case_budget == 32)
        );
        assert!(bundle.budget_rows.iter().any(|row| {
            row.architecture_family == TassadarArchitectureBakeoffFamily::MemoryAugmentedExecutor
        }));
        assert!(bundle.budget_rows.iter().any(|row| {
            row.architecture_family == TassadarArchitectureBakeoffFamily::HybridPlannerExecutor
        }));
    }

    #[test]
    fn architecture_bakeoff_budget_bundle_matches_committed_truth() {
        let expected = build_tassadar_architecture_bakeoff_budget_bundle();
        let committed = load_tassadar_architecture_bakeoff_budget_bundle(
            tassadar_architecture_bakeoff_budget_bundle_path(),
        )
        .expect("committed architecture bakeoff budget bundle");

        assert_eq!(committed, expected);
    }
}
