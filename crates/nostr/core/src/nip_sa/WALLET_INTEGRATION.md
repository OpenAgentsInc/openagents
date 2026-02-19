# Wallet Integration for NIP-SA Agent State

This document describes the integration between NIP-SA agent state and the Spark wallet.

## Overview

Agents need to track their available funds to make autonomous decisions about spending. The `wallet_integration` module provides functions to:

1. Query current balance from Spark SDK
2. Update agent state with balance
3. Make balance available for decision-making

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent State Update                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Query Balance                                           │
│     ┌──────────────┐      ┌──────────────────┐             │
│     │ Agent State  │─────►│  Spark Wallet    │             │
│     │              │      │  (openagents-    │             │
│     │              │      │   spark crate)   │             │
│     └──────────────┘      └──────────────────┘             │
│            │                       │                        │
│            │                       ▼                        │
│            │              ┌──────────────────┐             │
│            │              │ Breez SDK        │             │
│            │              │ (spark-sdk)      │             │
│            │              └──────────────────┘             │
│            │                       │                        │
│            │                       ▼                        │
│            │              Balance{                          │
│            │                spark_sats: 100000,             │
│            │                lightning_sats: 50000,          │
│            │                onchain_sats: 0                 │
│            │              }                                 │
│            │                       │                        │
│  2. Update State                  │                        │
│            ▼◄──────────────────────┘                        │
│     state.wallet_balance_sats = 150000                     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### With Spark Integration (Feature Flag)

When the `spark-integration` feature is enabled and the Spark SDK is available:

```rust
use nostr::nip_sa::{AgentStateContent, update_wallet_balance};

// Initialize agent state
let mut state = AgentStateContent::new();

// Query balance from Spark wallet and update state
let balance = update_wallet_balance(&mut state).await?;

println!("Spark L2: {} sats", balance.spark_sats);
println!("Lightning: {} sats", balance.lightning_sats);
println!("On-chain: {} sats", balance.onchain_sats);
println!("Total: {} sats", state.wallet_balance_sats);
```

### Manual Balance Updates (Testing/Fallback)

When Spark integration is not available, use manual updates:

```rust
use nostr::nip_sa::{AgentStateContent, update_wallet_balance_manual};

let mut state = AgentStateContent::new();

// Manually set balance (e.g., from cached value or test data)
update_wallet_balance_manual(&mut state, 100_000);

assert_eq!(state.wallet_balance_sats, 100_000);
```

### Read-Only Balance Queries

Query balance without modifying state:

```rust
use nostr::nip_sa::query_wallet_balance;

let balance = query_wallet_balance().await?;
println!("Current balance: {} sats", balance.total_sats());
```

## Balance Breakdown

The `WalletBalance` struct provides granular balance information:

- `spark_sats`: Funds in Spark Layer 2 (instant, low-fee transfers)
- `lightning_sats`: Funds in Lightning channels (fast payments)
- `onchain_sats`: Funds in cooperative exit or on-chain wallet

## Integration Points

### Autopilot Tick Cycle

Agents should update their balance at the start of each tick:

```rust
// At tick start
let balance = update_wallet_balance(&mut state).await?;

// Make decisions based on available funds
if state.wallet_balance_sats < MIN_BALANCE_SATS {
    state.add_memory(MemoryEntry::new(
        "warning",
        "Low balance - pausing non-essential tasks"
    ));
}
```

### Budget Enforcement

Before spending operations, check available balance:

```rust
fn can_afford_operation(state: &AgentStateContent, cost_sats: u64) -> bool {
    state.wallet_balance_sats >= cost_sats
}
```

### State Encryption

Balance is included in encrypted state (kind:39201), so it's:
- Confidential (encrypted to agent pubkey)
- Threshold-protected (requires marketplace signer to decrypt)
- Persistent across ticks

## Implementation Status

### ✅ Completed
- `AgentStateContent` has `wallet_balance_sats` field
- `update_balance()` method for setting balance
- `wallet_integration` module with async query functions
- Manual balance update for testing

### 🚧 Blocked (Waiting on Spark SDK Integration)
- `update_wallet_balance()` implementation (d-001)
- `query_wallet_balance()` implementation (d-001)
- Wallet singleton/context management

The Spark SDK (`~/code/spark-sdk`) integration is tracked in directive d-001. Once the SDK is integrated into `crates/spark/`, the async functions will be implemented.

## Related Files

- `crates/nostr/core/src/nip_sa/state.rs` - Agent state types
- `crates/nostr/core/src/nip_sa/wallet_integration.rs` - Balance query functions
- `crates/spark/src/wallet.rs` - Spark wallet wrapper
- `~/code/spark-sdk/` - Breez Spark SDK (external)

## Testing

### Unit Tests

```rust
#[test]
fn test_balance_calculation() {
    let balance = WalletBalance {
        spark_sats: 100_000,
        lightning_sats: 50_000,
        onchain_sats: 25_000,
    };
    assert_eq!(balance.total_sats(), 175_000);
}

#[test]
fn test_manual_balance_update() {
    let mut state = AgentStateContent::new();
    update_wallet_balance_manual(&mut state, 100_000);
    assert_eq!(state.wallet_balance_sats, 100_000);
}
```

### Integration Tests (When Spark SDK Available)

```rust
#[tokio::test]
async fn test_balance_query_and_update() {
    let mut state = AgentStateContent::new();
    let balance = update_wallet_balance(&mut state).await.unwrap();

    assert_eq!(state.wallet_balance_sats, balance.total_sats());
    assert!(state.wallet_balance_sats > 0);
}
```

## Future Enhancements

1. **Balance History**: Track balance changes over time in agent memory
2. **Spending Analytics**: Track what agent spent funds on
3. **Low Balance Alerts**: Automatic notifications when balance drops below threshold
4. **Multi-Currency**: Support for other assets beyond Bitcoin
