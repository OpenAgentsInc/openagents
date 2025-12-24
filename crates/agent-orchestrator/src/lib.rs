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
    ActionMetric, AgentIdentity, ApmTracker, AutopilotIntegration, AutonomyLevel, BackendConfig,
    BackendProvider, BudgetConfig, BudgetStatus, CostRecord, CostTracker, CostTrackingHook,
    DirectiveContext, DirectiveInjectionConfig, DirectiveSummary, IssueClaimHook,
    IssueCompleteHook, MarketplaceIntegration, MultiBackendRouter, PendingApproval,
    SkillLicenseHook, SkillLicenseInfo, SkillPricing, SkillUsageHook, SolverAgentCoordinator,
    ThresholdConfig, TrajectoryLogger,
};
pub use registry::AgentRegistry;
