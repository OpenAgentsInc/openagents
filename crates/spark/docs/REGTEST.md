# Spark Regtest Guide

Spark uses the Breez SDK for Lightning payments. This document explains how regtest works and how to get test sats for development.

## Key Concept: Hosted Regtest

**Spark regtest is NOT a local bitcoind instance.** The Breez SDK connects to Lightspark's hosted regtest infrastructure:

```
https://regtest-mempool.us-west-2.sparkinfra.net/api
```

You cannot run your own `bitcoind -regtest` and use it with Spark. The SDK requires Lightspark's infrastructure for the Spark protocol layer.

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

### Option 1: Web UI (Easiest)

Go to **https://app.lightspark.com/regtest-faucet** in your browser.

1. Log in with your Lightspark account (create one if needed)
2. Enter your Spark deposit address
3. Request test sats

This is the easiest way to fund a test wallet manually.

### Option 2: Programmatic Faucet (Requires Credentials)

The GraphQL faucet at `https://api.lightspark.com/graphql/spark/rc` requires authentication.

Set these environment variables:
```bash
export FAUCET_USERNAME="your-username"
export FAUCET_PASSWORD="your-password"
```

Then use the `RegtestFaucet` from the `openagents-testing` crate:

```rust
use openagents_testing::faucet::RegtestFaucet;

let faucet = RegtestFaucet::new()?;
let txid = faucet.fund_address("bcrt1q...", 10_000).await?;
```

Or via curl:
```bash
curl -X POST https://api.lightspark.com/graphql/spark/rc \
  -u "$FAUCET_USERNAME:$FAUCET_PASSWORD" \
  -H "Content-Type: application/json" \
  -d '{
    "operationName": "RequestRegtestFunds",
    "variables": {
      "address": "YOUR_DEPOSIT_ADDRESS",
      "amount_sats": 10000
    },
    "query": "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash }}"
  }'
```

**Note:** The faucet credentials are separate from the Breez API key. Contact Lightspark or check their documentation for how to obtain faucet credentials for CI/automation.

### Option 3: Pre-funded Test Wallets

For E2E tests, you can use wallets that have already been funded:

```bash
SPARK_E2E_SENDER_MNEMONIC="your funded wallet mnemonic" \
SPARK_E2E_RECEIVER_MNEMONIC="another funded wallet mnemonic" \
cargo test -p openagents-spark --test integration -- --ignored
```

## Environment Variables

### Faucet Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `FAUCET_URL` | `https://api.lightspark.com/graphql/spark/rc` | Faucet GraphQL endpoint |
| `FAUCET_USERNAME` | (none) | Basic auth username **(required for programmatic access)** |
| `FAUCET_PASSWORD` | (none) | Basic auth password **(required for programmatic access)** |

### Test Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SPARK_E2E_USE_FAUCET` | (unset) | Set to `1` to auto-fund via faucet (requires credentials) |
| `SPARK_E2E_NETWORK` | `testnet` | Network for E2E tests |
| `SPARK_E2E_AMOUNT_SATS` | `100` | Amount to send in E2E payment tests |
| `SPARK_E2E_SENDER_MNEMONIC` | (test mnemonic) | BIP39 mnemonic for sender wallet |
| `SPARK_E2E_RECEIVER_MNEMONIC` | (test mnemonic) | BIP39 mnemonic for receiver wallet |
| `SPARK_E2E_TIMEOUT_SECS` | `180` | Test timeout in seconds |
| `RUN_NETWORK_TESTS` | (unset) | Enable network-dependent tests |
| `BREEZ_API_KEY` | (from .env.local) | Required for mainnet only |

## Running Tests

```bash
# Basic regtest tests (no network needed)
cargo test -p openagents-spark --test integration

# With faucet funding (requires FAUCET_USERNAME + FAUCET_PASSWORD)
SPARK_E2E_USE_FAUCET=1 cargo test -p openagents-spark --test integration -- --ignored

# With pre-funded wallets
SPARK_E2E_SENDER_MNEMONIC="word1 word2 ... word12" \
SPARK_E2E_RECEIVER_MNEMONIC="other1 other2 ... other12" \
SPARK_E2E_AMOUNT_SATS=1000 \
cargo test -p openagents-spark --test integration -- --ignored

# Pylon payment loop test
cargo test -p nostr-client --test agent_chat_e2e -- --ignored --nocapture
```

## Pylon Configuration

In `~/.openagents/pylon/config.toml`:

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
| Breez API Key | Not required | Required |
| Faucet Credentials | Required for programmatic | N/A |
| Funds | Free via faucet | Real Bitcoin |
| Infrastructure | Lightspark hosted | Breez production |
| Use Case | Development, testing | Production |

## Troubleshooting

### "Failed to connect to network"
- Regtest requires internet access to reach Lightspark's servers
- Check your network connection

### "Faucet request failed" / 401 Unauthorized
- The faucet requires `FAUCET_USERNAME` and `FAUCET_PASSWORD`
- Use the web UI at https://app.lightspark.com/regtest-faucet instead

### "Insufficient balance"
- Fund your wallet via the web faucet
- Wait for the transaction to confirm (regtest confirms quickly)

### "API key required"
- You're connecting to mainnet instead of regtest
- Set `network = "regtest"` in your config

## Architecture Note

Spark's regtest is a shared test environment hosted by Lightspark. Multiple developers use it simultaneously. This is fine for testing, but don't expect complete isolation like you'd get with a local `bitcoind -regtest`.

For fully offline testing without any network dependency, use the mock payment provider in `crates/agent-orchestrator/src/integrations/spark_bridge.rs`.
