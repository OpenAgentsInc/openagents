# Settlement Protocol

Documentation for the neobank settlement engine, which handles the actual transfer of funds between trading counterparties.

## Overview

The settlement module provides multiple settlement modes for agent-to-agent trades:

| Mode | Security | Speed | Use Case |
|------|----------|-------|----------|
| Mock | None | Instant | Testing |
| ReputationBased | Trust-based | Fast | Established traders |
| AtomicP2PK | Cryptographic | Medium | High-value trades |

## Settlement Modes

### Mock Mode

For testing and development. Simulates settlement without moving real funds.

```rust
use neobank::settlement::SettlementEngine;

let engine = SettlementEngine::new_mock();

// Settle a trade instantly
let receipt = engine.settle(&trade).await?;
assert_eq!(receipt.method, SettlementMethod::Mock);
```

### Reputation-Based Settlement (v0)

Trust-based settlement where the higher-reputation party pays first. This is the recommended mode for established traders with good reputation.

```rust
use neobank::{
    settlement::{SettlementEngine, SettlementMode},
    wallet::CashuWallet,
    reputation::ReputationService,
};
use std::sync::Arc;
use std::time::Duration;
use std::path::Path;

// Create wallets
let btc_wallet = Arc::new(CashuWallet::new(
    "https://mint.cashu.space",
    Currency::Btc,
    &seed,
    Path::new("btc_wallet.db"),
).await?);

// Create settlement engine with reputation mode
let engine = SettlementEngine::new_reputation_based(
    btc_wallet,
    Duration::from_secs(60),  // 60 second timeout
);

// Settlement flow:
// 1. Query both parties' reputation
// 2. Higher-reputation party sends proofs first
// 3. Lower-reputation party sends after receiving
// 4. Both publish attestations
let receipt = engine.settle(&trade).await?;
```

**Flow Diagram:**

```
┌─────────────┐                    ┌─────────────┐
│   Seller    │                    │   Buyer     │
│ (higher rep)│                    │ (lower rep) │
└──────┬──────┘                    └──────┬──────┘
       │                                  │
       │ 1. Send BTC proofs ──────────────►
       │                                  │
       │ 2. Verify proofs                 │
       │                                  │
       ◄────────── 3. Send USD proofs ────│
       │                                  │
       │ 4. Verify proofs                 │
       │                                  │
       │ 5. Both publish attestations     │
       ▼                                  ▼
   ┌─────────┐                      ┌─────────┐
   │ Success │                      │ Success │
   └─────────┘                      └─────────┘
```

### Atomic P2PK Settlement (v1)

Cryptographically secure settlement using P2PK (Pay-to-Public-Key) locked proofs with HTLC-like timeouts. Recommended for high-value trades or unknown counterparties.

```rust
use neobank::settlement::{SettlementEngine, SettlementMode};
use std::time::Duration;

let engine = SettlementEngine::new_atomic_p2pk(
    btc_wallet,
    Duration::from_secs(300),  // 5 minute HTLC timeout
);

// Atomic flow using locked proofs:
// 1. Both parties lock proofs to each other's pubkeys
// 2. Proofs are released atomically via preimage exchange
// 3. Either both succeed or both fail (timeout refund)
let receipt = engine.settle(&trade).await?;
```

**Flow Diagram:**

```
┌─────────────┐                    ┌─────────────┐
│   Seller    │                    │   Buyer     │
└──────┬──────┘                    └──────┬──────┘
       │                                  │
       │ 1. Lock BTC to buyer_pubkey ─────►
       │    (with timeout)                │
       │                                  │
       ◄──── 2. Lock USD to seller_pubkey │
       │        (with timeout)            │
       │                                  │
       │ 3. Unlock BTC (has seller key)   │
       │                                  │
       │ 4. Unlock USD ◄──────────────────│
       │    (has buyer key)               │
       │                                  │
       ▼                                  ▼
   ┌─────────┐                      ┌─────────┐
   │ Success │                      │ Success │
   └─────────┘                      └─────────┘

   OR (timeout case)

       │                                  │
       │ 5. Timeout reached               │
       │                                  │
       │ 6. Claim back own proofs         │
       ▼                                  ▼
   ┌──────────┐                    ┌──────────┐
   │ Refunded │                    │ Refunded │
   └──────────┘                    └──────────┘
```

## Types

### SettlementMode

```rust
pub enum SettlementMode {
    /// Mock settlement for testing
    Mock,
    /// Reputation-based (higher rep pays first)
    ReputationBased { timeout: Duration },
    /// Atomic P2PK with HTLC-like timeout
    AtomicP2PK { htlc_timeout: Duration },
}
```

### SettlementMethod

```rust
pub enum SettlementMethod {
    /// Mock (no actual transfer)
    Mock,
    /// Reputation-based transfer
    ReputationBased,
    /// Atomic P2PK transfer
    AtomicCashu,
}
```

### SettlementReceipt

