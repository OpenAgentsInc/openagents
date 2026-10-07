//! Signature-routing fixtures for public/synthetic legal benchmark failures.
//!
//! Psionic owns the benchmark evidence and the legal failure taxonomy. Probe
//! owns runtime signature selection. This module defines the Psionic-side
//! handoff fixture: structured legal failure families select specific Probe
//! signature ids without exposing hidden Harvey labels or scoring rubrics.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{stable_json_digest, Metadata};

/// Current schema version for legal benchmark signature-routing fixtures.
pub const LEGAL_BENCHMARK_SIGNATURE_ROUTING_SCHEMA_VERSION: u16 = 1;

/// Probe signature id for legal deliverable file discipline.
pub const LEGAL_DELIVERABLE_FILE_WORKFLOW_SIGNATURE_ID: &str = "legal.deliverable_file_workflow";
/// Probe signature id for required output path contracts.
pub const LEGAL_OUTPUT_PATH_CONTRACT_SIGNATURE_ID: &str = "legal.output_path_contract";
/// Probe signature id for source-grounded legal answers.
pub const LEGAL_SOURCE_GROUNDING_TRACE_SIGNATURE_ID: &str = "legal.source_grounding_trace";
/// Probe signature id for citation/source provenance checks.
pub const LEGAL_CITATION_PROVENANCE_CHECK_SIGNATURE_ID: &str = "legal.citation_provenance_check";
/// Probe signature id for answer actor/hash integrity.
pub const LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID: &str = "legal.answer_integrity_guard";
/// Probe signature id for benchmark judge/scorer supervision.
pub const BENCHMARK_LEGAL_JUDGE_SUPERVISOR_SIGNATURE_ID: &str = "benchmark.legal_judge_supervisor";

/// Public/synthetic legal failure family used as structured selector input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkSignatureFailureFamily {
    /// The agent did not produce the required legal deliverable file.
    MissingDeliverable,
    /// The agent wrote a plausible answer to the wrong path.
    WrongOutputPath,
    /// The answer did not ground material assertions in supplied sources.
    SourceGroundingMissing,
    /// The answer cited or referenced sources without retained provenance.
    CitationProvenanceMissing,
    /// The answer file was created or changed outside model-authored writes.
    AnswerIntegrityInvalid,
    /// Judge/scorer outputs need supervisor review or preservation.
    JudgeSupervisorNeeded,
}

/// Minimal task envelope Probe receives before starting a Codex-backed legal run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureTaskEnvelope {
    /// Dataset or benchmark family slug.
    pub dataset_slug: String,
    /// Dataset or benchmark version.
    pub dataset_version: String,
    /// Stable task id.
    pub task_id: String,
    /// Public/synthetic failure family.
    pub failure_family: LegalBenchmarkSignatureFailureFamily,
    /// Agent-visible instruction summary.
    pub visible_instruction_summary: String,
    /// Required answer path for output-path checks.
    pub required_answer_path: String,
    /// Source document ids visible to the agent.
    pub source_document_ids: Vec<String>,
    /// Allowed tool names for this fixture.
    pub allowed_tools: Vec<String>,
    /// Data class label.
    pub data_classification: String,
    /// Whether hidden labels or private criteria are exposed to the agent.
    pub hidden_criteria_visible_to_agent: bool,
}

/// Deterministic fixture for one raw-Codex or Probe+Codex legal run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureAgentRunFixture {
    /// Agent slug, for example `raw_codex` or `probe_codex_signatures`.
    pub agent_slug: String,
    /// Scenario status for the fixture.
    pub status: LegalBenchmarkSignatureAgentRunStatus,
    /// Selected Probe signature ids.
    pub selected_signature_ids: Vec<String>,
    /// Legal score in basis points for the deterministic fixture.
    pub score_bps: u32,
    /// Submitted answer path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_answer_path: Option<String>,
    /// SHA-256 digest for the answer text, if retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_sha256: Option<String>,
    /// Retained evidence references for human review.
    pub evidence_refs: Vec<String>,
    /// Failure taxonomy labels.
    pub failure_taxonomy: Vec<String>,
    /// Human-readable notes.
    pub notes: String,
}

/// Status for one deterministic legal signature-routing agent fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkSignatureAgentRunStatus {
    /// The fixture reaches a passing submitted answer.
    Passed,
    /// The fixture fails in a classified way.
    Failed,
}

