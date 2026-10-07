use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

const TASSADAR_SHARED_DEPTH_EXECUTOR_PUBLICATION_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_SHARED_DEPTH_EXECUTOR_CLAIM_CLASS: &str = "research_only_architecture";
pub const TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_depth_architecture_report.json";
pub const TASSADAR_SHARED_DEPTH_KERNEL_WORKLOAD_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_exactness_report.json";
pub const TASSADAR_SHARED_DEPTH_MODULE_WORKLOAD_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json";

/// Repo-facing status for the shared-depth executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedDepthExecutorPublicationStatus {
    /// The lane exists as an early research surface.
    ImplementedEarly,
}

/// Workload family used by the shared-depth executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedDepthWorkloadFamily {
    /// Loop-heavy bounded kernel traces.
    LoopHeavyKernel,
    /// Call-heavy bounded module traces.
    CallHeavyModule,
}

impl TassadarSharedDepthWorkloadFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoopHeavyKernel => "loop_heavy_kernel",
            Self::CallHeavyModule => "call_heavy_module",
        }
    }
}

/// Halting mode surfaced by the shared-depth lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedDepthHaltingMode {
    /// Shared-depth refinement under one fixed iteration budget.
    FixedIterationBudget,
    /// Shared-depth refinement with an explicit dynamic halting head.
    DynamicHalting,
}

/// One curriculum anchor for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthCurriculumAnchor {
    /// Stable stage identifier.
    pub stage_id: String,
    /// Human-readable stage summary.
    pub summary: String,
    /// Workload families trained or evaluated in the stage.
    pub workload_families: Vec<TassadarSharedDepthWorkloadFamily>,
    /// Halting modes active in the stage.
    pub halting_modes: Vec<TassadarSharedDepthHaltingMode>,
    /// Fixed later-window boundary for the stage.
    pub later_window_start_step: u16,
    /// Fixed refinement-budget cap for the stage.
    pub max_iteration_budget: u16,
    /// Fixed target-token cap for the stage.
    pub target_token_cap: u16,
}

