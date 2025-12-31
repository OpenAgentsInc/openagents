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
    AuthEntry, AuthStatus, AuthStore, check_openagents_auth, check_opencode_auth,
    copy_opencode_auth, get_provider_auth, has_anthropic_auth, openagents_auth_path,
    opencode_auth_path,
};

pub use ci::{CICheckResult, CIClient, CIStatus, CISystem, CheckDetail, detect_test_command};

pub use claude::{
    ClaudeEvent, ClaudeToken, ClaudeUsageData, run_claude_execution, run_claude_planning,
    run_claude_review,
};

pub use logger::{LogEntry, SessionLogger, generate_session_id};

pub use github::{
    ConnectedRepo, GitHubClient, GitHubOAuthConfig, GitHubToken, RepoPermissions,
    branch_name_for_issue, check_github_auth, github_token_path, load_github_token,
    save_github_token,
};

pub use preflight::{
    AuthInfo, ComputeMix, GitInfo, InferenceInfo, LocalBackend, PreflightConfig, ProjectInfo,
    ProviderAuth, PylonInfo, SwarmProvider, ToolInfo, ToolsInfo, run_preflight,
};

pub use pylon_integration::{
    check_pylon_running, detect_local_backends, discover_swarm_providers, get_pylon_status,
    start_pylon,
};

pub use startup::{ClaudeModel, LogLine, LogStatus, StartupPhase, StartupSection, StartupState};

pub use streaming::{
    HarmonySegment, StreamToken, extract_final_content, parse_harmony_stream, query_issue_summary,
    stream_gpt_oss_analysis,
};

pub use utils::{sanitize_text, shorten_path, wrap_text};

pub use workflow::{GitHubWorkflow, IssueWorkflowResult, WorkflowReceipt};

pub use verification::{
    CheckResult, TerminationChecklist, VerificationRunner, generate_fix_prompt, should_force_stop,
};

pub use replay::{ReplayBundle, ReplayMetadata, ReplayReceipts, TimelineEvent, redact_replay};

pub use report::{
    AfterActionReport, SessionStats, collect_session_stats, generate_questions_for_user,
    generate_suggested_next_steps,
};

pub use checkpoint::{CHECKPOINT_VERSION, SessionCheckpoint, SessionSummary};