/// One public/synthetic legal signature-routing fixture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureRoutingFixture {
    /// Stable fixture id.
    pub fixture_id: String,
    /// Structured task envelope.
    pub envelope: LegalBenchmarkSignatureTaskEnvelope,
    /// Signature ids expected from the selector.
    pub expected_signature_ids: Vec<String>,
    /// Raw-Codex baseline fixture.
    pub raw_codex: LegalBenchmarkSignatureAgentRunFixture,
    /// Probe+Codex signature-routed fixture.
    pub probe_codex: LegalBenchmarkSignatureAgentRunFixture,
    /// Required evidence classes for promotion or human review.
    pub required_evidence: Vec<String>,
    /// Extra metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// A suite of public/synthetic legal signature-routing fixtures.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureRoutingSuite {
    /// Schema version.
    pub schema_version: u16,
    /// Stable suite id.
    pub suite_id: String,
    /// Boundary statement.
    pub boundary: String,
    /// Public/synthetic-only flag.
    pub public_synthetic_only: bool,
    /// Fixture rows.
    pub fixtures: Vec<LegalBenchmarkSignatureRoutingFixture>,
    /// Extra metadata.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Per-fixture report row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureRoutingReportRow {
    /// Fixture id.
    pub fixture_id: String,
    /// Task id.
    pub task_id: String,
    /// Failure family.
    pub failure_family: LegalBenchmarkSignatureFailureFamily,
    /// Expected signature ids.
    pub expected_signature_ids: Vec<String>,
    /// Actual signature ids selected from the structured failure family.
    pub selected_signature_ids: Vec<String>,
    /// Whether selected ids exactly matched expected ids.
    pub selection_passed: bool,
    /// Raw-Codex score in basis points.
    pub raw_codex_score_bps: u32,
    /// Probe+Codex score in basis points.
    pub probe_codex_score_bps: u32,
    /// Probe+Codex score delta versus raw Codex.
    pub score_delta_bps: i32,
    /// Raw-Codex failure labels.
    pub raw_codex_failures: Vec<String>,
    /// Probe+Codex retained evidence references.
    pub probe_codex_evidence_refs: Vec<String>,
}

/// Summary over a legal signature-routing fixture suite.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureRoutingSummary {
    /// Number of fixtures.
    pub fixture_count: u64,
    /// Number whose selected ids matched expected ids.
    pub selection_pass_count: u64,
    /// Selection pass rate in basis points.
    pub selection_pass_rate_bps: u32,
    /// Mean raw-Codex score in basis points.
    pub raw_codex_mean_score_bps: u32,
    /// Mean Probe+Codex score in basis points.
    pub probe_codex_mean_score_bps: u32,
    /// Mean Probe+Codex delta in basis points.
    pub mean_score_delta_bps: i32,
    /// Failure-family counts.
    pub failure_family_counts: BTreeMap<LegalBenchmarkSignatureFailureFamily, u64>,
}

/// Deterministic report for the public/synthetic legal signature-routing suite.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSignatureRoutingReport {
    /// Schema version.
    pub schema_version: u16,
    /// Suite id.
    pub suite_id: String,
    /// Suite digest.
    pub suite_hash: String,
    /// Report digest.
    pub report_hash: String,
    /// Boundary statement.
    pub boundary: String,
    /// Summary.
    pub summary: LegalBenchmarkSignatureRoutingSummary,
    /// Per-fixture rows.
    pub rows: Vec<LegalBenchmarkSignatureRoutingReportRow>,
}

/// Validation or I/O error for legal signature-routing fixtures.
#[derive(Debug, Error)]
pub enum LegalBenchmarkSignatureRoutingError {
    /// File-system operation failed.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
    /// JSON parsing or digesting failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Suite validation failed.
    #[error("invalid legal signature-routing suite: {0}")]
    InvalidSuite(String),
}

