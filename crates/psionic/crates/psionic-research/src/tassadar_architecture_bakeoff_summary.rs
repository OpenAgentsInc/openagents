use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::{
    TassadarArchitectureBakeoffCell, TassadarArchitectureOwnershipPosture,
    build_tassadar_architecture_bakeoff_report,
};
use psionic_models::TASSADAR_ARCHITECTURE_BAKEOFF_SUMMARY_REF;

/// Research-facing summary over the architecture bakeoff matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBakeoffSummary {
    pub summary_id: String,
    pub report_id: String,
    pub strongest_architecture_by_workload: Vec<(String, String)>,
    pub owned_workloads_by_architecture: Vec<(String, Vec<String>)>,
    pub no_honest_owner_workloads: Vec<String>,
    pub refusal_first_workloads: Vec<String>,
    pub claim_boundary: String,
    pub summary_digest: String,
}

/// Builds the committed research summary for the architecture bakeoff lane.
#[must_use]
pub fn build_tassadar_architecture_bakeoff_summary() -> TassadarArchitectureBakeoffSummary {
    let report = build_tassadar_architecture_bakeoff_report();
    let strongest_architecture_by_workload = report
        .publication
        .workload_families
        .iter()
        .map(|workload| {
            let winning_cell = report
                .matrix_cells
                .iter()
                .filter(|cell| cell.workload_family == *workload)
                .max_by_key(|cell| {
                    (
                        ownership_rank(cell.ownership_posture),
                        cell.exactness_bps,
                        cell.stability_bps,
                        u32::MAX - cell.cost_score_bps,
                    )
                })
                .expect("each workload should have at least one architecture row");
            (
                workload.clone(),
                winning_cell.architecture_family.as_str().to_string(),
            )
        })
        .collect::<Vec<_>>();
    let owned_workloads_by_architecture = report
        .publication
        .architecture_families
        .iter()
        .map(|family| {
            let workloads = report
                .matrix_cells
                .iter()
                .filter(|cell| {
                    cell.architecture_family == *family
                        && cell.ownership_posture == TassadarArchitectureOwnershipPosture::Owns
                })
                .map(|cell| cell.workload_family.clone())
                .collect::<Vec<_>>();
            (family.as_str().to_string(), workloads)
        })
        .collect::<Vec<_>>();
    let refusal_first_workloads = refusal_first_workloads(&report.matrix_cells);
    let mut summary = TassadarArchitectureBakeoffSummary {
        summary_id: String::from("tassadar.architecture_bakeoff.summary.v1"),
        report_id: report.report_id,
        strongest_architecture_by_workload,
        owned_workloads_by_architecture,
        no_honest_owner_workloads: report.no_honest_owner_workloads,
        refusal_first_workloads,
        claim_boundary: String::from(
            "this summary remains a research-only architecture comparison over the broadened program-family suite. It makes workload-family ownership, no-owner regions, and refusal-first regions explicit and does not widen served capability or broad learned-compute claims by itself",
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest =
        stable_digest(b"psionic_tassadar_architecture_bakeoff_summary|", &summary);
    summary
}

/// Returns the canonical absolute path for the committed research summary.
#[must_use]
pub fn tassadar_architecture_bakeoff_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARCHITECTURE_BAKEOFF_SUMMARY_REF)
}

/// Writes the committed research summary.
pub fn write_tassadar_architecture_bakeoff_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArchitectureBakeoffSummary, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_architecture_bakeoff_summary();
    let json =
        serde_json::to_string_pretty(&summary).expect("architecture bakeoff summary serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

#[cfg(test)]
pub fn load_tassadar_architecture_bakeoff_summary(
    path: impl AsRef<Path>,
) -> Result<TassadarArchitectureBakeoffSummary, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn ownership_rank(posture: TassadarArchitectureOwnershipPosture) -> u8 {
    match posture {
        TassadarArchitectureOwnershipPosture::Owns => 4,
        TassadarArchitectureOwnershipPosture::Competitive => 3,
        TassadarArchitectureOwnershipPosture::ResearchOnly => 2,
        TassadarArchitectureOwnershipPosture::RefuseFirst => 1,
    }
}

fn refusal_first_workloads(matrix_cells: &[TassadarArchitectureBakeoffCell]) -> Vec<String> {
    let mut counts = BTreeMap::new();
    for cell in matrix_cells {
        if cell.ownership_posture == TassadarArchitectureOwnershipPosture::RefuseFirst {
            *counts.entry(cell.workload_family.clone()).or_insert(0u32) += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(workload, count)| (count >= 2).then_some(workload))
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
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
        build_tassadar_architecture_bakeoff_summary, load_tassadar_architecture_bakeoff_summary,
        tassadar_architecture_bakeoff_summary_path,
    };

    #[test]
    fn architecture_bakeoff_summary_keeps_ownership_and_refusal_regions_explicit() {
        let summary = build_tassadar_architecture_bakeoff_summary();

        assert!(summary.strongest_architecture_by_workload.contains(&(
            String::from("sudoku_backtracking_search"),
            String::from("search_native_executor")
        )));
        assert!(
            summary
                .refusal_first_workloads
                .contains(&String::from("module_scale_wasm_loop"))
        );
        assert_eq!(
            summary.no_honest_owner_workloads,
            vec![
                String::from("linked_program_bundle"),
                String::from("import_mediated_process"),
                String::from("stateful_process_loop"),
            ]
        );
    }

    #[test]
    fn architecture_bakeoff_summary_matches_committed_truth() {
        let expected = build_tassadar_architecture_bakeoff_summary();
        let committed = load_tassadar_architecture_bakeoff_summary(
            tassadar_architecture_bakeoff_summary_path(),
        )
        .expect("committed architecture bakeoff summary");

        assert_eq!(committed, expected);
    }
}
