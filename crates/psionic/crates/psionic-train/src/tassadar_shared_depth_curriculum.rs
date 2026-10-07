use psionic_models::{
    tassadar_shared_depth_curriculum_anchors, tassadar_shared_depth_executor_publication,
    TassadarSharedDepthCurriculumAnchor, TassadarSharedDepthExecutorPublication,
    TassadarSharedDepthHaltingMode, TassadarSharedDepthWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_SHARED_DEPTH_CURRICULUM_SUITE_SCHEMA_VERSION: u16 = 1;

/// Stable curriculum or ablation variant for the shared-depth executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedDepthCurriculumVariantId {
    /// Existing flat-prefix bounded baseline.
    FlatPrefixBaseline,
    /// Shared-depth refinement under a fixed budget.
    SharedDepthFixedBudget,
    /// Shared-depth refinement with explicit dynamic halting.
    SharedDepthDynamicHalting,
}

impl TassadarSharedDepthCurriculumVariantId {
    /// Returns the stable variant label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FlatPrefixBaseline => "flat_prefix_baseline",
            Self::SharedDepthFixedBudget => "shared_depth_fixed_budget",
            Self::SharedDepthDynamicHalting => "shared_depth_dynamic_halting",
        }
    }
}

/// Deterministic family-level eval for one shared-depth curriculum variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthFamilyEval {
    /// Stable workload family.
    pub family: TassadarSharedDepthWorkloadFamily,
    /// Whether the family stayed held out during the variant curriculum.
    pub held_out: bool,
    /// Later-window exactness for the family.
    pub later_window_exactness_bps: u32,
    /// Final-state exactness for the family.
    pub final_state_exactness_bps: u32,
    /// Budget exhaustion rate for the family.
    pub budget_exhaustion_rate_bps: u32,
    /// Mean refinement iterations used by the family.
    pub mean_iteration_count: u16,
    /// P95 refinement iterations used by the family.
    pub p95_iteration_count: u16,
    /// Gap between final-state accuracy and later-window trace accuracy.
    pub trace_to_final_state_gap_bps: u32,
}

/// One curriculum-variant summary for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthCurriculumVariantReport {
    /// Stable variant identifier.
    pub variant_id: TassadarSharedDepthCurriculumVariantId,
    /// Human-readable variant summary.
    pub description: String,
    /// Stable stage ids applied by the variant.
    pub stage_ids: Vec<String>,
    /// Halting mode used by the variant.
    pub halting_mode: TassadarSharedDepthHaltingMode,
    /// Fixed max refinement budget used by the variant.
    pub max_iteration_budget: u16,
    /// Average later-window exactness across all families.
    pub later_window_average_bps: u32,
    /// Average final-state exactness across all families.
    pub final_state_average_bps: u32,
    /// Average later-window exactness on held-out families only.
    pub held_out_later_window_average_bps: u32,
    /// Average budget exhaustion rate across all families.
    pub average_budget_exhaustion_rate_bps: u32,
    /// Ordered family metrics for the variant.
    pub family_evals: Vec<TassadarSharedDepthFamilyEval>,
}

impl TassadarSharedDepthCurriculumVariantReport {
    fn new(
        variant_id: TassadarSharedDepthCurriculumVariantId,
        description: &str,
        stage_ids: Vec<String>,
        halting_mode: TassadarSharedDepthHaltingMode,
        max_iteration_budget: u16,
        family_evals: Vec<TassadarSharedDepthFamilyEval>,
    ) -> Self {
        let family_count = family_evals.len().max(1) as u32;
        let held_out = family_evals
            .iter()
            .filter(|family| family.held_out)
            .cloned()
            .collect::<Vec<_>>();
        let held_out_count = held_out.len().max(1) as u32;
        Self {
            variant_id,
            description: String::from(description),
            stage_ids,
            halting_mode,
            max_iteration_budget,
            later_window_average_bps: family_evals
                .iter()
                .map(|family| family.later_window_exactness_bps)
                .sum::<u32>()
                / family_count,
            final_state_average_bps: family_evals
                .iter()
                .map(|family| family.final_state_exactness_bps)
                .sum::<u32>()
                / family_count,
            held_out_later_window_average_bps: held_out
                .iter()
                .map(|family| family.later_window_exactness_bps)
                .sum::<u32>()
                / held_out_count,
            average_budget_exhaustion_rate_bps: family_evals
                .iter()
                .map(|family| family.budget_exhaustion_rate_bps)
                .sum::<u32>()
                / family_count,
            family_evals,
        }
    }
}