/// Selects legal Probe signature ids from a structured legal failure family.
#[must_use]
pub fn select_legal_benchmark_signature_ids(
    failure_family: LegalBenchmarkSignatureFailureFamily,
) -> Vec<&'static str> {
    match failure_family {
        LegalBenchmarkSignatureFailureFamily::MissingDeliverable => vec![
            LEGAL_DELIVERABLE_FILE_WORKFLOW_SIGNATURE_ID,
            LEGAL_OUTPUT_PATH_CONTRACT_SIGNATURE_ID,
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
        ],
        LegalBenchmarkSignatureFailureFamily::WrongOutputPath => vec![
            LEGAL_OUTPUT_PATH_CONTRACT_SIGNATURE_ID,
            LEGAL_DELIVERABLE_FILE_WORKFLOW_SIGNATURE_ID,
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
        ],
        LegalBenchmarkSignatureFailureFamily::SourceGroundingMissing => vec![
            LEGAL_SOURCE_GROUNDING_TRACE_SIGNATURE_ID,
            LEGAL_CITATION_PROVENANCE_CHECK_SIGNATURE_ID,
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
        ],
        LegalBenchmarkSignatureFailureFamily::CitationProvenanceMissing => vec![
            LEGAL_CITATION_PROVENANCE_CHECK_SIGNATURE_ID,
            LEGAL_SOURCE_GROUNDING_TRACE_SIGNATURE_ID,
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
        ],
        LegalBenchmarkSignatureFailureFamily::AnswerIntegrityInvalid => vec![
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
            LEGAL_DELIVERABLE_FILE_WORKFLOW_SIGNATURE_ID,
            LEGAL_OUTPUT_PATH_CONTRACT_SIGNATURE_ID,
        ],
        LegalBenchmarkSignatureFailureFamily::JudgeSupervisorNeeded => vec![
            BENCHMARK_LEGAL_JUDGE_SUPERVISOR_SIGNATURE_ID,
            LEGAL_ANSWER_INTEGRITY_GUARD_SIGNATURE_ID,
            LEGAL_SOURCE_GROUNDING_TRACE_SIGNATURE_ID,
        ],
    }
}

/// Loads a legal signature-routing suite from JSON.
pub fn load_legal_benchmark_signature_routing_suite(
    path: impl AsRef<Path>,
) -> Result<LegalBenchmarkSignatureRoutingSuite, LegalBenchmarkSignatureRoutingError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| LegalBenchmarkSignatureRoutingError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let suite = serde_json::from_slice::<LegalBenchmarkSignatureRoutingSuite>(&bytes)?;
    validate_legal_benchmark_signature_routing_suite(&suite)?;
    Ok(suite)
}

/// Validates that a legal signature-routing suite is public/synthetic and typed.
pub fn validate_legal_benchmark_signature_routing_suite(
    suite: &LegalBenchmarkSignatureRoutingSuite,
) -> Result<(), LegalBenchmarkSignatureRoutingError> {
    if suite.schema_version != LEGAL_BENCHMARK_SIGNATURE_ROUTING_SCHEMA_VERSION {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "schema_version must be {LEGAL_BENCHMARK_SIGNATURE_ROUTING_SCHEMA_VERSION}"
        )));
    }
    if suite.suite_id.is_empty() {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(
            String::from("suite_id must not be empty"),
        ));
    }
    if !suite.public_synthetic_only {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(
            String::from("suite must be public_synthetic_only"),
        ));
    }
    let mut fixture_ids = BTreeSet::new();
    let mut families = BTreeSet::new();
    for fixture in &suite.fixtures {
        validate_fixture(fixture)?;
        if !fixture_ids.insert(fixture.fixture_id.as_str()) {
            return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
                "duplicate fixture id {}",
                fixture.fixture_id
            )));
        }
        families.insert(fixture.envelope.failure_family);
    }
    for family in [
        LegalBenchmarkSignatureFailureFamily::MissingDeliverable,
        LegalBenchmarkSignatureFailureFamily::WrongOutputPath,
        LegalBenchmarkSignatureFailureFamily::SourceGroundingMissing,
        LegalBenchmarkSignatureFailureFamily::CitationProvenanceMissing,
        LegalBenchmarkSignatureFailureFamily::AnswerIntegrityInvalid,
        LegalBenchmarkSignatureFailureFamily::JudgeSupervisorNeeded,
    ] {
        if !families.contains(&family) {
            return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
                "missing failure family {family:?}"
            )));
        }
    }
    Ok(())
}

