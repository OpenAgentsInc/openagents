//! Public OpenAgents HTTP client. Authentication is supplied explicitly; this
//! client never adopts CLI credentials, launches a browser, or loads a host.

pub mod agent_lab;
pub mod agents;
mod auth;
pub mod chats;
pub mod cloud_host;
pub mod computers;
pub mod continuations;
pub mod dashboard;
pub mod generated;
pub mod history;
pub mod labs;
mod payment;
pub mod rooms;
pub mod schedules;
pub mod sessions;
pub mod terminal_chats;
mod transport;

pub use auth::nostr::{
    EventTemplate, HTTP_AUTH_KIND, KEY_BODY_MAX, KeySignIn, LinkedKey, NostrSigner, POW_BITS,
    ScannedLogin, ScannedLoginAbout, ScannedLoginApproved, SignerError, solve_pow,
};
pub use auth::{
    Account, AccountView, AppleSignIn, CredentialRecord, CredentialStore, Login, Poll, Session,
    StorageError, Workspace,
};
pub use history::{
    AttachmentRef, CaptureState, ChatReader, ChatRef, ChatSource, ChatSummary, FileKind,
    HistoryIndex, HistoryPage, HistoryProject, MessageRole, ProjectReader, SavedCapture, SavedFile,
    SavedLabCard, SavedMessage, ToolDetail, ToolStatus, TranscriptPage, TranscriptReader,
};
pub use payment::{MAX_PAYMENT_REQUIRED_HEADER_BYTES, PaymentRequired, PaymentRequiredHeaderError};
pub use tokio_util::sync::CancellationToken;
pub use transport::{Client, Deployment, Error};

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
