//! Typed server-side Lightning with explicit payment recovery and pinned APIs.
pub mod error;
pub mod macaroon;

pub mod transport;
pub use error::{Error, Result};
pub mod fake;
pub mod invoices;
pub mod lightning;
pub mod proto;
pub mod router;
pub mod service;
pub mod state;
#[cfg(all(feature = "supervisor", unix))]
pub mod supervisor;
pub mod wallet;
pub use lightning::Client;
pub mod backups;
pub mod health;
pub mod liquidity;
pub mod payout;
#[cfg(test)]
mod tests;
