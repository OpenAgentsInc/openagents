//! Optimizable DSPy signatures for agent workflows.
//!
//! This module contains signatures for:
//! - Retrieval policy (query composition, routing, reranking)
//! - Chunk analysis (task selection, aggregation)
//! - Sandbox operations (profile selection, failure triage)
//! - Budgeting and memory (lane allocation, redundancy detection)
//! - Code change workflow (task understanding, code editing, verification)

pub mod agent_memory;
pub mod candidate_rerank;
pub mod chunk_aggregator;
pub mod chunk_task;
pub mod code_edit;
pub mod context_selection;
pub mod failure_triage;
pub mod issue_suggestion;
pub mod issue_validation;
pub mod lane_budgeter;
pub mod planning;
pub mod query_composer;
pub mod retrieval_router;
pub mod sandbox_profile;
pub mod task_understanding;
pub mod tool_call;
pub mod tool_result;
pub mod unblock_suggestion;
pub mod verification;

// Re-export all signatures
pub use agent_memory::AgentMemorySignature;
pub use candidate_rerank::CandidateRerankSignature;
pub use chunk_aggregator::ChunkAnalysisToActionSignature;
pub use chunk_task::ChunkTaskSelectorSignature;
pub use code_edit::CodeEditSignature;
pub use context_selection::ContextSelectionSignature;
pub use failure_triage::FailureTriageSignature;
pub use issue_suggestion::IssueSuggestionSignature;
pub use issue_validation::IssueValidationSignature;
pub use lane_budgeter::LaneBudgeterSignature;
pub use planning::PlanningSignature;
pub use query_composer::QueryComposerSignature;
pub use retrieval_router::RetrievalRouterSignature;
pub use sandbox_profile::SandboxProfileSelectionSignature;
pub use task_understanding::TaskUnderstandingSignature;
pub use tool_call::ToolCallSignature;
pub use tool_result::ToolResultSignature;
pub use unblock_suggestion::UnblockSuggestionSignature;
pub use verification::VerificationSignature;
