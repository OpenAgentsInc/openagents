# Pylon Persistence & Payments Implementation

**Date:** 2025-12-28 09:29 CST
**Commit:** a2375b909

## Overview

Implemented the full persistence and payments wiring for Pylon MVP, enabling providers to track jobs, create invoices, monitor payments, and record earnings.

---

## Phase 1: Event Persistence

### Changes

**File: `crates/pylon/src/cli/start.rs`**
- Added `JobStarted` handler → updates job status to "processing"
- Added `InvoiceCreated` handler → records invoice in database
- Enhanced `PaymentReceived` handler → marks invoice as paid

**File: `crates/pylon/src/db/mod.rs`**
- Added migration `002_invoices` with new table:
```sql
CREATE TABLE invoices (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(id),
    bolt11 TEXT NOT NULL,
    amount_msats INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'paid', 'expired', 'cancelled')),
    paid_amount_msats INTEGER,
    created_at INTEGER NOT NULL,
    paid_at INTEGER
);
```

**File: `crates/pylon/src/db/jobs.rs`**
- Added `record_invoice(job_id, bolt11, amount_msats)`
- Added `mark_invoice_paid(job_id, amount_msats)`
- Added `count_pending_invoices()`
- Added `expire_old_invoices(max_age_secs)`

---

## Phase 2: Wallet Initialization

### Changes

**File: `crates/pylon/src/config.rs`**
- Added config fields:
  - `enable_payments: bool` - Enable payment processing
  - `spark_url: Option<String>` - Spark wallet URL
  - `spark_token: Option<String>` - Spark auth token

**File: `crates/pylon/src/provider.rs`**
- Added `wallet: Option<Arc<SparkWallet>>` field to `PylonProvider`
- Added `init_wallet()` method that:
  - Converts network string to `SparkNetwork`
  - Creates `WalletConfig` with storage directory
  - Clones `SparkSigner` from `UnifiedIdentity`
  - Creates `SparkWallet` instance
- Modified `init_services()` to initialize wallet when `enable_payments` is true
- Wallet is passed to `DvmService.set_wallet()`

---

## Phase 3: Payment Monitoring

### Changes

**File: `crates/compute/src/services/dvm_service.rs`**
- Added background task after job processing task spawn:
  - Runs every 5 seconds
  - Gets list of pending invoices
  - For each invoice, lists recent payments via `SparkWallet`
  - Matches payments by amount and completed status
  - When payment found:
    - Removes from pending invoices
    - Emits `PaymentReceived` event
    - Updates job status to `Pending`
    - Processes job (runs inference)
    - Publishes result to relays
  - Expires invoices older than 1 hour:
    - Marks job as failed
    - Emits `JobFailed` event

---

## Phase 4: Test Fixes

### Changes

**Files:**
- `crates/compute/tests/payment_integration.rs`
- `crates/compute/tests/payment_stress.rs`

Added `network: "regtest".to_string()` to `DvmConfig` struct initializations where missing.

---

## Test Results

- **Pylon tests:** 23 passed
- **Compute tests:** 43 passed

---

## Configuration

To enable payments in Pylon:

```toml
# ~/.config/pylon/config.toml
enable_payments = true
# spark_url = "https://localhost:9737"  # Optional
# spark_token = "your-token"  # Optional
```

When `enable_payments = true`:
1. Wallet is initialized from provider identity
2. Incoming jobs create invoices
3. Payment monitoring task checks for payments
4. Jobs are processed after payment confirmation

When `enable_payments = false` (default):
- Provider runs in "free mode"
- Jobs are processed immediately without payment

---

## Files Changed

| File | Lines Added | Description |
|------|-------------|-------------|
| `crates/pylon/src/cli/start.rs` | +19 | Event handlers |
| `crates/pylon/src/db/mod.rs` | +20 | Invoices migration |
| `crates/pylon/src/db/jobs.rs` | +58 | Invoice methods |
| `crates/pylon/src/config.rs` | +12 | Wallet config |
| `crates/pylon/src/provider.rs` | +55 | Wallet initialization |
| `crates/compute/src/services/dvm_service.rs` | +172 | Payment monitoring |
| `crates/compute/tests/*.rs` | +4 | Test fixes |
| **Total** | **+337** | |

---

## Next Steps

1. Test with real SparkWallet on regtest
2. Add invoice status to NIP-90 status events
3. Implement NIP-89 handler recommendation using payment info
4. Add CLI commands for invoice management
