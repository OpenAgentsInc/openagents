//! Public OpenAgents HTTP client. Authentication is supplied explicitly; this
//! client never adopts CLI credentials, launches a browser, or loads a host.

pub mod agents;
mod auth;
pub mod chats;
pub mod computers;
pub mod dashboard;
pub mod generated;
pub mod history;
mod payment;
pub mod labs;
mod transport;

pub use auth::{
    Account, AccountView, CredentialRecord, CredentialStore, Login, Poll, Session, StorageError,
    Workspace,
};
pub use history::{
    AttachmentRef, CaptureState, ChatReader, ChatRef, ChatSource, ChatSummary, FileKind,
    HistoryIndex, HistoryPage, HistoryProject, MessageRole, ProjectReader, SavedCapture, SavedFile,
    SavedMessage, ToolDetail, ToolStatus, TranscriptPage, TranscriptReader,
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
