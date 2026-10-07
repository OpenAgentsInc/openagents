use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_data::{
    TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF, TassadarSearchNativeWorkloadFamily,
    tassadar_search_native_executor_contract,
};
use psionic_models::tassadar_search_native_executor_publication;

const BUNDLE_SCHEMA_VERSION: u16 = 1;

/// One same-budget training case in the search-native lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeEvidenceCase {
    pub case_id: String,
    pub workload_family: TassadarSearchNativeWorkloadFamily,
    pub train_budget_tokens: u32,
    pub eval_case_budget: u32,
    pub search_budget_limit: u32,
    pub straight_trace_baseline_ref: String,
    pub verifier_guided_baseline_ref: String,
    pub target_signal_refs: Vec<String>,
    pub note: String,
}

/// Train-side evidence bundle for the search-native lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub contract_ref: String,
    pub publication_id: String,
    pub workload_families: Vec<String>,
    pub evidence_cases: Vec<TassadarSearchNativeEvidenceCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Builds the canonical train-side search-native evidence bundle.
#[must_use]
pub fn build_tassadar_search_native_executor_evidence_bundle() -> TassadarSearchNativeEvidenceBundle
{
    let contract = tassadar_search_native_executor_contract();
    let publication = tassadar_search_native_executor_publication();
    let evidence_cases = vec![
        case(
            "sudoku_backtracking_search",
            TassadarSearchNativeWorkloadFamily::SudokuBacktrackingSearch,
            12,
            "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            &[
                "fixtures/tassadar/runs/tassadar_verifier_guided_search_trace_family_v1/search_trace_family_report.json",
                "fixtures/tassadar/reports/tassadar_supervision_density_report.json",
            ],
            "Sudoku keeps guess, contradiction, and backtrack signals first-class under one shared budget",
        ),
        case(
            "branch_heavy_clrs_variant",
            TassadarSearchNativeWorkloadFamily::BranchHeavyClrsVariant,
            10,
            "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_pointer_memory_scratchpad_report.json",
            ],
            "branch-heavy CLRS keeps graph-search structure and branch-summary signals explicit instead of forcing a straight trace",
        ),
        case(
            "search_kernel_recovery",
            TassadarSearchNativeWorkloadFamily::SearchKernelRecovery,
            8,
            "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            &[
                "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_report.json",
                "fixtures/tassadar/reports/tassadar_error_regime_catalog.json",
            ],
            "search-kernel recovery keeps guess/verify/backtrack state aligned with explicit recovery metrics under one fixed budget",
        ),
        case(
            "verifier_heavy_workload_pack",
            TassadarSearchNativeWorkloadFamily::VerifierHeavyWorkloadPack,
            10,
            "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            &[
                "fixtures/tassadar/reports/tassadar_latency_evidence_tradeoff_report.json",
                "fixtures/tassadar/reports/tassadar_receipt_supervision_report.json",
            ],
            "verifier-heavy pack keeps search-budget exhaustion explicit so the family can refuse instead of silently degrading",
        ),
    ];
    let mut bundle = TassadarSearchNativeEvidenceBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_id: String::from("tassadar.search_native_executor.evidence_bundle.v1"),
        contract_ref: contract.contract_ref,
        publication_id: publication.publication_id,
        workload_families: publication.workload_families,
        evidence_cases,
        claim_boundary: String::from(
            "this train bundle freezes one same-budget search-native study over Sudoku, branch-heavy CLRS, search-kernel recovery, and verifier-heavy packs. It stays benchmark-bound and does not widen served capability or imply broad learned search closure",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Search-native evidence bundle freezes {} workload families with {} same-budget cases.",
        bundle.workload_families.len(),
        bundle.evidence_cases.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_search_native_executor_evidence_bundle|",
        &bundle,
    );
    bundle
}

/// Returns the canonical absolute path for the committed search-native evidence bundle.
#[must_use]
pub fn tassadar_search_native_executor_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF)
}

/// Writes the committed search-native evidence bundle.
pub fn write_tassadar_search_native_executor_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeEvidenceBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_search_native_executor_evidence_bundle();
    let json =
        serde_json::to_string_pretty(&bundle).expect("search-native evidence bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_search_native_executor_evidence_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeEvidenceBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn case(
    case_id: &str,
    workload_family: TassadarSearchNativeWorkloadFamily,
    search_budget_limit: u32,
    straight_trace_baseline_ref: &str,
    verifier_guided_baseline_ref: &str,
    target_signal_refs: &[&str],
    note: &str,
) -> TassadarSearchNativeEvidenceCase {
    TassadarSearchNativeEvidenceCase {
        case_id: String::from(case_id),
        workload_family,
        train_budget_tokens: 1_100_000,
        eval_case_budget: 20,
        search_budget_limit,
        straight_trace_baseline_ref: String::from(straight_trace_baseline_ref),
        verifier_guided_baseline_ref: String::from(verifier_guided_baseline_ref),
        target_signal_refs: target_signal_refs
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
        build_tassadar_search_native_executor_evidence_bundle,
        load_tassadar_search_native_executor_evidence_bundle,
        tassadar_search_native_executor_evidence_bundle_path,
    };
    use psionic_data::TassadarSearchNativeWorkloadFamily;

    #[test]
    fn search_native_executor_evidence_bundle_is_machine_legible() {
        let bundle = build_tassadar_search_native_executor_evidence_bundle();

        assert_eq!(bundle.evidence_cases.len(), 4);
        assert!(
            bundle
                .evidence_cases
                .iter()
                .all(|case| case.eval_case_budget == 20)
        );
        assert!(bundle.evidence_cases.iter().any(|case| {
            case.workload_family == TassadarSearchNativeWorkloadFamily::VerifierHeavyWorkloadPack
                && case.search_budget_limit == 10
        }));
    }

    #[test]
    fn search_native_executor_evidence_bundle_matches_committed_truth() {
        let expected = build_tassadar_search_native_executor_evidence_bundle();
        let committed = load_tassadar_search_native_executor_evidence_bundle(
            tassadar_search_native_executor_evidence_bundle_path(),
        )
        .expect("committed search-native evidence bundle");

        assert_eq!(committed, expected);
    }
}
