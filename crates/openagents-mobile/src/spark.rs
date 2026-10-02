//! The Wallet tab's Spark wallet: the shared [`SparkNode`]
//! (`crates/spark-wallet`, which `openagents wallet` on computers also
//! runs) over Breez's SQLite store under the wallet home, one directory per
//! network and wallet.

use breez_sdk_spark::Network;
use std::path::Path;

pub use openagents_spark::spark::{BREEZ_API_KEY, SparkNode, moonpay_url, sdk_config};

/// Start the wallet with a mnemonic under `home`. Blocking; it reaches the
/// Spark operators.
pub fn open(home: &Path, network: Network, mnemonic: &str) -> Result<SparkNode, String> {
    let storage =
        breez_sdk_spark::default_storage(home.join("breez").to_string_lossy().into_owned());
    SparkNode::open(storage, network, mnemonic)
}
