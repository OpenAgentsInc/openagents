pub mod agents;
pub mod background;
pub mod config;
pub mod error;
pub mod hooks;
pub mod integrations;
pub mod registry;

pub use agents::builtin_agents;
pub use background::{BackgroundTask, BackgroundTaskManager, SessionId, TaskId, TaskStatus};
pub use config::{AgentConfig, AgentMode, AgentPermission, BashPermission, PermissionLevel};
pub use error::{Error, Result};
pub use hooks::{Hook, HookManager, HookResult};
pub use integrations::{
    ActionMetric, ApmTracker, AutopilotIntegration, DirectiveContext, DirectiveInjectionConfig,
    DirectiveSummary, IssueClaimHook, IssueCompleteHook, MarketplaceIntegration, SkillLicenseHook,
    SkillLicenseInfo, SkillPricing, SkillUsageHook, TrajectoryLogger,
};
pub use registry::AgentRegistry;
