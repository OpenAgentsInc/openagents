//! `kiln-git`: the maintainer's and owner's side of Kiln's NIP-34 patches
//! (`docs/kiln/workstation.md`).
//!
//! - [`keys`]: the signing key, from a file or an `openagents` profile.
//! - [`relay`]: publish an event and wait for the relay's answer; read
//!   stored events.
//!
//! Nothing here merges: a status event records what a maintainer did.

pub mod keys;
pub mod relay;
