#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! Shared ARC contracts for tasks, analysis views, and solver-facing envelopes.
//!
//! `arc-core` owns ARC-specific schema and value semantics that every later
//! `crates/arc/*` package needs to share. It must stay below benchmark, client,
//! solver, and ARC-ML policy, and it must not absorb reusable Psionic substrate.

pub mod analysis;
pub mod envelopes;
pub mod schema;

pub use analysis::{
    ANALYSIS_BOUNDARY_SUMMARY, ArcBoundingBox, GridAnalysisSummary, canonical_palette,
    summarize_grid,
};
pub use envelopes::{
    ArcRefusalCode, ArcSolveOutcome, ArcSolveRefusal, ArcSolveResultEnvelope,
    EXECUTION_ENVELOPE_BOUNDARY_SUMMARY, SolveBudget,
};
pub use schema::{
    ARC_ACTION6_COORDINATE_MAX, ARC_CORE_SCHEMA_VERSION, ARC_FRAME_MAX_EDGE, ARC_GRID_MAX_EDGE,
    ARC_PALETTE_SIZE, ArcAction, ArcActionError, ArcBenchmark, ArcEpisodeStep, ArcExample,
    ArcFrameData, ArcFrameDataError, ArcGrid, ArcGridError, ArcLevelScore, ArcObservation,
    ArcRecording, ArcRecordingError, ArcScorecard, ArcScorecardMetadata, ArcTask, ArcTaskError,
    ArcTaskId, ArcTaskIdError, SCHEMA_BOUNDARY_SUMMARY,
};

/// Stable internal layers that downstream ARC crates are allowed to build on.
pub const ARC_CORE_LAYER_BOUNDARIES: [&str; 3] = ["schema", "analysis", "envelopes"];

#[cfg(test)]
mod tests {
    use crate::{ARC_CORE_LAYER_BOUNDARIES, ArcAction, ArcGrid, ArcTaskId};

    #[test]
    fn arc_task_id_normalizes_and_validates() {
        let task_id = ArcTaskId::new("  demo-task-01  ").expect("task id should normalize");
        assert_eq!(task_id.as_str(), "demo-task-01");
        assert!(ArcTaskId::new("   ").is_err());
        assert!(ArcTaskId::new("demo task").is_err());
    }

    #[test]
    fn arc_core_layer_boundaries_are_explicit() {
        assert_eq!(
            ARC_CORE_LAYER_BOUNDARIES,
            ["schema", "analysis", "envelopes"]
        );
    }

    #[test]
    fn arc_grid_rejects_invalid_shapes() {
        let invalid = ArcGrid::new(2, 2, vec![0, 1, 2]);
        assert!(invalid.is_err());
    }

    #[test]
    fn action6_rejects_out_of_bounds_coordinates() {
        let invalid = ArcAction::action6(64, 0);
        assert!(invalid.is_err());
    }
}
