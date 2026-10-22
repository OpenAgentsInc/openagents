//! `kiln-git`: the maintainer's and owner's side of Kiln's NIP-34 patches
//! (`docs/kiln/workstation.md`).
//!
//! - [`keys`]: the signing key, from a file or an `openagents` profile.
//! - [`relay`]: publish an event and wait for the relay's answer; read
//!   stored events; ask a runner to run a job.
//! - [`review`]: a patch read against its repository: author, check, and
//!   status. The bridge and the Grid Foreman read patches through it.
//! - [`apply`]: a reviewed patch on a branch of its own, with credit.
//! - [`pull`]: NIP-34 pull requests (`1618`, `1619`) from the author's own
//!   clone.
//! - [`bridge`]: maintainer-opened patches become draft pull requests on
//!   GitHub, and merges come back as `1631`.
//!
//! Nothing here merges: a status event records what a maintainer did.

pub mod apply;
pub mod bridge;
pub mod keys;
pub mod pull;
pub mod relay;
pub mod review;
