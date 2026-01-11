// Core agent module - works on both native and WASM
pub mod agent;
pub mod deprecation;

// Browser entry point - WASM only
#[cfg(feature = "wasm")]
pub mod browser;

// Native-only modules
#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod checkpoint;
#[cfg(not(target_arch = "wasm32"))]
pub mod ci;
#[cfg(not(target_arch = "wasm32"))]
pub mod claude;
#[cfg(not(target_arch = "wasm32"))]
pub mod github;
#[cfg(not(target_arch = "wasm32"))]
pub mod logger;
#[cfg(not(target_arch = "wasm32"))]
pub mod preflight;
#[cfg(not(target_arch = "wasm32"))]
pub mod pylon_integration;
#[cfg(not(target_arch = "wasm32"))]
pub mod replay;
#[cfg(not(target_arch = "wasm32"))]
pub mod report;
#[cfg(not(target_arch = "wasm32"))]
pub mod startup;
#[cfg(not(target_arch = "wasm32"))]
pub mod streaming;
#[cfg(not(target_arch = "wasm32"))]
pub mod utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod verification;
#[cfg(not(target_arch = "wasm32"))]
pub mod workflow;

// DSPy modules (native-only)
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_execution;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_hub;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_optimization;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_planning;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_router;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_training;
#[cfg(not(target_arch = "wasm32"))]
pub mod dspy_verify;

// Native-only re-exports
#[cfg(not(target_arch = "wasm32"))]
pub use auth::{
    AuthEntry, AuthStatus, AuthStore, check_openagents_auth, check_opencode_auth,
    copy_opencode_auth, get_provider_auth, has_anthropic_auth, openagents_auth_path,
    opencode_auth_path,
};

#[cfg(not(target_arch = "wasm32"))]
pub use ci::{CICheckResult, CIClient, CIStatus, CISystem, CheckDetail, detect_test_command};

#[cfg(not(target_arch = "wasm32"))]
pub use claude::{
    ClaudeEvent, ClaudeToken, ClaudeUsageData, run_claude_execution, run_claude_planning,
    run_claude_review,
};

#[cfg(not(target_arch = "wasm32"))]
pub use logger::{LogEntry, SessionLogger, generate_session_id};

#[cfg(not(target_arch = "wasm32"))]
pub use github::{
    ConnectedRepo, GitHubClient, GitHubOAuthConfig, GitHubToken, RepoPermissions,
    branch_name_for_issue, check_github_auth, github_token_path, load_github_token,
    save_github_token,
};

#[cfg(not(target_arch = "wasm32"))]
pub use preflight::{
    AuthInfo, ComputeMix, GitInfo, InferenceInfo, PreflightConfig, ProjectInfo, ProviderAuth,
    PylonInfo, SwarmProvider, ToolInfo, ToolsInfo, run_preflight,
};

#[cfg(not(target_arch = "wasm32"))]
pub use pylon_integration::{
    check_pylon_running, discover_swarm_providers, get_pylon_status, start_pylon,
};

#[cfg(not(target_arch = "wasm32"))]
pub use startup::{ClaudeModel, LogLine, LogStatus, StartupPhase, StartupSection, StartupState};

#[cfg(not(target_arch = "wasm32"))]
pub use streaming::query_issue_summary;

#[cfg(not(target_arch = "wasm32"))]
pub use utils::{sanitize_text, shorten_path, wrap_text};

#[cfg(not(target_arch = "wasm32"))]
pub use workflow::{GitHubWorkflow, IssueWorkflowResult, WorkflowReceipt};

#[cfg(not(target_arch = "wasm32"))]
pub use verification::{
    CheckResult, TerminationChecklist, VerificationRunner, generate_fix_prompt, should_force_stop,
};

#[cfg(not(target_arch = "wasm32"))]
pub use replay::{ReplayBundle, ReplayMetadata, ReplayReceipts, TimelineEvent, redact_replay};

#[cfg(not(target_arch = "wasm32"))]
pub use report::{
    AfterActionReport, SessionStats, collect_session_stats, generate_questions_for_user,
    generate_suggested_next_steps,
};

#[cfg(not(target_arch = "wasm32"))]
pub use checkpoint::{CHECKPOINT_VERSION, SessionCheckpoint, SessionSummary};

// DSPy exports (native-only)
#[cfg(not(target_arch = "wasm32"))]
pub use dspy_execution::{
    ExecutionAction, ExecutionDecision, ExecutionInput, ExecutionPipeline, RecoveryInput,
    RecoveryResult, RecoveryStrategy, ToolSelectionInput, ToolSelectionResult,
};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_optimization::{
    ExecutionExample, OptimizationDataset, OptimizerType, PlanningExample, VerificationExample,
    execution_metric, planning_metric, verification_metric,
};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_planning::{Complexity, PlanningInput, PlanningPipeline, PlanningResult};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_verify::{
    ExecutionReviewInput, ExecutionReviewResult, PlanAdherence, RequirementResult, ReviewVerdict,
    VerificationInput, VerificationPipeline, VerificationResult, VerificationVerdict,
};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_hub::{DspyHub, RoutingStrategy, StoredModule};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_router::{RoutingDecision, RoutingSummary, ShadowStats, SignatureRouter};

#[cfg(not(target_arch = "wasm32"))]
pub use dspy_training::{ExtractedExamples, SavedTrainingPaths, SuccessCriteria, TrainingExtractor};

// Agent exports - available on all platforms
pub use agent::{AutopilotAgent, AutopilotConfig, AutopilotPhase, AutopilotState};

// Browser exports - WASM only
#[cfg(feature = "wasm")]
pub use browser::{init_runtime, list_agents, spawn_autopilot, spawn_autopilot_with_config, tick_agent};

pub const ACP_PHASE_META_KEY: &str = "openagents_phase";
pub const ACP_SESSION_META_KEY: &str = "openagents_session_id";
pub const ACP_TOOL_PROGRESS_META_KEY: &str = "openagents_tool_progress_secs";
pub const ACP_TOOL_NAME_META_KEY: &str = "openagents_tool_name";
