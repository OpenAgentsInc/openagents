//! Legacy Spark wallet wrapper for OpenAgents desktop.
//!
//! TODO(ldk-v0.2): keep this crate available for historical receipt reads and
//! explicit final-drain/recovery work only. New Nexus/Pylon funding and payout
//! paths should move to the LDK provider boundary.
//!
//! Network contract:
//! - Supported: `mainnet`, `regtest`
//! - Unsupported: `testnet`, `signet` (returns `SparkError::UnsupportedNetwork`)

mod error;
mod signer;
mod wallet;

pub use error::SparkError;
pub use signer::SparkSigner;
pub use wallet::{
    Balance, DepositClaimFeePolicy, Network, NetworkStatus, NetworkStatusReport, PaymentSummary,
    SparkWallet, UnclaimedDeposit, WalletConfig,
};
