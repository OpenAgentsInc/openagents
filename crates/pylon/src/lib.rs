//! Pylon: a compute provider and its client over NIP-PYLON and NIP-CJ.
//!
//! A pylon publishes `30200` beacons, takes free NIP-CJ conversation jobs
//! (`25900`, NIP-44 encrypted) from admitted buyers, runs them on a model
//! engine, and answers with `26900` results. In production the engine is a
//! Psionic model server built from `crates/psionic` and listening on
//! loopback. A buyer finds a pylon by its beacon, sends one job, and
//! publishes a `3201` receipt. An aggregator counts a pool's beacons and
//! receipts into a `30201` aggregate that any reader can recompute.
//!
//! Phase P1 of `docs/compute/verse-compute.md`: free jobs only, so every
//! receipt's payment is null. See `docs/compute/pylon.md`.

pub mod cli;
pub mod client;
pub mod engine;
pub mod field;
pub mod identity;
pub mod job;
pub mod pool;
pub mod provider;
pub mod relay;

/// The production relay.
pub const DEFAULT_RELAY: &str = "wss://relay.openagents.com";

/// The Unix time in seconds.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Where pylon keys, receipts, and counters live: `OPENAGENTS_PYLON_HOME`,
/// else `~/.openagents/compute`.
#[must_use]
pub fn home() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_PYLON_HOME") {
        return dir.into();
    }
    std::env::var_os("HOME")
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from)
        .join(".openagents")
        .join("compute")
}
