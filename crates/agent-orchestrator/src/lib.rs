pub mod agents;
pub mod background;
pub mod config;
pub mod dspy_agents;
pub mod dspy_delegation;
pub mod dspy_pipelines;
pub mod error;
pub mod hooks;
pub mod integrations;
pub mod registry;

pub use agents::builtin_agents;
pub use background::{BackgroundTask, BackgroundTaskManager, SessionId, TaskId, TaskStatus};
pub use config::{AgentConfig, AgentMode, AgentPermission, BashPermission, PermissionLevel};

// DSPy signatures for learned agent behavior
pub use dspy_agents::{
    ArchitectureComplexity, ArchitectureSignature, CodeExplorationSignature, DocType,
    DocumentationSignature, LibraryLookupSignature, MediaAnalysisSignature, MediaType,
    SearchType, UIDesignSignature,
};
pub use dspy_delegation::{DelegationSignature, TargetAgent};
pub use dspy_pipelines::{DelegationInput, DelegationPipeline, DelegationResult};
pub use error::{Error, Result};
pub use hooks::{Hook, HookManager, HookResult};
pub use integrations::{
    ActionMetric, AgentIdentity, ApmTracker, AutonomyLevel, AutopilotIntegration, BackendConfig,
    BackendProvider, BudgetConfig, BudgetStatus, CostPaymentBridge, CostRecord, CostTracker,
    CostTrackingHook, DirectiveContext, DirectiveInjectionConfig, DirectiveSummary, FrostShareInfo,
    FrostrBridgeError, IssueClaimHook, IssueCompleteHook, MarketplaceIntegration,
    MockPaymentProvider, MultiBackendRouter, PaymentError, PaymentProvider, PaymentResult,
    PaymentStatus, PendingApproval, SkillLicenseHook, SkillLicenseInfo, SkillPricing,
    SkillUsageHook, SolverAgentCoordinator, SparkPaymentProvider, ThresholdConfig,
    TrajectoryLogger, WalletBalance, generate_threshold_identity, generate_threshold_shares,
    is_frostr_available, is_spark_available,
};
pub use registry::AgentRegistry;
