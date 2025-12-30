# Spark Crate Docs

This folder documents the OpenAgents Spark integration (Breez SDK wrapper).

## Contents
- CONFIGURATION.md: WalletConfig vs Breez SDK Config and SparkWalletBuilder usage.
- API.md: Wrapper API coverage and common workflows.

## Quickstart

```rust
use openagents_spark::{SparkSigner, SparkWallet, WalletConfig, Network};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
let signer = SparkSigner::from_mnemonic(mnemonic, "")?;
let config = WalletConfig {
    network: Network::Regtest,
    api_key: None,
    ..Default::default()
};

let wallet = SparkWallet::new(signer, config).await?;
let balance = wallet.get_balance().await?;
println!("Total sats: {}", balance.total_sats());
# Ok(())
# }
```

## When to use SparkWalletBuilder
Use SparkWalletBuilder when you need advanced Breez SDK configuration (LNURL domain, private mode, max deposit claim fee, real-time sync settings) or to select a key set/account path. See CONFIGURATION.md.

## Network support
Breez SDK supports Mainnet and Regtest. In this crate, Network::Testnet and Network::Signet map to Regtest.
