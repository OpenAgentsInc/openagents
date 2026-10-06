//! The retail cloud computer (`docs/cloud/retail-contract.md`) behind the
//! paid-availability gate.
//!
//! This crate composes the central money ledger ([`pay_ledger::compute`]),
//! the router contract ([`route_contract`]), and the receiver wallet's
//! contract ([`openagents_wallet::LightningWallet`]) into the retail flow:
//! top-ups, offers, holds, provisioning, dispatch, metering, teardown,
//! settlement, and recovery. Providers, wallets, and task owners are traits;
//! [`fake`] holds the simulated ones every test and the fake-payment
//! acceptance run use. Nothing here sells anything: paid availability stays
//! off until the owner confirms the retail contract and the funded
//! qualification passes (`NEEDS_OWNER.md`).
//!
//! - [`topup`]: Lightning top-ups credited once to the shared balance.

pub mod fake;
pub mod topup;

/// Why a retail operation was refused.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Ledger(#[from] pay_ledger::Error),
    #[error(transparent)]
    Wallet(#[from] openagents_wallet::WalletError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("{0}")]
    Invalid(&'static str),
    /// The same identity was used again with other terms.
    #[error("conflict: {0}")]
    Conflict(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

/// SHA-256 of `bytes`, as 64 lowercase hex digits.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(bytes))
}
