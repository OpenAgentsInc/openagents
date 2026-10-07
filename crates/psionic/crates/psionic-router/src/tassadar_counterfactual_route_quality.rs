use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_counterfactual_route_quality_report.json";

/// Route family compared in the counterfactual route-quality report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCounterfactualRouteFamily {
    LanguageOnly,
    CompiledExact,
    InternalExecutor,
    ExternalTool,
    HybridComposite,
}

/// One realized or counterfactual route option on a fixed task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteOption {
    pub route_family: TassadarCounterfactualRouteFamily,
    pub task_success_bps: u32,
    pub accepted_outcome_ready: bool,
    pub evidence_quality_bps: u32,
    pub cost_milliunits: u32,
    pub latency_ms: u32,
    pub note: String,
}

/// One same-task realized-versus-counterfactual routing case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteCase {
    pub case_id: String,
    pub workload_family: String,
    pub realized_route: TassadarCounterfactualRouteOption,
    pub counterfactual_routes: Vec<TassadarCounterfactualRouteOption>,
    pub best_available_route_family: TassadarCounterfactualRouteFamily,
    pub better_counterfactual_exists: bool,
    pub accepted_outcome_delta_bps: i32,
    pub success_delta_bps: i32,
    pub evidence_delta_bps: i32,
    pub cost_regret_milliunits: i32,
    pub latency_regret_ms: i32,
    pub note: String,
}

/// Router-owned report comparing realized routes against best available counterfactuals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteQualityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub evaluated_cases: Vec<TassadarCounterfactualRouteCase>,
    pub better_counterfactual_case_count: u32,
    pub accepted_outcome_regret_case_count: u32,
    pub overuse_case_count: u32,
    pub underuse_case_count: u32,
    pub average_cost_regret_milliunits: i32,
    pub average_latency_regret_ms: i32,
    pub generated_from_refs: Vec<String>,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the committed counterfactual route-quality report.
#[must_use]
pub fn build_tassadar_counterfactual_route_quality_report()
-> TassadarCounterfactualRouteQualityReport {
    let evaluated_cases = seeded_cases();
    let better_counterfactual_case_count = evaluated_cases
        .iter()
        .filter(|case| case.better_counterfactual_exists)
        .count() as u32;
    let accepted_outcome_regret_case_count = evaluated_cases
        .iter()
        .filter(|case| case.accepted_outcome_delta_bps > 0)
        .count() as u32;
    let overuse_case_count = evaluated_cases
        .iter()
        .filter(|case| is_overuse(case))
        .count() as u32;
    let underuse_case_count = evaluated_cases
        .iter()
        .filter(|case| is_underuse(case))
        .count() as u32;
    let case_count = evaluated_cases.len() as i32;
    let average_cost_regret_milliunits = evaluated_cases
        .iter()
        .map(|case| case.cost_regret_milliunits)
        .sum::<i32>()
        / case_count.max(1);
    let average_latency_regret_ms = evaluated_cases
        .iter()
        .map(|case| case.latency_regret_ms)
        .sum::<i32>()
        / case_count.max(1);
    let mut generated_from_refs = vec![
        String::from(
            "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
        ),
        String::from("fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_composite_routing_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_workload_capability_frontier_report.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarCounterfactualRouteQualityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.counterfactual_route_quality.report.v1"),
        evaluated_cases,
        better_counterfactual_case_count,
        accepted_outcome_regret_case_count,
        overuse_case_count,
        underuse_case_count,
        average_cost_regret_milliunits,
        average_latency_regret_ms,
        generated_from_refs,
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical accepted-outcome and settlement closure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this router report is a research-only analysis over realized and counterfactual lane choices on the same tasks. It keeps accepted-outcome posture, evidence quality, cost, and latency deltas explicit without treating realized-route success as authority closure or served capability widening",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Counterfactual route-quality report covers {} cases with {} better-counterfactual cases, {} overuse cases, and {} underuse cases.",
        report.evaluated_cases.len(),
        report.better_counterfactual_case_count,
        report.overuse_case_count,
        report.underuse_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_counterfactual_route_quality_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed counterfactual route-quality report.
#[must_use]
pub fn tassadar_counterfactual_route_quality_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_REPORT_REF)
}

