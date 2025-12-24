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
    generate_threshold_identity, generate_threshold_shares, is_frostr_available, is_spark_available,
    ActionMetric, AgentIdentity, ApmTracker, AutopilotIntegration, AutonomyLevel, BackendConfig,
    BackendProvider, BudgetConfig, BudgetStatus, CostPaymentBridge, CostRecord, CostTracker,
    CostTrackingHook, DirectiveContext, DirectiveInjectionConfig, DirectiveSummary,
    FrostShareInfo, FrostrBridgeError, IssueClaimHook, IssueCompleteHook, MarketplaceIntegration,
    MockPaymentProvider, MultiBackendRouter, PaymentError, PaymentProvider, PaymentResult,
    PaymentStatus, PendingApproval, SkillLicenseHook, SkillLicenseInfo, SkillPricing,
    SkillUsageHook, SolverAgentCoordinator, SparkPaymentProvider, ThresholdConfig,
    TrajectoryLogger, WalletBalance,
};
pub use registry::AgentRegistry;
