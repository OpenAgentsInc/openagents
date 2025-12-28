# Plan: Pylon MVP - Persistence & Payments Wiring

## Overview

Wire up the persistence layer to the new job event processing loop and connect the payment flow so providers can actually earn sats.

**Context:**
- Job event processing loop was just fixed (receiver was being dropped)
- SQLite persistence layer already exists but not fully wired
- Payment code exists in DvmService but SparkWallet is never initialized

---

## Current State (Updated)

| Component | Status | Location |
|-----------|--------|----------|
| Provider mode | 90% (job processing works) | `crates/pylon/src/` |
| SQLite persistence | 80% (schema done, events partial) | `crates/pylon/src/db/` |
| Payment flow code | 70% (logic exists, wallet=None) | `crates/compute/src/services/dvm_service.rs` |
| Event handlers | 60% (missing JobStarted, InvoiceCreated) | `crates/pylon/src/cli/start.rs` |
| Wallet integration | 0% (never initialized) | Missing |

---

## Phase 1: Complete Event Persistence

Add missing event handlers to persist all job lifecycle events.

### File: `crates/pylon/src/cli/start.rs` (lines 250-331)

**Add JobStarted handler:**
```rust
DomainEvent::JobStarted { job_id, .. } => {
    if let Err(e) = db.update_job_status(&job_id, "processing") {
        tracing::warn!("Failed to update job status: {}", e);
    }
}
```

**Add InvoiceCreated handler:**
```rust
DomainEvent::InvoiceCreated { job_id, bolt11, amount_msats, .. } => {
    if let Err(e) = db.record_invoice(&job_id, &bolt11, amount_msats) {
        tracing::warn!("Failed to record invoice: {}", e);
    }
}
```

**Enhance PaymentReceived handler:**
```rust
DomainEvent::PaymentReceived { job_id, amount_msats, .. } => {
    earnings_msats += amount_msats;
    // Update job with payment info
    if let Err(e) = db.mark_job_paid(&job_id, amount_msats) {
        tracing::warn!("Failed to mark job paid: {}", e);
    }
}
```

### File: `crates/pylon/src/db/mod.rs`

**Add invoices table to schema (after line 110):**
```sql
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    job_id TEXT REFERENCES jobs(id),
    bolt11 TEXT NOT NULL,
    amount_msats INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    paid_at INTEGER
);
```

**Add new methods:**
- `record_invoice(job_id, bolt11, amount_msats)`
- `mark_invoice_paid(job_id)`
- `mark_job_paid(job_id, amount_msats)`
- `update_job_status(job_id, status)`

---

## Phase 2: Initialize SparkWallet

The DvmService has wallet code but it's never initialized. Wire it up.

### File: `crates/pylon/src/provider.rs`

**Add wallet field to PylonProvider struct (around line 30):**
```rust
pub struct PylonProvider {
    // ... existing fields
    wallet: Option<Arc<SparkWallet>>,
}
```

**Initialize wallet in init_services() (around line 147):**
```rust
async fn init_services(&mut self) -> Result<(), ProviderError> {
    // ... existing code

    // Initialize wallet if configured
    if self.config.enable_payments {
        match SparkWallet::new(&self.config.spark_url, &self.config.spark_token).await {
            Ok(wallet) => {
                let wallet = Arc::new(wallet);
                self.wallet = Some(wallet.clone());
                dvm_service.set_wallet(wallet).await;
                tracing::info!("Spark wallet connected");
            }
            Err(e) => {
                tracing::warn!("Failed to connect wallet: {}", e);
                // Continue without wallet - free mode
            }
        }
    }
}
```

### File: `crates/pylon/src/config.rs`

**Add wallet config fields:**
```rust
pub struct PylonConfig {
    // ... existing fields
    pub enable_payments: bool,
    pub spark_url: Option<String>,
    pub spark_token: Option<String>,
    pub require_payment: bool,
}
```

---

## Phase 3: Payment Monitoring

Add background task to poll for payment confirmations.

### File: `crates/compute/src/services/dvm_service.rs`

**Spawn payment monitor in start() (after line 396):**
```rust
// Spawn payment monitoring task
if self.wallet.read().await.is_some() {
    let wallet = self.wallet.clone();
    let pending_invoices = self.pending_invoices.clone();
    let event_tx = self.event_tx.clone();
    let running = self.running.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        while *running.read().await {
            interval.tick().await;
            // Check each pending invoice
            let invoices = pending_invoices.read().await.clone();
            for (job_id, (bolt11, amount)) in invoices {
                if check_payment(&wallet, &bolt11).await {
                    // Emit PaymentReceived event
                    let _ = event_tx.send(DomainEvent::PaymentReceived {
                        job_id: job_id.clone(),
                        amount_msats: amount,
                        timestamp: Utc::now(),
                    });
                    pending_invoices.write().await.remove(&job_id);
                }
            }
        }
    });
}
```

**Add invoice expiry cleanup (1 hour timeout):**
```rust
// In the same task, clean up expired invoices
for (job_id, (_, _, created_at)) in invoices {
    if Utc::now().signed_duration_since(created_at) > Duration::hours(1) {
        pending_invoices.write().await.remove(&job_id);
        // Emit JobFailed for expired invoice
    }
}
```

---

## Phase 4: CLI Earnings Enhancement

Make `pylon earnings` show real data from the new persistence.

### File: `crates/pylon/src/cli/earnings.rs`

Already implemented - just verify it works with:
- Total earnings from completed jobs
- Pending invoices count
- Jobs by status breakdown

---

## Implementation Order

```
Phase 1 (Event Persistence) ─────┐
                                 │
Phase 2 (Wallet Init) ───────────┼──> Phase 4 (CLI Verify)
                                 │
Phase 3 (Payment Monitor) ───────┘
```

| Phase | Effort | Files |
|-------|--------|-------|
| 1. Event Persistence | Low | start.rs, db/mod.rs |
| 2. Wallet Init | Medium | provider.rs, config.rs |
| 3. Payment Monitor | Medium | dvm_service.rs |
| 4. CLI Verify | Low | earnings.rs |

---

## Key Files Summary

| File | Changes |
|------|---------|
| `crates/pylon/src/cli/start.rs:250-331` | Add JobStarted, InvoiceCreated, enhance PaymentReceived handlers |
| `crates/pylon/src/db/mod.rs:98-161` | Add invoices table, new methods |
| `crates/pylon/src/provider.rs:147-168` | Initialize SparkWallet, pass to DvmService |
| `crates/pylon/src/config.rs` | Add enable_payments, spark_url, spark_token |
| `crates/compute/src/services/dvm_service.rs:396+` | Spawn payment monitor task |

---

## Success Criteria

1. `pylon earnings` shows jobs completed with earnings
2. Provider creates invoices for paid jobs
3. Payment confirmation triggers job processing
4. Expired invoices are cleaned up
5. All job state transitions are persisted