/// Writes the committed counterfactual route-quality report.
pub fn write_tassadar_counterfactual_route_quality_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCounterfactualRouteQualityReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_counterfactual_route_quality_report();
    let json =
        serde_json::to_string_pretty(&report).expect("counterfactual route-quality serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_counterfactual_route_quality_report(
    path: impl AsRef<Path>,
) -> Result<TassadarCounterfactualRouteQualityReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn seeded_cases() -> Vec<TassadarCounterfactualRouteCase> {
    vec![
        case(
            "case.patch_apply_internal_underuse",
            "patch_apply_internal_exact",
            route(
                TassadarCounterfactualRouteFamily::LanguageOnly,
                7_600,
                false,
                4_100,
                500,
                30,
                "realized language-only route finishes the patch analysis but cannot satisfy accepted-outcome posture",
            ),
            vec![
                route(
                    TassadarCounterfactualRouteFamily::CompiledExact,
                    10_000,
                    true,
                    9_300,
                    2_100,
                    70,
                    "compiled exact route keeps bounded exactness explicit with lower cost than the full hybrid path",
                ),
                route(
                    TassadarCounterfactualRouteFamily::InternalExecutor,
                    10_000,
                    true,
                    9_100,
                    2_200,
                    60,
                    "internal executor stays benchmark-gated and accepted-outcome-ready on the seeded patch family",
                ),
                route(
                    TassadarCounterfactualRouteFamily::ExternalTool,
                    9_800,
                    false,
                    8_900,
                    7_000,
                    150,
                    "external delegation solves the task but overpays for evidence and still does not improve accepted-outcome readiness here",
                ),
                route(
                    TassadarCounterfactualRouteFamily::HybridComposite,
                    10_000,
                    true,
                    9_600,
                    3_800,
                    85,
                    "hybrid composite gives the strongest evidence posture on the seeded patch case",
                ),
            ],
            "the realized language-only route hides that an accepted-outcome-ready alternative existed on the same task",
        ),
        case(
            "case.long_loop_internal_overuse",
            "long_loop_validator_heavy",
            route(
                TassadarCounterfactualRouteFamily::InternalExecutor,
                7_200,
                false,
                5_800,
                3_600,
                95,
                "realized internal executor route underplays refusal pressure on long-loop validator-heavy work",
            ),
            vec![
                route(
                    TassadarCounterfactualRouteFamily::LanguageOnly,
                    5_100,
                    false,
                    3_500,
                    700,
                    20,
                    "language-only reasoning is cheap but too weak for the seeded long-loop case",
                ),
                route(
                    TassadarCounterfactualRouteFamily::CompiledExact,
                    7_600,
                    false,
                    6_300,
                    4_800,
                    140,
                    "compiled exactness is stronger than the realized route but still not accepted-outcome-ready here",
                ),
                route(
                    TassadarCounterfactualRouteFamily::ExternalTool,
                    9_800,
                    false,
                    9_200,
                    7_600,
                    220,
                    "external delegation is robust but expensive and not authority-ready on its own",
                ),
                route(
                    TassadarCounterfactualRouteFamily::HybridComposite,
                    9_900,
                    true,
                    9_400,
                    6_500,
                    190,
                    "hybrid routing keeps the internal fast path where honest and escalates into validator-compatible evidence when needed",
                ),
            ],
            "the realized internal route overuses the executor lane without acknowledging a better hybrid alternative",
        ),
        case(
            "case.article_hybrid_correct",
            "article_hybrid_workflow",
            route(
                TassadarCounterfactualRouteFamily::HybridComposite,
                9_700,
                true,
                9_300,
                4_100,
                90,
                "realized hybrid article route already balances evidence, cost, and latency well",
            ),
            vec![
                route(
                    TassadarCounterfactualRouteFamily::LanguageOnly,
                    6_200,
                    false,
                    3_900,
                    600,
                    25,
                    "language-only reasoning cannot close the workflow evidence gap for the article workload",
                ),
                route(
                    TassadarCounterfactualRouteFamily::CompiledExact,
                    9_100,
                    true,
                    8_900,
                    5_200,
                    120,
                    "compiled exactness improves authority posture but stays slower and more expensive than the realized hybrid path",
                ),
                route(
                    TassadarCounterfactualRouteFamily::InternalExecutor,
                    9_000,
                    false,
                    7_200,
                    3_000,
                    65,
                    "internal executor alone under-evidences the workflow closure even when it is cheaper",
                ),
                route(
                    TassadarCounterfactualRouteFamily::ExternalTool,
                    9_400,
                    false,
                    8_200,
                    6_800,
                    160,
                    "external tools stay slower and less evidence-complete than the realized hybrid route",
                ),
            ],
            "the realized hybrid route is already the best available option on this seeded workflow",
        ),
        case(
            "case.parity_hybrid_overuse",
            "parity_short_bounded",
            route(
                TassadarCounterfactualRouteFamily::HybridComposite,
                10_000,
                true,
                8_500,
                1_800,
                55,
                "realized hybrid route succeeds but adds extra lane overhead on a bounded parity workload",
            ),
            vec![
                route(
                    TassadarCounterfactualRouteFamily::LanguageOnly,
                    9_800,
                    false,
                    4_500,
                    400,
                    20,
                    "language-only reasoning works often enough but is weaker on exact bounded parity",
                ),
                route(
                    TassadarCounterfactualRouteFamily::CompiledExact,
                    10_000,
                    true,
                    8_600,
                    900,
                    25,
                    "compiled exactness keeps the same task success while cutting latency and cost",
                ),
                route(
                    TassadarCounterfactualRouteFamily::InternalExecutor,
                    10_000,
                    true,
                    8_400,
                    950,
                    25,
                    "internal executor is also viable but slightly weaker on evidence than the compiled exact row",
                ),
                route(
                    TassadarCounterfactualRouteFamily::ExternalTool,
                    10_000,
                    false,
                    9_100,
                    2_600,
                    130,
                    "external tools buy evidence but overpay for the bounded parity case",
                ),
            ],
            "the realized hybrid route hides a cheaper compiled-exact alternative on the same bounded task",
        ),
    ]
}

