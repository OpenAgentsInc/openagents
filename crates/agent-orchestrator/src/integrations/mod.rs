//! OpenAgents integrations for agent orchestration
//!
//! This module provides integration points with OpenAgents-specific infrastructure:
//!
//! - **Directives**: Load and inject active directives into agent context
//! - **Autopilot**: Issue tracking hooks for claim/complete workflows
//! - **Trajectory**: APM metrics and action logging for performance tracking
//! - **Marketplace**: Skill licensing and usage tracking (NIP-SA kinds 38020, 38021)

pub mod autopilot;
pub mod directives;
pub mod marketplace;
pub mod trajectory;

pub use autopilot::{AutopilotIntegration, IssueClaimHook, IssueCompleteHook};
pub use directives::{DirectiveContext, DirectiveInjectionConfig, DirectiveSummary};
pub use marketplace::{
    MarketplaceIntegration, SkillLicenseHook, SkillLicenseInfo, SkillPricing, SkillUsageHook,
};
pub use trajectory::{ActionMetric, ApmTracker, TrajectoryLogger};
