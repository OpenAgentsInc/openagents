//! Autopilot - autonomous agent runtime
//!
//! Provides authentication management, preflight configuration, and in-process
//! agent execution using the Claude Agent SDK.

pub mod auth;
pub mod preflight;

pub use auth::{
    AuthEntry, AuthStatus, AuthStore,
    check_openagents_auth, check_opencode_auth, copy_opencode_auth,
    get_provider_auth, has_anthropic_auth,
    openagents_auth_path, opencode_auth_path,
};

pub use preflight::{
    PreflightConfig, GitInfo, AuthInfo, ProjectInfo, InferenceInfo,
    ToolsInfo, ToolInfo, LocalBackend, ProviderAuth,
    run_preflight,
};