```rust
pub struct SettlementReceipt {
    /// Trade ID
    pub trade_id: String,
    /// Method used
    pub method: SettlementMethod,
    /// Amount settled (sats)
    pub amount_sats: u64,
    /// How long settlement took
    pub duration: Duration,
    /// Method-specific proof (txid, preimage, etc.)
    pub proof: Option<String>,
}
```

### SettlementStatus

```rust
pub enum SettlementStatus {
    /// Not yet started
    Pending,
    /// Proofs locked, awaiting counterparty
    Locked,
    /// Settlement complete
    Completed,
    /// Settlement failed (timeout, etc.)
    Failed,
    /// Refunded after timeout
    Refunded,
}
```

### LockedProof

```rust
pub struct LockedProof {
    /// Lock ID
    pub id: String,
    /// Locked proofs as token string
    pub token: String,
    /// Recipient pubkey
    pub recipient_pubkey: String,
    /// Lock expiration
    pub expires_at: u64,
    /// Current status
    pub status: LockStatus,
}

pub enum LockStatus {
    Locked,
    Unlocked,
    Expired,
}
```

### TokenTransfer

```rust
pub struct TokenTransfer {
    /// Transfer ID
    pub id: String,
    /// Sender pubkey
    pub from_pubkey: String,
    /// Recipient pubkey
    pub to_pubkey: String,
    /// Amount in sats
    pub amount_sats: u64,
    /// Token string
    pub token: String,
    /// When created
    pub created_at: u64,
    /// Current status
    pub status: TransferStatus,
}

pub enum TransferStatus {
    Pending,
    Sent,
    Received,
    Failed,
}
```

## SettlementEngine API

### Construction

```rust
impl SettlementEngine {
    /// Create a mock settlement engine for testing
    pub fn new_mock() -> Self;

    /// Create a reputation-based settlement engine
    ///
    /// # Arguments
    /// * `wallet` - Cashu wallet for token operations
    /// * `timeout` - Maximum time to wait for counterparty
    pub fn new_reputation_based(
        wallet: Arc<CashuWallet>,
        timeout: Duration,
    ) -> Self;

    /// Create an atomic P2PK settlement engine
    ///
    /// # Arguments
    /// * `wallet` - Cashu wallet for token operations
    /// * `htlc_timeout` - Timeout for HTLC-like locks
    pub fn new_atomic_p2pk(
        wallet: Arc<CashuWallet>,
        htlc_timeout: Duration,
    ) -> Self;

    /// Get the settlement mode
    pub fn mode(&self) -> SettlementMode;
}
```

### Settlement Operations

```rust
impl SettlementEngine {
    /// Settle a trade
    ///
    /// Uses the configured settlement mode to transfer funds.
    pub async fn settle(&self, trade: &Trade) -> Result<SettlementReceipt>;

    /// Settle using reputation-based flow
    ///
    /// Higher-reputation party pays first.
    pub async fn settle_reputation(&self, trade: &Trade) -> Result<SettlementReceipt>;

    /// Settle using atomic P2PK flow
    ///
    /// Both parties lock proofs, then unlock atomically.
    pub async fn settle_atomic(&self, trade: &Trade) -> Result<SettlementReceipt>;
}
```

### Token Operations

```rust
impl SettlementEngine {
    /// Send proofs to a counterparty
    ///
    /// Creates a token and transfers it (simulated in mock mode).
    pub async fn send_proofs(
        &self,
        to_pubkey: &str,
        amount_sats: u64,
        mint_url: &Url,
    ) -> Result<String>;

    /// Receive proofs from a counterparty
    ///
    /// Verifies and stores the token.
    pub async fn receive_proofs(&self, token: &str) -> Result<u64>;

    /// Lock proofs with P2PK
    ///
    /// Locks proofs to a recipient's pubkey with timeout.
    pub async fn lock_proofs_p2pk(
        &self,
        recipient_pubkey: &str,
        amount_sats: u64,
        timeout: Duration,
    ) -> Result<LockedProof>;

    /// Unlock P2PK proofs
    ///
    /// Uses private key to unlock proofs locked to our pubkey.
    pub async fn unlock_proofs(&self, locked: &LockedProof) -> Result<u64>;
}
```

### Status and History

```rust
impl SettlementEngine {
    /// Get status of a settlement
    pub async fn get_status(&self, trade_id: &str) -> Result<SettlementStatus>;

    /// Get a locked proof by ID
    pub async fn get_locked_proof(&self, lock_id: &str) -> Result<Option<LockedProof>>;

    /// Get a transfer by ID
    pub async fn get_transfer(&self, transfer_id: &str) -> Result<Option<TokenTransfer>>;

    /// List all locked proofs
    pub async fn list_locked_proofs(&self) -> Result<Vec<LockedProof>>;

    /// List all transfers
    pub async fn list_transfers(&self) -> Result<Vec<TokenTransfer>>;
}
```

## Integration with ExchangeClient

The `ExchangeClient` uses a `SettlementEngine` internally:

