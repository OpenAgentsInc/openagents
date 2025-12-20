//! Marketplace for plugins, skills, and agents
//!
//! This crate provides infrastructure for discovering, installing, and managing
//! marketplace items including plugins, skills, and agents.

pub mod creator_dashboard;
pub mod data_consumer;
pub mod data_contribution;
pub mod db;
pub mod discovery;
pub mod job_routing;
pub mod ledger;
pub mod provider_reputation;
pub mod redaction;
pub mod repository;
pub mod skills;
pub mod types;

pub use creator_dashboard::{
    CreatorAnalytics, CreatorDashboard, CreatorSkillSummary, DashboardError,
    EarningsSummary, PayoutRecord, PayoutStatus, PeriodEarnings,
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
pub use job_routing::{
    FailoverChain, FailoverPolicy, JobErrorType, ProviderScore, RetryDecision,
    RoutingError, SelectionCriteria, should_retry,
};
pub use ledger::{
    Balance, Direction, LedgerAmounts, LedgerEntry, LedgerEntryType, LedgerError,
    LedgerFilters, LedgerOperation, LedgerParties, LedgerReferences,
};
pub use provider_reputation::{
    EconomicScore, ProviderReputation, ReputationError, ReputationTier, SocialScore,
    TierBenefits, TrackRecordScore, VerificationScore,
};
pub use redaction::{
    CustomRedactionPattern, RedactionEngine, RedactionError, RedactionPreferences,
    RedactionRecord, RedactionResult, RedactionType, SecretDetection, TextSpan,
};
pub use repository::{Repository, Skill as SkillRecord, SkillRepository, SkillVersion};
pub use skills::{Skill, SkillError, SkillManifest, SkillMetadata, discover_skills, validate_skill_name};
pub use types::{ItemStatus, MarketplaceItemType};
