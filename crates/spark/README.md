# Spark Bitcoin Payment Integration

Self-custodial Bitcoin payments via Lightning, Spark Layer 2, and on-chain using the Breez SDK.

## Overview

This crate provides Bitcoin payment capabilities with unified identity management, sharing the same BIP39 mnemonic between Nostr (NIP-06) and Bitcoin (BIP44) keys.

## Status

### ✅ Phase 1: Core Integration (COMPLETED)

- ✅ **SparkSigner** - BIP44 key derivation (m/44'/0'/0'/0/0)
- ✅ **UnifiedIdentity integration** - Spark keys alongside Nostr keys
- ✅ **Basic wallet types** - Balance, WalletInfo, Network, WalletConfig
- ✅ **Breez SDK wiring** - Connect + wallet scaffolding

### 🚧 Phase 2: Wallet Operations (IN PROGRESS)

Current state: **Breez SDK connected** - Core wallet operations are implemented.

Implemented:

- Breez SDK connect using local spark-sdk dependency
- Spark + Bitcoin receive addresses
- Spark invoice creation
- Prepare/send payments + payment history listing
- Event listeners, network status checks, HTLC claim
- LNURL pay/withdraw + Lightning address management APIs
- On-chain deposit claim/refund + recommended fee lookup
- Token metadata + issuer API access
- Message signing/verification, user settings, leaf optimization controls
- Passphrase support for wallet initialization
- Advanced SDK config + key set selection via SparkWalletBuilder

Remaining for Phase 2:

- Expose advanced config through WalletConfig + CLI/GUI surfaces
- Token-aware balance shaping + WalletInfo population
- CLI/GUI wiring for LNURL, on-chain claims, tokens, user settings, and optimization

### ⏸️ Phase 3: Payment Extensions (PLANNED)

- Lightning receive (BOLT-11 invoice)
- LNURL support (pay, withdraw)
- Lightning address receive
- On-chain deposit/withdrawal workflows
- Payment history UI/export

### ⏸️ Phase 4: Token Support (PLANNED)

- Issue Spark tokens (BTKN)
- Send/receive tokens
- Token metadata queries

### ⏸️ Phase 5: Multi-Network (PLANNED)

- Mainnet (production)
- Testnet (testing)
- Signet (staging)
- Regtest (local dev)

### ⏸️ Phase 6: Agent Integration (PLANNED)

- MCP payment tools
- Autopilot CLI commands
- Event streaming
- Desktop UI components

## Architecture

```
                    BIP39 Mnemonic (12/24 words)
                              |
        +---------------------+---------------------+
        |                                           |
   m/44'/1237'/0'/0/0                        m/44'/0'/0'/0/0
   (NIP-06 Nostr)                            (BIP44 Bitcoin)
        |                                           |
   Nostr Keypair                             Spark Signer
   (crates/nostr/core)                       (crates/spark)
        |                                           |
        +---------------------+---------------------+
                              |
                      UnifiedIdentity
                   (crates/compute/domain)
```

## Usage

### Current (Phase 1 Complete)

```rust
use spark::SparkSigner;

// Derive Bitcoin keys from mnemonic
let mnemonic = "your twelve or twenty-four word mnemonic here";
let signer = SparkSigner::from_mnemonic(mnemonic, "")?;

// Get public key
let pubkey = signer.public_key_hex();
println!("Bitcoin public key: {}", pubkey);
```

### Seed Entropy (Web/External Storage)

```rust
use spark::SparkSigner;

// Derive Bitcoin keys from raw seed entropy (16-64 bytes)
let entropy = [0u8; 32];
let signer = SparkSigner::from_entropy(&entropy)?;
println!("Bitcoin public key: {}", signer.public_key_hex());
```

### Wallet Operations (Available)

```rust
use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

// Create wallet
let signer = SparkSigner::from_mnemonic(mnemonic, "")?;
let config = WalletConfig {
    network: Network::Testnet,
    api_key: Some("your-breez-api-key".to_string()),
    ..Default::default()
};
let wallet = SparkWallet::new(signer, config).await?;

// Get balance
let balance = wallet.get_balance().await?;
println!("Spark: {} sats", balance.spark_sats);
println!("Lightning: {} sats", balance.lightning_sats);
println!("On-chain: {} sats", balance.onchain_sats);
println!("Total: {} sats", balance.total_sats());

// Check network status (uses sync)
let report = wallet.network_status(std::time::Duration::from_secs(5)).await;
println!("Network: {}", report.status.as_str());
```

Note: Breez SDK currently supports Mainnet and Regtest. Network::Testnet and Network::Signet map to Regtest in this crate.

Note: In wasm builds, SparkWallet uses in-memory storage by default. Use SparkWalletBuilder::with_storage
to supply a custom storage backend when targeting the web.

## Breez SDK Integration

### Repository

- **GitHub**: https://github.com/breez/spark-sdk
- **Docs**: https://sdk-doc-spark.breez.technology/
- **API**: https://breez.github.io/spark-sdk/breez_sdk_spark/

### Key Dependencies

```toml
[dependencies]
# Local dependency on breez-sdk-spark core crate
breez-sdk-spark = { path = "../../../spark-sdk/crates/breez-sdk/core" }

# Or when published to crates.io:
# breez-sdk-spark = "0.1.0"
```

### API Key

An API key is required from Breez for production use. Request one at:
https://breez.technology/request-api-key/

For development/testing, the API key is optional.

## Security

- Mnemonic stored in memory only (not persisted by this crate)
- All signing uses FROST threshold signatures
- Private keys never leave the device
- Cooperative exit allows on-chain recovery anytime
- Same security model as NIP-06 (user controls seed phrase)

## Testing

```bash
# Run tests
cargo test -p openagents-spark

# Future: Integration tests with regtest
# cargo test -p openagents-spark --features integration-tests
```

## Docs

- `crates/spark/docs/README.md`
- `crates/spark/docs/CONFIGURATION.md`
- `crates/spark/docs/API.md`

## Related

- **Directive**: [d-001](../../.openagents/directives/d-001-breez-spark-sdk.md)
- **Issues**: #154, #155, #156, #157
- **Nostr Integration**: [crates/nostr/core](../nostr/core)
- **UnifiedIdentity**: [crates/compute/src/domain/identity.rs](../compute/src/domain/identity.rs)
