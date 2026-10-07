use psionic_models::TassadarWorkloadClass;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_DELEGATION_BENCHMARK_SUITE_VERSION: &str =
    "psionic.tassadar_delegation_benchmark_suite.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDelegationBenchmarkSuiteCase {
    pub case_id: String,
    pub workload_class: TassadarWorkloadClass,
    pub environment_ref: String,
    pub benchmark_profile_ref: String,
    pub cpu_reference_authority: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDelegationBenchmarkSuite {
    pub suite_id: String,
    pub suite_version: String,
    pub cases: Vec<TassadarDelegationBenchmarkSuiteCase>,
    pub compared_lanes: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub claim_boundary: String,
    pub suite_digest: String,
}

impl TassadarDelegationBenchmarkSuite {
    fn new() -> Self {
        let mut suite = Self {
            suite_id: String::from("tassadar.delegation_benchmark_suite.v1"),
            suite_version: String::from(TASSADAR_DELEGATION_BENCHMARK_SUITE_VERSION),
            cases: vec![
                suite_case(
                    "arithmetic_microprogram_exact_transform",
                    TassadarWorkloadClass::ArithmeticMicroprogram,
                    "tassadar/eval/arithmetic_microprogram_v1",
                    "benchmarks/tassadar/arithmetic_microprogram_v1",
                    "cpu_reference/arithmetic_microprogram_v1",
                    "bounded arithmetic transform where the internal lane should compete honestly against both CPU-reference and sandbox delegation",
                ),
                suite_case(
                    "memory_heavy_kernel_patch",
                    TassadarWorkloadClass::MemoryHeavyKernel,
                    "tassadar/eval/memory_heavy_kernel_v1",
                    "benchmarks/tassadar/memory_heavy_kernel_v1",
                    "cpu_reference/memory_heavy_kernel_v1",
                    "memory-heavy exact patch workload shared across internal, CPU-reference, and external sandbox lanes",
                ),
                suite_case(
                    "long_loop_kernel_robust",
                    TassadarWorkloadClass::LongLoopKernel,
                    "tassadar/eval/long_loop_kernel_v1",
                    "benchmarks/tassadar/long_loop_kernel_v1",
                    "cpu_reference/long_loop_kernel_v1",
                    "long-loop robustness row where fallback churn and refusal posture matter as much as nominal correctness",
                ),
                suite_case(
                    "sudoku_candidate_check",
                    TassadarWorkloadClass::SudokuClass,
                    "tassadar/eval/sudoku_candidate_check_v1",
                    "benchmarks/tassadar/sudoku_candidate_check_v1",
                    "cpu_reference/sudoku_candidate_check_v1",
                    "exact-search validation row where CPU-reference remains the strongest exact authority baseline",
                ),
                suite_case(
                    "branch_heavy_control_repair",
                    TassadarWorkloadClass::BranchHeavyKernel,
                    "tassadar/eval/branch_heavy_control_repair_v1",
                    "benchmarks/tassadar/branch_heavy_control_repair_v1",
                    "cpu_reference/branch_heavy_control_repair_v1",
                    "branch-heavy control-flow row that exposes hybrid routing as the only honest posture on the current public substrate",
                ),
                suite_case(
                    "clrs_shortest_path_verification",
                    TassadarWorkloadClass::ClrsShortestPath,
                    "tassadar/eval/clrs_shortest_path_v1",
                    "benchmarks/tassadar/clrs_shortest_path_v1",
                    "cpu_reference/clrs_shortest_path_v1",
                    "CLRS bridge row where CPU-reference and external delegation remain stronger than the current internal public lane",
                ),
            ],
            compared_lanes: vec![
                String::from("internal_exact_compute"),
                String::from("cpu_reference"),
                String::from("external_sandbox"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-environments"),
                String::from("crates/psionic-sandbox"),
                String::from("crates/psionic-router"),
                String::from("crates/psionic-provider"),
                String::from("crates/psionic-eval"),
            ],
            claim_boundary: String::from(
                "this suite is a benchmark-bound matched-workload contract across internal exact-compute, cpu-reference, and external sandbox lanes. It does not promote any lane or treat one favorable row as broad executor closure",
            ),
            suite_digest: String::new(),
        };
        suite.suite_digest = stable_digest(b"psionic_tassadar_delegation_benchmark_suite|", &suite);
        suite
    }
}

#[must_use]
pub fn tassadar_delegation_benchmark_suite() -> TassadarDelegationBenchmarkSuite {
    TassadarDelegationBenchmarkSuite::new()
}

fn suite_case(
    case_id: &str,
    workload_class: TassadarWorkloadClass,
    environment_ref: &str,
    benchmark_profile_ref: &str,
    cpu_reference_authority: &str,
    note: &str,
) -> TassadarDelegationBenchmarkSuiteCase {
    TassadarDelegationBenchmarkSuiteCase {
        case_id: String::from(case_id),
        workload_class,
        environment_ref: String::from(environment_ref),
        benchmark_profile_ref: String::from(benchmark_profile_ref),
        cpu_reference_authority: String::from(cpu_reference_authority),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::tassadar_delegation_benchmark_suite;
    use psionic_models::TassadarWorkloadClass;

    #[test]
    fn delegation_benchmark_suite_is_machine_legible() {
        let suite = tassadar_delegation_benchmark_suite();

        assert_eq!(suite.cases.len(), 6);
        assert_eq!(suite.compared_lanes.len(), 3);
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.workload_class == TassadarWorkloadClass::LongLoopKernel)
        );
        assert!(
            suite
                .target_surfaces
                .contains(&String::from("crates/psionic-sandbox"))
        );
    }
}
