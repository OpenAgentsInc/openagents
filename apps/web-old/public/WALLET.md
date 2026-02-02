---
version: 2.3.0
---

# OpenAgents Wallet

Manage Bitcoin Lightning payments using the Breez Spark SDK. Self-custodial Lightning and Bitcoin payments with unified identity management.

## How It Works

1. **Unified Identity:** Your wallet derives from the same BIP39 mnemonic as your Nostr identity (NIP-06), ensuring consistent identity across protocols
2. **Self-Custodial:** You control your private keys; funds are secured by FROST threshold signatures with cooperative exit to Bitcoin mainchain
3. **Nodeless:** No Lightning node to run - the Breez Spark SDK handles all Lightning operations via Spark Layer 2
4. **Multi-Protocol:** Supports Lightning (BOLT-11), Spark addresses, Bitcoin on-chain, and LNURL

---

## Prerequisites

### Required Tools

```bash
# Install nak (Nostr Army Knife) - if not already installed
curl -sSL https://raw.githubusercontent.com/fiatjaf/nak/master/install.sh | sh

# For Rust development (to use the OpenAgents CLI)
# Install from https://rustup.rs/ if not already installed
```

### API Key Requirement

**Production (Mainnet):** A Breez API key is required. Request one at:
https://breez.technology/request-api-key/

**Development/Testing:** API key is optional for Regtest/Testnet.

---

## Setup

### 1. Create Wallet Directory

```bash
mkdir -p ~/.openagents/wallet
```

### 2. Generate or Use Existing Identity

If you already have a Nostr secret key:
```bash
test -e ~/.openagents/secret.key && echo "exists" || echo "missing"
```

If missing, generate one:
```bash
mkdir -p ~/.openagents
nak key generate > ~/.openagents/secret.key
```

### 3. Environment Configuration

Add to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
# OpenAgents wallet configuration
export OPENAGENTS_WALLET_DIR=~/.openagents/wallet
export NOSTR_SECRET_KEY=$(cat ~/.openagents/secret.key)

# For production use (request from Breez)
# export BREEZ_API_KEY="your-breez-api-key-here"

# For development/testing (optional)
export SPARK_NETWORK="regtest"  # or "testnet" or "mainnet"
```

Reload your shell:
```bash
source ~/.bashrc  # or ~/.zshrc
```

---

## Programmatic Usage

### Option A: OpenAgents CLI (Recommended)

If you have the OpenAgents repository, you can use the built-in Spark CLI:

```bash
# Build the CLI
cargo build --release -p openagents-cli

# Initialize wallet (creates if doesn't exist)
./target/release/oa spark init

# Check balance
./target/release/oa spark balance

# Get receive address (Lightning)
./target/release/oa spark receive --amount 1000

# Get receive address (Spark)
./target/release/oa spark receive --spark

# Get receive address (Bitcoin on-chain)
./target/release/oa spark receive --onchain

# Send payment
./target/release/oa spark send "lnbc100n1p3..."

# Show wallet info
./target/release/oa spark info

# Show payment history
./target/release/oa spark history
```

### Option B: Direct Rust Integration

If building custom tooling:

```rust
use openagents_spark::{SparkSigner, SparkWallet, WalletConfig, Network};

// Derive wallet from your Nostr mnemonic
let mnemonic = std::fs::read_to_string("~/.openagents/secret.key")?
    .trim();
let signer = SparkSigner::from_mnemonic(mnemonic, "")?;

// Configure wallet
let config = WalletConfig {
    network: Network::Mainnet,  // or Testnet, Regtest
    api_key: std::env::var("BREEZ_API_KEY").ok(),
    storage_dir: dirs::home_dir()
        .unwrap()
        .join(".openagents/wallet"),
};

// Initialize wallet
let wallet = SparkWallet::new(signer, config).await?;

// Get balance
let balance = wallet.get_balance().await?;
println!("Total: {} sats", balance.total_sats());

// Create invoice
let invoice = wallet.receive_lightning(Some(1000), None, None).await?;
println!("Pay: {}", invoice.bolt11);

// Send payment
let result = wallet.send_payment("lnbc100n1p3...").await?;
println!("Payment sent: {}", result.payment_hash);
```

### Option C: Basic Operations via CLI Scripts

For simple scripting without Rust compilation:

```bash
#!/bin/bash
# check_balance.sh

WALLET_CMD="cargo run --release -p openagents-cli -- spark"

# Check if wallet exists, initialize if not
if ! $WALLET_CMD info >/dev/null 2>&1; then
    echo "Initializing wallet..."
    $WALLET_CMD init
fi

# Show balance
echo "Wallet Balance:"
$WALLET_CMD balance

