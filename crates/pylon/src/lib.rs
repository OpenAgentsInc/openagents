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
//! Phases P1 to P4 of `docs/compute/verse-compute.md`. A checker
//! ([`check`]) runs canaries and redundant jobs and signs NIP-32 verdicts;
//! the pylon league ([`league`]) ranks pylons per class on pinned suites.
//! Paid jobs (P3, test sats first): a priced pylon sells each job under
//! NIP-X402's native purchase records ([`paid`]), and OpenAgents' broker
//! settles customers' x402 payments through the x402 facilitator, records
//! brokered sales in the split ledger, and sweeps provider balances
//! ([`broker`]). The agent market (P4): agents offer services under
//! NIP-MKT, hire each other through NIP-LAB orders whose compute runs on
//! the pool, and the broker settles each order with the seller's fee, the
//! provider's share, and OpenAgents' tied to the job's receipt
//! ([`market`]).
//! See `docs/compute/pylon.md`.

#[cfg(feature = "broker")]
pub mod broker;
pub mod attested;
pub mod check;
pub mod cli;
pub mod client;
pub mod decide;
pub mod engine;
pub mod field;
#[cfg(feature = "fixture")]
pub mod fixture;
pub mod identity;
pub mod inflight;
pub mod job;
pub mod league;
pub mod lease;
pub mod market;
pub mod paid;
pub mod pool;
pub mod provider;
pub mod relay;
pub mod route;
pub mod share;

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