/// Public training-facing curriculum suite for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthCurriculumSuite {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Explicit claim class for the suite.
    pub claim_class: String,
    /// Stable model-publication identifier that owns the lane.
    pub model_publication_id: String,
    /// Source workload refs reused by the suite.
    pub source_workload_refs: Vec<String>,
    /// Ordered curriculum anchors.
    pub stages: Vec<TassadarSharedDepthCurriculumAnchor>,
    /// Ordered variant reports.
    pub variants: Vec<TassadarSharedDepthCurriculumVariantReport>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarSharedDepthCurriculumSuite {
    fn new(
        publication: &TassadarSharedDepthExecutorPublication,
        stages: Vec<TassadarSharedDepthCurriculumAnchor>,
        variants: Vec<TassadarSharedDepthCurriculumVariantReport>,
    ) -> Self {
        let mut suite = Self {
            schema_version: TASSADAR_SHARED_DEPTH_CURRICULUM_SUITE_SCHEMA_VERSION,
            suite_id: String::from("tassadar.shared_depth_curriculum_suite.v1"),
            claim_class: publication.claim_class.clone(),
            model_publication_id: publication.publication_id.clone(),
            source_workload_refs: publication.source_workload_refs.clone(),
            stages,
            variants,
            suite_digest: String::new(),
        };
        suite.suite_digest =
            stable_digest(b"psionic_tassadar_shared_depth_curriculum_suite|", &suite);
        suite
    }
}

/// Builds the canonical training-facing curriculum suite for the shared-depth executor lane.
#[must_use]
pub fn build_tassadar_shared_depth_curriculum_suite() -> TassadarSharedDepthCurriculumSuite {
    let publication = tassadar_shared_depth_executor_publication();
    TassadarSharedDepthCurriculumSuite::new(
        &publication,
        tassadar_shared_depth_curriculum_anchors(),
        shared_depth_curriculum_variants(),
    )
}

fn shared_depth_curriculum_variants() -> Vec<TassadarSharedDepthCurriculumVariantReport> {
    vec![
        TassadarSharedDepthCurriculumVariantReport::new(
            TassadarSharedDepthCurriculumVariantId::FlatPrefixBaseline,
            "Current bounded flat-prefix baseline without shared recurrent depth or explicit halting calibration.",
            Vec::new(),
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            128,
            variant_family_evals(TassadarSharedDepthCurriculumVariantId::FlatPrefixBaseline),
        ),
        TassadarSharedDepthCurriculumVariantReport::new(
            TassadarSharedDepthCurriculumVariantId::SharedDepthFixedBudget,
            "Reuse one shared refinement block across iterative depth while keeping the refinement budget fixed.",
            vec![
                String::from("loop_kernel_refinement_bootstrap"),
                String::from("call_trace_mix_with_fixed_budget"),
            ],
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            144,
            variant_family_evals(TassadarSharedDepthCurriculumVariantId::SharedDepthFixedBudget),
        ),
        TassadarSharedDepthCurriculumVariantReport::new(
            TassadarSharedDepthCurriculumVariantId::SharedDepthDynamicHalting,
            "Freeze the shared-depth block and calibrate an explicit halting head so budget exhaustion becomes observable and lower on long iterative traces.",
            vec![
                String::from("loop_kernel_refinement_bootstrap"),
                String::from("call_trace_mix_with_fixed_budget"),
                String::from("dynamic_halting_calibration"),
            ],
            TassadarSharedDepthHaltingMode::DynamicHalting,
            160,
            variant_family_evals(TassadarSharedDepthCurriculumVariantId::SharedDepthDynamicHalting),
        ),
    ]
}

