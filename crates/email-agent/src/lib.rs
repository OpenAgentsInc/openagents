#![cfg_attr(test, allow(clippy::expect_used))]

mod gmail_connector;
mod gmail_sync;
mod normalization;
mod retrieval;

pub use gmail_connector::{
    GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailBackfillResult,
    GmailConnectorError, GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader,
    GmailMessageMetadata, GmailMessagePayload, GmailThreadParticipant, run_gmail_backfill,
};
pub use gmail_sync::{
    GmailDeltaItem, GmailDeltaOperation, GmailHistoryProvider, GmailSyncBatch, GmailSyncCursor,
    GmailSyncError, GmailSyncOutcome, GmailSyncState, apply_gmail_incremental_sync,
};
pub use normalization::{NormalizationConfig, NormalizedConversationItem, normalize_gmail_message};
pub use retrieval::{RetrievalIndex, RetrievalQuery, RetrievedContextChunk};
