use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COMPOSITE_ROUTING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_composite_routing_report.json";
const TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";
const TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json";
const TASSADAR_EXECUTION_UNIT_REGISTRATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompositeRouteLaneKind {
    Planner,
    CpuReference,
    GpuExecutor,
    ModuleExecutor,
    Validator,
    Sandbox,
    Cluster,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompositeRouteSelection {
    CompositePreferred,
    SingleLanePreferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeRouteStep {
    pub step_id: String,
    pub lane_kind: TassadarCompositeRouteLaneKind,
    pub lane_ref: String,
    pub evidence_ref: String,
    pub fallback_capable: bool,
    pub challenge_path_attached: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSingleLaneBaseline {
    pub baseline_id: String,
    pub lane_kind: TassadarCompositeRouteLaneKind,
    pub route_ref: String,
    pub correctness_bps: u32,
    pub evidence_quality_bps: u32,
    pub latency_ms: u32,
    pub cost_milliunits: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeRoutingCase {
    pub case_id: String,
    pub workload_family: String,
    pub mount_id: String,
    pub composite_steps: Vec<TassadarCompositeRouteStep>,
    pub single_lane_baselines: Vec<TassadarSingleLaneBaseline>,
    pub selected_route: TassadarCompositeRouteSelection,
    pub composite_correctness_bps: u32,
    pub composite_evidence_quality_bps: u32,
    pub composite_latency_ms: u32,
    pub composite_cost_milliunits: u32,
    pub fallback_path_used: bool,
    pub challenge_path_used: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompositeRoutingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub evaluated_cases: Vec<TassadarCompositeRoutingCase>,
    pub composite_preferred_case_count: u32,
    pub single_lane_preferred_case_count: u32,
    pub fallback_case_count: u32,
    pub challenge_path_case_count: u32,
    pub composite_evidence_lift_bps_vs_best_single_lane: i32,
    pub composite_cost_delta_milliunits_vs_best_single_lane: i32,
    pub composite_latency_delta_ms_vs_best_single_lane: i32,
    pub generated_from_refs: Vec<String>,
    pub sandbox_dependency_marker: String,
    pub clusters_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCompositeRoutingReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_composite_routing_report() -> TassadarCompositeRoutingReport {
    let evaluated_cases = seeded_cases();
    let composite_preferred_case_count = evaluated_cases
        .iter()
        .filter(|case| case.selected_route == TassadarCompositeRouteSelection::CompositePreferred)
        .count() as u32;
    let single_lane_preferred_case_count = evaluated_cases
        .iter()
        .filter(|case| case.selected_route == TassadarCompositeRouteSelection::SingleLanePreferred)
        .count() as u32;
    let fallback_case_count = evaluated_cases
        .iter()
        .filter(|case| case.fallback_path_used)
        .count() as u32;
    let challenge_path_case_count = evaluated_cases
        .iter()
        .filter(|case| case.challenge_path_used)
        .count() as u32;
    let composite_evidence_lift_total = evaluated_cases
        .iter()
        .map(|case| {
            case.composite_evidence_quality_bps as i32
                - best_single_lane_baseline(case).evidence_quality_bps as i32
        })
        .sum::<i32>();
    let composite_cost_delta_total = evaluated_cases
        .iter()
        .map(|case| {
            case.composite_cost_milliunits as i32
                - best_single_lane_baseline(case).cost_milliunits as i32
        })
        .sum::<i32>();
    let composite_latency_delta_total = evaluated_cases
        .iter()
        .map(|case| {
            case.composite_latency_ms as i32 - best_single_lane_baseline(case).latency_ms as i32
        })
        .sum::<i32>();
    let case_count = evaluated_cases.len() as i32;
    let mut generated_from_refs = vec![
        String::from(TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF),
        String::from(TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
        String::from(TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF),
        String::from(TASSADAR_EXECUTION_UNIT_REGISTRATION_REPORT_REF),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarCompositeRoutingReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.composite_routing.report.v1"),
        evaluated_cases,
        composite_preferred_case_count,
        single_lane_preferred_case_count,
        fallback_case_count,
        challenge_path_case_count,
        composite_evidence_lift_bps_vs_best_single_lane: composite_evidence_lift_total / case_count,
        composite_cost_delta_milliunits_vs_best_single_lane: composite_cost_delta_total
            / case_count,
        composite_latency_delta_ms_vs_best_single_lane: composite_latency_delta_total / case_count,
        generated_from_refs,
        sandbox_dependency_marker: String::from(
            "psionic-sandbox remains the owner of canonical delegated execution admission and side-effect policy outside standalone psionic",
        ),
        clusters_dependency_marker: String::from(
            "clusters remain the owner of canonical multi-host resource placement and replay-safe distributed execution outside standalone psionic",
        ),
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical task-scoped composite-route mounting outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this router report is a research-only heterogeneous routing surface over seeded composite plans. It keeps lane identity, fallback paths, challenge paths, evidence obligations, and single-lane baselines explicit without treating composite routing as accepted-outcome authority, settlement truth, or served capability widening",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Composite routing covers {} seeded cases with {} composite-preferred cases, {} single-lane-preferred cases, {} fallback cases, and {} challenge-path cases.",
        report.evaluated_cases.len(),
        report.composite_preferred_case_count,
        report.single_lane_preferred_case_count,
        report.fallback_case_count,
        report.challenge_path_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_composite_routing_report|", &report);
    report
}

#[must_use]
pub fn tassadar_composite_routing_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPOSITE_ROUTING_REPORT_REF)
}

pub fn write_tassadar_composite_routing_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompositeRoutingReport, TassadarCompositeRoutingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompositeRoutingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_composite_routing_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCompositeRoutingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_composite_routing_report(
    path: impl AsRef<Path>,
) -> Result<TassadarCompositeRoutingReport, TassadarCompositeRoutingReportError> {
    read_json(path)
}

fn seeded_cases() -> Vec<TassadarCompositeRoutingCase> {
    vec![
        TassadarCompositeRoutingCase {
            case_id: String::from("composite.patch_validator_bridge"),
            workload_family: String::from("patch_apply_internal_exact"),
            mount_id: String::from("mount.patch.validator.v1"),
            composite_steps: vec![
                step(
                    "planner",
                    TassadarCompositeRouteLaneKind::Planner,
                    "planner.policy.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                    false,
                    false,
                    "planner selects a hybrid path instead of a single-lane winner",
                ),
                step(
                    "module_executor",
                    TassadarCompositeRouteLaneKind::ModuleExecutor,
                    "module.frontier_relax_core@1.0.0",
                    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json",
                    true,
                    false,
                    "module executor handles the exact patch core under benchmark-gated posture",
                ),
                step(
                    "cpu_reference",
                    TassadarCompositeRouteLaneKind::CpuReference,
                    "cpu.reference.patch_apply.v1",
                    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json",
                    false,
                    false,
                    "cpu reference certifies exactness on the bounded patch family",
                ),
                step(
                    "validator",
                    TassadarCompositeRouteLaneKind::Validator,
                    "validator.patch_apply.strict.v1",
                    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json",
                    false,
                    true,
                    "validator closes the evidence gap for accepted-outcome-ready patch work",
                ),
            ],
            single_lane_baselines: vec![
                baseline(
                    "baseline.module_only",
                    TassadarCompositeRouteLaneKind::ModuleExecutor,
                    "route.internal_exact_only",
                    9300,
                    7200,
                    60,
                    2500,
                    "module-only route is cheap but weaker on evidence quality",
                ),
                baseline(
                    "baseline.sandbox_only",
                    TassadarCompositeRouteLaneKind::Sandbox,
                    "route.sandbox_only",
                    9800,
                    9000,
                    150,
                    7000,
                    "sandbox-only route is strong but more expensive and slower",
                ),
            ],
            selected_route: TassadarCompositeRouteSelection::CompositePreferred,
            composite_correctness_bps: 10000,
            composite_evidence_quality_bps: 9600,
            composite_latency_ms: 85,
            composite_cost_milliunits: 3800,
            fallback_path_used: false,
            challenge_path_used: true,
            note: String::from(
                "patch work prefers the composite lane because CPU reference and validator attachment lift evidence quality enough to justify the extra steps",
            ),
        },
        TassadarCompositeRoutingCase {
            case_id: String::from("composite.long_loop_sandbox_fallback"),
            workload_family: String::from("long_loop_validator_heavy"),
            mount_id: String::from("mount.long_loop.validator.v1"),
            composite_steps: vec![
                step(
                    "planner",
                    TassadarCompositeRouteLaneKind::Planner,
                    "planner.policy.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                    false,
                    false,
                    "planner starts with an internal exact-compute attempt but keeps fallback explicit",
                ),
                step(
                    "module_executor",
                    TassadarCompositeRouteLaneKind::ModuleExecutor,
                    "module.checkpoint_backtrack_core@1.0.0",
                    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json",
                    true,
                    false,
                    "module executor handles the bounded fast path before the fallback fires",
                ),
                step(
                    "sandbox_fallback",
                    TassadarCompositeRouteLaneKind::Sandbox,
                    "sandbox.delegate.long_loop.v1",
                    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json",
                    false,
                    false,
                    "sandbox delegation remains explicit once the long-loop path exceeds the internal lane's honest closure",
                ),
                step(
                    "validator",
                    TassadarCompositeRouteLaneKind::Validator,
                    "validator.long_loop.strict.v1",
                    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json",
                    false,
                    true,
                    "validator stays attached across the fallback rather than being silently dropped",
                ),
            ],
            single_lane_baselines: vec![
                baseline(
                    "baseline.module_only",
                    TassadarCompositeRouteLaneKind::ModuleExecutor,
                    "route.internal_exact_only",
                    7200,
                    5400,
                    70,
                    3000,
                    "module-only route underperforms on long-loop correctness and evidence",
                ),
                baseline(
                    "baseline.sandbox_only",
                    TassadarCompositeRouteLaneKind::Sandbox,
                    "route.sandbox_only",
                    9800,
                    9200,
                    220,
                    7600,
                    "sandbox-only route is robust but more expensive and slower than the hybrid fallback plan",
                ),
            ],
            selected_route: TassadarCompositeRouteSelection::CompositePreferred,
            composite_correctness_bps: 9900,
            composite_evidence_quality_bps: 9400,
            composite_latency_ms: 190,
            composite_cost_milliunits: 6500,
            fallback_path_used: true,
            challenge_path_used: true,
            note: String::from(
                "the hybrid fallback route wins because it keeps the internal fast path where honest, then escalates to sandbox and validator lanes when needed",
            ),
        },
        TassadarCompositeRoutingCase {
            case_id: String::from("composite.gpu_cluster_article"),
            workload_family: String::from("article_hybrid_workflow"),
            mount_id: String::from("mount.article.hybrid.v1"),
            composite_steps: vec![
                step(
                    "planner",
                    TassadarCompositeRouteLaneKind::Planner,
                    "planner.policy.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json",
                    false,
                    false,
                    "planner opens a GPU plus cluster path for the article workload",
                ),
                step(
                    "gpu_executor",
                    TassadarCompositeRouteLaneKind::GpuExecutor,
                    "gpu.executor.article.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json",
                    false,
                    false,
                    "GPU executor handles the dense article path efficiently",
                ),
                step(
                    "cluster_fanout",
                    TassadarCompositeRouteLaneKind::Cluster,
                    "cluster.article.hybrid.v1",
                    "docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md",
                    false,
                    false,
                    "cluster fanout keeps multi-host work explicit rather than hidden in the executor lane",
                ),
                step(
                    "cpu_reference",
                    TassadarCompositeRouteLaneKind::CpuReference,
                    "cpu.reference.article.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_trace_abi_decision_report.json",
                    false,
                    false,
                    "CPU reference closes the replayable trace identity for the composite route",
                ),
            ],
            single_lane_baselines: vec![
                baseline(
                    "baseline.gpu_only",
                    TassadarCompositeRouteLaneKind::GpuExecutor,
                    "route.gpu_only",
                    9300,
                    6500,
                    60,
                    2600,
                    "GPU-only route is fast but under-evidenced for the hybrid article workflow",
                ),
                baseline(
                    "baseline.cpu_reference_only",
                    TassadarCompositeRouteLaneKind::CpuReference,
                    "route.cpu_reference_only",
                    10000,
                    9700,
                    280,
                    9300,
                    "CPU-reference-only route is strongest on raw evidence but too slow and costly",
                ),
            ],
            selected_route: TassadarCompositeRouteSelection::CompositePreferred,
            composite_correctness_bps: 9700,
            composite_evidence_quality_bps: 9300,
            composite_latency_ms: 90,
            composite_cost_milliunits: 4100,
            fallback_path_used: false,
            challenge_path_used: false,
            note: String::from(
                "the GPU plus cluster composite path wins because it preserves most of the evidence benefit at materially lower latency and cost than CPU-only reference execution",
            ),
        },
        TassadarCompositeRoutingCase {
            case_id: String::from("single_lane.module_parity"),
            workload_family: String::from("parity_short_bounded"),
            mount_id: String::from("mount.parity.short.v1"),
            composite_steps: vec![
                step(
                    "planner",
                    TassadarCompositeRouteLaneKind::Planner,
                    "planner.policy.hybrid.v1",
                    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                    false,
                    false,
                    "planner considers a module plus validator composite path for parity work",
                ),
                step(
                    "module_executor",
                    TassadarCompositeRouteLaneKind::ModuleExecutor,
                    "module.parity_core@1.0.0",
                    "fixtures/tassadar/reports/tassadar_execution_unit_registration_report.json",
                    false,
                    false,
                    "module executor is already exact on the short bounded parity workload",
                ),
                step(
                    "validator",
                    TassadarCompositeRouteLaneKind::Validator,
                    "validator.parity.optional.v1",
                    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json",
                    false,
                    true,
                    "optional validator attachment adds cost without meaningful evidence lift on this bounded case",
                ),
            ],
            single_lane_baselines: vec![baseline(
                "baseline.module_only",
                TassadarCompositeRouteLaneKind::ModuleExecutor,
                "route.internal_exact_only",
                10000,
                8600,
                25,
                900,
                "single-lane module execution is already sufficient on short bounded parity workloads",
            )],
            selected_route: TassadarCompositeRouteSelection::SingleLanePreferred,
            composite_correctness_bps: 10000,
            composite_evidence_quality_bps: 8500,
            composite_latency_ms: 55,
            composite_cost_milliunits: 1800,
            fallback_path_used: false,
            challenge_path_used: true,
            note: String::from(
                "the composite plan is intentionally not preferred here because more lanes do not buy enough evidence lift to justify the latency and cost overhead",
            ),
        },
    ]
}

fn step(
    step_id: &str,
    lane_kind: TassadarCompositeRouteLaneKind,
    lane_ref: &str,
    evidence_ref: &str,
    fallback_capable: bool,
    challenge_path_attached: bool,
    note: &str,
) -> TassadarCompositeRouteStep {
    TassadarCompositeRouteStep {
        step_id: String::from(step_id),
        lane_kind,
        lane_ref: String::from(lane_ref),
        evidence_ref: String::from(evidence_ref),
        fallback_capable,
        challenge_path_attached,
        note: String::from(note),
    }
}

fn baseline(
    baseline_id: &str,
    lane_kind: TassadarCompositeRouteLaneKind,
    route_ref: &str,
    correctness_bps: u32,
    evidence_quality_bps: u32,
    latency_ms: u32,
    cost_milliunits: u32,
    note: &str,
) -> TassadarSingleLaneBaseline {
    TassadarSingleLaneBaseline {
        baseline_id: String::from(baseline_id),
        lane_kind,
        route_ref: String::from(route_ref),
        correctness_bps,
        evidence_quality_bps,
        latency_ms,
        cost_milliunits,
        note: String::from(note),
    }
}

fn best_single_lane_baseline(case: &TassadarCompositeRoutingCase) -> &TassadarSingleLaneBaseline {
    case.single_lane_baselines
        .iter()
        .max_by_key(|baseline| {
            (
                baseline.correctness_bps + baseline.evidence_quality_bps,
                -(baseline.cost_milliunits as i32),
                -(baseline.latency_ms as i32),
            )
        })
        .expect("seeded case should define at least one baseline")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarCompositeRoutingReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarCompositeRoutingReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCompositeRoutingReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        TassadarCompositeRouteSelection, build_tassadar_composite_routing_report,
        load_tassadar_composite_routing_report, tassadar_composite_routing_report_path,
    };

    #[test]
    fn composite_routing_report_keeps_fallback_and_single_lane_boundaries_explicit() {
        let report = build_tassadar_composite_routing_report();

        assert_eq!(report.composite_preferred_case_count, 3);
        assert_eq!(report.single_lane_preferred_case_count, 1);
        assert_eq!(report.fallback_case_count, 1);
        assert_eq!(report.challenge_path_case_count, 3);
        assert!(report.composite_evidence_lift_bps_vs_best_single_lane > 0);
        assert!(report.evaluated_cases.iter().any(|case| {
            case.fallback_path_used
                && case.selected_route == TassadarCompositeRouteSelection::CompositePreferred
        }));
        assert!(report.evaluated_cases.iter().any(|case| {
            case.selected_route == TassadarCompositeRouteSelection::SingleLanePreferred
                && case.note.contains("not preferred")
        }));
    }

    #[test]
    fn composite_routing_report_matches_committed_truth() {
        let expected = build_tassadar_composite_routing_report();
        let committed =
            load_tassadar_composite_routing_report(tassadar_composite_routing_report_path())
                .expect("committed composite-routing report");

        assert_eq!(committed, expected);
    }
}
