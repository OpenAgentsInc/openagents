//! Verse's Nostr session plumbing, split out of `crates/verse` so an edit to
//! a zone does not recompile it: the relay link (offline in a browser), the
//! player identity, the block and mute lists, NIP-MV frames, chat lines, the
//! chat feed, and, with the `xp` feature, the XP ledger client. `verse` re-exports each module under
//! its old path. Read `docs/verse/README.md`.

pub mod blocklist;
pub mod chat;
pub mod feed;
pub mod identity;
pub mod mv;
pub mod net;
#[cfg(feature = "xp")]
pub mod xp;

/// The plaza world's NIP-MV identifier.
pub const PLAZA_WORLD: &str = "verse-plaza";
/// The world of the OpenAgents app's bare plaza. It is a separate coordinate
/// space from [`PLAZA_WORLD`]: the bare world has none of the plaza's
/// buildings or collision, so its positions are not valid plaza positions
/// (NIP-MV requires a distinct world identifier for each separately loaded
/// coordinate space).
pub const BARE_WORLD: &str = "verse-bare";
/// Public plaza relay used by unconfigured Coder mobile installs.
pub const PUBLIC_RELAY: &str = "wss://relay.openagents.com";
