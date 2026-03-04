#![cfg_attr(test, allow(clippy::expect_used))]

mod approval_workflow;
mod draft_pipeline;
mod follow_up_scheduler;
mod gmail_connector;
mod gmail_sync;
mod knowledge_base;
mod normalization;
mod observability;
mod quality_scoring;
mod retrieval;
mod security_privacy;
mod send_execution;
mod style_profile;
mod tenant_isolation;

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
pub use observability::{
    LifecycleStage, PipelineAuditError, PipelineAuditTrail, PipelineEvent, PipelineEventInput,
    PipelineEventStatus, RedactedPipelineEvent, SendTraceReport, derive_correlation_id,
    diagnostics_for_correlation, record_pipeline_event,
};
pub use quality_scoring::{
    QualityCase, QualityCaseScore, QualityDimensionScores, QualityEvaluationReport,
    QualityGateError, QualityThresholds, enforce_quality_gate, evaluate_quality_corpus,
};
pub use retrieval::{RetrievalIndex, RetrievalQuery, RetrievedContextChunk};
pub use security_privacy::{
    AccessAction, AccessAuditEvent, AccessRole, DataCategory, DataRecord, DeletionReceipt,
    DeletionRequest, ExportBundle, ExportRecord, ExportRequest, ExportScope, RetentionPolicy,
    RetentionSweepOutcome, SecurityPrivacyError, SecurityPrivacyState, enforce_retention_policy,
    export_records, redact_debug_trace, run_deletion_workflow,
};
pub use send_execution::{
    GmailSendProvider, GmailSendSuccess, SendAuditEventType, SendAuditRecord, SendDeliveryState,
    SendExecutionError, SendExecutionOutcome, SendExecutionPolicy, SendExecutionState,
    SendFailureClass, SendProviderError, SendRecord, SendRequest, execute_send_with_idempotency,
};
pub use style_profile::{StyleProfile, StyleTone, derive_style_profile};
pub use tenant_isolation::{
    TenantEnvironment, TenantIsolationError, TenantIsolationReport, TenantIsolationState,
    TenantNetworkBoundary, TenantProvisionRequest, TenantRuntimeIdentity, TenantSecretScope,
    TenantStorageLayout, TenantTeardownPlan, provision_tenant_environment,
    rotate_tenant_secret_scope, teardown_tenant_environment, verify_hard_tenant_isolation,
};