/// Builds the deterministic raw-Codex versus Probe+Codex routing report.
pub fn build_legal_benchmark_signature_routing_report(
    suite: &LegalBenchmarkSignatureRoutingSuite,
) -> Result<LegalBenchmarkSignatureRoutingReport, LegalBenchmarkSignatureRoutingError> {
    validate_legal_benchmark_signature_routing_suite(suite)?;
    let suite_hash =
        stable_json_digest("psionic.legal_benchmark.signature_routing.suite.v1", suite)?;
    let rows = suite
        .fixtures
        .iter()
        .map(|fixture| {
            let selected_signature_ids =
                select_legal_benchmark_signature_ids(fixture.envelope.failure_family)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
            let selection_passed = selected_signature_ids == fixture.expected_signature_ids;
            let score_delta_bps = i32::try_from(fixture.probe_codex.score_bps).unwrap_or(i32::MAX)
                - i32::try_from(fixture.raw_codex.score_bps).unwrap_or(i32::MAX);
            LegalBenchmarkSignatureRoutingReportRow {
                fixture_id: fixture.fixture_id.clone(),
                task_id: fixture.envelope.task_id.clone(),
                failure_family: fixture.envelope.failure_family,
                expected_signature_ids: fixture.expected_signature_ids.clone(),
                selected_signature_ids,
                selection_passed,
                raw_codex_score_bps: fixture.raw_codex.score_bps,
                probe_codex_score_bps: fixture.probe_codex.score_bps,
                score_delta_bps,
                raw_codex_failures: fixture.raw_codex.failure_taxonomy.clone(),
                probe_codex_evidence_refs: fixture.probe_codex.evidence_refs.clone(),
            }
        })
        .collect::<Vec<_>>();
    let summary = routing_summary(&rows);
    let mut report = LegalBenchmarkSignatureRoutingReport {
        schema_version: LEGAL_BENCHMARK_SIGNATURE_ROUTING_SCHEMA_VERSION,
        suite_id: suite.suite_id.clone(),
        suite_hash,
        report_hash: String::new(),
        boundary: suite.boundary.clone(),
        summary,
        rows,
    };
    report.report_hash = stable_json_digest(
        "psionic.legal_benchmark.signature_routing.report.v1",
        &report,
    )?;
    Ok(report)
}

/// Writes a legal signature-routing report as pretty JSON.
pub fn write_legal_benchmark_signature_routing_report(
    path: impl AsRef<Path>,
    report: &LegalBenchmarkSignatureRoutingReport,
) -> Result<(), LegalBenchmarkSignatureRoutingError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalBenchmarkSignatureRoutingError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(report)?;
    fs::write(path, bytes).map_err(|source| LegalBenchmarkSignatureRoutingError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_fixture(
    fixture: &LegalBenchmarkSignatureRoutingFixture,
) -> Result<(), LegalBenchmarkSignatureRoutingError> {
    if fixture.fixture_id.is_empty() {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(
            String::from("fixture_id must not be empty"),
        ));
    }
    if fixture.envelope.hidden_criteria_visible_to_agent {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} exposes hidden criteria",
            fixture.fixture_id
        )));
    }
    if fixture.envelope.required_answer_path.is_empty() {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} missing required_answer_path",
            fixture.fixture_id
        )));
    }
    let selected = select_legal_benchmark_signature_ids(fixture.envelope.failure_family)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if selected != fixture.expected_signature_ids {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} expected signatures do not match structured family",
            fixture.fixture_id
        )));
    }
    if !fixture.raw_codex.selected_signature_ids.is_empty() {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} raw Codex baseline must not select Probe signatures",
            fixture.fixture_id
        )));
    }
    if fixture.probe_codex.selected_signature_ids != fixture.expected_signature_ids {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} Probe+Codex selections do not match expected signatures",
            fixture.fixture_id
        )));
    }
    if fixture.probe_codex.status != LegalBenchmarkSignatureAgentRunStatus::Passed {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} Probe+Codex fixture must pass",
            fixture.fixture_id
        )));
    }
    if fixture.probe_codex.answer_sha256.is_none()
        || fixture.probe_codex.submitted_answer_path.is_none()
    {
        return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
            "fixture {} Probe+Codex fixture must retain answer path and hash",
            fixture.fixture_id
        )));
    }
    for required in &fixture.required_evidence {
        if !fixture
            .probe_codex
            .evidence_refs
            .iter()
            .any(|evidence| evidence == required)
        {
            return Err(LegalBenchmarkSignatureRoutingError::InvalidSuite(format!(
                "fixture {} missing Probe+Codex evidence {}",
                fixture.fixture_id, required
            )));
        }
    }
    Ok(())
}

