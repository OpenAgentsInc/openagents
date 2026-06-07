# Issue 4548 LDK tracked payout reconciliation

Date: 2026-06-07

## Summary

Production Nexus treasury remained degraded with
`continuity_alert:confirmations_stalled` after the local Pylon v0.2 MDK proof
smoke passed. The live status showed one accepted-work payout stuck in the
pending-confirmation path while the LDK wallet runtime was connected and
funded.

The root cause in source was that the live wallet refresh implementation built
a `TreasuryWalletRefreshPlan` containing tracked dispatched payment IDs, but
`load_live_wallet_refresh_result_with_plan` ignored that plan for the LDK live
wallet path. It loaded balances, produced an empty payment list, and handed
that empty snapshot to the ledger reconciliation path. A dispatched payout
with a provider `payment_id` therefore could not become `confirmed` even if
the LDK provider already knew the payment status.

## Evidence

- Public production status reported:
  - `payout_loop_health=degraded`
  - `degraded_reason=continuity_alert:confirmations_stalled`
  - `pending_confirmation_count=1`
  - `tracked_payment_backlog_count=1`
  - `accepted_work_pending_payout_count=1`
  - `wallet_runtime_status=connected`
- An admin treasury refresh did not clear the alert before this code fix.
- The admin projections endpoint did not expose a non-confirmed recent payout
  row in the queried window, so the stale row was not debuggable from that
  projection alone.
- VM-local inspection was blocked by expired local `gcloud` credentials, so
  the immediate fix was derived from the source-level reconciliation path and
  local regression tests.

## Fix

`load_live_wallet_refresh_result_with_plan` now applies the refresh plan on the
LDK live wallet path:

- It creates one LDK client for balances plus tracked payment lookup.
- For each tracked dispatched payment ID, it calls the provider
  `get_payment` API with a bounded timeout.
- It maps provider payment status into the same `PaymentSummary` shape already
  consumed by treasury ledger reconciliation.
- It passes the mapped payments into `apply_wallet_snapshot`, allowing a
  provider-reported `succeeded` payment to confirm the matching dispatched
  payout, record a `treasury.payout.confirmed` receipt, clear pending
  confirmation counts, and count paid accepted-work bitcoin.
- It preserves wallet refresh progress metadata so unresolved tracked payment
  IDs keep the backlog visible instead of disappearing silently.

Payment IDs are not written directly into timeout diagnostics; timeout errors
use the existing treasury hash helper.

## Local verification

Commands run from the `openagents` repo:

```bash
cargo test -p nexus-control ldk_payment_summary_maps_provider_status_for_tracked_refresh
cargo test -p nexus-control tracked_ldk_payment_summary_confirms_dispatched_payout
cargo test -p nexus-control wallet_refresh_plan_tracks_only_unconfirmed_dispatched_payment_ids
cargo test -p nexus-control track_wallet_refresh_payment_deduplicates_and_clears_tracked_ids
cargo test -p nexus-control wallet_refresh_progress
cargo test -p nexus-control recent_dispatched_payments_do_not_degrade_before_confirmation_stall_threshold
cargo check -p nexus-control
```

All passed. The tests emitted existing dead-code warnings in `nexus-control`.

## Production closeout still required

This fix still needs deployment to the live Nexus control service. After
deployment, run an admin treasury refresh and verify
`https://nexus.openagents.com/v1/treasury/status` clears
`continuity_alert:confirmations_stalled`, reduces
`pending_confirmation_count` / `tracked_payment_backlog_count`, and shows no
accepted-work payout stuck pending confirmation.

Do not use the payout-ledger cleanup command for this case unless a separate
inspection proves the row is a non-LDK or unknown-target orphan. The observed
failure mode is a tracked LDK payout reconciliation miss, not a ledger row that
should be retired.
