# Neobank Storage

This document describes how Neobank stores wallet data and state.

## Overview

Neobank provides Cashu ecash wallet functionality for OpenAgents. It stores:

1. **BTC Wallet** - Bitcoin-denominated Cashu tokens
2. **USD Wallet** - USD-denominated Cashu tokens (optional)
3. **Mint Trust Data** - Trusted mint configurations
4. **Reputation Data** - Counterparty reputation scores

## Directory Structure

When used with Pylon, Neobank stores data under the Pylon directory:

```
~/.openagents/pylon/neobank/
├── btc_wallet.redb           # BTC Cashu wallet
├── usd_wallet.redb           # USD Cashu wallet (if configured)
└── mint_trust.json           # Mint trust configuration
```

## Configuration

Neobank is configured via `NeobankConfig`:

```rust
pub struct NeobankConfig {
    /// Data directory for wallet storage
    pub data_dir: PathBuf,

    /// BTC mint URL
    pub btc_mint_url: Url,

    /// USD mint URL (optional)
    pub usd_mint_url: Option<Url>,

    /// Whether treasury agent is enabled
    pub treasury_enabled: bool,

    /// Treasury spread in basis points
    pub treasury_spread_bps: u16,

    /// Settlement timeout in seconds
    pub settlement_timeout_secs: u64,
}
```

### Default Configuration

```rust
impl Default for NeobankConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("~/.openagents/pylon/neobank"),
            btc_mint_url: MintConfig::default_btc_mint().url,
            usd_mint_url: None,
            treasury_enabled: false,
            treasury_spread_bps: 50,  // 0.5%
            settlement_timeout_secs: 60,
        }
    }
}
```

## Wallet Storage

### redb Database

