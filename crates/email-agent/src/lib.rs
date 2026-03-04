#![cfg_attr(test, allow(clippy::expect_used))]

mod approval_workflow;
mod draft_pipeline;
mod follow_up_scheduler;
mod gmail_connector;
mod gmail_sync;
mod knowledge_base;
mod normalization;
mod retrieval;
mod send_execution;
mod style_profile;

pub use approval_workflow::{
    ApprovalDecisionAction, ApprovalDecisionInput, ApprovalMode, ApprovalPolicyPath,
    ApprovalWorkflowError, ApprovalWorkflowState, DraftApprovalItem, DraftApprovalStatus,
    DraftEnqueueRequest, QueueControlAction, QueueControlEvent, SendAuthorization,
    authorize_draft_send, enqueue_draft_for_approval, record_approval_decision,
    set_approval_kill_switch, set_approval_queue_paused,
};
pub use draft_pipeline::{
    DraftArtifact, DraftGenerationError, DraftGenerationInput, DraftPolicy, generate_draft,
};
pub use follow_up_scheduler::{
    FollowUpEvent, FollowUpEventType, FollowUpJob, FollowUpJobStatus, FollowUpRule,
    FollowUpRuleKind, FollowUpSchedulerError, FollowUpSchedulerPolicy, FollowUpSchedulerState,
    FollowUpTickOutcome, ThreadFollowUpContext, run_follow_up_scheduler_tick,
};
pub use gmail_connector::{
    GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailBackfillResult,
    GmailConnectorError, GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader,
    GmailMessageMetadata, GmailMessagePayload, GmailThreadParticipant, run_gmail_backfill,
};
pub use gmail_sync::{
    GmailDeltaItem, GmailDeltaOperation, GmailHistoryProvider, GmailSyncBatch, GmailSyncCursor,
    GmailSyncError, GmailSyncOutcome, GmailSyncState, apply_gmail_incremental_sync,
};
pub use knowledge_base::{
    GroundingReference, KnowledgeBase, KnowledgeChunk, KnowledgeChunkingConfig, KnowledgeDocument,
};
pub use normalization::{NormalizationConfig, NormalizedConversationItem, normalize_gmail_message};
pub use retrieval::{RetrievalIndex, RetrievalQuery, RetrievedContextChunk};
pub use send_execution::{
    GmailSendProvider, GmailSendSuccess, SendAuditEventType, SendAuditRecord, SendDeliveryState,
    SendExecutionError, SendExecutionOutcome, SendExecutionPolicy, SendExecutionState,
    SendFailureClass, SendProviderError, SendRecord, SendRequest, execute_send_with_idempotency,
};
pub use style_profile::{StyleProfile, StyleTone, derive_style_profile};
