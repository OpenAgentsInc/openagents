# Marketplace Payment Integration Status

## Overview

The marketplace payment layer is **fully implemented** and ready for use once the Spark SDK becomes available. All payment flows, hold invoice support, preimage verification, and Flow of Funds revenue splitting are complete.

## Implementation Status

### ✅ Completed (Phase 1 of d-008)

1. **Payment Manager** (`src/core/payments.rs`)
   - Spark wallet integration via `PaymentManager::new(wallet)`
   - Pay compute jobs: `pay_compute_job()`
   - Pay skill licenses: `pay_skill_license()`
   - Pay data access: `pay_data_access()`
   - Create invoices: `create_invoice()`
   - Invoice amount parsing from BOLT11
   - Payment status tracking

2. **Hold Invoice Support**
   - Create hold invoices: `create_hold_invoice()`
   - Settle hold invoices: `settle_hold_invoice()`
   - Cancel hold invoices: `cancel_hold_invoice()`
   - Preimage verification: `verify_preimage()`

3. **Flow of Funds Revenue Splitting** (`src/core/revenue.rs`)
   - `RevenueSplitConfig` with default splits:
     - Creator: 55%
     - Compute: 25%
     - Platform: 12%
     - Referrer: 8%
   - `RevenueSplit::calculate()` with rounding handling
   - Automatic referrer share reallocation to creator when no referrer

4. **Mock Payment Service**
   - `MockPaymentService` for testing without Breez/Spark SDK
   - Invoice creation, payment tracking, completion simulation

### ⏸️ Blocked (Awaiting Spark SDK)

The Spark SDK integration at `crates/spark/` is currently stubbed, awaiting the actual `breez-sdk-spark` dependency. All Spark wallet methods return:

```rust
Err(SparkError::InitializationFailed(
    "Spark SDK not available - awaiting spark-sdk integration"
))
```

**What's needed:**
- Add `breez-sdk-spark` dependency to `crates/spark/Cargo.toml`
- Implement `SparkWallet::new()` using `BreezSdk::connect()`
- Implement actual payment methods once SDK is available

See [d-001 directive](../../.openagents/directives/d-001.md) for Spark SDK integration roadmap.

## Usage Examples

### Creating a Payment Manager

```rust
use marketplace::core::payments::PaymentManager;
use openagents_spark::SparkWallet;
use std::sync::Arc;

// With Spark wallet (once SDK is available)
let wallet = SparkWallet::new(signer, config).await?;
let manager = PaymentManager::new(Some(Arc::new(wallet)));

// Without wallet (returns helpful errors)
let manager = PaymentManager::new(None);
```

### Paying for a Compute Job

```rust
// Provider sends invoice via NIP-90 feedback event
let invoice = "lnbc100u1...";

// Consumer pays the job
let payment = manager
    .pay_compute_job("job-123", invoice, None)
    .await?;

println!("Payment status: {:?}", payment.status);
println!("Preimage: {:?}", payment.preimage);
```

### Hold Invoice Flow (Compute Jobs)

```rust
// 1. Consumer creates hold invoice with pre-computed hash
let payment_hash = "abc123...";
let hold_invoice = manager
    .create_hold_invoice(10_000, "Compute job payment", payment_hash)
    .await?;

// 2. Provider pays the hold invoice (funds locked)
// ... payment happens ...

// 3. Provider delivers results
// ... job execution ...

// 4. Consumer verifies results and settles invoice
let preimage = "xyz789...";
let settled = manager
    .settle_hold_invoice(payment_hash, preimage)
    .await?;
```

### Revenue Splitting

```rust
use marketplace::core::revenue::{RevenueSplit, RevenueSplitConfig};

let config = RevenueSplitConfig::default();
let split = RevenueSplit::calculate(
    100_000,  // 100k sats
    &config,
    true,     // has referrer
);

println!("Creator: {} sats", split.creator_sats);  // 55,000
println!("Compute: {} sats", split.compute_sats);  // 25,000
println!("Platform: {} sats", split.platform_sats); // 12,000
println!("Referrer: {} sats", split.referrer_sats); // 8,000

split.verify()?; // Ensures total == gross
```

## Payment Flows

