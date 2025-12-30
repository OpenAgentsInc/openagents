pub mod auth;
pub mod checkpoint;
pub mod ci;
pub mod claude;
pub mod deprecation;
pub mod github;
pub mod logger;
pub mod preflight;
pub mod pylon_integration;
pub mod replay;
pub mod report;
pub mod startup;
pub mod streaming;
pub mod utils;
pub mod verification;
pub mod workflow;

pub use auth::{
    AuthEntry, AuthStatus, AuthStore,
    check_openagents_auth, check_opencode_auth, copy_opencode_auth,
    get_provider_auth, has_anthropic_auth,
    openagents_auth_path, opencode_auth_path,
};

pub use ci::{
    CIClient, CISystem, CIStatus, CICheckResult, CheckDetail,
    detect_test_command,
};

pub use claude::{ClaudeToken, ClaudeEvent, run_claude_planning, run_claude_execution, run_claude_review};

pub use logger::{SessionLogger, LogEntry, generate_session_id};

pub use github::{
    GitHubClient, GitHubToken, GitHubOAuthConfig,
    ConnectedRepo, RepoPermissions,
    load_github_token, save_github_token, check_github_auth,
    branch_name_for_issue, github_token_path,
};

pub use preflight::{
    PreflightConfig, GitInfo, AuthInfo, ProjectInfo, InferenceInfo,
    ToolsInfo, ToolInfo, LocalBackend, ProviderAuth,
    PylonInfo, SwarmProvider, ComputeMix,
    run_preflight,
};

pub use pylon_integration::{
    check_pylon_running, get_pylon_status, start_pylon,
    detect_local_backends, discover_swarm_providers,
};

pub use startup::{StartupState, StartupPhase, StartupSection, LogLine, LogStatus, ClaudeModel};

pub use streaming::{
    StreamToken, HarmonySegment,
    query_issue_summary, stream_gpt_oss_analysis,
    parse_harmony_stream, extract_final_content,
};

pub use utils::{shorten_path, sanitize_text, wrap_text};

pub use workflow::{
    GitHubWorkflow, IssueWorkflowResult, WorkflowReceipt,
};

pub use verification::{
    TerminationChecklist, CheckResult, VerificationRunner,
    generate_fix_prompt, should_force_stop,
};

pub use replay::{
    ReplayBundle, ReplayMetadata, TimelineEvent, ReplayReceipts,
    redact_replay,
};

pub use report::{
    AfterActionReport, SessionStats,
    collect_session_stats, generate_suggested_next_steps, generate_questions_for_user,
};

pub use checkpoint::{
    SessionCheckpoint, SessionSummary, CHECKPOINT_VERSION,
};
