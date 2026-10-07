use serde_json::json;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EvalRuntimeError,
};
use psionic_data::DatasetKey;
use psionic_environments::EnvironmentPackageKey;

/// Stable benchmark ref for the canonical actual-lane checkpoint eval pack.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_REF: &str =
    "benchmark://psion/actual_pretraining/checkpoint_eval";

/// Stable version for the canonical actual-lane checkpoint eval pack.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_VERSION: &str = "2026.04.02";

/// Canonical committed fixture path for the actual-lane checkpoint eval pack.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_eval_benchmark_pack_v1.json";

const ENVIRONMENT_REF: &str = "env.psion.actual_pretraining.checkpoint_eval";
const DATASET_REF: &str = "dataset://psion/actual_pretraining/checkpoint_eval";

/// Builds the canonical benchmark package for actual-lane checkpoint evaluation.
pub fn build_psion_actual_pretraining_checkpoint_eval_benchmark_package()
-> Result<BenchmarkPackage, EvalRuntimeError> {
    let environment = EnvironmentPackageKey::new(
        ENVIRONMENT_REF,
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_VERSION,
    );
    let dataset = DatasetKey::new(
        DATASET_REF,
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_VERSION,
    );
    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_REF,
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_VERSION,
        ),
        "Psion Actual Pretraining Checkpoint Eval Pack",
        environment,
        1,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_dataset(dataset, Some(String::from("checkpoint_eval_review")))
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: false,
        require_token_accounting: false,
        require_final_state_capture: true,
        require_execution_strategy: false,
    })
    .with_cases(vec![
        benchmark_case(
            "architecture_reasoning_gate",
            1,
            "architecture_reasoning",
            "architecture_reasoning",
            8200,
        ),
        benchmark_case(
            "normative_spec_reading_gate",
            2,
            "normative_spec_reading",
            "normative_spec_reading",
            8600,
        ),
        benchmark_case(
            "engineering_spec_interpretation_gate",
            3,
            "engineering_spec_interpretation",
            "engineering_spec_interpretation",
            8500,
        ),
        benchmark_case(
            "memorization_versus_reasoning_gate",
            4,
            "memorization_versus_reasoning",
            "memorization_versus_reasoning",
            7900,
        ),
    ]);
    package.metadata.insert(
        String::from("lane_id"),
        json!("psion_actual_pretraining_v1"),
    );
    package.metadata.insert(
        String::from("pack_id"),
        json!("psion_actual_pretraining_checkpoint_eval_pack_v1"),
    );
    package.metadata.insert(
        String::from("claim_boundary"),
        json!(
            "This benchmark pack is the checkpoint-time review surface for the canonical actual pretraining lane only. It does not widen the actual lane into a detached curriculum program or claim that distributed broader-pretraining execution is already closed."
        ),
    );
    package.metadata.insert(
        String::from("required_package_families"),
        json!([
            "architecture_reasoning",
            "normative_spec_reading",
            "engineering_spec_interpretation",
            "memorization_versus_reasoning"
        ]),
    );
    package.validate()?;
    Ok(package)
}

fn benchmark_case(
    case_id: &str,
    ordinal: u64,
    package_family: &str,
    acceptance_family: &str,
    threshold_bps: u32,
) -> BenchmarkCase {
    let mut case = BenchmarkCase::new(case_id);
    case.ordinal = Some(ordinal);
    case.input_ref = Some(format!("family://psion/{package_family}"));
    case.expected_output_ref = Some(format!("acceptance://psion/{acceptance_family}"));
    case.metadata = json!({
        "package_family": package_family,
        "acceptance_family": acceptance_family,
        "metric_kind": "pass_rate_bps",
        "threshold_bps": threshold_bps,
        "detail": format!(
            "Checkpoint eval gate keeps the actual lane bound to the frozen `{package_family}` family before later continue-vs-restart logic consumes the retained checkpoint decision."
        ),
    });
    case
}

#[cfg(test)]
mod tests {
    use super::build_psion_actual_pretraining_checkpoint_eval_benchmark_package;

    #[test]
    fn actual_pretraining_checkpoint_eval_benchmark_package_is_valid() {
        let package = build_psion_actual_pretraining_checkpoint_eval_benchmark_package()
            .expect("checkpoint eval benchmark package should build");
        package
            .validate()
            .expect("checkpoint eval benchmark package should validate");
        assert_eq!(package.cases.len(), 4);
    }
}
