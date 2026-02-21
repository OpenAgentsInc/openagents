# Agent Payments + Spark + L402 (Rust Services)

Status: current architecture note.

## Ownership

1. Policy/reconcile: `apps/lightning-ops/`
2. Wallet execution: `apps/lightning-wallet-executor/`
3. Control/runtime coordination: Rust control/runtime services

## Invariants

1. Payment authority is service-side; clients consume receipts/projections.
2. No client-side secret custody.
3. Receipts remain replayable and auditable.

Use `docs/ARCHITECTURE-RUST.md` and lightning runbooks under `docs/lightning/` for current operational detail.
