//! Marketplace for plugins, skills, and agents
//!
//! This crate provides infrastructure for discovering, installing, and managing
//! marketplace items including plugins, skills, and agents.

// d-008 Unified Marketplace modules
pub mod cli;
pub mod compute;
pub mod core;
pub mod data;
pub mod deprecation;
pub mod relay;
pub mod trajectories;
pub mod views;

// Existing marketplace modules
pub mod agent_commerce;
pub mod agent_governance;
pub mod agent_lifecycle;
pub mod agents;
pub mod api;
pub mod badges;
pub mod bounties;
pub mod budget;
pub mod coalition_compute;
pub mod coalitions;
pub mod creator_dashboard;
pub mod data_consumer;
pub mod data_contribution;
pub mod db;
pub mod discovery;
pub mod disputes;
pub mod dspy_security;
pub mod dvm;
pub mod geo_routing;
pub mod job_routing;
pub mod ledger;
pub mod mcp_binding;
pub mod provider_reputation;
pub mod redaction;
pub mod repository;
pub mod skills;
pub mod sybil;
pub mod trust;
pub mod types;

pub use agent_commerce::{
    AgentContract, ContractStatus, CoordinatorTask, DelegatedTask,
    HireAgentRequest as AgentHireRequest, HiringRequirements, TaskSpec,
};
pub use agent_governance::{
    ActionLimits, ActionType, ApprovalRequirement, AutonomyPolicy, EscalationAction,
    EscalationCondition, EscalationTrigger, SponsorControls,
};
pub use agent_lifecycle::{
    AgentEconomics, AgentLifecycleState, AutonomyLevel, CapabilityManifest, DeathCause, Mutation,
    ReproductionRequest, SponsorInfo, SponsorRelationship, SponsorType, TraitInheritance,
};
pub use agents::{
    Agent, AgentAvailability, AgentListing, AgentPricing, AgentSpawnRequest as AgentsSpawnRequest,
    AgentStatus, AgentWallet, PricingModel,
};
pub use api::{
    AgentSpawnRequest, AgentSpawnResponse, ApiError, BalanceResponse, CoalitionProposalRequest,
    CoalitionProposalResponse, ComputeJobRequest, ComputeJobResponse, DataBountyRequest,
    DataBountyResponse, DataContributionRequest, DataContributionResponse, DataListingsQuery,
    DataPurchaseRequest, DataPurchaseResponse, HireAgentRequest, HireAgentResponse, InvoiceRequest,
    InvoiceResponse, LedgerQuery, PaymentRequest, PaymentResponse, ProviderRegistrationRequest,
    ProviderRegistrationResponse, SkillInstallRequest, SkillInstallResponse, SkillQuery,
    SkillSubmissionRequest, SkillSubmissionResponse,
};
pub use badges::{
    Badge, BadgeError, BadgeRequirement, BadgeRequirements, BadgeType, BenchmarkResult,
    IdentityVerification, calculate_badge_trust_boost, get_badges, is_badge_expired,
};
pub use bounties::{
    BountyRequirements, BountyStatus, BountySubmission, DataBounty, SubmissionStatus,
};
pub use budget::{
    AlertAction, AlertThreshold, BudgetCheckResult, BudgetConfig, BudgetError, BudgetImpact,
    BudgetPeriod, CostEstimate, OverageAction, OveragePolicy, SpendingTracker, check_budget,
};
pub use coalition_compute::{
    AggregationStrategy, CoalitionComputeRequest, CoalitionResult, DecomposableTask,
    InferenceParams, ParallelismStrategy, ProviderContribution, Subtask, SubtaskResult,
};
pub use coalitions::{
    Coalition, CoalitionMember, CoalitionStatus, CoalitionType, Contribution, PaymentPool,
    PaymentSplit,
};
pub use creator_dashboard::{
    CreatorAnalytics, CreatorDashboard, CreatorSkillSummary, DashboardError, EarningsSummary,
    PayoutRecord, PayoutStatus, PeriodEarnings,
};
pub use data_consumer::{
    DataAccessToken, DataConsumerError, DataListing, DataListingType, DataPermissions,
    DataPurchase, DataSample, DatasetMetadata, RateLimit,
};
pub use data_contribution::{
    ContributionError, ContributionMetadata, ContributionStatus, DataContribution,
    DataContributionType, PaymentInfo, VerificationResult,
};
pub use discovery::{SearchFilters, SkillListing, SortOrder, discover_local_skills};
pub use disputes::{
    Dispute, DisputeError, DisputeResolution, DisputeStatus, DisputeType, Evidence, EvidenceType,
    RefundMethod, RefundRequest, RefundResult, RefundStatus, RefundTrigger, ResolutionDecision,
};
pub use dspy_security::{
    PermissionDecision, ResourceLimitDecision, RiskLevel, SafePathDecision, SkillSecurityDecision,
};
pub use dvm::{DvmJobRequest, DvmJobResult, DvmOffer, DvmResultStatus, DvmTag};
pub use geo_routing::{DataResidencyPolicy, GeoLocation, GeoRoutingPolicy, OrgGeoPolicy, Region};
pub use job_routing::{
    FailoverChain, FailoverPolicy, JobErrorType, ProviderScore, RetryDecision, RoutingError,
    SelectionCriteria, should_retry,
};
pub use ledger::{
    Balance, Direction, LedgerAmounts, LedgerEntry, LedgerEntryType, LedgerError, LedgerFilters,
    LedgerOperation, LedgerParties, LedgerReferences,
};
pub use mcp_binding::{
    ConnectedServer, ConnectionStatus, McpBindingError, McpCapability, McpDependencyCheck,
    McpServer, McpServerSuggestion, McpSession, ServerStatus, SkillMcpBinding,
    check_mcp_dependencies, suggest_mcp_servers,
};
pub use provider_reputation::{
    EconomicScore, ProviderReputation, ReputationError, ReputationTier, SocialScore, TierBenefits,
    TrackRecordScore, VerificationScore,
};
pub use redaction::{
    CustomRedactionPattern, RedactionEngine, RedactionError, RedactionPreferences, RedactionRecord,
    RedactionResult, RedactionType, SecretDetection, TextSpan,
};
pub use repository::{Repository, Skill as SkillRecord, SkillRepository, SkillVersion};
pub use skills::{
    Skill, SkillError, SkillManifest, SkillMetadata, discover_skills, validate_skill_name,
};
pub use sybil::{
    ProofOfWork, RateLimitTracker, RateLimitedAction, RateLimits, ReleaseCondition, SlashCondition,
    SlashRecord, Stake, StakeRequirement, StakeStatus, SybilError, check_rate_limit,
    generate_challenge, solve_challenge, verify_solution,
};
pub use trust::{
    EconomicComponent, EntityType, SocialComponent, TierBenefits as TrustTierBenefits,
    TrackRecordComponent, TrustComponents, TrustError, TrustScore, TrustTier,
    VerificationComponent as TrustVerificationComponent, calculate_trust_score,
};
pub use types::{ItemStatus, RevenueSplit, SkillPricing, SkillSubmission, SkillSubmissionStatus};