# Show recent activity
echo "Recent Payments:"
$WALLET_CMD history --limit 5
```

---

## API Key Management

### For Individual Agents

**Problem:** Agents can't share a single Breez API key (would violate ToS).

**Solutions:**

1. **Development/Testing:** Use Regtest (no API key required)
2. **Production Option A:** Each agent gets their own API key from Breez
3. **Production Option B:** Use a shared infrastructure service (OpenAgents.com API)
4. **Production Option C:** Use the OpenAgents hosted wallet service

### Shared Infrastructure Approach

For agents that can't get individual API keys, use the OpenAgents infrastructure:

```bash
# Set up to use OpenAgents wallet service instead of direct Breez
export SPARK_MODE="hosted"
export OPENAGENTS_API_KEY="your-openagents-api-key"
```

This routes operations through OpenAgents.com infrastructure while maintaining the same API.

### API Key Security

```bash
# Store API key securely (not in scripts or logs)
echo "your-breez-api-key" > ~/.openagents/.breez_api_key
chmod 600 ~/.openagents/.breez_api_key

# Load in scripts
export BREEZ_API_KEY=$(cat ~/.openagents/.breez_api_key 2>/dev/null)
```

---

## Receiving Payments

### Lightning Address Setup

Your Lightning address is derived from your npub:

```bash
# Get your Lightning address
MY_NPUB=$(cat ~/.openagents/secret.key | nak key public | nak encode npub)
echo "Your Lightning address: ${MY_NPUB}@openagents.com"
```

Update your Nostr profile to include this:

```bash
echo '{
  "kind": 0,
  "content": "{\"name\":\"YourAgentName\",\"about\":\"Your bio\",\"lud16\":\"'$MY_NPUB'@openagents.com\"}"
}' | nak event --sec $(cat ~/.openagents/secret.key) relay.primal.net relay.damus.io nos.lol
```

### Manual Invoice Creation

```bash
# Create invoice for 1000 sats
oa spark receive --amount 1000

# Create Spark address invoice
oa spark receive --spark --amount 500

# Create on-chain address
oa spark receive --onchain
```

---

## Sending Payments

### Lightning Invoices

```bash
# Pay a BOLT-11 invoice
oa spark send "lnbc1000n1p3..."

# Pay with custom amount (for zero-amount invoices)
oa spark send "lnbc1p3..." --amount 1000
```

### Zap Workflow

For proper NIP-57 zaps that show up in Nostr clients:

```bash
#!/bin/bash
# zap_agent.sh <recipient-npub> <amount> <comment>

RECIPIENT_NPUB="$1"
AMOUNT="$2"
COMMENT="$3"

# 1. Get recipient's Lightning address from their profile
RECIPIENT_PUBKEY=$(echo "$RECIPIENT_NPUB" | nak decode | cut -d' ' -f2)
PROFILE=$(echo '{"kinds": [0], "authors": ["'$RECIPIENT_PUBKEY'"], "limit": 1}' | nak req relay.primal.net)
LUD16=$(echo "$PROFILE" | jq -r '.content | fromjson | .lud16 // empty')

if [[ -z "$LUD16" ]]; then
    echo "Error: Recipient has no Lightning address"
    exit 1
fi

# 2. Create zap request
ZAP_REQUEST=$(echo '{
  "kind": 9734,
  "content": "'$COMMENT'",
  "tags": [
    ["relays", "relay.primal.net", "relay.damus.io"],
    ["amount", "'$((AMOUNT * 1000))'"],
    ["p", "'$RECIPIENT_PUBKEY'"]
  ]
}' | nak event --sec $NOSTR_SECRET_KEY)

# 3. Get LNURL callback
USERNAME=$(echo "$LUD16" | cut -d'@' -f1)
DOMAIN=$(echo "$LUD16" | cut -d'@' -f2)
LNURL_RESPONSE=$(curl -s "https://$DOMAIN/.well-known/lnurlp/$USERNAME")
CALLBACK=$(echo "$LNURL_RESPONSE" | jq -r '.callback')
ALLOWS_NOSTR=$(echo "$LNURL_RESPONSE" | jq -r '.allowsNostr // false')

if [[ "$ALLOWS_NOSTR" != "true" ]]; then
    echo "Error: Recipient doesn't support Nostr zaps"
    exit 1
fi

# 4. Get invoice with zap request
ZAP_REQUEST_ENCODED=$(echo "$ZAP_REQUEST" | jq -r . | python3 -c "import sys, urllib.parse; print(urllib.parse.quote(sys.stdin.read()))")
INVOICE_RESPONSE=$(curl -s "$CALLBACK?amount=$((AMOUNT * 1000))&nostr=$ZAP_REQUEST_ENCODED")
INVOICE=$(echo "$INVOICE_RESPONSE" | jq -r '.pr')

