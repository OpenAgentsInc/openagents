//! Pure Nostr protocol and verification primitives.
//!
//! Extracted from the public `immortal-core` crate (CC0): NIP-01 events,
//! tags, filters, canonical id, replacement and deletion semantics, NIP-19
//! key encoding, NIP-44 encryption, the Block extension kinds, the
//! relay-side helpers (`RelaySigner`, NIP-98 HTTP auth, NIP-29 group
//! actions), and the NIP-CJ decision-job protocol layer. No storage or
//! network I/O, so protocol decisions stay deterministic and
//! fixture-testable.

#![forbid(unsafe_code)]

pub mod cap;
pub mod contracts;
pub mod decision;
pub mod domain;
pub mod nip19;
pub mod nip44;
