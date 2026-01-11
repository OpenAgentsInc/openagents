//! OpenAgents integrations for agent orchestration
//!
//! This module provides integration points with OpenAgents-specific infrastructure:
//!
//! - **Directives**: Load and inject active directives into agent context
//! - **Autopilot**: Issue tracking hooks for claim/complete workflows
//! - **Trajectory**: APM metrics and action logging for performance tracking
//! - **Marketplace**: Skill licensing and usage tracking (NIP-SA kinds 39220, 39221)
//! - **Advanced**: FROSTR threshold signatures, NIP-SA solver agents, multi-backend routing
//! - **FROSTR Bridge**: Real threshold signing with FROSTR (feature-gated)
//! - **Spark Bridge**: Bitcoin payment provider integration (feature-gated)

pub mod advanced;
pub mod autopilot;
pub mod directives;
pub mod frostr_bridge;
pub mod marketplace;
pub mod spark_bridge;
pub mod trajectory;

pub use advanced::{
    AgentIdentity, AutonomyLevel, BackendConfig, BackendProvider, BudgetConfig, BudgetStatus,
    CostRecord, CostTracker, CostTrackingHook, MultiBackendRouter, PendingApproval,
    SolverAgentCoordinator, ThresholdConfig,
};
pub use autopilot_core::{AutopilotIntegration, IssueClaimHook, IssueCompleteHook};
pub use directives::{DirectiveContext, DirectiveInjectionConfig, DirectiveSummary};
pub use frostr_bridge::{
    FrostShareInfo, FrostrBridgeError, generate_threshold_identity, generate_threshold_shares,
    is_frostr_available,
};
pub use marketplace::{
    MarketplaceIntegration, SkillLicenseHook, SkillLicenseInfo, SkillPricing, SkillUsageHook,
};
pub use spark_bridge::{
    CostPaymentBridge, MockPaymentProvider, PaymentError, PaymentProvider, PaymentResult,
    PaymentStatus, SparkPaymentProvider, WalletBalance, is_spark_available,
};
pub use trajectory::{ActionMetric, ApmTracker, TrajectoryLogger};