```rust
use neobank::{ExchangeClient, SettlementEngine};
use std::sync::Arc;

// Create settlement engine
let settlement = SettlementEngine::new_reputation_based(
    wallet,
    Duration::from_secs(60),
);

// Create exchange with relay and settlement
let relay = Arc::new(ExchangeRelay::connect(&["wss://relay.example.com"]).await?);
let exchange = ExchangeClient::new_with_relay(
    "my_pubkey",
    secret_key,
    settlement,
    relay,
);

// Settle a trade
let trade = exchange.accept_order(&order_id).await?;
let receipt = exchange.settle(&trade).await?;
```

## Error Handling

Settlement-specific errors:

```rust
use neobank::{Error, Result};

match engine.settle(&trade).await {
    Ok(receipt) => {
        println!("Settlement complete: {:?}", receipt);
    }
    Err(Error::Network(msg)) => {
        // Network issue - may retry
        eprintln!("Network error: {}", msg);
    }
    Err(Error::Timeout) => {
        // Counterparty didn't respond in time
        // Proofs will be refunded automatically
        eprintln!("Settlement timed out");
    }
    Err(Error::Database(msg)) => {
        // Internal error
        eprintln!("Database error: {}", msg);
    }
    Err(e) => {
        eprintln!("Unknown error: {:?}", e);
    }
}
```

## Timeout Handling

### Reputation-Based Mode

If the counterparty doesn't send proofs within the timeout:

1. The trade is marked as failed
2. The sending party keeps their proofs
3. A `Default` attestation is published

```rust
// 60 second timeout
let engine = SettlementEngine::new_reputation_based(wallet, Duration::from_secs(60));

// If counterparty doesn't respond in 60s, this returns Err(Error::Timeout)
match engine.settle(&trade).await {
    Err(Error::Timeout) => {
        // Publish default attestation
        exchange.attest_trade(&trade, TradeOutcome::Default, 60_000).await?;
    }
    _ => {}
}
```

### Atomic P2PK Mode

With atomic P2PK, locked proofs automatically become reclaimable after timeout:

1. Both parties lock proofs
2. If one party disappears, the timeout expires
3. Original owners reclaim their locked proofs
4. No funds are lost

```rust
// 5 minute HTLC timeout
let engine = SettlementEngine::new_atomic_p2pk(wallet, Duration::from_secs(300));

// Lock proofs
let locked = engine.lock_proofs_p2pk(
    &counterparty_pubkey,
    10_000,
    Duration::from_secs(300),
).await?;

// After timeout, we can reclaim
if locked.expires_at < now() {
    let amount = engine.unlock_proofs(&locked).await?;
    println!("Reclaimed {} sats", amount);
}
```

## Dispute Resolution

For disputed trades, the escrow module provides collateral:

```rust
use neobank::{EscrowService, TradeSide};

// Create escrow with 5% bond
let escrow = escrow_service.create_escrow(&trade_id, trade_amount, Some(0.05)).await?;

// Fund bonds
escrow_service.fund_escrow(&escrow.id, TradeSide::Maker, &maker_pubkey).await?;
escrow_service.fund_escrow(&escrow.id, TradeSide::Taker, &taker_pubkey).await?;

// If dispute occurs
let dispute_id = escrow_service.initiate_dispute(&escrow.id, &initiator, "Reason").await?;

// Resolve dispute (typically by oracle/mediator)
escrow_service.resolve_dispute(&dispute_id, &winner_pubkey, Some("Resolution notes")).await?;
```

## Security Considerations

### Reputation-Based Mode

- **Trust Assumption**: Higher-reputation party is trusted to pay first
- **Risk**: First payer takes counterparty risk
- **Mitigation**: Use only with established traders; use escrow for larger amounts
- **Recommendation**: Limit to trades under 100k sats with new counterparties

### Atomic P2PK Mode

- **Cryptographic Security**: Uses secp256k1 P2PK locks
- **Timeout Safety**: Locked proofs are reclaimable after timeout
- **No Third Party**: True P2P without trusted intermediary
- **Recommendation**: Preferred for high-value trades or unknown counterparties

### General Best Practices

1. **Verify Counterparty Reputation** before trading
2. **Use Escrow** for amounts over 100k sats with new counterparties
3. **Set Appropriate Timeouts** (60s for reputation, 5min for atomic)
4. **Publish Attestations** immediately after settlement
5. **Monitor Locked Proofs** and reclaim timed-out locks

## Testing

```bash
# Run settlement tests
cargo test -p neobank settlement

# Run integration tests
cargo test -p neobank --test integration_tests test_settlement

# Run with live mint (requires setup)
cargo test -p neobank --test mint_integration
```

## See Also

- [EXCHANGE-API.md](./EXCHANGE-API.md) - Exchange client documentation
- [escrow.rs](../src/escrow.rs) - Escrow and collateral system
- [reputation.rs](../src/reputation.rs) - Reputation scoring
- [wallet.rs](../src/wallet.rs) - Cashu wallet operations
