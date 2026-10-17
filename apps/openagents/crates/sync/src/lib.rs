//! Loro room clients for Coder sync and an optional SQLite snapshot store.
//! The `store` feature is enabled by default. Clients in the root workspace
//! disable it to use the shared protocol with their existing local storage.
//!
//! - [`ChatClient`]: joins a ChatRoom DO (`wss://…/chat2/{chatId}/ws?token=`),
//!   catches up via checkpoint + row backfill, pushes local loro updates as
//!   rows, and reconnects with exponential backoff.
//! - [`RegistryClient`]: the per-profile workspace registry room (sidebar rows,
//!   presence).
//! - `DocsStore`: snapshot persistence (the doc IS the outbox — commands + user entries
//!   flush immediately) and the processed-command ledger with mark-BEFORE-execute semantics.

pub mod budget;
pub mod chat_client;
pub mod chat_frames;
pub mod device_channel;
pub mod dial;
pub mod net_path;
pub mod paired_device;
pub mod registry;
pub mod socket;
#[cfg(feature = "store")]
mod store;
#[cfg(feature = "store")]
mod sync_jobs;
mod types;
pub mod wake;

pub use chat_client::{
    ChatClient, ChatDocSink, ChatEvent, ChatStatsSnapshot, ChatTuning, CheckpointFetcher,
};
pub use registry::{
    ReconnectState, RegistryClient, RegistryEvent, RegistryTransport, RegistryTuning,
};
#[cfg(feature = "store")]
pub use store::{DocsStore, StoreError};
pub use types::{RoomStatsSnapshot, StaticUrl, SyncError, UrlProvider};