fn routing_summary(
    rows: &[LegalBenchmarkSignatureRoutingReportRow],
) -> LegalBenchmarkSignatureRoutingSummary {
    let fixture_count = u64::try_from(rows.len()).unwrap_or(0);
    let selection_pass_count = rows.iter().filter(|row| row.selection_passed).count();
    let raw_score_sum = rows
        .iter()
        .map(|row| u64::from(row.raw_codex_score_bps))
        .sum::<u64>();
    let probe_score_sum = rows
        .iter()
        .map(|row| u64::from(row.probe_codex_score_bps))
        .sum::<u64>();
    let delta_sum = rows
        .iter()
        .map(|row| i64::from(row.score_delta_bps))
        .sum::<i64>();
    let mut failure_family_counts = BTreeMap::new();
    for row in rows {
        *failure_family_counts.entry(row.failure_family).or_insert(0) += 1;
    }
    LegalBenchmarkSignatureRoutingSummary {
        fixture_count,
        selection_pass_count: u64::try_from(selection_pass_count).unwrap_or(u64::MAX),
        selection_pass_rate_bps: ratio_bps(
            u64::try_from(selection_pass_count).unwrap_or(0),
            fixture_count,
        ),
        raw_codex_mean_score_bps: average_bps(raw_score_sum, fixture_count),
        probe_codex_mean_score_bps: average_bps(probe_score_sum, fixture_count),
        mean_score_delta_bps: if fixture_count == 0 {
            0
        } else {
            i32::try_from(delta_sum / i64::try_from(fixture_count).unwrap_or(1)).unwrap_or(i32::MAX)
        },
        failure_family_counts,
    }
}

fn ratio_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from(numerator.saturating_mul(10_000) / denominator).unwrap_or(u32::MAX)
}

fn average_bps(sum: u64, count: u64) -> u32 {
    if count == 0 {
        return 0;
    }
    u32::try_from(sum / count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_suite() -> LegalBenchmarkSignatureRoutingSuite {
        serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/signature_routing/harvey_public_synthetic_signature_routing_suite.json"
        ))
        .expect("suite parses")
    }

    #[test]
    fn legal_failure_families_select_expected_signatures() {
        let suite = fixture_suite();
        validate_legal_benchmark_signature_routing_suite(&suite).expect("valid suite");

        for fixture in &suite.fixtures {
            let selected = select_legal_benchmark_signature_ids(fixture.envelope.failure_family)
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            assert_eq!(selected, fixture.expected_signature_ids, "{fixture:?}");
        }
    }

    #[test]
    fn wrong_path_and_missing_deliverable_fixtures_fail_clearly() {
        let suite = fixture_suite();
        let missing = suite
            .fixtures
            .iter()
            .find(|fixture| {
                fixture.envelope.failure_family
                    == LegalBenchmarkSignatureFailureFamily::MissingDeliverable
            })
            .expect("missing-deliverable fixture");
        let wrong_path = suite
            .fixtures
            .iter()
            .find(|fixture| {
                fixture.envelope.failure_family
                    == LegalBenchmarkSignatureFailureFamily::WrongOutputPath
            })
            .expect("wrong-output-path fixture");

        assert_eq!(
            missing.raw_codex.status,
            LegalBenchmarkSignatureAgentRunStatus::Failed
        );
        assert!(missing
            .raw_codex
            .failure_taxonomy
            .contains(&String::from("missing_deliverable")));
        assert_eq!(
            wrong_path.raw_codex.status,
            LegalBenchmarkSignatureAgentRunStatus::Failed
        );
        assert!(wrong_path
            .raw_codex
            .failure_taxonomy
            .contains(&String::from("wrong_output_path")));
    }

    #[test]
    fn probe_codex_preserves_review_evidence() {
        let suite = fixture_suite();
        for fixture in &suite.fixtures {
            assert_eq!(
                fixture.probe_codex.status,
                LegalBenchmarkSignatureAgentRunStatus::Passed
            );
            assert_eq!(
                fixture.probe_codex.selected_signature_ids,
                fixture.expected_signature_ids
            );
            assert_eq!(fixture.probe_codex.score_bps, 10_000);
            assert_eq!(
                fixture.probe_codex.submitted_answer_path.as_deref(),
                Some(fixture.envelope.required_answer_path.as_str())
            );
            assert!(fixture.probe_codex.answer_sha256.is_some());
            for evidence in &fixture.required_evidence {
                assert!(
                    fixture.probe_codex.evidence_refs.contains(evidence),
                    "missing evidence {evidence} for {}",
                    fixture.fixture_id
                );
            }
        }
    }

    #[test]
    fn routing_report_summarizes_raw_codex_probe_codex_delta() {
        let suite = fixture_suite();
        let report = build_legal_benchmark_signature_routing_report(&suite).expect("report");

        assert_eq!(report.summary.fixture_count, 6);
        assert_eq!(report.summary.selection_pass_rate_bps, 10_000);
        assert_eq!(report.summary.probe_codex_mean_score_bps, 10_000);
        assert!(
            report.summary.raw_codex_mean_score_bps < report.summary.probe_codex_mean_score_bps
        );
        assert!(report.summary.mean_score_delta_bps > 0);
        assert_eq!(report.rows.len(), 6);
        assert!(!report.report_hash.is_empty());
    }
}
