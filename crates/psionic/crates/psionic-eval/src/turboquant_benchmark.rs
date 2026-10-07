use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EnvironmentPackageKey, EvalRuntimeError,
};

/// Stable benchmark package ref for the bounded TurboQuant served-generation lane.
pub const TURBOQUANT_SERVED_GENERATION_BENCHMARK_REF: &str =
    "benchmark://openagents/turboquant/gpt_oss_served_generation";

/// Stable claim boundary for the bounded TurboQuant served-generation lane.
pub const TURBOQUANT_SERVED_GENERATION_CLAIM_BOUNDARY: &str = "approximate_kv_cache_only";

/// Stable lane identifier inside the TurboQuant benchmark package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurboQuantBenchmarkLaneKind {
    /// Explicit non-claim boundary for exactness-sensitive routes.
    ExactnessBoundary,
    /// Long-context quality compared against dense decode.
    LongContextQuality,
    /// Context tiers above the published validation band.
    ContextGeneralization,
    /// Device-footprint and live-session density measurements.
    Cost,
    /// Median and tail latency under concurrency.
    Latency,
    /// Unsupported backend/model/route/prefix-reuse behavior.
    Refusal,
}

impl TurboQuantBenchmarkLaneKind {
    /// Returns a stable machine-checkable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactnessBoundary => "exactness_boundary",
            Self::LongContextQuality => "long_context_quality",
            Self::ContextGeneralization => "context_generalization",
            Self::Cost => "cost",
            Self::Latency => "latency",
            Self::Refusal => "refusal",
        }
    }
}

/// One benchmark lane inside the TurboQuant publication contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurboQuantBenchmarkLane {
    /// Stable case identifier.
    pub case_id: String,
    /// Lane kind.
    pub lane: TurboQuantBenchmarkLaneKind,
    /// Dense or refusal baseline used by the lane.
    pub baseline: String,
    /// Whether this lane is allowed to widen exactness claims.
    pub exactness_claim_allowed: bool,
    /// Tests that must exist before the lane is considered covered.
    pub required_tests: Vec<String>,
    /// Plain-language lane detail.
    pub detail: String,
}

/// Benchmark-gated publication contract for the bounded TurboQuant lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurboQuantBenchmarkContract {
    /// Stable benchmark package key.
    pub benchmark_package: BenchmarkPackageKey,
    /// Stable claim boundary.
    pub claim_boundary: String,
    /// Whether this lane is allowed to claim exactness.
    pub exactness_feature: bool,
    /// Runtime backends that may host the current approximate lane.
    pub supported_runtime_backends: Vec<String>,
    /// Capability lanes that must never publish TurboQuant.
    pub refused_publication_lanes: Vec<String>,
    /// Required refusal and downgrade rules.
    pub refusal_rules: Vec<String>,
    /// Route-selection rules that higher layers must keep explicit.
    pub route_selection_rules: Vec<String>,
    /// Benchmark lanes that define the package.
    pub lanes: Vec<TurboQuantBenchmarkLane>,
    /// Plain-language detail.
    pub detail: String,
}

