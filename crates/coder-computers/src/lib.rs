//! Coder's Computers screens, built once as Rust Native projections.
//!
//! The screens show your hosts with an honest status, the ways to add one,
//! who has access to each, first run, and activity that needs attention.
//! [`Computers`] is the application host for one surface lifetime: it holds
//! the current validated view, resolves revision-bound activations to typed
//! [`Intent`] values, checks each intent against the current [`Snapshot`],
//! and calls a [`ComputersService`]. An intent never grants authority; the
//! host checks its own grant record for every operation.
//!
//! Coder copy, palette, and states live here, not in `rust-native`. Platform
//! adapters mount the tree and collect the few values a tree cannot, such as
//! a scanned invitation, through [`InputRequest`].
#![forbid(unsafe_code)]

pub mod authority;
pub mod cache;
pub mod connect;
mod controller;
pub mod intent;
#[cfg(feature = "live")]
pub mod live;
pub mod model;
pub mod project;
pub mod qr;
pub mod service;
pub mod synthetic;
pub mod terminal;

pub use authority::{Action, Denial};
pub use controller::{
    Capture, CaptureKind, Computers, InputPurpose, InputRequest, MAX_CAPTURE_BYTES,
    MAX_INPUT_BYTES, MAX_PREVIEW_BYTES, MAX_PULL_BYTES, Notice, NoticeKind, Outcome, Refusal,
    describe,
};
pub use intent::{Intent, Screen};
pub use model::{
    Capabilities, Compatibility, CreatedInvitation, DataState, DeviceList, DeviceRow,
    DirectoryState, Enrollment, HostRecord, HostStatus, LOCAL_WEIGHT, Listing, ListingChange,
    LocalHost, NotEnrolledCause, OfflineCause, OutOfDate, PendingEnrollment, Platform,
    ServiceState, Snapshot, SshAttempt, SshRemoval, SshStage, Tunnel,
};
pub use service::{ComputersService, Unavailable};

#[cfg(test)]
mod tests;