fn case(
    case_id: &str,
    workload_family: &str,
    realized_route: TassadarCounterfactualRouteOption,
    counterfactual_routes: Vec<TassadarCounterfactualRouteOption>,
    note: &str,
) -> TassadarCounterfactualRouteCase {
    let best_route = best_route(&realized_route, &counterfactual_routes);
    TassadarCounterfactualRouteCase {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        better_counterfactual_exists: best_route.route_family != realized_route.route_family,
        best_available_route_family: best_route.route_family,
        accepted_outcome_delta_bps: bool_to_bps(best_route.accepted_outcome_ready)
            - bool_to_bps(realized_route.accepted_outcome_ready),
        success_delta_bps: best_route.task_success_bps as i32
            - realized_route.task_success_bps as i32,
        evidence_delta_bps: best_route.evidence_quality_bps as i32
            - realized_route.evidence_quality_bps as i32,
        cost_regret_milliunits: realized_route.cost_milliunits as i32
            - best_route.cost_milliunits as i32,
        latency_regret_ms: realized_route.latency_ms as i32 - best_route.latency_ms as i32,
        realized_route,
        counterfactual_routes,
        note: String::from(note),
    }
}

fn route(
    route_family: TassadarCounterfactualRouteFamily,
    task_success_bps: u32,
    accepted_outcome_ready: bool,
    evidence_quality_bps: u32,
    cost_milliunits: u32,
    latency_ms: u32,
    note: &str,
) -> TassadarCounterfactualRouteOption {
    TassadarCounterfactualRouteOption {
        route_family,
        task_success_bps,
        accepted_outcome_ready,
        evidence_quality_bps,
        cost_milliunits,
        latency_ms,
        note: String::from(note),
    }
}

