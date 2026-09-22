//! Pure Nostr protocol and verification primitives.
//!
//! Extracted from the public `immortal-core` crate (CC0): NIP-01 events,
//! tags, filters, canonical id, replacement and deletion semantics, NIP-19
//! key encoding, NIP-44 encryption, the Block extension kinds, the
//! relay-side helpers (`RelaySigner`, NIP-98 HTTP auth, NIP-29 group
//! actions), and the NIP-CJ decision and execution protocol layers. No
//! storage or network I/O, so protocol decisions stay deterministic and
//! fixture-testable.

#![forbid(unsafe_code)]

pub mod block_lane;
pub mod cap;
pub mod channel_window;
pub mod contracts;
pub mod decision;
pub mod domain;
pub mod execution;
pub mod ext;
pub mod git_sign;
pub mod lane;
pub mod negentropy;
pub mod nip04;
pub mod nip17;
pub mod nip19;
pub mod nip44;
pub mod prg;
pub mod profile;
pub mod push_lease;
pub mod run;
