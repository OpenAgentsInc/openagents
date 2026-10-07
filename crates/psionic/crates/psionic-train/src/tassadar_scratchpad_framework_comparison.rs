use std::{fs, path::Path};

use psionic_ir::{
    tassadar_symbolic_program_examples, TassadarControlledPositionScheme,
    TassadarScratchpadEncoding, TassadarScratchpadFormatConfig, TassadarSymbolicExpr,
    TassadarSymbolicStatement,
};
use psionic_models::{
    inspect_tassadar_scratchpad_framework, TassadarScratchpadPositionFramework,
    TassadarScratchpadWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tassadar_executor_no_hint_corpus;

pub const TASSADAR_SCRATCHPAD_FRAMEWORK_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_FILE: &str =
    "tassadar_scratchpad_framework_comparison_report.json";
pub const TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_scratchpad_framework_comparison_report.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadFrameworkCaseComparison {
    pub case_id: String,
    pub prompt_token_count: u32,
    pub target_token_count: u32,
    pub baseline_token_count: u32,
    pub candidate_token_count: u32,
    pub baseline_max_output_local_position_index: u32,
    pub candidate_max_output_local_position_index: u32,
    pub locality_gain_bps: u32,
    pub scratchpad_overhead_bps: u32,
    pub final_output_tokens_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadFrameworkVariantReport {
    pub variant_id: String,
    pub workload_family: TassadarScratchpadWorkloadFamily,
    pub baseline_framework_id: String,
    pub candidate_framework_id: String,
    pub baseline_framework_digest: String,
    pub candidate_framework_digest: String,
    pub case_count: u32,
    pub case_reports: Vec<TassadarScratchpadFrameworkCaseComparison>,
    pub mean_locality_gain_bps: u32,
    pub max_candidate_output_local_position_index: u32,
    pub max_scratchpad_overhead_bps: u32,
    pub output_exact_case_count: u32,
    pub position_reset_count_total: u32,
    pub locality_note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadFrameworkComparisonReport {
    pub schema_version: u16,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub variants: Vec<TassadarScratchpadFrameworkVariantReport>,
    pub claim_class: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarScratchpadFrameworkComparisonError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_scratchpad_framework_comparison_report(
) -> TassadarScratchpadFrameworkComparisonReport {
    let variants = vec![
        build_variant_report(
            "arithmetic_segment_reset_scratchpad",
            TassadarScratchpadWorkloadFamily::Arithmetic,
            arithmetic_cases(),
            TassadarScratchpadPositionFramework::new(
                "tassadar.scratchpad.arithmetic.flat.absolute.v0",
                TassadarScratchpadWorkloadFamily::Arithmetic,
                TassadarScratchpadFormatConfig::new(
                    TassadarScratchpadEncoding::FlatTrace,
                    TassadarControlledPositionScheme::AbsoluteMonotonic,
                    4,
                ),
                "bounded learned sequence-format comparison only",
            ),
            TassadarScratchpadPositionFramework::new(
                "tassadar.scratchpad.arithmetic.segment_reset.v0",
                TassadarScratchpadWorkloadFamily::Arithmetic,
                TassadarScratchpadFormatConfig::new(
                    TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                    TassadarControlledPositionScheme::SegmentReset,
                    4,
                ),
                "bounded learned sequence-format comparison only",
            ),
        ),
        build_variant_report(
            "algorithmic_trace_schema_scratchpad",
            TassadarScratchpadWorkloadFamily::Algorithmic,
            algorithmic_cases(),
            TassadarScratchpadPositionFramework::new(
                "tassadar.scratchpad.algorithmic.flat.absolute.v0",
                TassadarScratchpadWorkloadFamily::Algorithmic,
                TassadarScratchpadFormatConfig::new(
                    TassadarScratchpadEncoding::FlatTrace,
                    TassadarControlledPositionScheme::AbsoluteMonotonic,
                    4,
                ),
                "bounded learned sequence-format comparison only",
            ),
            TassadarScratchpadPositionFramework::new(
                "tassadar.scratchpad.algorithmic.trace_schema.v0",
                TassadarScratchpadWorkloadFamily::Algorithmic,
                TassadarScratchpadFormatConfig::new(
                    TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                    TassadarControlledPositionScheme::TraceSchemaBuckets,
                    4,
                ),
                "bounded learned sequence-format comparison only",
            ),
        ),
    ];

    let arithmetic_baseline_max = variants
        .iter()
        .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Arithmetic)
        .expect("arithmetic variant should exist")
        .case_reports
        .iter()
        .map(|case| case.baseline_max_output_local_position_index)
        .max()
        .unwrap_or(0);
    let arithmetic_candidate_max = variants
        .iter()
        .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Arithmetic)
        .expect("arithmetic variant should exist")
        .max_candidate_output_local_position_index;
    let algorithmic_baseline_max = variants
        .iter()
        .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Algorithmic)
        .expect("algorithmic variant should exist")
        .case_reports
        .iter()
        .map(|case| case.baseline_max_output_local_position_index)
        .max()
        .unwrap_or(0);
    let algorithmic_candidate_max = variants
        .iter()
        .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Algorithmic)
        .expect("algorithmic variant should exist")
        .max_candidate_output_local_position_index;

    let mut report = TassadarScratchpadFrameworkComparisonReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_ref: String::from(TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF),
        regeneration_commands: vec![
            String::from(
                "cargo run -p psionic-train --example tassadar_scratchpad_framework_comparison",
            ),
            String::from(
                "cargo test -p psionic-train scratchpad_framework_report_matches_committed_truth -- --nocapture",
            ),
        ],
        variants,
        claim_class: String::from("learned_bounded_success"),
        claim_boundary: String::from(
            "this report only compares bounded scratchpad formatting and controlled position-ID schemes on seeded arithmetic symbolic traces and seeded algorithmic target sequences; it preserves final output tokens but does not claim trained exactness or served-lane promotion",
        ),
        summary: format!(
            "Public scratchpad/position framework report now freezes arithmetic and algorithmic sequence comparisons: the arithmetic segment-reset variant cuts max output local position from {} to {}, and the algorithmic trace-schema variant cuts it from {} to {}; both preserve final output tokens exactly while surfacing explicit scratchpad overhead and reset counts.",
            arithmetic_baseline_max,
            arithmetic_candidate_max,
            algorithmic_baseline_max,
            algorithmic_candidate_max,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_scratchpad_framework_comparison_report|",
        &report,
    );
    report
}

