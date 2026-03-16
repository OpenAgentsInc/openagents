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

pub mod arbiter;
pub mod dsl;
pub mod interactive;
pub mod interactive_adcr;
pub mod interactive_baselines;
pub mod interactive_context;
pub mod interactive_eval;
pub mod interactive_parity;
pub mod interactive_receipts;
pub mod interpreter;
pub mod mdl;
pub mod model;
pub mod recursive;
pub mod symbolic;
pub mod trace;
pub mod transductive;
pub mod verifier;

pub use arbiter::{
    PORTFOLIO_ARBITER_BOUNDARY_SUMMARY, PortfolioArbiter, PortfolioArbiterCandidateScore,
    PortfolioArbiterConfig, PortfolioArbiterError, PortfolioArbiterRun, SecondAttemptCandidateGate,
    SecondAttemptPolicyOutcome,
};
pub use dsl::{
    ARC_SOLVER_BOUNDARY_SUMMARY, ArcDslTier, ArcGridBinding, ArcGridExpr, ArcObjectSelector,
    ArcObjectTransform, ArcProgram, ArcProgramMetadata, ArcSymbol, ArcSymbolError,
};
pub use interactive::{
    ArcInteractiveAgent, ArcInteractiveAgentDefinition, ArcInteractiveAgentError,
    ArcInteractiveAgentRegistry, ArcInteractiveCheckpointHandoff, ArcInteractiveEnvironment,
    ArcInteractiveEnvironmentKind, ArcInteractiveGameStep, ArcInteractiveRunArtifacts,
    ArcInteractiveRunner, ArcInteractiveRunnerConfig, ArcInteractiveRunnerConfigError,
    ArcInteractiveRunnerError, ArcInteractiveSessionContext, INTERACTIVE_RUNNER_BOUNDARY_SUMMARY,
};
pub use interactive_adcr::{
    ArcAdcrAnalysis, ArcAdcrBaselineAgent, ArcAdcrConfig, ArcAdcrConfigError, ArcAdcrHumanAction,
    ArcAdcrHumanActionError, ArcAdcrMode, ArcAdcrProgressDelta, ArcAdcrPromptTemplates,
    ArcAdcrReplayFallbackPolicy, ArcAdcrReplayProgram, ArcAdcrReplayProgramError,
    INTERACTIVE_ADCR_BOUNDARY_SUMMARY,
};
pub use interactive_baselines::{
    ArcRandomBaselineAgent, ArcRandomBaselineConfig, ArcRandomBaselineConfigError,
    ArcScriptedBaselineAgent, ArcScriptedBaselineConfigError, ArcScriptedBaselineProgram,
    ArcScriptedFallbackPolicy, INTERACTIVE_BASELINES_BOUNDARY_SUMMARY,
};
pub use interactive_context::{
    ArcInteractiveContextCheckpointState, ArcInteractiveContextFrame,
    ArcInteractiveContextPolicyError, ArcInteractiveContextRetentionPolicy,
    ArcInteractiveMemoryEntry, ArcInteractiveProgressState, ArcInteractivePromptPlan,
    ArcInteractivePromptPolicy, ArcInteractivePromptResumeSummary, ArcInteractivePromptSection,
    ArcInteractivePromptSectionView, ArcInteractiveResumeContextMode, ArcInteractiveSessionMemory,
    INTERACTIVE_CONTEXT_BOUNDARY_SUMMARY,
};
pub use interactive_eval::{
    ArcInteractiveEvalAggregate, ArcInteractiveEvalCaseResult, ArcInteractiveEvalConfig,
    ArcInteractiveEvalError, ArcInteractiveEvalRound, ArcInteractiveEvalRoundSummary,
    INTERACTIVE_EVAL_BOUNDARY_SUMMARY, run_repeated_interactive_eval,
};
pub use interactive_parity::{
    ArcInteractiveRunnerExpectedDifference, ArcInteractiveRunnerExpectedDifferenceField,
    ArcInteractiveRunnerParityField, ArcInteractiveRunnerParityMismatch,
    ArcInteractiveRunnerParityOutcome, ArcInteractiveRunnerParityReport,
    compare_interactive_run_artifacts,
};
pub use interactive_receipts::{
    ArcInteractiveReplayLocator, ArcInteractiveScoreDelta, ArcInteractiveTrajectoryBundle,
    ArcInteractiveTrajectoryExport, ArcInteractiveTrajectoryExportError,
    ArcInteractiveTrajectoryTurn, INTERACTIVE_RECEIPTS_BOUNDARY_SUMMARY,
    arc_interactive_environment_package,
};
pub use interpreter::{ArcInterpreter, ArcInterpreterError};
pub use mdl::{
    ArcMdlCandidateReport, ArcMdlInitializationMode, ArcMdlLane, ArcMdlLaneConfig, ArcMdlLaneError,
    ArcMdlLaneRun, ArcMdlRepresentation, MDL_LANE_BOUNDARY_SUMMARY, MDL_LANE_ID,
};
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
pub use recursive::{
    ArcRecursiveTinyModelBootstrap, ArcRecursiveTinyModelBootstrapMode,
    ArcRecursiveTinyModelConfig, ArcRecursiveTinyModelLane, ArcRecursiveTinyModelLaneError,
    ArcRecursiveTinyModelLaneRun, ArcRecursiveTinyModelState, ArcRecursiveTinyModelStepDecision,
    ArcRecursiveTinyModelStepOutput, ArcRecursiveTinyModelStepTrace,
    ArcRecursiveTinyModelTracePhase, ArcTinyModel, RECURSIVE_TINY_MODEL_BOUNDARY_SUMMARY,
    RECURSIVE_TINY_MODEL_LANE_ID,
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
pub use transductive::{
    ArcLocalModelAdapter, ArcTransductiveAdapterRequest, ArcTransductiveAdapterResponse,
    ArcTransductiveLane, ArcTransductiveLaneConfig, ArcTransductiveLaneError,
    ArcTransductiveLaneRun, ArcTransductivePrompt, PsionicTextGenerationAdapter,
    TRANSDUCTIVE_LANE_BOUNDARY_SUMMARY, TRANSDUCTIVE_LANE_ID,
};
pub use verifier::{
    ArcCommonVerifier, ArcVerifierAugmentation, ArcVerifierConfig, ArcVerifierError,
    CandidateVerifier, FalsificationCheckKind, FalsificationCheckResult, FalsificationCheckStatus,
    PairVerificationResult, VERIFIER_BOUNDARY_SUMMARY, VerificationReport,
};

/// Stable role summary for downstream ARC crates.
pub const CRATE_ROLE: &str =
    "ARC solver DSL, pure interpreter, and later lane/verifier/arbiter substrate";