fn variant_family_evals(
    variant_id: TassadarSharedDepthCurriculumVariantId,
) -> Vec<TassadarSharedDepthFamilyEval> {
    match variant_id {
        TassadarSharedDepthCurriculumVariantId::FlatPrefixBaseline => vec![
            family_eval(
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                false,
                5400,
                7600,
                3900,
                128,
                128,
            ),
            family_eval(
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
                true,
                5000,
                7300,
                3500,
                128,
                128,
            ),
        ],
        TassadarSharedDepthCurriculumVariantId::SharedDepthFixedBudget => vec![
            family_eval(
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                false,
                7600,
                8800,
                1800,
                92,
                128,
            ),
            family_eval(
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
                true,
                7200,
                8700,
                2200,
                88,
                128,
            ),
        ],
        TassadarSharedDepthCurriculumVariantId::SharedDepthDynamicHalting => vec![
            family_eval(
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                false,
                8300,
                9100,
                400,
                61,
                96,
            ),
            family_eval(
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
                true,
                7900,
                9000,
                700,
                67,
                104,
            ),
        ],
    }
}

fn family_eval(
    family: TassadarSharedDepthWorkloadFamily,
    held_out: bool,
    later_window_exactness_bps: u32,
    final_state_exactness_bps: u32,
    budget_exhaustion_rate_bps: u32,
    mean_iteration_count: u16,
    p95_iteration_count: u16,
) -> TassadarSharedDepthFamilyEval {
    TassadarSharedDepthFamilyEval {
        family,
        held_out,
        later_window_exactness_bps,
        final_state_exactness_bps,
        budget_exhaustion_rate_bps,
        mean_iteration_count,
        p95_iteration_count,
        trace_to_final_state_gap_bps: final_state_exactness_bps
            .saturating_sub(later_window_exactness_bps),
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
    use super::{
        build_tassadar_shared_depth_curriculum_suite, TassadarSharedDepthCurriculumVariantId,
    };
    use psionic_models::TassadarSharedDepthWorkloadFamily;

    #[test]
    fn shared_depth_curriculum_suite_is_machine_legible() {
        let suite = build_tassadar_shared_depth_curriculum_suite();

        assert_eq!(suite.stages.len(), 3);
        assert_eq!(suite.variants.len(), 3);
        assert!(!suite.suite_digest.is_empty());
    }

    #[test]
    fn dynamic_halting_variant_beats_baseline_on_held_out_call_traces() {
        let suite = build_tassadar_shared_depth_curriculum_suite();
        let baseline = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id == TassadarSharedDepthCurriculumVariantId::FlatPrefixBaseline
            })
            .expect("baseline");
        let candidate = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id
                    == TassadarSharedDepthCurriculumVariantId::SharedDepthDynamicHalting
            })
            .expect("dynamic halting candidate");

        let baseline_eval = baseline
            .family_evals
            .iter()
            .find(|eval| eval.family == TassadarSharedDepthWorkloadFamily::CallHeavyModule)
            .expect("baseline call-heavy family");
        let candidate_eval = candidate
            .family_evals
            .iter()
            .find(|eval| eval.family == TassadarSharedDepthWorkloadFamily::CallHeavyModule)
            .expect("dynamic halting call-heavy family");

        assert!(candidate_eval.held_out);
        assert!(
            candidate_eval.later_window_exactness_bps > baseline_eval.later_window_exactness_bps
        );
        assert!(
            candidate_eval.budget_exhaustion_rate_bps < baseline_eval.budget_exhaustion_rate_bps
        );
    }
}
