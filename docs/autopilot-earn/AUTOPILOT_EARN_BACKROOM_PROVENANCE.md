# Autopilot Earn Backroom Provenance

Date: 2026-03-04
Issue: #2820

This file records each restored backroom block, source path, and adaptation notes.

Scope note: restored blocks listed here support the compute-provider MVP lane; no liquidity-solver components were restored.

## Restored Blocks

### 1) NIP-90 submit + await helper sequencing

- Source: `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/main.rs` (`submit_nip90_text_generation`)
- Destination: `crates/nostr/client/src/dvm.rs`
- Restored behavior:
  - publish request event,
  - subscribe for matching result events,
  - await correlated result with timeout,
  - return terminal timeout error when no result arrives.
- Modifications from source:
  - moved from app binary function to reusable crate helper,
  - adapted to current `RelayPool`/`RelayMessage` transport,
  - uses request correlation through `e` tag matching.

### 2) Provider domain lifecycle mapping patterns

- Source: `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/provider_domain.rs`
- Destination: `apps/autopilot-desktop/src/state/provider_runtime.rs` + Mission Control projection in `apps/autopilot-desktop/src/render.rs`
- Restored behavior:
  - explicit mode transitions (`offline/connecting/online/degraded`),
  - heartbeat/uptime visibility,
  - last authoritative status + error detail surfaces.
- Modifications from source:
  - no direct dependency on archived `pylon` runtime/db APIs,
  - retained as app-owned projection state per MVP boundaries.

### 3) Wallet bridge normalization patterns

- Source: `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/wallet_domain.rs`
- Destination: `apps/autopilot-desktop/src/spark_wallet.rs`, `apps/autopilot-desktop/src/state/earnings_gate.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- Restored behavior:
  - authoritative wallet status/balance/payment history as payout source,
  - payout correlation via wallet payment pointer,
  - rejection of synthetic pointers for settlement claims.
- Modifications from source:
  - reused current `SparkWalletWorker` and MVP state lanes,
  - payout gate wired to wallet evidence rather than synthetic placeholders.

## Explicit Non-Restores

- `crates/pylon/*` and `crates/compute/*` were intentionally not restored into this MVP repo.
- Archived web/app surfaces outside MVP ownership boundaries were not imported.