/// Returns the current TurboQuant publication contract.
#[must_use]
pub fn turboquant_served_generation_benchmark_contract(
    version: &str,
) -> TurboQuantBenchmarkContract {
    TurboQuantBenchmarkContract {
        benchmark_package: BenchmarkPackageKey::new(
            TURBOQUANT_SERVED_GENERATION_BENCHMARK_REF,
            version,
        ),
        claim_boundary: String::from(TURBOQUANT_SERVED_GENERATION_CLAIM_BOUNDARY),
        exactness_feature: false,
        supported_runtime_backends: vec![String::from("cuda")],
        refused_publication_lanes: vec![
            String::from("cpu_reference_exactness"),
            String::from("tassadar_exactness"),
        ],
        refusal_rules: vec![
            String::from("do_not_publish_for_cpu_reference_exact_lanes"),
            String::from("do_not_publish_for_tassadar_exactness_lanes"),
            String::from("do_not_reuse_prefix_artifacts_across_incompatible_cache_encoding_signatures"),
            String::from("do_not_silently_fall_through_to_unsupported_kernels"),
            String::from("unsupported_cache_encoding_requests_must_refuse_or_emit_explicit_dense_downgrade"),
        ],
        route_selection_rules: vec![
            String::from("exactness_sensitive_routes_must_refuse_turboquant"),
            String::from("approximate_text_generation_routes_may_admit_turboquant_only_when_benchmark_gated"),
            String::from("dense_fallback_routes_must_emit_requested_vs_active_kv_cache_encoding_receipts"),
        ],
        lanes: vec![
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.exactness_boundary.declared_approximate"),
                lane: TurboQuantBenchmarkLaneKind::ExactnessBoundary,
                baseline: String::from("dense_reference_claim_boundary"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("capability_publication_tests"),
                    String::from("receipt_claim_boundary_tests"),
                ],
                detail: String::from(
                    "state directly that TurboQuant is an approximate KV-cache lane and not an exactness feature",
                ),
            },
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.long_context_quality.dense_baseline"),
                lane: TurboQuantBenchmarkLaneKind::LongContextQuality,
                baseline: String::from("dense_f16_kv_decode"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("benchmark_package_coverage_tests"),
                    String::from("dense_baseline_quality_comparison_tests"),
                ],
                detail: String::from(
                    "compare long-context quality directly against the dense baseline on the active served family",
                ),
            },
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.context_generalization.above_validated_band"),
                lane: TurboQuantBenchmarkLaneKind::ContextGeneralization,
                baseline: String::from("published_kv_cache_encoding_policy.context_length_bound"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("benchmark_package_coverage_tests"),
                    String::from("validated_band_overflow_tests"),
                ],
                detail: String::from(
                    "measure context tiers above the published validation band instead of widening the claim implicitly",
                ),
            },
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.cost.device_bytes_and_session_density"),
                lane: TurboQuantBenchmarkLaneKind::Cost,
                baseline: String::from("dense_f16_kv_decode"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("device_bytes_per_token_tests"),
                    String::from("max_live_sessions_tests"),
                    String::from("decode_throughput_tests"),
                ],
                detail: String::from(
                    "track memory reduction and admitted live-session density against the dense baseline",
                ),
            },
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.latency.concurrent_decode_tail"),
                lane: TurboQuantBenchmarkLaneKind::Latency,
                baseline: String::from("dense_f16_kv_decode"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("median_latency_tests"),
                    String::from("tail_latency_under_concurrency_tests"),
                ],
                detail: String::from(
                    "measure median and tail decode latency under concurrency instead of one headline token/s number",
                ),
            },
            TurboQuantBenchmarkLane {
                case_id: String::from("turboquant.refusal.unsupported_model_backend_route_prefix"),
                lane: TurboQuantBenchmarkLaneKind::Refusal,
                baseline: String::from("explicit_refusal_or_dense_downgrade"),
                exactness_claim_allowed: false,
                required_tests: vec![
                    String::from("refusal_path_tests"),
                    String::from("prefix_reuse_boundary_tests"),
                    String::from("route_selection_tests"),
                ],
                detail: String::from(
                    "cover unsupported model, backend, route, and incompatible prefix-reuse behavior with explicit refusal or explicit dense downgrade receipts",
                ),
            },
        ],
        detail: String::from(
            "bounded benchmark contract for the approximate CUDA TurboQuant KV lane over GPT-OSS served generation",
        ),
    }
}

