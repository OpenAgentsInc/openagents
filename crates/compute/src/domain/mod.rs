//! Domain types for the compute provider

pub mod code_review;
pub mod earnings;
pub mod events;
pub mod identity;
pub mod job;
pub mod patch_gen;
pub mod pricing;
pub mod repo_index;
pub mod sandbox_run;

pub use code_review::{
    ApprovalStatus, CodeReviewRequest, CodeReviewResult, IssueCategory, IssueSeverity,
    ReviewInput, ReviewIssue, ReviewStats,
};
pub use earnings::EarningsTracker;
pub use events::DomainEvent;
pub use identity::UnifiedIdentity;
pub use job::{Job, JobStatus};
pub use patch_gen::{
    PatchGenRequest, PatchGenResult, PatchVerification, PathFilter, TokenUsage,
};
pub use pricing::{
    quote_repo_index, quote_sandbox_run, PriceBook, Quote, RepoIndexPricing, SandboxRunPricing,
};
pub use repo_index::{IndexData, IndexType, RepoIndexRequest, RepoIndexResult, Symbol};
pub use sandbox_run::{
    ArtifactHash, CommandResult, ResourceLimits, ResourceUsage, SandboxRunRequest,
    SandboxRunResult,
};
