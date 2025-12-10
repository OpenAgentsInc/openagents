//! HillClimber: MAP-based overnight optimization for Terminal-Bench.
//!
//! This crate implements the Modular Agentic Planner (MAP) architecture for
//! solving Terminal-Bench tasks using Apple's on-device Foundation Model.
//!
//! # Architecture
//!
//! ```text
//! MAP ORCHESTRATOR (coordinates everything)
//!   │
//!   ├── TASK DECOMPOSER (breaks into subtasks)
//!   ├── ACTOR (FM) (generates actions)
//!   ├── MONITOR (validates actions)
//!   └── EVALUATOR (tests and provides feedback)
//!       │
//!       ├── PARALLEL SAMPLER (test-time compute)
//!       └── DOCKER VERIFIER (runs pytest)
//! ```
//!
//! # Example
//!
//! ```no_run
//! use hillclimber::{HillClimberStore, HillClimberConfigInput};
//!
//! fn main() -> hillclimber::Result<()> {
//!     let store = HillClimberStore::open(".openagents/openagents.db")?;
//!
//!     // Ensure we have a config for the task
//!     let config = store.ensure_default_config("regex-log")?;
//!
//!     // Get stats
//!     let stats = store.get_stats()?;
//!     println!("Total runs: {}", stats.total_runs);
//!
//!     Ok(())
//! }
//! ```

pub mod decomposer;
pub mod error;
pub mod evaluator;
pub mod monitor;
pub mod orchestrator;
pub mod prompt;
pub mod runner;
pub mod sampler;
pub mod scoring;
pub mod store;
pub mod types;

// Re-export main types for convenience
pub use decomposer::{
    build_subtask_prompt, create_fallback_decomposition, decompose_task, get_current_subtask,
    is_subtask_complete,
};
pub use error::{HillClimberError, Result};
pub use evaluator::{
    evaluate_progress, format_for_prompt, generate_suggestion, is_docker_available,
    parse_pytest_output, quick_evaluate, run_docker_verification, run_local_pytest,
};
pub use monitor::{create_action_signature, is_same_action, monitor_action};
pub use orchestrator::{
    FMClient, HillClimberEmitter, MAPOrchestrator, NoopEmitter, ToolExecutor, WorkspaceExecutor,
};
pub use prompt::{build_fm_context, build_user_prompt, parse_fm_response, sanitize_for_fm, SYSTEM_PROMPT};
pub use runner::{create_task, load_task, FMBridgeAdapter, HillClimberRunner, RunOptions};
pub use sampler::{
    quick_sample, ParallelSampler, DEFAULT_CANDIDATE_COUNT, DEFAULT_TEMPERATURES,
};
pub use scoring::{
    format_run_summary, format_score, is_better_result, is_better_score, is_stable_for_export,
    score_result, EXPORT_THRESHOLD, MIN_CONSECUTIVE_PASSES, MIN_SCORE, PASS_BONUS, TURN_BASE,
};
pub use store::{hash_config, HillClimberStore};
pub use types::{
    // Config types
    BestConfig,
    HillClimberConfig,
    HillClimberConfigInput,
    HillClimberRun,
    HillClimberRunInput,
    // Execution types
    ActionContext,
    ActionResult,
    ExecutionState,
    FMAction,
    FMContext,
    MonitorDecision,
    // Evaluator types
    EvaluatorResult,
    FailureDetail,
    // Orchestrator types
    MAPOrchestratorOptions,
    MAPOrchestratorResult,
    StepDecision,
    // Task types
    Subtask,
    SubtaskState,
    SubtaskStatus,
    TaskDecomposition,
    TerminalBenchTask,
    VerificationConfig,
    // Sampling types
    CandidateResult,
    SamplingResult,
    // Stats types
    HillClimberStats,
    TaskStats,
    // Config change types
    ConfigChange,
    ConfigChangeType,
    // Helpers
    generate_run_id,
    generate_session_id,
};
