# LLP Lightning Node Backend (Phase 0)

This document records the initial LLP Lightning node backend decision and the minimal integration
surface required by `docs/plans/draft/liquidity-pool.md` (channel balances + channel health in LLP
snapshots).

## Decision

**Initial backend: Core Lightning (CLN) via JSON-RPC over the `lightning-rpc` Unix socket.**

Rationale:

- CLN has a stable, well-scoped JSON-RPC surface for channel and funds inspection.
- A Rust client exists (`cln-rpc`) which keeps this integration Rust-native and testable.
- This fits operator-managed LLP deployments (cloud servers, dedicated nodes) immediately.

Non-goal (Phase 0):

- Embedded, cross-platform routing node (LDK). We expect to add an LDK backend later for
  desktop-first "any device can be a node" operators, but CLN gets LLP channel health online first.

## Configuration (Runtime)

Runtime reads these environment variables:

- `RUNTIME_LLP_LIGHTNING_BACKEND`:
  - `noop` (default): do not query a node; snapshots show `backend=noop`.
  - `cln`: use CLN JSON-RPC.
- `RUNTIME_LLP_CLN_RPC_PATH` (required when `backend=cln`):
  - path to the `lightning-rpc` socket (example: `/home/user/.lightning/bitcoin/lightning-rpc`)
- `RUNTIME_LLP_PEER_ALLOWLIST` (optional):
  - comma-separated peer pubkeys allowed for channel operations.
- `RUNTIME_LLP_MAX_CHANNEL_SATS_PER_PEER` (optional, default `5_000_000`):
  - hard cap for `open_channel`.
- `RUNTIME_LLP_MAX_DAILY_REBALANCE_SATS` (optional, default `500_000`):
  - hard cap for `rebalance` budget (Phase 0 rebalancing is an explicit "unsupported" error).

## What Runtime Publishes (Phase 0)

LLP pool snapshots (`assets_json`) include a `lightning` object:

- `backend` (`noop|cln`)
- coarse channel liquidity totals (sats):
  - `channelTotalSats`, `channelOutboundSats`, `channelInboundSats`
- coarse channel health:
  - `channelCount`, `connectedChannelCount`
- `lastError` (if node queries fail)

This is intentionally coarse; it is enough to validate "channel-backed assets exist" and to
instrument `/stats` dashboards.

