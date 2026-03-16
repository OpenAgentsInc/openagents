#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC evaluator-first ML layer over benchmark truth.
//!
//! `arc-ml` sits above ARC benchmark/runtime contracts and below any future
//! train-class ARC model work. The first retained slice is evaluator-first:
//! typed practice suites, synthetic ARC-AGI-3-style attempt evaluation, and
//! `pass@k` aggregation over benchmark-owned run reports.

mod eval;

pub use eval::{
    estimate_pass_at_k, evaluate_interactive_practice_suite, ArcAggregatePassAtK,
    ArcInteractivePracticeAttempt, ArcInteractivePracticeAttemptReport, ArcInteractivePracticeCase,
    ArcInteractivePracticeCaseReport, ArcInteractivePracticeReport, ArcInteractivePracticeSuite,
    ArcMlDataProvenance, ArcMlEvalError, ArcPassAtKCaseEstimate, ARC_ML_BOUNDARY_SUMMARY,
};

/// Human-readable ownership summary for this crate.
pub const CRATE_ROLE: &str =
    "ARC evaluator-first ML practice layer and later ARC-specific model work over Psionic";
