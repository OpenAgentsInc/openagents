//! Public OpenAgents HTTP client. Authentication is supplied explicitly; this
//! client never adopts CLI credentials, launches a browser, or loads a host.
//!
//! Without its default `transport` feature, the crate is only what a chat's
//! records and events are ([`chats::wire`] and the values in [`MessageRole`]'s
//! family), with no network or runtime, so a browser build shares them.

#[cfg(feature = "transport")]
pub mod agent_lab;
#[cfg(feature = "transport")]
pub mod agents;
#[cfg(feature = "transport")]
pub mod attention;
#[cfg(feature = "transport")]
mod auth;
#[cfg(feature = "transport")]
pub mod browser_wallet;
pub mod chats;
#[cfg(feature = "transport")]
pub mod cloud_host;
#[cfg(feature = "transport")]
pub mod computers;
#[cfg(feature = "transport")]
pub mod continuations;
#[cfg(feature = "transport")]
pub mod dashboard;
#[cfg(feature = "transport")]
pub mod generated;
#[cfg(feature = "transport")]
pub mod history;
#[cfg(feature = "transport")]
pub mod issues;
#[cfg(feature = "transport")]
pub mod labs;
#[cfg(feature = "transport")]
pub mod land;
#[cfg(feature = "transport")]
pub mod lightning_address;
mod model;
#[cfg(feature = "transport")]
pub mod owner_notifications;
#[cfg(feature = "transport")]
mod payment;
#[cfg(feature = "transport")]
pub mod profiles;
#[cfg(feature = "transport")]
pub mod rooms;
#[cfg(feature = "transport")]
pub mod runs;
#[cfg(feature = "transport")]
pub mod schedules;
#[cfg(feature = "transport")]
pub mod sessions;
#[cfg(feature = "transport")]
pub mod terminal_chats;
#[cfg(feature = "transport")]
mod transport;
#[cfg(feature = "transport")]
pub mod work;

#[cfg(feature = "transport")]
pub use auth::nostr::{
    EventTemplate, HTTP_AUTH_KIND, KEY_BODY_MAX, KeySignIn, LinkedKey, NostrSigner, POW_BITS,
    ScannedLogin, ScannedLoginAbout, ScannedLoginApproved, SignerError,
    play_integrity_request_hash, solve_pow,
};
#[cfg(feature = "transport")]
pub use auth::{
    Account, AccountView, CredentialRecord, CredentialStore, DELETE_ACCOUNT_CONFIRM, Login, Poll,
    Session, StorageError, Workspace,
};
#[cfg(feature = "transport")]
pub use history::{
    AttachmentRef, ChatReader, ChatRef, ChatSummary, HistoryIndex, HistoryPage, HistoryProject,
    ProjectReader, SavedCapture, SavedFile, SavedLabCard, SavedMessage, ToolDetail, TranscriptPage,
    TranscriptReader,
};
pub use model::{CaptureState, ChatSource, FileKind, MessageRole, ToolStatus};
#[cfg(feature = "transport")]
pub use payment::{MAX_PAYMENT_REQUIRED_HEADER_BYTES, PaymentRequired, PaymentRequiredHeaderError};
#[cfg(feature = "transport")]
pub use tokio_util::sync::CancellationToken;
#[cfg(feature = "transport")]
pub use transport::{Client, Deployment, Error};

#[cfg(feature = "transport")]
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