pub fn run_tassadar_scratchpad_framework_comparison_report(
    output_dir: &Path,
) -> Result<TassadarScratchpadFrameworkComparisonReport, TassadarScratchpadFrameworkComparisonError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarScratchpadFrameworkComparisonError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_scratchpad_framework_comparison_report();
    let report_path = output_dir.join(TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarScratchpadFrameworkComparisonError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_variant_report(
    variant_id: &str,
    workload_family: TassadarScratchpadWorkloadFamily,
    cases: Vec<(String, Vec<String>, Vec<String>)>,
    baseline_framework: TassadarScratchpadPositionFramework,
    candidate_framework: TassadarScratchpadPositionFramework,
) -> TassadarScratchpadFrameworkVariantReport {
    let mut case_reports = Vec::new();
    let mut position_reset_count_total = 0_u32;
    for (case_id, prompt_tokens, target_tokens) in cases {
        let (_, baseline_evidence) = inspect_tassadar_scratchpad_framework(
            &prompt_tokens,
            &target_tokens,
            &baseline_framework,
        );
        let (_, candidate_evidence) = inspect_tassadar_scratchpad_framework(
            &prompt_tokens,
            &target_tokens,
            &candidate_framework,
        );
        position_reset_count_total =
            position_reset_count_total.saturating_add(candidate_evidence.position_reset_count);
        case_reports.push(TassadarScratchpadFrameworkCaseComparison {
            case_id,
            prompt_token_count: prompt_tokens.len() as u32,
            target_token_count: target_tokens.len() as u32,
            baseline_token_count: baseline_evidence.token_count,
            candidate_token_count: candidate_evidence.token_count,
            baseline_max_output_local_position_index: baseline_evidence
                .max_output_local_position_index,
            candidate_max_output_local_position_index: candidate_evidence
                .max_output_local_position_index,
            locality_gain_bps: locality_gain_bps(
                baseline_evidence.max_output_local_position_index,
                candidate_evidence.max_output_local_position_index,
            ),
            scratchpad_overhead_bps: candidate_evidence.scratchpad_overhead_bps,
            final_output_tokens_preserved: candidate_evidence.final_output_tokens_preserved,
        });
    }

    let case_count = case_reports.len() as u32;
    let mean_locality_gain_bps = if case_reports.is_empty() {
        0
    } else {
        case_reports
            .iter()
            .map(|case| u64::from(case.locality_gain_bps))
            .sum::<u64>() as u32
            / case_reports.len() as u32
    };
    TassadarScratchpadFrameworkVariantReport {
        variant_id: String::from(variant_id),
        workload_family,
        baseline_framework_id: baseline_framework.framework_id.clone(),
        candidate_framework_id: candidate_framework.framework_id.clone(),
        baseline_framework_digest: baseline_framework.framework_digest.clone(),
        candidate_framework_digest: candidate_framework.framework_digest.clone(),
        case_count,
        max_candidate_output_local_position_index: case_reports
            .iter()
            .map(|case| case.candidate_max_output_local_position_index)
            .max()
            .unwrap_or(0),
        max_scratchpad_overhead_bps: case_reports
            .iter()
            .map(|case| case.scratchpad_overhead_bps)
            .max()
            .unwrap_or(0),
        output_exact_case_count: case_reports
            .iter()
            .filter(|case| case.final_output_tokens_preserved)
            .count() as u32,
        position_reset_count_total,
        locality_note: format!(
            "framework={} mean_locality_gain_bps={} max_candidate_output_local_position_index={} position_resets={}",
            candidate_framework.framework_id,
            mean_locality_gain_bps,
            case_reports
                .iter()
                .map(|case| case.candidate_max_output_local_position_index)
                .max()
                .unwrap_or(0),
            position_reset_count_total,
        ),
        case_reports,
        mean_locality_gain_bps,
    }
}

fn arithmetic_cases() -> Vec<(String, Vec<String>, Vec<String>)> {
    tassadar_symbolic_program_examples()
        .into_iter()
        .map(|example| {
            (
                example.case_id,
                vec![
                    String::from("<program>"),
                    example.program.program_id.clone(),
                ],
                symbolic_target_tokens(example.program.statements.as_slice()),
            )
        })
        .collect()
}

fn algorithmic_cases() -> Vec<(String, Vec<String>, Vec<String>)> {
    tassadar_executor_no_hint_corpus()
        .into_iter()
        .map(|example| {
            let prompt_tokens = example
                .summary
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>();
            (example.example_id, prompt_tokens, example.full_hint_targets)
        })
        .collect()
}

fn symbolic_target_tokens(statements: &[TassadarSymbolicStatement]) -> Vec<String> {
    statements
        .iter()
        .flat_map(|statement| match statement {
            TassadarSymbolicStatement::Let { name, expr } => {
                let mut tokens = vec![String::from("let"), name.clone()];
                match expr {
                    TassadarSymbolicExpr::Operand { .. } => {
                        tokens.push(String::from("operand"));
                    }
                    TassadarSymbolicExpr::Binary { op, .. } => {
                        tokens.push(op.as_str().to_string());
                    }
                }
                tokens
            }
            TassadarSymbolicStatement::Store { slot, .. } => {
                vec![String::from("store"), format!("slot_{slot}")]
            }
            TassadarSymbolicStatement::Output { .. } => vec![String::from("output")],
        })
        .collect()
}

fn locality_gain_bps(baseline: u32, candidate: u32) -> u32 {
    if baseline == 0 || candidate >= baseline {
        0
    } else {
        ((u64::from(baseline - candidate) * 10_000) / u64::from(baseline)) as u32
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("scratchpad framework comparison report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_scratchpad_framework_comparison_report,
        run_tassadar_scratchpad_framework_comparison_report,
        TassadarScratchpadFrameworkComparisonReport, TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF,
    };
    use psionic_models::TassadarScratchpadWorkloadFamily;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn scratchpad_framework_reduces_local_span_for_arithmetic_and_algorithmic_cases(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_scratchpad_framework_comparison_report();
        assert_eq!(report.variants.len(), 2);
        assert!(report.variants.iter().all(|variant| {
            variant
                .case_reports
                .iter()
                .all(|case| case.final_output_tokens_preserved)
        }));
        let arithmetic = report
            .variants
            .iter()
            .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Arithmetic)
            .expect("arithmetic variant");
        let algorithmic = report
            .variants
            .iter()
            .find(|variant| {
                variant.workload_family == TassadarScratchpadWorkloadFamily::Algorithmic
            })
            .expect("algorithmic variant");
        assert!(arithmetic.mean_locality_gain_bps > 0);
        assert!(algorithmic.mean_locality_gain_bps > 0);
        Ok(())
    }

    #[test]
    fn scratchpad_framework_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_scratchpad_framework_comparison_report();
        let persisted: TassadarScratchpadFrameworkComparisonReport =
            read_repo_json(TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn scratchpad_framework_report_writes_current_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let output_dir = tempdir()?;
        let report = run_tassadar_scratchpad_framework_comparison_report(output_dir.path())?;
        let persisted: TassadarScratchpadFrameworkComparisonReport =
            serde_json::from_slice(&std::fs::read(
                output_dir
                    .path()
                    .join("tassadar_scratchpad_framework_comparison_report.json"),
            )?)?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
