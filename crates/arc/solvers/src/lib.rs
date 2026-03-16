#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]
//! ARC solver-domain crate owning the typed DSL and pure interpreter seed.
//!
//! `arc-solvers` sits above shared ARC contracts and below later verifier,
//! lane, and arbiter policy. It must not absorb benchmark scoring truth, app
//! UX, or reusable Psionic substrate.

pub mod dsl;
pub mod interpreter;
pub mod model;
pub mod symbolic;
pub mod trace;
pub mod verifier;

pub use dsl::{
    ARC_SOLVER_BOUNDARY_SUMMARY, ArcDslTier, ArcGridBinding, ArcGridExpr, ArcObjectSelector,
    ArcObjectTransform, ArcProgram, ArcProgramMetadata, ArcSymbol, ArcSymbolError,
};
pub use interpreter::{ArcInterpreter, ArcInterpreterError};
pub use model::{
    ArcDigest, ArcDigestError, BudgetCounterDelta, BudgetCounterKind, BudgetCounterSummary,
    BudgetLedger, BudgetLedgerError, CandidateDeduplicationDecision, CandidateDeduplicationStatus,
    CandidateDeduplicator, CandidateIdentity, CandidateIdentityError, Hypothesis, HypothesisError,
    HypothesisId, HypothesisKind, PlannedActionStep, RefusalEnvelope, RefusalEnvelopeError,
    SOLVER_MODEL_BOUNDARY_SUMMARY, SecondAttemptDistinctness, SecondAttemptDistinctnessField,
    SolveAttemptEnvelope, SolveAttemptEnvelopeError, SolveAttemptStatus,
    SolveAttemptVerificationSummary, SolverIdError, SolverLaneId, SolverPhase, SolverRefusalCode,
    TaskBudget,
};
pub use symbolic::{
    SYMBOLIC_LANE_BOUNDARY_SUMMARY, SYMBOLIC_LANE_ID, SymbolicLane, SymbolicLaneConfig,
    SymbolicLaneError, SymbolicLaneRun, SymbolicOutputTransformRepair, SymbolicRepairAttempt,
    SymbolicRepairOperator, SymbolicSeedTemplate,
};
pub use trace::{
    ArbiterDecision, LaneBatchStatus, LaneProposalBatch, ProposalPhase, SolverTraceBundle,
    TRACE_BUNDLE_BOUNDARY_SUMMARY, TraceBundleError, TraceBundleManifest, TraceBundleReplayReport,
    TracedLaneProposal, read_trace_bundle_json_file, replay_trace_bundle,
    write_trace_bundle_json_file,
};
pub use verifier::{
    ArcCommonVerifier, ArcVerifierAugmentation, ArcVerifierConfig, ArcVerifierError,
    CandidateVerifier, FalsificationCheckKind, FalsificationCheckResult, FalsificationCheckStatus,
    PairVerificationResult, VERIFIER_BOUNDARY_SUMMARY, VerificationReport,
};

/// Stable role summary for downstream ARC crates.
pub const CRATE_ROLE: &str =
    "ARC solver DSL, pure interpreter, and later lane/verifier/arbiter substrate";
