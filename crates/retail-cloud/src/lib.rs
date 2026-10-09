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
//! - [`contract`]: the v1 computer and task classes as data.
//! - [`authority`]: observation, execution, disclosure, and spending,
//!   admitted and checked independently before every side effect.
//! - [`offer`]: immutable retail offers and their one funded request.
//! - [`reserve`]: the funded request's hold in the central ledger.
//! - [`provision`]: one admitted sandbox per funded execution.
//! - [`material`]: admitted source and the customer's key, nothing else.
//! - [`dispatch`]: one funded task, streamed by cursor.
//! - [`meter`]: retained cumulative usage and quoted-ceiling enforcement.
//! - [`cancel`]: separate stop, teardown, usage, and settled-charge receipts.
//! - [`settle`]: atomic measured debits and unused hold releases.
//! - [`journal`]: the durable intents and observations a restart reads.
//! - [`environment`]: saved customer environments (ENV-10), their price
//!   book, holds, settlement, prepaid retention, and recovery, closed until
//!   the owner reviews and qualifies them.
//! - `boat` (feature `boat`): the live Boat binding for the provider,
//!   sandbox, task-owner, stop, and artifact seams (#10748).

pub mod authority;
#[cfg(feature = "boat")]
pub mod boat;
pub mod cancel;
pub mod contract;
pub mod dispatch;
pub mod environment;
pub mod fake;
pub mod journal;
pub mod material;
pub mod meter;
pub mod offer;
pub mod provision;
pub mod recover;
pub mod reserve;
pub mod retain;
pub mod settle;
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// An offer or confirmation was refused, with its typed outcome.
    #[error("refused: {0:?}")]
    Refused(offer::OfferRefusal),
    /// An authority the step needs is missing.
    #[error("denied: {0:?}")]
    Denied(authority::Denial),
    /// Material was not delivered.
    #[error("material refused: {0:?}")]
    Material(material::MaterialRefusal),
    /// A saved-environment step was refused, with its typed reason.
    #[error("environment refused: {0:?}")]
    Environment(environment::Refusal),
    /// A live binding's call failed; its effect may be unknown.
    #[error("remote: {0}")]
    Remote(String),
}

/// Journal tables the modules add, created when the journal opens.
pub(crate) const EXTRA_SCHEMAS: &[&str] = &[
    provision::SCHEMA,
    material::SCHEMA,
    dispatch::SCHEMA,
    meter::SCHEMA,
    retain::SCHEMA,
    cancel::SCHEMA,
    recover::SCHEMA,
    settle::SCHEMA,
    environment::SCHEMA,
];

pub type Result<T> = std::result::Result<T, Error>;

/// SHA-256 of `bytes`, as 64 lowercase hex digits.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(bytes))
}
