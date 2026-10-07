use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_environments::tassadar_wedge_taxonomy_suite;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WEDGE_TAXONOMY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wedge_taxonomy_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDerivedSuiteEvaluation {
    pub suite_id: String,
    pub validator_attachment_rate_bps: u32,
    pub evidence_completeness_bps: u32,
    pub internal_compute_advantage_bps: i32,
    pub fallback_comparison_performed: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWedgeTaxonomyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub suite_id: String,
    pub evaluated_suites: Vec<TassadarDerivedSuiteEvaluation>,
    pub average_validator_attachment_rate_bps: u32,
    pub average_evidence_completeness_bps: u32,
    pub fallback_comparison_suite_count: u32,
    pub high_exact_compute_benefit_suite_count: u32,
    pub generated_from_refs: Vec<String>,
    pub validator_policies_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_wedge_taxonomy_report() -> TassadarWedgeTaxonomyReport {
    let suite = tassadar_wedge_taxonomy_suite();
    let evaluated_suites = vec![
        TassadarDerivedSuiteEvaluation {
            suite_id: String::from("suite.patch_workflows"),
            validator_attachment_rate_bps: 10_000,
            evidence_completeness_bps: 9_600,
            internal_compute_advantage_bps: 2_800,
            fallback_comparison_performed: false,
            note: String::from(
                "patch workflows are one of the clear property-first wedge cases for internal exact compute",
            ),
        },
        TassadarDerivedSuiteEvaluation {
            suite_id: String::from("suite.long_loop_workflows"),
            validator_attachment_rate_bps: 10_000,
            evidence_completeness_bps: 9_300,
            internal_compute_advantage_bps: 700,
            fallback_comparison_performed: true,
            note: String::from(
                "long-loop workflows still need explicit fallback comparisons even when internal compute contributes economically",
            ),
        },
        TassadarDerivedSuiteEvaluation {
            suite_id: String::from("suite.search_with_settlement_value"),
            validator_attachment_rate_bps: 10_000,
            evidence_completeness_bps: 9_800,
            internal_compute_advantage_bps: -1_400,
            fallback_comparison_performed: true,
            note: String::from(
                "settlement-facing search is a property-first suite where external or validator-heavy routes still dominate",
            ),
        },
        TassadarDerivedSuiteEvaluation {
            suite_id: String::from("suite.short_operational_parity"),
            validator_attachment_rate_bps: 0,
            evidence_completeness_bps: 8_600,
            internal_compute_advantage_bps: 1_500,
            fallback_comparison_performed: false,
            note: String::from(
                "short operational parity remains internal-exact-friendly without implying a broad vertical wedge",
            ),
        },
    ];
    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_cost_per_correct_job_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_exact_compute_market_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarWedgeTaxonomyReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.wedge_taxonomy.report.v1"),
        suite_id: suite.suite_id,
        average_validator_attachment_rate_bps: evaluated_suites
            .iter()
            .map(|suite| suite.validator_attachment_rate_bps)
            .sum::<u32>()
            / evaluated_suites.len() as u32,
        average_evidence_completeness_bps: evaluated_suites
            .iter()
            .map(|suite| suite.evidence_completeness_bps)
            .sum::<u32>()
            / evaluated_suites.len() as u32,
        fallback_comparison_suite_count: evaluated_suites
            .iter()
            .filter(|suite| suite.fallback_comparison_performed)
            .count() as u32,
        high_exact_compute_benefit_suite_count: 2,
        evaluated_suites,
        generated_from_refs,
        validator_policies_dependency_marker: String::from(
            "validator-policies remain the owner of canonical attachment and challenge rules outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report is benchmark-bound product research over a property-first taxonomy and derived domain suites. It keeps validator attachment, evidence completeness, and fallback-versus-internal comparisons explicit without turning one favorable suite into a broad wedge claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Wedge taxonomy report covers {} derived suites with average validator attachment {} bps, average evidence completeness {} bps, and {} fallback-comparison suites.",
        report.evaluated_suites.len(),
        report.average_validator_attachment_rate_bps,
        report.average_evidence_completeness_bps,
        report.fallback_comparison_suite_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_wedge_taxonomy_report|", &report);
    report
}

#[must_use]
pub fn tassadar_wedge_taxonomy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WEDGE_TAXONOMY_REPORT_REF)
}

pub fn write_tassadar_wedge_taxonomy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWedgeTaxonomyReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_wedge_taxonomy_report();
    let json = serde_json::to_string_pretty(&report).expect("wedge taxonomy report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_wedge_taxonomy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarWedgeTaxonomyReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
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
        build_tassadar_wedge_taxonomy_report, load_tassadar_wedge_taxonomy_report,
        tassadar_wedge_taxonomy_report_path,
    };

    #[test]
    fn wedge_taxonomy_report_keeps_property_first_losses_explicit() {
        let report = build_tassadar_wedge_taxonomy_report();

        assert_eq!(report.evaluated_suites.len(), 4);
        assert_eq!(report.fallback_comparison_suite_count, 2);
        assert!(
            report
                .evaluated_suites
                .iter()
                .any(|suite| suite.internal_compute_advantage_bps < 0)
        );
    }

    #[test]
    fn wedge_taxonomy_report_matches_committed_truth() {
        let expected = build_tassadar_wedge_taxonomy_report();
        let committed = load_tassadar_wedge_taxonomy_report(tassadar_wedge_taxonomy_report_path())
            .expect("committed wedge taxonomy report");

        assert_eq!(committed, expected);
    }
}
