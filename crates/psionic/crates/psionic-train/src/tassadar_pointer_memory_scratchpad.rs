use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_models::{TassadarSeparationStudyAxis, tassadar_pointer_memory_scratchpad_publication};

const BUNDLE_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_POINTER_MEMORY_SCRATCHPAD_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_pointer_memory_scratchpad_study_v1/pointer_memory_scratchpad_ablation_bundle.json";

/// Same-budget ablation row for one study axis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadBudgetRow {
    pub study_axis: TassadarSeparationStudyAxis,
    pub train_budget_tokens: u32,
    pub eval_case_budget: u32,
    pub source_refs: Vec<String>,
    pub note: String,
}

/// Train-side same-budget bundle for the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadAblationBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub publication_id: String,
    pub workload_families: Vec<String>,
    pub budget_rows: Vec<TassadarPointerMemoryScratchpadBudgetRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

/// Builds the canonical separation-study ablation bundle.
#[must_use]
pub fn build_tassadar_pointer_memory_scratchpad_ablation_bundle()
-> TassadarPointerMemoryScratchpadAblationBundle {
    let publication = tassadar_pointer_memory_scratchpad_publication();
    let budget_rows = vec![
        row(
            TassadarSeparationStudyAxis::PointerPrediction,
            &["fixtures/tassadar/reports/tassadar_conditional_masking_report.json"],
            "pointer prediction ablations stay on the shared budget and reuse the landed conditional-masking lane",
        ),
        row(
            TassadarSeparationStudyAxis::MutableMemoryAccess,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json"],
            "mutable memory access ablations stay on the shared budget and reuse the landed working-memory lane",
        ),
        row(
            TassadarSeparationStudyAxis::ScratchpadLocalReasoning,
            &["fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json"],
            "scratchpad-local reasoning ablations stay on the shared budget and reuse the landed locality-scratchpad lane",
        ),
        row(
            TassadarSeparationStudyAxis::CombinedReference,
            &["fixtures/tassadar/reports/tassadar_locality_envelope_report.json"],
            "combined-reference rows stay on the same budget and only combine already-landed mechanisms",
        ),
    ];
    let mut bundle = TassadarPointerMemoryScratchpadAblationBundle {
        schema_version: BUNDLE_SCHEMA_VERSION,
        bundle_id: String::from("tassadar.pointer_memory_scratchpad.ablation_bundle.v1"),
        publication_id: publication.publication_id,
        workload_families: publication.workload_families,
        budget_rows,
        claim_boundary: String::from(
            "this bundle freezes one same-budget separation study over pointer prediction, mutable memory access, scratchpad-local reasoning, and a combined reference row. It does not promote any one axis into a broad architectural claim by itself",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Pointer/memory/scratchpad ablation bundle freezes {} study axes over {} shared workload families at one common budget.",
        bundle.budget_rows.len(),
        bundle.workload_families.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_pointer_memory_scratchpad_ablation_bundle|",
        &bundle,
    );
    bundle
}

/// Returns the canonical absolute path for the committed ablation bundle.
#[must_use]
pub fn tassadar_pointer_memory_scratchpad_ablation_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POINTER_MEMORY_SCRATCHPAD_BUNDLE_REF)
}

/// Writes the committed ablation bundle.
pub fn write_tassadar_pointer_memory_scratchpad_ablation_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadAblationBundle, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundle = build_tassadar_pointer_memory_scratchpad_ablation_bundle();
    let json =
        serde_json::to_string_pretty(&bundle).expect("pointer/memory/scratchpad bundle serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_pointer_memory_scratchpad_ablation_bundle(
    path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadAblationBundle, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn row(
    study_axis: TassadarSeparationStudyAxis,
    source_refs: &[&str],
    note: &str,
) -> TassadarPointerMemoryScratchpadBudgetRow {
    TassadarPointerMemoryScratchpadBudgetRow {
        study_axis,
        train_budget_tokens: 900_000,
        eval_case_budget: 24,
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
        build_tassadar_pointer_memory_scratchpad_ablation_bundle,
        load_tassadar_pointer_memory_scratchpad_ablation_bundle,
        tassadar_pointer_memory_scratchpad_ablation_bundle_path,
    };
    use psionic_models::TassadarSeparationStudyAxis;

    #[test]
    fn pointer_memory_scratchpad_ablation_bundle_is_machine_legible() {
        let bundle = build_tassadar_pointer_memory_scratchpad_ablation_bundle();

        assert_eq!(bundle.budget_rows.len(), 4);
        assert!(
            bundle
                .budget_rows
                .iter()
                .all(|row| row.eval_case_budget == 24)
        );
        assert!(
            bundle
                .budget_rows
                .iter()
                .any(|row| { row.study_axis == TassadarSeparationStudyAxis::CombinedReference })
        );
    }

    #[test]
    fn pointer_memory_scratchpad_ablation_bundle_matches_committed_truth() {
        let expected = build_tassadar_pointer_memory_scratchpad_ablation_bundle();
        let committed = load_tassadar_pointer_memory_scratchpad_ablation_bundle(
            tassadar_pointer_memory_scratchpad_ablation_bundle_path(),
        )
        .expect("committed pointer/memory/scratchpad bundle");

        assert_eq!(committed, expected);
    }
}
