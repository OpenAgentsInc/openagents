//! Verse's Nostr session plumbing, split out of `crates/verse` so an edit to
//! a zone does not recompile it: the relay link (offline in a browser), the
//! player identity, NIP-MV frames, chat lines, the chat feed, and, with the
//! `xp` feature, the XP ledger client. `verse` re-exports each module under
//! its old path. Read `docs/verse/README.md`.

pub mod chat;
pub mod feed;
pub mod identity;
pub mod mv;
pub mod net;
#[cfg(feature = "xp")]
pub mod xp;