Neobank uses [redb](https://github.com/cberner/redb) for wallet storage:

- **Format**: Embedded key-value database
- **ACID**: Full transaction support
- **Concurrent**: Safe for multi-threaded access
- **Portable**: Single-file storage

### Wallet Files

| File | Purpose | Contents |
|------|---------|----------|
| `btc_wallet.redb` | BTC Cashu wallet | Proofs, pending tokens, mint keys |
| `usd_wallet.redb` | USD Cashu wallet | Proofs, pending tokens, mint keys |

### Data Stored

Each wallet database contains:

1. **Proofs** - Cashu ecash proofs (unspent tokens)
2. **Pending Proofs** - Tokens being spent/received
3. **Mint Keys** - Public keys from the mint
4. **Mint Quotes** - Pending Lightning deposits
5. **Melt Quotes** - Pending Lightning withdrawals

## CashuWallet API

```rust
use neobank::{CashuWallet, Currency};

// Create a wallet
let wallet = CashuWallet::new(
    mint_url,
    Currency::Btc,
    &seed,           // 32-byte seed from identity
    &db_path,        // Path to .redb file
).await?;

// Check balance
let balance = wallet.balance().await?;
println!("Balance: {} sats", balance.value);

// Create deposit invoice
let quote = wallet.create_mint_quote(1000).await?;
println!("Pay this invoice: {}", quote.bolt11);

// Check if paid and mint tokens
let proofs = wallet.mint(&quote.id).await?;

// Send tokens (creates cashu token string)
let token = wallet.send_token(500).await?;

// Receive tokens
let amount = wallet.receive_token(&token).await?;

// Pay Lightning invoice
let melt_quote = wallet.create_melt_quote(&bolt11).await?;
let result = wallet.melt(&melt_quote.id).await?;
```

## Seed Derivation

Wallet seeds are derived from the UnifiedIdentity:

```rust
fn derive_wallet_seed(identity: &UnifiedIdentity) -> [u8; 32] {
    // Use the Nostr private key as the wallet seed
    *identity.private_key_bytes()
}
```

This ensures:
- Same identity = same wallet
- Deterministic key derivation
- Single backup (the mnemonic)

## Mint Configuration

### Default Mints

```rust
impl MintConfig {
    pub fn default_btc_mint() -> Self {
        Self {
            url: Url::parse("https://mint.coinos.io").unwrap(),
            currency: Currency::Btc,
            trusted: true,
        }
    }
}
```

### Mint Trust Service

Tracks which mints are trusted:

```rust
let mut trust_service = MintTrustService::new();

// Trust a mint
trust_service.trust_mint(&mint_url);

// Check trust
if trust_service.is_trusted(&mint_url) {
    // Safe to receive tokens from this mint
}

// Untrust a mint
trust_service.untrust_mint(&mint_url);
```

## Integration with Pylon

When Pylon starts, it initializes Neobank:

```rust
// In Pylon daemon startup
let neobank_config = NeobankConfig {
    data_dir: pylon_dir.join("neobank"),
    ..Default::default()
};

let mut neobank = NeobankService::new(neobank_config);
neobank.init(&identity).await?;
```

### IPC Commands

Neobank is accessed via Pylon's control socket:

| Command | Response |
|---------|----------|
| `NeobankBalance { currency }` | `NeobankBalance { sats }` |
| `NeobankPay { bolt11 }` | `NeobankPayment { preimage }` |
| `NeobankSend { amount, currency }` | `NeobankSend { token }` |
| `NeobankReceive { token }` | `NeobankReceive { amount_sats }` |
| `NeobankStatus` | Treasury status with balances |

### CLI Usage

```bash
# Check balance (via pylon)
pylon neobank balance btc

# Pay invoice
pylon neobank pay lnbc...

# Send tokens
pylon neobank send 1000 btc
# Returns: cashuAeyJ...

# Receive tokens
pylon neobank receive cashuAeyJ...
```

## Treasury Agent

When treasury is enabled, Neobank runs an automated market maker:

```rust
let neobank_config = NeobankConfig {
    treasury_enabled: true,
    treasury_spread_bps: 50,  // 0.5% spread
    ..Default::default()
};
```

### Treasury Features

1. **Market Making** - Provides BTC/USD liquidity
2. **Spread Management** - Configurable bid/ask spread
3. **Auto-rebalancing** - Maintains target ratios
4. **Rate Fetching** - Gets live BTC/USD rates

### Treasury Status

```rust
let status = neobank.get_treasury_status().await?;

println!("BTC: {} sats", status.btc_balance_sats);
println!("USD: {} cents", status.usd_balance_cents);
println!("Rate: {:?}", status.btc_usd_rate);
println!("Active: {}", status.treasury_active);
```

## Backup and Recovery

### Backup

The wallet can be fully recovered from the mnemonic, but backing up wallet files preserves:
- Token history
- Pending operations
- Mint trust settings

```bash
# Backup wallet files
cp -r ~/.openagents/pylon/neobank/ ~/neobank-backup-$(date +%Y%m%d)/
```

### Recovery from Mnemonic

If wallet files are lost:

1. Tokens in your possession are **lost** (Cashu is bearer ecash)
2. Re-initialize with the same mnemonic
3. New wallet starts with zero balance

**Important**: Cashu tokens are bearer instruments. If you lose the wallet before spending tokens, those tokens are lost forever.

### Recovery from Backup

```bash
# Stop pylon
pylon stop

# Restore wallet files
cp -r ~/neobank-backup-20240115/* ~/.openagents/pylon/neobank/

# Restart
pylon start
```

## Security Considerations

### Token Security

- **Bearer Ecash**: Whoever has the tokens can spend them
- **No Reversibility**: Spent tokens cannot be recovered
- **Backup Important**: Loss of wallet = loss of tokens

### Storage Security

```bash
# Restrict access to wallet directory
chmod 700 ~/.openagents/pylon/neobank/
chmod 600 ~/.openagents/pylon/neobank/*.redb
```

### Mint Trust

- Only receive tokens from trusted mints
- Verify mint reputation before trusting
- Mints can be rugged (exit scam with reserves)

## Troubleshooting

### "Wallet not initialized"

Neobank wasn't initialized with an identity:

```rust
let mut neobank = NeobankService::new(config);
neobank.init(&identity).await?;  // Required!
```

### "Mint unreachable"

The mint server is down or URL is wrong:

```bash
# Test mint connectivity
curl https://mint.coinos.io/v1/info
```

### "Insufficient balance"

Not enough tokens for the operation:

```rust
let balance = wallet.balance().await?;
if balance.value < amount_needed {
    // Need to deposit more
}
```

### "Token already spent"

Double-spend detected by mint:

- Token was already redeemed
- Wallet state out of sync
- May need to resync with mint

## Future Enhancements

1. **Multi-mint Support** - Store tokens from multiple mints
2. **Encrypted Storage** - Password-protect wallet files
3. **Watch-only Mode** - View balance without spending capability
4. **Token Export/Import** - Better token management
5. **Offline Signing** - Air-gapped transaction signing