fn best_route<'a>(
    realized_route: &'a TassadarCounterfactualRouteOption,
    counterfactual_routes: &'a [TassadarCounterfactualRouteOption],
) -> &'a TassadarCounterfactualRouteOption {
    let mut best = realized_route;
    for option in counterfactual_routes {
        if route_better(option, best) {
            best = option;
        }
    }
    best
}

fn route_better(
    candidate: &TassadarCounterfactualRouteOption,
    incumbent: &TassadarCounterfactualRouteOption,
) -> bool {
    if candidate.accepted_outcome_ready != incumbent.accepted_outcome_ready {
        return candidate.accepted_outcome_ready && !incumbent.accepted_outcome_ready;
    }
    if candidate.task_success_bps != incumbent.task_success_bps {
        return candidate.task_success_bps > incumbent.task_success_bps;
    }
    if candidate.evidence_quality_bps != incumbent.evidence_quality_bps {
        return candidate.evidence_quality_bps > incumbent.evidence_quality_bps;
    }
    if candidate.cost_milliunits != incumbent.cost_milliunits {
        return candidate.cost_milliunits < incumbent.cost_milliunits;
    }
    candidate.latency_ms < incumbent.latency_ms
}

fn is_overuse(case: &TassadarCounterfactualRouteCase) -> bool {
    matches!(
        case.realized_route.route_family,
        TassadarCounterfactualRouteFamily::InternalExecutor
            | TassadarCounterfactualRouteFamily::ExternalTool
            | TassadarCounterfactualRouteFamily::HybridComposite
    ) && matches!(
        case.best_available_route_family,
        TassadarCounterfactualRouteFamily::LanguageOnly
            | TassadarCounterfactualRouteFamily::CompiledExact
    ) && case.better_counterfactual_exists
}

fn is_underuse(case: &TassadarCounterfactualRouteCase) -> bool {
    matches!(
        case.realized_route.route_family,
        TassadarCounterfactualRouteFamily::LanguageOnly
            | TassadarCounterfactualRouteFamily::CompiledExact
    ) && matches!(
        case.best_available_route_family,
        TassadarCounterfactualRouteFamily::InternalExecutor
            | TassadarCounterfactualRouteFamily::ExternalTool
            | TassadarCounterfactualRouteFamily::HybridComposite
    ) && case.better_counterfactual_exists
}

fn bool_to_bps(value: bool) -> i32 {
    if value { 10_000 } else { 0 }
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
        TassadarCounterfactualRouteFamily, build_tassadar_counterfactual_route_quality_report,
        load_tassadar_counterfactual_route_quality_report,
        tassadar_counterfactual_route_quality_report_path,
    };

    #[test]
    fn counterfactual_route_quality_report_keeps_underuse_and_overuse_explicit() {
        let report = build_tassadar_counterfactual_route_quality_report();

        assert_eq!(report.evaluated_cases.len(), 4);
        assert!(report.evaluated_cases.iter().any(|case| {
            case.case_id == "case.patch_apply_internal_underuse"
                && case.best_available_route_family
                    == TassadarCounterfactualRouteFamily::HybridComposite
                && case.accepted_outcome_delta_bps == 10_000
        }));
        assert!(report.evaluated_cases.iter().any(|case| {
            case.case_id == "case.parity_hybrid_overuse"
                && case.best_available_route_family
                    == TassadarCounterfactualRouteFamily::CompiledExact
                && case.cost_regret_milliunits > 0
        }));
    }

    #[test]
    fn counterfactual_route_quality_report_matches_committed_truth() {
        let expected = build_tassadar_counterfactual_route_quality_report();
        let committed = load_tassadar_counterfactual_route_quality_report(
            tassadar_counterfactual_route_quality_report_path(),
        )
        .expect("committed counterfactual route-quality report");

        assert_eq!(committed, expected);
    }
}
