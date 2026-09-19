//! A hardened Nostr relay. One binary, one Postgres.
//!
//! Extracted from the public `immortal-relay` crate (CC0), minus the
//! market, Boltz, and OpenAgents-lane code.

#![forbid(unsafe_code)]

pub use nostr::{domain, nip44};

pub mod bulk_import;
pub mod gateway;
pub mod store;