### Compute Job Payment Flow

```
┌─────────┐                    ┌──────────┐
│ Consumer│                    │ Provider │
└────┬────┘                    └────┬─────┘
     │                              │
     │  1. Submit job (NIP-90)      │
     │ ──────────────────────────>  │
     │                              │
     │  2. Payment required event   │
     │     (with invoice)           │
     │ <────────────────────────── │
     │                              │
     │  3. Pay hold invoice         │
     │     (funds locked)           │
     │ ──────────────────────────>  │
     │                              │
     │  4. Execute job              │
     │                              │
     │  5. Deliver result (NIP-90)  │
     │ <────────────────────────── │
     │                              │
     │  6. Verify result            │
     │                              │
     │  7. Settle hold invoice      │
     │     (reveal preimage)        │
     │ ──────────────────────────>  │
     │                              │
     │  8. Revenue split            │
     │     - Creator: 55%           │
     │     - Compute: 25%           │
     │     - Platform: 12%          │
     │     - Referrer: 8%           │
     └──────────────────────────────┘
```

### Skill Purchase Payment Flow

```
1. Browse skills (NIP-89 discovery)
2. Select skill and view license fee
3. Pay invoice to creator
4. Receive encrypted skill via NIP-SA gift wrap
5. Revenue split automatically applied
```

### Data Access Payment Flow

```
1. Browse datasets (NIP-94/95)
2. Purchase access with Lightning payment
3. Receive NIP-44 decryption key
4. Download and decrypt data
5. Revenue split automatically applied
```

## Testing

### Unit Tests

```bash
cargo test -p marketplace payments
cargo test -p marketplace revenue
```

### Integration Tests

Use `MockPaymentService` for E2E tests without Spark SDK:

```rust
use marketplace::core::payments::MockPaymentService;

let mut mock = MockPaymentService::new();

// Create mock invoice
let invoice = mock.create_invoice(10_000, "Test payment");

// Pay mock invoice
let preimage = mock.pay_invoice(&invoice.invoice)?;

assert!(mock.is_paid(&invoice.invoice));
```

## Next Steps

1. **Spark SDK Integration** (blocked on d-001)
   - Add `breez-sdk-spark` crate dependency
   - Implement actual Lightning operations
   - Test with testnet/regtest

2. **Database Persistence**
   - Store payment records in `marketplace.db`
   - Track payment status across restarts
   - Payment history queries

3. **Automatic Revenue Distribution**
   - Split payments on settlement
   - Track earnings per participant
   - Withdrawal to Lightning addresses

4. **HODL Invoice Support**
   - Requires Lightning implementation with hold invoice support
   - Most implementations support this (LND, CLN with plugin)

## Related Files

- `crates/marketplace/src/core/payments.rs` - Payment manager
- `crates/marketplace/src/core/revenue.rs` - Revenue splitting
- `crates/spark/src/wallet.rs` - Spark wallet (stubbed)
- `crates/spark/src/lib.rs` - Spark SDK integration
- `.openagents/directives/d-001.md` - Spark SDK directive
- `.openagents/directives/d-008.md` - Marketplace directive

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Marketplace Payments                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────┐         ┌──────────────────┐         │
│  │ PaymentManager   │────────>│   SparkWallet    │         │
│  │                  │         │  (Breez SDK)     │         │
│  │ - pay_compute    │         │                  │         │
│  │ - pay_skill      │         │ - send_payment   │         │
│  │ - pay_data       │         │ - create_invoice │         │
│  │ - create_invoice │         │ - get_balance    │         │
│  │ - verify_preimage│         └──────────────────┘         │
│  └──────────────────┘                  │                    │
│           │                            │                    │
│           │                            ▼                    │
│           │                   ┌──────────────────┐         │
│           │                   │  Lightning       │         │
│           │                   │  Network         │         │
│           │                   └──────────────────┘         │
│           │                                                 │
│           ▼                                                 │
│  ┌──────────────────┐                                      │
│  │ RevenueSplit     │                                      │
│  │                  │                                      │
│  │ - Calculate      │                                      │
│  │ - Verify         │                                      │
│  │ - Distribute     │                                      │
│  └──────────────────┘                                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```
