#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC benchmark scoring and benchmark-facing report contracts.
//!
//! `arc-benchmark` owns benchmark truth for ARC-specific scoring and reporting.
//! The first landed slices are exact-match static scoring for ARC-AGI-1 and
//! ARC-AGI-2 plus bounded interactive RHAE scoring for deterministic
//! ARC-AGI-3 recordings. Scorecard lifecycle policy, checkpoints, resume
//! behavior, and JSONL parity remain follow-on work.

mod checkpoint;
mod exact_match;
mod interactive;

pub use checkpoint::{
    ArcBenchmarkUsageTotals, ArcCheckpointErrorRecord, ArcInteractiveCheckpointBundle,
    ArcInteractiveCheckpointMetadata, ArcRunManifest, ArcRunManifestManager, ArcRunTaskProgress,
    ArcRunTaskStatus, ArcTaskAttemptCheckpoint, ArcTaskCheckpoint, ArcTaskCheckpointManager,
};
pub use exact_match::{
    ArcBenchmarkError, ArcExactMatchAttemptReport, ArcExactMatchBenchmarkSummary,
    ArcExactMatchPairReport, ArcExactMatchTaskReport, ArcStaticAnswerKey, ArcStaticPairSubmission,
    ArcStaticTaskSubmission, score_exact_match_task,
};
pub use interactive::{
    ArcInteractiveRunReport, ArcInteractiveStepSummary, score_interactive_recording,
};

/// Human-readable ownership summary for this crate.
pub const CRATE_ROLE: &str =
    "ARC benchmark scoring, score summaries, and benchmark-facing report contracts";

/// Stable boundary summary for downstream ARC crates.
pub const BENCHMARK_BOUNDARY_SUMMARY: &str = "arc-benchmark owns exact-match and interactive scoring truth, benchmark reports, scorecards, recordings, checkpoints, and run-manifest policy";
