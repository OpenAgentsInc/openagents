use serde_json::json;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EvalRuntimeError,
};
use psionic_data::DatasetKey;
use psionic_environments::EnvironmentPackageKey;

/// Stable benchmark ref for the canonical actual-lane continuation eval pack.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_REF: &str =
    "benchmark://psion/actual_pretraining/continuation_stage";

/// Stable version for the canonical actual-lane continuation eval pack.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_VERSION: &str = "2026.04.02";

/// Canonical committed fixture path for the actual-lane continuation eval pack.
pub const PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_continuation_eval_benchmark_pack_v1.json";

const ENVIRONMENT_REF: &str = "env.psion.actual_pretraining.continuation_stage";
const DATASET_REF: &str = "dataset://psion/actual_pretraining/continuation_stage";

/// Builds the canonical benchmark package for actual-lane continuation review.
pub fn build_psion_actual_pretraining_continuation_eval_benchmark_package()
-> Result<BenchmarkPackage, EvalRuntimeError> {
    let environment = EnvironmentPackageKey::new(
        ENVIRONMENT_REF,
        PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_VERSION,
    );
    let dataset = DatasetKey::new(
        DATASET_REF,
        PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_VERSION,
    );
    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_REF,
            PSION_ACTUAL_PRETRAINING_CONTINUATION_EVAL_BENCHMARK_VERSION,
        ),
        "Psion Actual Pretraining Continuation Eval Pack",
        environment,
        1,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_dataset(dataset, Some(String::from("continuation_stage_review")))
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: false,
        require_token_accounting: false,
        require_final_state_capture: true,
        require_execution_strategy: false,
    })
    .with_cases(vec![
        benchmark_case(
            "reasoning_style_plurality_gate",
            1,
            "reasoning_style_plurality",
            "reasoning_style_plurality",
            9000,
        ),
        benchmark_case(
            "plugin_result_interpretation_gate",
            2,
            "plugin_result_interpretation",
            "plugin_result_interpretation",
            8000,
        ),
        benchmark_case(
            "rollout_policy_lineage_gate",
            3,
            "rollout_policy_lineage",
            "rollout_policy_lineage",
            10000,
        ),
        benchmark_case(
            "post_training_benchmark_consistency_gate",
            4,
            "post_training_benchmark_consistency",
            "post_training_benchmark_consistency",
            7800,
        ),
    ]);
    package.metadata.insert(
        String::from("lane_id"),
        json!("psion_actual_pretraining_v1"),
    );
    package.metadata.insert(
        String::from("continuation_target_id"),
        json!("psion_actual_pretraining_general_sft_agentic_sft_v1"),
    );
    package.metadata.insert(
        String::from("pack_id"),
        json!("psion_actual_pretraining_continuation_eval_pack_v1"),
    );
    package.metadata.insert(
        String::from("claim_boundary"),
        json!(
            "This benchmark pack reviews the bounded continuation-stage alignment above the canonical actual pretraining lane. It binds the reasoning bridge, the plugin-conditioned agentic stage, and the current repo-owned post-training reference surface without claiming plugin-conditioned RL execution or cluster-scale continuation closure."
        ),
    );
    package.metadata.insert(
        String::from("required_package_families"),
        json!([
            "reasoning_style_plurality",
            "plugin_result_interpretation",
            "rollout_policy_lineage",
            "post_training_benchmark_consistency"
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
            "Continuation-stage review keeps the actual-lane handoff tied to the frozen `{package_family}` family before any later continuation rehearsal or promotion claim consumes it."
        ),
    });
    case
}

#[cfg(test)]
mod tests {
    use super::build_psion_actual_pretraining_continuation_eval_benchmark_package;

    #[test]
    fn actual_pretraining_continuation_eval_benchmark_package_is_valid() {
        let package = build_psion_actual_pretraining_continuation_eval_benchmark_package()
            .expect("continuation eval benchmark package should build");
        package
            .validate()
            .expect("continuation eval benchmark package should validate");
        assert_eq!(package.cases.len(), 4);
    }
}
