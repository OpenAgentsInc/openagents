# Spark Regtest Guide

Spark uses the Breez SDK for Lightning payments. This document explains how regtest works and how to get test sats for development.

## Key Concept: Hosted Regtest

**Spark regtest is NOT a local bitcoind instance.** The Breez SDK connects to Lightspark's hosted regtest environment at:

```
https://api.lightspark.com/graphql/spark/rc
```

You cannot run your own `bitcoind -regtest` and use it with Spark. The SDK requires Lightspark's infrastructure for the Lightning network layer.

## Network Mapping

The Breez SDK only supports two networks natively: **Mainnet** and **Regtest**.

| Your Config | SDK Network | Notes |
|-------------|-------------|-------|
| `mainnet` | Mainnet | Real Bitcoin, requires API key |
| `testnet` | Regtest | Maps to hosted regtest |
| `signet` | Regtest | Maps to hosted regtest |
| `regtest` | Regtest | Maps to hosted regtest |

From `wallet.rs`:
```rust
match self {
    Network::Mainnet => SdkNetwork::Mainnet,
    // All test networks map to SdkNetwork::Regtest
    Network::Testnet | Network::Signet | Network::Regtest => SdkNetwork::Regtest,
}
```

## Getting Regtest Sats

### Option 1: Programmatic Faucet (Recommended)

Use the `RegtestFaucet` from the `openagents-testing` crate:

```rust
use openagents_testing::faucet::RegtestFaucet;

let faucet = RegtestFaucet::new()?;
let txid = faucet.fund_address("spark1...", 10_000).await?;
```

The faucet calls Lightspark's GraphQL API:
```graphql
mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) {
  request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) {
    transaction_hash
  }
}
```

### Option 2: Environment Variable in Tests

Set `SPARK_E2E_USE_FAUCET=1` when running integration tests:

```bash
SPARK_E2E_USE_FAUCET=1 cargo test -p openagents-spark --test integration -- --ignored
```

### Option 3: Manual GraphQL Request

```bash
curl -X POST https://api.lightspark.com/graphql/spark/rc \
  -H "Content-Type: application/json" \
  -d '{
    "operationName": "RequestRegtestFunds",
    "variables": {
      "address": "YOUR_SPARK_ADDRESS",
      "amount_sats": 10000
    },
    "query": "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash }}"
  }'
```

## Environment Variables

### Faucet Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `FAUCET_URL` | `https://api.lightspark.com/graphql/spark/rc` | Faucet GraphQL endpoint |
| `FAUCET_USERNAME` | (none) | Basic auth username (if required) |
| `FAUCET_PASSWORD` | (none) | Basic auth password (if required) |

### Test Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SPARK_E2E_USE_FAUCET` | (unset) | Set to `1` to auto-fund wallets via faucet |
| `SPARK_E2E_NETWORK` | `testnet` | Network for E2E tests |
| `SPARK_E2E_AMOUNT_SATS` | `100` | Amount to send in E2E payment tests |
| `SPARK_E2E_SENDER_MNEMONIC` | (test mnemonic) | BIP39 mnemonic for sender wallet |
| `SPARK_E2E_RECEIVER_MNEMONIC` | (test mnemonic) | BIP39 mnemonic for receiver wallet |
| `SPARK_E2E_TIMEOUT_SECS` | `180` | Test timeout in seconds |
| `RUN_NETWORK_TESTS` | (unset) | Enable network-dependent tests |
| `BREEZ_API_KEY` | (from .env.local) | Required for mainnet only |

## Running Tests

```bash
# Basic regtest tests (no API key needed)
cargo test -p openagents-spark --test integration

# With faucet funding for payment tests
SPARK_E2E_USE_FAUCET=1 cargo test -p openagents-spark --test integration -- --ignored

# Full E2E with custom wallets
SPARK_E2E_SENDER_MNEMONIC="word1 word2 ... word12" \
SPARK_E2E_RECEIVER_MNEMONIC="other1 other2 ... other12" \
SPARK_E2E_AMOUNT_SATS=1000 \
cargo test -p openagents-spark --test integration -- --ignored

# Pylon payment loop test
cargo test -p nostr-client --test agent_chat_e2e -- --ignored --nocapture
```

## Pylon Configuration

In `~/.config/pylon/config.toml`:

```toml
[wallet]
network = "regtest"  # or "testnet" - both use hosted regtest
enable_payments = true
```

Or via CLI:
```bash
pylon start --mode provider --network regtest
```

## Mainnet vs Regtest

| Aspect | Regtest | Mainnet |
|--------|---------|---------|
| API Key | Not required | Required (Breez API key) |
| Funds | Free via faucet | Real Bitcoin |
| Infrastructure | Lightspark hosted | Breez production |
| Use Case | Development, testing | Production |

## Troubleshooting

### "Failed to connect to network"
- Regtest requires internet access to reach Lightspark's servers
- Check your network connection
- Verify the faucet URL is accessible

### "Insufficient balance"
- Use the faucet to fund your test wallet
- Wait for the transaction to confirm (regtest confirms quickly)

### "API key required"
- You're accidentally connecting to mainnet
- Set `network = "regtest"` in your config

## Architecture Note

Spark's regtest is a shared test environment. Multiple developers may be using it simultaneously. This is fine for testing - the Lightning network handles this gracefully. However, don't expect complete isolation like you'd get with a local `bitcoind -regtest`.

For fully offline testing without any network dependency, use the mock payment provider in `crates/agent-orchestrator/src/integrations/spark_bridge.rs`.
