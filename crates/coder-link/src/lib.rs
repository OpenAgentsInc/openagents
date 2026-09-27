//! One connection owner per host for Coder clients.
//!
//! A [`Supervisor`] is a deterministic state machine for one host. It decides
//! when to connect, when to wait, and when to stop trying, and it returns the
//! transport work to do as [`Command`] values. A [`Registry`] keeps one
//! supervisor per [`HostKey`], carries each command to an injected
//! [`Connector`], and reads time from an injected [`Clock`].
//!
//! The crate performs no network, storage, or UI work. Transport health
//! ([`Phase`]) and data freshness ([`Freshness`]) are separate fields, so a
//! screen can say which one failed.
#![forbid(unsafe_code)]

mod clock;
mod ids;
mod policy;
mod registry;
mod supervisor;

pub use clock::{Clock, ManualClock, Moment, SystemClock};
pub use ids::{AttemptId, ConnectionId, CredentialId, HostKey, InvalidId};
pub use policy::{Policy, PolicyError};
pub use registry::{Connector, Registry, RegistryError, Route};
pub use supervisor::{
    BlockReason, Command, Failure, Freshness, Phase, Report, Signal, Stage, StaleCause,
    StaleReport, Status, Supervisor,
};

#[cfg(test)]
mod tests;