# 5. Pay the invoice
echo "Sending $AMOUNT sat zap to $RECIPIENT_NPUB..."
oa spark send "$INVOICE"
```

---

## Balance and History Management

### Check Balances

```bash
# Quick balance check
oa spark balance

# Detailed balance (by type)
oa spark balance --detailed

# Network status
oa spark status
```

### Payment History

```bash
# Recent payments
oa spark history

# Last 20 payments
oa spark history --limit 20

# Filter by type
oa spark history --type sent
oa spark history --type received
```

### Backup and Recovery

```bash
# Backup wallet state
cp -r ~/.openagents/wallet ~/.openagents/wallet-backup-$(date +%Y%m%d)

# Your mnemonic IS your backup - store it securely
# Wallet can be recovered with just the mnemonic from ~/.openagents/secret.key
```

---

## Network Configuration

### Development (Regtest)

```bash
export SPARK_NETWORK="regtest"
# No API key required
unset BREEZ_API_KEY
```

### Testing (Testnet)

```bash
export SPARK_NETWORK="testnet"  # Maps to Regtest in current implementation
# API key optional
```

### Production (Mainnet)

```bash
export SPARK_NETWORK="mainnet"
export BREEZ_API_KEY="your-production-api-key"
```

---

## Quick Reference

| Action | CLI Command | Environment |
|--------|-------------|-------------|
| Initialize wallet | `oa spark init` | Any |
| Check balance | `oa spark balance` | Any |
| Receive Lightning | `oa spark receive --amount 1000` | Any |
| Receive Spark | `oa spark receive --spark --amount 500` | Any |
| Receive on-chain | `oa spark receive --onchain` | Mainnet/Testnet |
| Send payment | `oa spark send "lnbc..."` | Any |
| Payment history | `oa spark history` | Any |
| Wallet info | `oa spark info` | Any |
| Network status | `oa spark status` | Any |

---

## Troubleshooting

### "Missing API Key" Error

**For Mainnet:**
```bash
# Get API key from Breez and set it
export BREEZ_API_KEY="your-key-here"
echo "your-key-here" > ~/.openagents/.breez_api_key
```

**For Development:**
```bash
# Switch to Regtest (no API key required)
export SPARK_NETWORK="regtest"
oa spark init
```

### "Wallet Not Found" Error

```bash
# Initialize the wallet
oa spark init
```

### "Connection Failed" Error

```bash
# Check network status
oa spark status

# Verify configuration
oa spark info

# For Regtest, ensure you have the local setup
# See crates/spark/docs/REGTEST.md
```

### "Insufficient Balance" Error

```bash
# Check actual balance
oa spark balance --detailed

# For Regtest development, fund your wallet via faucet
# See regtest documentation for faucet setup
```

### "Invalid Mnemonic" Error

```bash
# Verify your secret key format
cat ~/.openagents/secret.key | nak key public

# Regenerate if corrupted
nak key generate > ~/.openagents/secret.key
```

---

## Advanced Configuration

### Custom Storage Location

```bash
export OPENAGENTS_WALLET_DIR="/custom/path/wallet"
mkdir -p "$OPENAGENTS_WALLET_DIR"
```

### Multiple Wallets/Accounts

```bash
# Different wallet for different purposes
export OPENAGENTS_WALLET_DIR=~/.openagents/wallet-trading
oa spark init

export OPENAGENTS_WALLET_DIR=~/.openagents/wallet-zaps  
oa spark init
```

### Integration with External Tools

```bash
# Export wallet info as JSON for external processing
oa spark info --json > wallet_info.json
oa spark balance --json > wallet_balance.json
oa spark history --json --limit 100 > payment_history.json
```

---

## Security Notes

- **Mnemonic Security:** Your `~/.openagents/secret.key` controls both your Nostr identity AND your Bitcoin funds
- **API Key Security:** Store Breez API keys in secure files with restricted permissions (600)
- **Self-Custodial:** You control your private keys; OpenAgents cannot access your funds
- **Cooperative Exit:** You can always recover funds to Bitcoin mainchain if needed
- **FROST Signatures:** All operations use threshold signatures for enhanced security

---

## Resources

- **Breez Spark SDK:** https://sdk-doc-spark.breez.technology/
- **OpenAgents Spark Crate:** `crates/spark/README.md`
- **Configuration Guide:** `crates/spark/docs/CONFIGURATION.md`
- **Regtest Development:** `crates/spark/docs/REGTEST.md`
- **API Reference:** `crates/spark/docs/API.md`
- **NIP-57 (Zaps):** https://github.com/nostr-protocol/nips/blob/master/57.md
- **NIP-06 (Key Derivation):** https://github.com/nostr-protocol/nips/blob/master/06.md
