//! Pure Nostr protocol and verification primitives.
//!
//! Extracted from the public `immortal-core` crate (CC0): NIP-01 events,
//! tags, filters, canonical id, replacement and deletion semantics, NIP-19
//! key encoding, NIP-44 encryption, the Block extension kinds, and the
//! relay-side helpers (`RelaySigner`, NIP-98 HTTP auth, NIP-29 group
//! actions). No storage or network I/O, so protocol decisions stay
//! deterministic and fixture-testable.

#![forbid(unsafe_code)]

pub mod domain;
pub mod nip19;
pub mod nip44;
