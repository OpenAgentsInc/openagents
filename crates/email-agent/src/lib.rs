#![cfg_attr(test, allow(clippy::expect_used))]

mod gmail_connector;

pub use gmail_connector::{
    GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailBackfillResult,
    GmailConnectorError, GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader,
    GmailMessageMetadata, GmailMessagePayload, GmailThreadParticipant, run_gmail_backfill,
};
