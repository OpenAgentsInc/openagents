//! Spark wallet wrapper for OpenAgents desktop.

mod error;
mod signer;
mod wallet;

pub use error::SparkError;
pub use signer::SparkSigner;
pub use wallet::{
    Balance, Network, NetworkStatus, NetworkStatusReport, PaymentSummary, SparkWallet, WalletConfig,
};
