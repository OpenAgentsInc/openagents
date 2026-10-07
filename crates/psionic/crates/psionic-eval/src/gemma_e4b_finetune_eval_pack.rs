use serde_json::json;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EvalRuntimeError,
};
use psionic_data::DatasetKey;
use psionic_environments::EnvironmentPackageKey;

/// Stable benchmark ref for the bounded Gemma e4b finetune eval pack.
pub const GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_REF: &str =
    "benchmark://psionic/gemma4/e4b/finetune_eval";

/// Stable version for the bounded Gemma e4b finetune eval pack.
pub const GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_VERSION: &str = "2026.04.03";

const ENVIRONMENT_REF: &str = "env.psionic.gemma4.e4b.finetune_eval";
const DATASET_REF: &str = "dataset://psionic/gemma4/e4b/finetune_eval";

/// Builds the canonical benchmark package for the first Gemma e4b finetune eval pack.
pub fn build_gemma_e4b_finetune_eval_benchmark_package()
-> Result<BenchmarkPackage, EvalRuntimeError> {
    let environment =
        EnvironmentPackageKey::new(ENVIRONMENT_REF, GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_VERSION);
    let dataset = DatasetKey::new(DATASET_REF, GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_VERSION);
    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_REF,
            GEMMA_E4B_FINETUNE_EVAL_BENCHMARK_VERSION,
        ),
        "Gemma e4b Finetune Eval Pack",
        environment,
        1,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_dataset(dataset, Some(String::from("held_out_validation")))
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: false,
        require_token_accounting: false,
        require_final_state_capture: true,
        require_execution_strategy: false,
    })
    .with_cases(vec![
        benchmark_case(
            "instruction_following_validation",
            1,
            "instruction_following",
            "held_out_instruction_following",
            8400,
        ),
        benchmark_case(
            "helpfulness_validation",
            2,
            "helpfulness",
            "held_out_helpfulness",
            8300,
        ),
        benchmark_case(
            "tool_call_shape_validation",
            3,
            "tool_call_shape",
            "gemma_native_tool_shape",
            9000,
        ),
        benchmark_case(
            "formatting_validation",
            4,
            "formatting",
            "bounded_formatting",
            9000,
        ),
        benchmark_case(
            "steerability_validation",
            5,
            "steerability",
            "developer_and_system_steerability",
            8500,
        ),
    ]);
    package.metadata.insert(
        String::from("lane_id"),
        json!("gemma4.e4b.cuda.adapter_sft.v1"),
    );
    package.metadata.insert(
        String::from("pack_id"),
        json!("gemma_e4b_finetune_eval_pack_v1"),
    );
    package.metadata.insert(
        String::from("required_split_refs"),
        json!([
            "train",
            "held_out_validation",
            "final_report",
            "baseline_short"
        ]),
    );
    package.metadata.insert(
        String::from("required_operator_review_template_id"),
        json!("gemma4.e4b.promoted_checkpoint_vibe_eval.v1"),
    );
    package.metadata.insert(
        String::from("claim_boundary"),
        json!(
            "This benchmark pack binds the first Gemma e4b adapter-SFT MVP to one bounded held-out validation surface. It does not claim multimodal finetuning, broad family coverage, RL-first work, or automatic promotion without operator review."
        ),
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
    case.input_ref = Some(format!("family://gemma4/e4b/{package_family}"));
    case.expected_output_ref = Some(format!("acceptance://gemma4/e4b/{acceptance_family}"));
    case.metadata = json!({
        "package_family": package_family,
        "acceptance_family": acceptance_family,
        "metric_kind": "pass_rate_bps",
        "threshold_bps": threshold_bps,
        "detail": format!(
            "The bounded Gemma e4b finetune lane keeps `{package_family}` explicit inside held-out validation before a checkpoint can enter promotion review."
        ),
    });
    case
}

#[cfg(test)]
mod tests {
    use super::build_gemma_e4b_finetune_eval_benchmark_package;

    #[test]
    fn gemma_e4b_finetune_eval_benchmark_package_is_valid() {
        let package = build_gemma_e4b_finetune_eval_benchmark_package()
            .expect("gemma finetune eval benchmark package should build");
        package
            .validate()
            .expect("gemma finetune eval benchmark package should validate");
        assert_eq!(package.cases.len(), 5);
        assert_eq!(package.split.as_deref(), Some("held_out_validation"));
    }
}
