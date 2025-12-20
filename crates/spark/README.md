# Spark Bitcoin Payment Integration

Self-custodial Bitcoin payments via Lightning, Spark Layer 2, and on-chain using the Breez SDK.

## Overview

This crate provides Bitcoin payment capabilities with unified identity management, sharing the same BIP39 mnemonic between Nostr (NIP-06) and Bitcoin (BIP44) keys.

## Status

### ✅ Phase 1: Core Integration (COMPLETED)

- ✅ **SparkSigner** - BIP44 key derivation (m/44'/0'/0'/0/0)
- ✅ **UnifiedIdentity integration** - Spark keys alongside Nostr keys
- ✅ **Basic wallet types** - Balance, WalletInfo, Network, WalletConfig
- ✅ **Stub implementations** - API surface defined for future integration

### 🚧 Phase 2: Wallet Operations (IN PROGRESS)

Current state: **Stub implementations** - Methods are defined but return placeholder data.

To complete Phase 2, we need to:

1. **Add Breez SDK dependency**
   - Add `spark-wallet` from GitHub: `breez/spark-sdk`
   - Or wait for crates.io publication
   - Add `spark` core crate dependency

2. **Implement wallet initialization**
   - Configure Breez SDK client
   - Set up Spark operator connections
   - Initialize wallet state from mnemonic + network

3. **Wire up balance operations**
   - Query Spark Layer 2 balance
   - Query Lightning channel balances
   - Query on-chain funds (cooperative exit)

4. **Add wallet sync**
   - Real-time sync with Spark operators
   - Background sync tasks
   - State persistence

### ⏸️ Phase 3: Payment Methods (PLANNED)

- Lightning send/receive (BOLT-11)
- Spark send/receive (SparkAddress)
- On-chain deposit/withdrawal
- LNURL support (pay, withdraw)
- Payment history

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

### Future (After Phase 2 Complete)

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

// Sync with operators
wallet.sync().await?;
```

## Breez SDK Integration

### Repository

- **GitHub**: https://github.com/breez/spark-sdk
- **Docs**: https://sdk-doc-spark.breez.technology/
- **API**: https://breez.github.io/spark-sdk/breez_sdk_spark/

### Key Dependencies Needed

```toml
[dependencies]
# From Breez spark-sdk (when available)
spark-wallet = { git = "https://github.com/breez/spark-sdk", branch = "main" }
spark = { git = "https://github.com/breez/spark-sdk", branch = "main" }

# Or when published to crates.io:
# spark-wallet = "0.1.0"
# spark = "0.1.0"
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
# Run tests (currently stub implementations)
cargo test -p spark

# Future: Integration tests with regtest
# cargo test -p spark --features integration-tests
```

## Related

- **Directive**: [d-001](../../.openagents/directives/d-001-breez-spark-sdk.md)
- **Issues**: #154, #155, #156, #157
- **Nostr Integration**: [crates/nostr/core](../nostr/core)
- **UnifiedIdentity**: [crates/compute/src/domain/identity.rs](../compute/src/domain/identity.rs)