/// Public repo-facing publication for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthExecutorPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value.
    pub status: TassadarSharedDepthExecutorPublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Stable model descriptor for the lane.
    pub model: ModelDescriptor,
    /// Stable baseline family refs used for comparison.
    pub baseline_family_refs: Vec<String>,
    /// Source workload refs for the lane.
    pub source_workload_refs: Vec<String>,
    /// Workload families targeted by the lane.
    pub workload_families: Vec<TassadarSharedDepthWorkloadFamily>,
    /// Curriculum anchors reused by the train lane.
    pub curriculum_anchors: Vec<TassadarSharedDepthCurriculumAnchor>,
    /// Maximum supported shared-depth refinement budget.
    pub max_shared_depth_steps: u16,
    /// Whether the lane surfaces explicit dynamic halting.
    pub supports_dynamic_halting: bool,
    /// Plain-language budget exhaustion posture.
    pub budget_exhaustion_posture: String,
    /// Stable implementation surfaces.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarSharedDepthExecutorPublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: TASSADAR_SHARED_DEPTH_EXECUTOR_PUBLICATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.shared_depth_executor.publication.v1"),
            status: TassadarSharedDepthExecutorPublicationStatus::ImplementedEarly,
            claim_class: String::from(TASSADAR_SHARED_DEPTH_EXECUTOR_CLAIM_CLASS),
            model: ModelDescriptor::new(
                "tassadar-shared-depth-executor-candidate-v0",
                "tassadar_shared_depth_executor",
                "v0",
            ),
            baseline_family_refs: vec![
                String::from("model-family://openagents/tassadar/flat_prefix_window_baseline"),
                String::from("model-family://openagents/tassadar/module_state_executor"),
            ],
            source_workload_refs: vec![
                String::from(TASSADAR_SHARED_DEPTH_KERNEL_WORKLOAD_REF),
                String::from(TASSADAR_SHARED_DEPTH_MODULE_WORKLOAD_REF),
            ],
            workload_families: vec![
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            ],
            curriculum_anchors: tassadar_shared_depth_curriculum_anchors(),
            max_shared_depth_steps: 160,
            supports_dynamic_halting: true,
            budget_exhaustion_posture: String::from(
                "budget exhaustion must stay explicit as a bounded refinement miss or refusal; the lane may not silently continue past the declared shared-depth budget",
            ),
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-research"),
            ],
            validation_refs: vec![String::from(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "the lane is research-only learned bounded architecture work and is not a served or benchmark-gated capability publication",
                ),
                String::from(
                    "shared-depth refinement here covers bounded loop-heavy kernel traces and bounded call-heavy module traces only; it does not imply arbitrary long-horizon learned exactness",
                ),
                String::from(
                    "dynamic halting is an explicit calibration surface with bounded refusal and degradation posture, not a claim of universal self-timed closure",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_shared_depth_executor_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the stable curriculum anchors for the shared-depth executor lane.
#[must_use]
pub fn tassadar_shared_depth_curriculum_anchors() -> Vec<TassadarSharedDepthCurriculumAnchor> {
    vec![
        TassadarSharedDepthCurriculumAnchor {
            stage_id: String::from("loop_kernel_refinement_bootstrap"),
            summary: String::from(
                "bootstrap shared-parameter recurrent refinement on loop-heavy kernels before mixing in call-heavy module traces",
            ),
            workload_families: vec![TassadarSharedDepthWorkloadFamily::LoopHeavyKernel],
            halting_modes: vec![TassadarSharedDepthHaltingMode::FixedIterationBudget],
            later_window_start_step: 16,
            max_iteration_budget: 128,
            target_token_cap: 96,
        },
        TassadarSharedDepthCurriculumAnchor {
            stage_id: String::from("call_trace_mix_with_fixed_budget"),
            summary: String::from(
                "mix call-heavy module traces under the same shared-depth block before enabling dynamic halting",
            ),
            workload_families: vec![
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            ],
            halting_modes: vec![TassadarSharedDepthHaltingMode::FixedIterationBudget],
            later_window_start_step: 24,
            max_iteration_budget: 144,
            target_token_cap: 128,
        },
        TassadarSharedDepthCurriculumAnchor {
            stage_id: String::from("dynamic_halting_calibration"),
            summary: String::from(
                "freeze shared-depth weights and calibrate an explicit halting head so budget exhaustion becomes measurable rather than implicit",
            ),
            workload_families: vec![
                TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
                TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            ],
            halting_modes: vec![
                TassadarSharedDepthHaltingMode::FixedIterationBudget,
                TassadarSharedDepthHaltingMode::DynamicHalting,
            ],
            later_window_start_step: 32,
            max_iteration_budget: 160,
            target_token_cap: 160,
        },
    ]
}

/// Returns the canonical public publication for the shared-depth executor lane.
#[must_use]
pub fn tassadar_shared_depth_executor_publication() -> TassadarSharedDepthExecutorPublication {
    TassadarSharedDepthExecutorPublication::new()
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
        tassadar_shared_depth_executor_publication, TassadarSharedDepthExecutorPublicationStatus,
        TassadarSharedDepthHaltingMode, TassadarSharedDepthWorkloadFamily,
    };

    #[test]
    fn shared_depth_executor_publication_is_machine_legible() {
        let publication = tassadar_shared_depth_executor_publication();

        assert_eq!(
            publication.status,
            TassadarSharedDepthExecutorPublicationStatus::ImplementedEarly
        );
        assert_eq!(publication.model.family, "tassadar_shared_depth_executor");
        assert!(publication.supports_dynamic_halting);
        assert!(!publication.publication_digest.is_empty());
    }

    #[test]
    fn shared_depth_executor_publication_carries_loop_and_call_workloads() {
        let publication = tassadar_shared_depth_executor_publication();

        assert!(publication
            .workload_families
            .contains(&TassadarSharedDepthWorkloadFamily::LoopHeavyKernel));
        assert!(publication
            .workload_families
            .contains(&TassadarSharedDepthWorkloadFamily::CallHeavyModule));
        let calibration_stage = publication
            .curriculum_anchors
            .iter()
            .find(|stage| stage.stage_id == "dynamic_halting_calibration")
            .expect("dynamic halting stage");
        assert!(calibration_stage
            .halting_modes
            .contains(&TassadarSharedDepthHaltingMode::DynamicHalting));
    }
}