/// Builds the canonical benchmark package for the bounded TurboQuant lane.
pub fn build_turboquant_served_generation_benchmark_package(
    version: &str,
) -> Result<BenchmarkPackage, EvalRuntimeError> {
    let contract = turboquant_served_generation_benchmark_contract(version);
    let cases = contract
        .lanes
        .iter()
        .enumerate()
        .map(|(ordinal, lane)| {
            let mut case = BenchmarkCase::new(lane.case_id.clone());
            case.ordinal = Some(ordinal as u64);
            case.input_ref = Some(format!("turboquant://lane/{}", lane.lane.as_str()));
            case.expected_output_ref = Some(format!("metric://turboquant/{}", lane.lane.as_str()));
            case.metadata = json!({
                "lane": lane.lane,
                "baseline": lane.baseline,
                "exactness_claim_allowed": lane.exactness_claim_allowed,
                "required_tests": lane.required_tests,
                "detail": lane.detail,
            });
            case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        contract.benchmark_package.clone(),
        "TurboQuant GPT-OSS Served Generation Benchmark",
        EnvironmentPackageKey::new(
            "env.openagents.turboquant.gpt_oss_served_generation",
            version,
        ),
        5,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: true,
        require_token_accounting: true,
        require_final_state_capture: true,
        require_execution_strategy: true,
    })
    .with_cases(cases);
    package.metadata.insert(
        String::from("turboquant.claim_boundary"),
        Value::String(contract.claim_boundary.clone()),
    );
    package.metadata.insert(
        String::from("turboquant.exactness_feature"),
        Value::Bool(contract.exactness_feature),
    );
    package.metadata.insert(
        String::from("turboquant.contract"),
        serde_json::to_value(&contract).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("turboquant.validated_context_bound_source"),
        Value::String(String::from(
            "published_kv_cache_encoding_policy.context_length_bound",
        )),
    );
    package.validate()?;
    Ok(package)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        build_turboquant_served_generation_benchmark_package,
        turboquant_served_generation_benchmark_contract, TurboQuantBenchmarkContract,
        TurboQuantBenchmarkLaneKind, TURBOQUANT_SERVED_GENERATION_BENCHMARK_REF,
    };

    #[test]
    fn turboquant_benchmark_package_declares_approximate_only_boundary(
    ) -> Result<(), Box<dyn Error>> {
        let package = build_turboquant_served_generation_benchmark_package("2026.03.24")?;
        let contract: TurboQuantBenchmarkContract = serde_json::from_value(
            package
                .metadata
                .get("turboquant.contract")
                .cloned()
                .ok_or("missing turboquant contract metadata")?,
        )?;

        assert_eq!(
            contract.benchmark_package.benchmark_ref,
            TURBOQUANT_SERVED_GENERATION_BENCHMARK_REF
        );
        assert!(!contract.exactness_feature);
        assert!(contract
            .refused_publication_lanes
            .contains(&String::from("cpu_reference_exactness")));
        assert!(contract
            .refused_publication_lanes
            .contains(&String::from("tassadar_exactness")));
        assert!(contract.route_selection_rules.contains(&String::from(
            "exactness_sensitive_routes_must_refuse_turboquant"
        )));
        Ok(())
    }

    #[test]
    fn turboquant_benchmark_package_covers_required_lane_set() -> Result<(), Box<dyn Error>> {
        let contract = turboquant_served_generation_benchmark_contract("2026.03.24");
        let mut lanes = contract
            .lanes
            .iter()
            .map(|lane| lane.lane)
            .collect::<Vec<_>>();
        lanes.sort_by_key(|lane| lane.as_str());

        assert_eq!(
            lanes,
            vec![
                TurboQuantBenchmarkLaneKind::ContextGeneralization,
                TurboQuantBenchmarkLaneKind::Cost,
                TurboQuantBenchmarkLaneKind::ExactnessBoundary,
                TurboQuantBenchmarkLaneKind::Latency,
                TurboQuantBenchmarkLaneKind::LongContextQuality,
                TurboQuantBenchmarkLaneKind::Refusal,
            ]
        );
        assert!(contract.refusal_rules.contains(&String::from(
            "do_not_silently_fall_through_to_unsupported_kernels"
        )));
        assert!(contract.refusal_rules.contains(&String::from(
            "do_not_reuse_prefix_artifacts_across_incompatible_cache_encoding_signatures"
        )));
        Ok(())
    }
}
