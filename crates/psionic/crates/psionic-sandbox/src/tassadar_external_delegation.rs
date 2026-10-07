use psionic_models::TassadarWorkloadClass;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExternalDelegationBaseline {
    pub baseline_id: String,
    pub product_id: String,
    pub sandbox_runtime_family: String,
    pub supported_workload_classes: Vec<TassadarWorkloadClass>,
    pub evidence_surface: String,
    pub benchmark_refs: Vec<String>,
    pub claim_boundary: String,
    pub baseline_digest: String,
}

impl TassadarExternalDelegationBaseline {
    fn new() -> Self {
        let mut supported_workload_classes = vec![
            TassadarWorkloadClass::ArithmeticMicroprogram,
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarWorkloadClass::LongLoopKernel,
            TassadarWorkloadClass::SudokuClass,
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarWorkloadClass::ClrsShortestPath,
        ];
        supported_workload_classes.sort_by_key(|class| class.as_str());
        let mut baseline = Self {
            baseline_id: String::from("tassadar.external_delegation_baseline.v1"),
            product_id: String::from("psionic.sandbox_execution"),
            sandbox_runtime_family: String::from("deterministic_sandbox_tool_loop"),
            supported_workload_classes,
            evidence_surface: String::from("sandbox_execution_receipt"),
            benchmark_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_negative_invocation_report.json"),
            ],
            claim_boundary: String::from(
                "this baseline is the explicit external sandbox or tool-loop foil for matched Tassadar delegation benchmarks. It does not imply product promotion or authority closure by itself",
            ),
            baseline_digest: String::new(),
        };
        baseline.baseline_digest =
            stable_digest(b"psionic_tassadar_external_delegation_baseline|", &baseline);
        baseline
    }
}

#[must_use]
pub fn tassadar_external_delegation_baseline() -> TassadarExternalDelegationBaseline {
    TassadarExternalDelegationBaseline::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::tassadar_external_delegation_baseline;
    use psionic_models::TassadarWorkloadClass;

    #[test]
    fn external_delegation_baseline_is_machine_legible() {
        let baseline = tassadar_external_delegation_baseline();

        assert_eq!(baseline.product_id, "psionic.sandbox_execution");
        assert!(
            baseline
                .supported_workload_classes
                .contains(&TassadarWorkloadClass::LongLoopKernel)
        );
        assert_eq!(baseline.benchmark_refs.len(), 3);
    }
}
