use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::parity::gap_matrix::{GapMatrixRow, GapMatrixSurface, ParityGapMatrix};

pub const PARITY_FIXTURE_CORPUS_ISSUE_ID: &str = "VCAD-PARITY-006";
const MATCHED_PER_SURFACE: usize = 3;
const MISSING_PER_SURFACE: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityFixtureCorpus {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_gap_matrix: String,
    pub seed_policy: FixtureSeedPolicy,
    pub fixtures: Vec<ParityFixtureSeed>,
    pub summary: FixtureCorpusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureSeedPolicy {
    pub matched_per_surface: usize,
    pub missing_per_surface: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityFixtureSeed {
    pub fixture_id: String,
    pub surface: String,
    pub state: String,
    pub reference_key: String,
    pub reference_label: String,
    pub reference_status: Option<String>,
    pub openagents_key: Option<String>,
    pub openagents_label: Option<String>,
    pub score: f64,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FixtureCorpusSummary {
    pub total_seed_count: usize,
    pub by_surface: BTreeMap<String, SurfaceSeedSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SurfaceSeedSummary {
    pub reference_total: usize,
    pub matched_seed_count: usize,
    pub missing_seed_count: usize,
    pub total_seed_count: usize,
}

pub fn build_fixture_corpus(
    matrix: &ParityGapMatrix,
    gap_matrix_path: &str,
) -> ParityFixtureCorpus {
    let mut fixtures = Vec::new();
    let mut by_surface = BTreeMap::new();

    append_surface_fixtures("docs", &matrix.docs, &mut fixtures, &mut by_surface);
    append_surface_fixtures("crates", &matrix.crates, &mut fixtures, &mut by_surface);
    append_surface_fixtures("commands", &matrix.commands, &mut fixtures, &mut by_surface);

    fixtures.sort_by(|left, right| left.fixture_id.cmp(&right.fixture_id));

    let summary = FixtureCorpusSummary {
        total_seed_count: fixtures.len(),
        by_surface,
    };

    ParityFixtureCorpus {
        manifest_version: 1,
        issue_id: PARITY_FIXTURE_CORPUS_ISSUE_ID.to_string(),
        vcad_commit: matrix.vcad_commit.clone(),
        openagents_commit: matrix.openagents_commit.clone(),
        generated_from_gap_matrix: gap_matrix_path.to_string(),
        seed_policy: FixtureSeedPolicy {
            matched_per_surface: MATCHED_PER_SURFACE,
            missing_per_surface: MISSING_PER_SURFACE,
        },
        fixtures,
        summary,
    }
}

fn append_surface_fixtures(
    surface_name: &str,
    surface: &GapMatrixSurface,
    fixtures: &mut Vec<ParityFixtureSeed>,
    by_surface: &mut BTreeMap<String, SurfaceSeedSummary>,
) {
    let mut matched_rows: Vec<&GapMatrixRow> = surface
        .rows
        .iter()
        .filter(|row| row.state == "matched")
        .collect();
    matched_rows.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.reference_key.cmp(&right.reference_key))
    });

    let mut missing_rows: Vec<&GapMatrixRow> = surface
        .rows
        .iter()
        .filter(|row| row.state == "missing")
        .collect();
    missing_rows.sort_by(|left, right| left.reference_key.cmp(&right.reference_key));

    let selected_matched = matched_rows.into_iter().take(MATCHED_PER_SURFACE);
    let selected_missing = missing_rows.into_iter().take(MISSING_PER_SURFACE);

    let mut matched_count = 0usize;
    let mut missing_count = 0usize;
    for (index, row) in selected_matched.enumerate() {
        matched_count += 1;
        fixtures.push(build_seed(surface_name, "matched", index, row));
    }
    for (index, row) in selected_missing.enumerate() {
        missing_count += 1;
        fixtures.push(build_seed(surface_name, "missing", index, row));
    }

    by_surface.insert(
        surface_name.to_string(),
        SurfaceSeedSummary {
            reference_total: surface.reference_count,
            matched_seed_count: matched_count,
            missing_seed_count: missing_count,
            total_seed_count: matched_count + missing_count,
        },
    );
}

fn build_seed(
    surface_name: &str,
    state: &str,
    index: usize,
    row: &GapMatrixRow,
) -> ParityFixtureSeed {
    ParityFixtureSeed {
        fixture_id: format!("{surface_name}.{state}.{:02}", index + 1),
        surface: surface_name.to_string(),
        state: state.to_string(),
        reference_key: row.reference_key.clone(),
        reference_label: row.reference_label.clone(),
        reference_status: row.reference_status.clone(),
        openagents_key: row.openagents_key.clone(),
        openagents_label: row.openagents_label.clone(),
        score: row.score,
        priority: priority_for(surface_name, state).to_string(),
    }
}

fn priority_for(surface_name: &str, state: &str) -> &'static str {
    match (surface_name, state) {
        ("commands", "missing") => "p0",
        ("commands", "matched") => "p0",
        ("docs", "missing") => "p0",
        ("docs", "matched") => "p1",
        ("crates", "missing") => "p1",
        ("crates", "matched") => "p2",
        _ => "p2",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_fixture_corpus, priority_for};
    use crate::parity::gap_matrix::{
        GapMatrixRow, GapMatrixSources, GapMatrixSummary, GapMatrixSurface, ParityGapMatrix,
    };

    #[test]
    fn priority_for_assigns_expected_levels() {
        assert_eq!(priority_for("commands", "missing"), "p0");
        assert_eq!(priority_for("docs", "matched"), "p1");
        assert_eq!(priority_for("crates", "matched"), "p2");
    }

    #[test]
    fn build_fixture_corpus_produces_stable_seed_counts() {
        let row_missing = GapMatrixRow {
            reference_key: "a".to_string(),
            reference_label: "a".to_string(),
            reference_status: None,
            openagents_key: None,
            openagents_label: None,
            openagents_status: None,
            score: 0.0,
            state: "missing".to_string(),
        };
        let row_matched = GapMatrixRow {
            reference_key: "b".to_string(),
            reference_label: "b".to_string(),
            reference_status: None,
            openagents_key: Some("b".to_string()),
            openagents_label: Some("b".to_string()),
            openagents_status: Some("matched".to_string()),
            score: 1.0,
            state: "matched".to_string(),
        };
        let surface = GapMatrixSurface {
            reference_count: 2,
            matched_count: 1,
            missing_count: 1,
            match_rate: 0.5,
            rows: vec![row_missing.clone(), row_matched.clone()],
        };
        let matrix = ParityGapMatrix {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-004".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from: GapMatrixSources {
                vcad_inventory_path: "a".to_string(),
                openagents_inventory_path: "b".to_string(),
            },
            docs: surface.clone(),
            crates: surface.clone(),
            commands: surface,
            summary: GapMatrixSummary {
                total_reference_count: 6,
                total_matched_count: 3,
                total_missing_count: 3,
                total_match_rate: 0.5,
            },
        };
        let corpus = build_fixture_corpus(&matrix, "gap.json");
        assert_eq!(corpus.summary.total_seed_count, 6);
        assert_eq!(corpus.fixtures.len(), 6);
    }
}
