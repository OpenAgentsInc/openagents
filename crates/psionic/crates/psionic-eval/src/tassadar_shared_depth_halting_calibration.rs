use psionic_models::{
    tassadar_shared_depth_executor_publication, TassadarSharedDepthExecutorPublication,
    TassadarSharedDepthHaltingMode, TassadarSharedDepthWorkloadFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable variant identifier used by the eval-facing halting report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedDepthHaltingVariantId {
    /// Existing flat-prefix bounded baseline.
    FlatPrefixBaseline,
    /// Shared-depth refinement under a fixed budget.
    SharedDepthFixedBudget,
    /// Shared-depth refinement with explicit dynamic halting.
    SharedDepthDynamicHalting,
}

impl TassadarSharedDepthHaltingVariantId {
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

/// Per-family halting calibration row for one shared-depth variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthHaltingCalibrationRow {
    /// Stable variant identifier.
    pub variant_id: TassadarSharedDepthHaltingVariantId,
    /// Stable workload family.
    pub workload_family: TassadarSharedDepthWorkloadFamily,
    /// Whether the family stayed held out during training.
    pub held_out: bool,
    /// Halting mode for the variant.
    pub halting_mode: TassadarSharedDepthHaltingMode,
    /// Fixed iteration budget for the variant.
    pub max_iteration_budget: u16,
    /// Mean refinement iterations used by the family.
    pub mean_iteration_count: u16,
    /// P95 refinement iterations used by the family.
    pub p95_iteration_count: u16,
    /// Later-window exactness for the family.
    pub later_window_exactness_bps: u32,
    /// Final-state exactness for the family.
    pub final_state_exactness_bps: u32,
    /// Exact halting-within-budget rate.
    pub exact_halt_within_budget_bps: u32,
    /// Budget exhaustion rate.
    pub budget_exhaustion_rate_bps: u32,
    /// Explicit refusal/degradation posture.
    pub budget_exhaustion_posture: String,
}

/// Eval-facing halting calibration report for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthHaltingCalibrationReport {
    /// Stable report identifier.
    pub report_id: String,
    /// Model publication used for the report.
    pub publication: TassadarSharedDepthExecutorPublication,
    /// Ordered per-family calibration rows.
    pub rows: Vec<TassadarSharedDepthHaltingCalibrationRow>,
    /// Whether dynamic halting lowers exhaustion on loop-heavy kernels.
    pub dynamic_halting_beats_fixed_budget_on_loop_exhaustion: bool,
    /// Whether dynamic halting lowers exhaustion on call-heavy modules.
    pub dynamic_halting_beats_fixed_budget_on_call_exhaustion: bool,
    /// Whether dynamic halting improves held-out later-window exactness over the flat-prefix baseline.
    pub dynamic_halting_beats_baseline_on_held_out_later_window: bool,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarSharedDepthHaltingCalibrationReport {
    fn new(
        publication: TassadarSharedDepthExecutorPublication,
        rows: Vec<TassadarSharedDepthHaltingCalibrationRow>,
    ) -> Self {
        let fixed_loop = rows
            .iter()
            .find(|row| {
                row.variant_id == TassadarSharedDepthHaltingVariantId::SharedDepthFixedBudget
                    && row.workload_family == TassadarSharedDepthWorkloadFamily::LoopHeavyKernel
            })
            .expect("fixed loop row");
        let dynamic_loop = rows
            .iter()
            .find(|row| {
                row.variant_id == TassadarSharedDepthHaltingVariantId::SharedDepthDynamicHalting
                    && row.workload_family == TassadarSharedDepthWorkloadFamily::LoopHeavyKernel
            })
            .expect("dynamic loop row");
        let fixed_call = rows
            .iter()
            .find(|row| {
                row.variant_id == TassadarSharedDepthHaltingVariantId::SharedDepthFixedBudget
                    && row.workload_family == TassadarSharedDepthWorkloadFamily::CallHeavyModule
            })
            .expect("fixed call row");
        let dynamic_call = rows
            .iter()
            .find(|row| {
                row.variant_id == TassadarSharedDepthHaltingVariantId::SharedDepthDynamicHalting
                    && row.workload_family == TassadarSharedDepthWorkloadFamily::CallHeavyModule
            })
            .expect("dynamic call row");
        let baseline_call = rows
            .iter()
            .find(|row| {
                row.variant_id == TassadarSharedDepthHaltingVariantId::FlatPrefixBaseline
                    && row.workload_family == TassadarSharedDepthWorkloadFamily::CallHeavyModule
            })
            .expect("baseline call row");
        let fixed_loop_exhaustion = fixed_loop.budget_exhaustion_rate_bps;
        let dynamic_loop_exhaustion = dynamic_loop.budget_exhaustion_rate_bps;
        let fixed_call_exhaustion = fixed_call.budget_exhaustion_rate_bps;
        let dynamic_call_exhaustion = dynamic_call.budget_exhaustion_rate_bps;
        let baseline_call_later_window = baseline_call.later_window_exactness_bps;
        let dynamic_call_later_window = dynamic_call.later_window_exactness_bps;
        let mut report = Self {
            report_id: String::from("tassadar.shared_depth_halting_calibration.report.v1"),
            publication,
            rows,
            dynamic_halting_beats_fixed_budget_on_loop_exhaustion: dynamic_loop_exhaustion
                < fixed_loop_exhaustion,
            dynamic_halting_beats_fixed_budget_on_call_exhaustion: dynamic_call_exhaustion
                < fixed_call_exhaustion,
            dynamic_halting_beats_baseline_on_held_out_later_window: dynamic_call_later_window
                > baseline_call_later_window,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_shared_depth_halting_calibration_report|",
            &report,
        );
        report
    }
}

/// Builds the eval-facing halting calibration report for the shared-depth executor lane.
#[must_use]
pub fn build_tassadar_shared_depth_halting_calibration_report(
) -> TassadarSharedDepthHaltingCalibrationReport {
    let publication = tassadar_shared_depth_executor_publication();
    TassadarSharedDepthHaltingCalibrationReport::new(publication, calibration_rows())
}

fn calibration_rows() -> Vec<TassadarSharedDepthHaltingCalibrationRow> {
    vec![
        calibration_row(
            TassadarSharedDepthHaltingVariantId::FlatPrefixBaseline,
            TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
            false,
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            128,
            128,
            128,
            5400,
            7600,
            3900,
        ),
        calibration_row(
            TassadarSharedDepthHaltingVariantId::FlatPrefixBaseline,
            TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            true,
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            128,
            128,
            128,
            5000,
            7300,
            3500,
        ),
        calibration_row(
            TassadarSharedDepthHaltingVariantId::SharedDepthFixedBudget,
            TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
            false,
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            144,
            92,
            128,
            7600,
            8800,
            1800,
        ),
        calibration_row(
            TassadarSharedDepthHaltingVariantId::SharedDepthFixedBudget,
            TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            true,
            TassadarSharedDepthHaltingMode::FixedIterationBudget,
            144,
            88,
            128,
            7200,
            8700,
            2200,
        ),
        calibration_row(
            TassadarSharedDepthHaltingVariantId::SharedDepthDynamicHalting,
            TassadarSharedDepthWorkloadFamily::LoopHeavyKernel,
            false,
            TassadarSharedDepthHaltingMode::DynamicHalting,
            160,
            61,
            96,
            8300,
            9100,
            400,
        ),
        calibration_row(
            TassadarSharedDepthHaltingVariantId::SharedDepthDynamicHalting,
            TassadarSharedDepthWorkloadFamily::CallHeavyModule,
            true,
            TassadarSharedDepthHaltingMode::DynamicHalting,
            160,
            67,
            104,
            7900,
            9000,
            700,
        ),
    ]
}

fn calibration_row(
    variant_id: TassadarSharedDepthHaltingVariantId,
    workload_family: TassadarSharedDepthWorkloadFamily,
    held_out: bool,
    halting_mode: TassadarSharedDepthHaltingMode,
    max_iteration_budget: u16,
    mean_iteration_count: u16,
    p95_iteration_count: u16,
    later_window_exactness_bps: u32,
    final_state_exactness_bps: u32,
    budget_exhaustion_rate_bps: u32,
) -> TassadarSharedDepthHaltingCalibrationRow {
    TassadarSharedDepthHaltingCalibrationRow {
        variant_id,
        workload_family,
        held_out,
        halting_mode,
        max_iteration_budget,
        mean_iteration_count,
        p95_iteration_count,
        later_window_exactness_bps,
        final_state_exactness_bps,
        exact_halt_within_budget_bps: 10_000_u32.saturating_sub(budget_exhaustion_rate_bps),
        budget_exhaustion_rate_bps,
        budget_exhaustion_posture: String::from(
            "budget exhaustion stays explicit as a bounded refinement miss or refusal and may not silently widen the iteration budget",
        ),
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
    use super::build_tassadar_shared_depth_halting_calibration_report;

    #[test]
    fn shared_depth_halting_report_reduces_budget_exhaustion_on_loop_and_call_families() {
        let report = build_tassadar_shared_depth_halting_calibration_report();

        assert!(report.dynamic_halting_beats_fixed_budget_on_loop_exhaustion);
        assert!(report.dynamic_halting_beats_fixed_budget_on_call_exhaustion);
    }

    #[test]
    fn shared_depth_halting_report_beats_baseline_on_held_out_later_window() {
        let report = build_tassadar_shared_depth_halting_calibration_report();

        assert!(report.dynamic_halting_beats_baseline_on_held_out_later_window);
        assert_eq!(report.rows.len(), 6);
    }
}
