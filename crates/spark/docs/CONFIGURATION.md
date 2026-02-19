# Configuration

This crate exposes two configuration layers:
- WalletConfig: OpenAgents wrapper config (network, API key, storage path).
- breez_sdk_spark::Config: Full Breez SDK configuration (sync interval, LNURL domain, private mode, etc.).

## WalletConfig
WalletConfig is intentionally minimal to keep existing call sites stable.

```rust
use openagents_spark::{WalletConfig, Network};
use std::path::PathBuf;

let config = WalletConfig {
    network: Network::Mainnet,
    api_key: Some("your-breez-api-key".to_string()),
    storage_dir: PathBuf::from("/var/lib/openagents/spark"),
};
```

Defaults:
- Native builds use `dirs::data_local_dir()/openagents/spark`.
- wasm builds set `storage_dir` to `openagents/spark` but default to in-memory storage unless overridden.

## Breez SDK Config via SparkWalletBuilder
When you need advanced configuration, use SparkWalletBuilder and pass a full breez_sdk_spark::Config.

```rust
use openagents_spark::{Config, OptimizationConfig, SparkSigner, SparkWallet, SparkWalletBuilder, WalletConfig, Network};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let signer = SparkSigner::from_mnemonic(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "",
)?;

let wallet_config = WalletConfig {
    network: Network::Regtest,
    api_key: None,
    ..Default::default()
};

let mut sdk_config = Config::default();
// Keep the SDK network aligned with WalletConfig.
sdk_config.network = wallet_config.network.to_sdk_network();
// Example advanced knobs.
sdk_config.sync_interval_secs = 30;
// Disable real-time sync if you do not want background networking.
sdk_config.real_time_sync_server_url = None;
// Configure leaf optimization policy.
sdk_config.optimization_config = OptimizationConfig {
    auto_enabled: true,
    multiplicity: 1,
};

let wallet = SparkWallet::builder(signer, wallet_config)
    .with_sdk_config(sdk_config)
    .build()
    .await?;
# Ok(())
# }
```

### Custom storage (wasm or embedded targets)
Use `with_storage` to provide your own `breez_sdk_spark::Storage` implementation.

```rust
use openagents_spark::{SparkWalletBuilder, WalletConfig, SparkSigner};
use std::sync::Arc;

# async fn example(storage: Arc<dyn breez_sdk_spark::Storage>) -> Result<(), Box<dyn std::error::Error>> {
let signer = SparkSigner::from_entropy(&[0u8; 32])?;
let wallet = SparkWalletBuilder::new(signer, WalletConfig::default())
    .with_storage(storage)
    .build()
    .await?;
# Ok(())
# }
```

## Key set and derivation control
The Breez SDK supports multiple key sets and account numbers. Use SparkWalletBuilder to select them.

```rust
use openagents_spark::{KeySetType, SparkSigner, SparkWallet, WalletConfig};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let signer = SparkSigner::from_mnemonic(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "",
)?;

let wallet = SparkWallet::builder(signer, WalletConfig::default())
    .with_key_set(KeySetType::Taproot, false, Some(0))
    .build()
    .await?;
# Ok(())
# }
```

## Passphrase handling
SparkSigner stores the optional BIP39 passphrase and SparkWallet initialization now forwards it into Breez Seed::Mnemonic.

## External input parsers
If you want custom parsers for parse_input, set them on the Breez SDK config and use SparkWalletBuilder.

```rust
use openagents_spark::{Config, ExternalInputParser, SparkSigner, SparkWallet, WalletConfig};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let signer = SparkSigner::from_mnemonic(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "",
)?;

let mut sdk_config = Config::default();
sdk_config.external_input_parsers = Some(vec![ExternalInputParser {
    provider_id: "my-parser".to_string(),
    input_regex: "(.*)(example.com)(.*)".to_string(),
    parser_url: "https://example.com/.well-known/lnurlp/<input>".to_string(),
}]);

let _wallet = SparkWallet::builder(signer, WalletConfig::default())
    .with_sdk_config(sdk_config)
    .build()
    .await?;
# Ok(())
# }
```

## Notes
- On Mainnet, a Breez API key is required.
- Testnet and Signet map to Regtest in this crate.
