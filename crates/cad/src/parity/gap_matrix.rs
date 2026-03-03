use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::parity::openagents_crawler::OpenagentsCapabilityInventory;
use crate::parity::vcad_crawler::VcadCapabilityInventory;

pub const PARITY_GAP_MATRIX_ISSUE_ID: &str = "VCAD-PARITY-004";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityGapMatrix {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from: GapMatrixSources,
    pub docs: GapMatrixSurface,
    pub crates: GapMatrixSurface,
    pub commands: GapMatrixSurface,
    pub summary: GapMatrixSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GapMatrixSources {
    pub vcad_inventory_path: String,
    pub openagents_inventory_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapMatrixSurface {
    pub reference_count: usize,
    pub matched_count: usize,
    pub missing_count: usize,
    pub match_rate: f64,
    pub rows: Vec<GapMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapMatrixRow {
    pub reference_key: String,
    pub reference_label: String,
    pub reference_status: Option<String>,
    pub openagents_key: Option<String>,
    pub openagents_label: Option<String>,
    pub openagents_status: Option<String>,
    pub score: f64,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapMatrixSummary {
    pub total_reference_count: usize,
    pub total_matched_count: usize,
    pub total_missing_count: usize,
    pub total_match_rate: f64,
}

#[derive(Clone)]
struct OpenagentsCandidate {
    key: String,
    label: String,
    status: Option<String>,
    tokens: BTreeSet<String>,
}

pub fn build_gap_matrix(
    vcad: &VcadCapabilityInventory,
    openagents: &OpenagentsCapabilityInventory,
    vcad_inventory_path: &str,
    openagents_inventory_path: &str,
) -> ParityGapMatrix {
    let doc_candidates: Vec<OpenagentsCandidate> = openagents
        .docs
        .iter()
        .map(|doc| OpenagentsCandidate {
            key: normalize_key(&doc.capability),
            label: format!("{} :: {}", doc.source_file, doc.capability),
            status: Some(doc.status.clone()),
            tokens: tokenize(&doc.capability),
        })
        .collect();

    let crate_candidates: Vec<OpenagentsCandidate> = openagents
        .crates
        .iter()
        .map(|entry| OpenagentsCandidate {
            key: normalize_key(&entry.package_name),
            label: format!("{} ({})", entry.package_name, entry.member_path),
            status: Some(entry.category.clone()),
            tokens: tokenize(&entry.package_name),
        })
        .collect();

    let command_candidates: Vec<OpenagentsCandidate> = openagents
        .commands
        .iter()
        .map(|command| {
            let key = command
                .detail
                .as_deref()
                .map_or_else(|| normalize_key(&command.command), normalize_key);
            OpenagentsCandidate {
                key,
                label: format!("{} ({})", command.command, command.source_file),
                status: Some(command.kind.clone()),
                tokens: tokenize(&command.command),
            }
        })
        .collect();

    let docs_rows = vcad
        .docs
        .iter()
        .map(|doc| {
            let reference_key = normalize_key(&doc.capability);
            let reference_tokens = tokenize(&doc.capability);
            let best = best_match(&reference_key, &reference_tokens, &doc_candidates, 0.45);
            build_row(
                reference_key,
                format!("{} :: {}", doc.source_file, doc.capability),
                Some(doc.status.clone()),
                best,
            )
        })
        .collect();

    let crate_rows = vcad
        .crates
        .iter()
        .map(|entry| {
            let reference_key = normalize_key(&entry.package_name);
            let reference_tokens = tokenize(&entry.package_name);
            let best = best_match(&reference_key, &reference_tokens, &crate_candidates, 0.5);
            build_row(
                reference_key,
                format!("{} ({})", entry.package_name, entry.member_path),
                Some(entry.category.clone()),
                best,
            )
        })
        .collect();

    let command_rows = vcad
        .commands
        .iter()
        .map(|command| {
            let reference_key = normalize_key(&command.cli_command);
            let reference_tokens = tokenize(&command.cli_command);
            let best = best_match(&reference_key, &reference_tokens, &command_candidates, 0.7);
            build_row(
                reference_key,
                format!("{} ({})", command.cli_command, command.variant),
                if command.description.is_empty() {
                    None
                } else {
                    Some(command.description.clone())
                },
                best,
            )
        })
        .collect();

    let docs = summarize_surface(docs_rows);
    let crates = summarize_surface(crate_rows);
    let commands = summarize_surface(command_rows);

    let total_reference_count =
        docs.reference_count + crates.reference_count + commands.reference_count;
    let total_matched_count = docs.matched_count + crates.matched_count + commands.matched_count;
    let total_missing_count = docs.missing_count + crates.missing_count + commands.missing_count;
    let total_match_rate = if total_reference_count == 0 {
        0.0
    } else {
        total_matched_count as f64 / total_reference_count as f64
    };

    let summary = GapMatrixSummary {
        total_reference_count,
        total_matched_count,
        total_missing_count,
        total_match_rate,
    };

    ParityGapMatrix {
        manifest_version: 1,
        issue_id: PARITY_GAP_MATRIX_ISSUE_ID.to_string(),
        vcad_commit: vcad.vcad_commit.clone(),
        openagents_commit: openagents.openagents_commit.clone(),
        generated_from: GapMatrixSources {
            vcad_inventory_path: vcad_inventory_path.to_string(),
            openagents_inventory_path: openagents_inventory_path.to_string(),
        },
        docs,
        crates,
        commands,
        summary,
    }
}

fn summarize_surface(mut rows: Vec<GapMatrixRow>) -> GapMatrixSurface {
    rows.sort_by(|left, right| left.reference_key.cmp(&right.reference_key));
    let reference_count = rows.len();
    let matched_count = rows.iter().filter(|row| row.state == "matched").count();
    let missing_count = reference_count.saturating_sub(matched_count);
    let match_rate = if reference_count == 0 {
        0.0
    } else {
        matched_count as f64 / reference_count as f64
    };
    GapMatrixSurface {
        reference_count,
        matched_count,
        missing_count,
        match_rate,
        rows,
    }
}

fn build_row(
    reference_key: String,
    reference_label: String,
    reference_status: Option<String>,
    best: Option<(OpenagentsCandidate, f64)>,
) -> GapMatrixRow {
    if let Some((candidate, score)) = best {
        return GapMatrixRow {
            reference_key,
            reference_label,
            reference_status,
            openagents_key: Some(candidate.key),
            openagents_label: Some(candidate.label),
            openagents_status: candidate.status,
            score,
            state: "matched".to_string(),
        };
    }

    GapMatrixRow {
        reference_key,
        reference_label,
        reference_status,
        openagents_key: None,
        openagents_label: None,
        openagents_status: None,
        score: 0.0,
        state: "missing".to_string(),
    }
}

fn best_match(
    reference_key: &str,
    reference_tokens: &BTreeSet<String>,
    candidates: &[OpenagentsCandidate],
    min_score: f64,
) -> Option<(OpenagentsCandidate, f64)> {
    let mut best: Option<(OpenagentsCandidate, f64)> = None;
    for candidate in candidates {
        if candidate.key == reference_key {
            return Some((candidate.clone(), 1.0));
        }

        let score = dice_similarity(reference_tokens, &candidate.tokens);
        if score < min_score {
            continue;
        }
        match &best {
            None => {
                best = Some((candidate.clone(), score));
            }
            Some((existing, existing_score)) => {
                if score > *existing_score
                    || ((score - *existing_score).abs() < f64::EPSILON
                        && candidate.key < existing.key)
                {
                    best = Some((candidate.clone(), score));
                }
            }
        }
    }
    best
}

fn dice_similarity(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    if intersection == 0 {
        return 0.0;
    }
    (2.0 * intersection as f64) / (left.len() as f64 + right.len() as f64)
}

fn normalize_key(value: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for char in value.chars() {
        if char.is_ascii_alphanumeric() {
            out.push(char.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn tokenize(value: &str) -> BTreeSet<String> {
    const STOP_WORDS: [&str; 20] = [
        "a",
        "an",
        "and",
        "app",
        "cad",
        "core",
        "feature",
        "features",
        "for",
        "in",
        "kernel",
        "mode",
        "of",
        "on",
        "openagents",
        "operation",
        "operations",
        "the",
        "to",
        "vcad",
    ];

    value
        .split(|char: char| !char.is_ascii_alphanumeric())
        .filter_map(|token| {
            let normalized = token.to_ascii_lowercase();
            if normalized.is_empty() || STOP_WORDS.contains(&normalized.as_str()) {
                return None;
            }
            Some(normalized)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{best_match, dice_similarity, normalize_key, tokenize};

    #[test]
    fn normalize_key_collapses_delimiters() {
        assert_eq!(normalize_key("ImportUrdf"), "importurdf");
        assert_eq!(normalize_key("import_urdf"), "import-urdf");
        assert_eq!(normalize_key("STEP Export"), "step-export");
    }

    #[test]
    fn tokenize_filters_stop_words() {
        let tokens = tokenize("CAD Feature Operations and STEP Export");
        assert!(tokens.contains("step"));
        assert!(tokens.contains("export"));
        assert!(!tokens.contains("cad"));
        assert!(!tokens.contains("operations"));
    }

    #[test]
    fn dice_similarity_matches_shared_tokens() {
        let left = tokenize("STEP import export");
        let right = tokenize("step export");
        let score = dice_similarity(&left, &right);
        assert!(score > 0.7);
    }

    #[test]
    fn best_match_prefers_exact_key() {
        let reference_key = "export";
        let reference_tokens = tokenize("export");
        let candidates = vec![
            super::OpenagentsCandidate {
                key: "export".to_string(),
                label: "Export".to_string(),
                status: None,
                tokens: tokenize("export"),
            },
            super::OpenagentsCandidate {
                key: "import".to_string(),
                label: "Import".to_string(),
                status: None,
                tokens: tokenize("import"),
            },
        ];
        let matched = best_match(reference_key, &reference_tokens, &candidates, 0.5)
            .expect("exact key should match");
        assert_eq!(matched.0.key, "export");
        assert_eq!(matched.1, 1.0);
    }
}
