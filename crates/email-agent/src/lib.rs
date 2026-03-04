#![cfg_attr(test, allow(clippy::expect_used))]

mod draft_pipeline;
mod gmail_connector;
mod gmail_sync;
mod knowledge_base;
mod normalization;
mod retrieval;
mod style_profile;

pub use draft_pipeline::{
    DraftArtifact, DraftGenerationError, DraftGenerationInput, DraftPolicy, generate_draft,
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
pub use style_profile::{StyleProfile, StyleTone, derive_style_profile};
