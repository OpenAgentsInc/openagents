# LLP Lightning Node Backend (Phase 0)

This document records the initial LLP Lightning node backend decision and the minimal integration
surface required by `docs/plans/hydra-liquidity-engine.md` (channel balances + channel health in LLP
snapshots).

## Decision

**Initial backend: LND via REST (gRPC gateway).**

Rationale:

- LND is our operational standard for Lightning node deployments.
- We already use an LND REST integration pattern elsewhere in this repo (macaroon header + optional
  custom TLS root) and can reuse that operational posture for LLP telemetry.
- This fits operator-managed LLP deployments (cloud servers, dedicated nodes) immediately.

Non-goal (Phase 0):

- Embedded, cross-platform routing node (LDK). We expect to add an LDK backend later for
  desktop-first "any device can be a node" operators, but LND gets LLP channel health online first.
- Multi-node balancing. Phase 0 is read-only telemetry for a single operator-managed LND node;
  later phases will balance channels/liquidity across multiple nodes (and potentially other backend
  types) without changing the quote/receipt surface.

## Configuration (Runtime)

Runtime reads these environment variables:

- `RUNTIME_LLP_LIGHTNING_BACKEND`:
  - `noop` (default): do not query a node; snapshots show `backend=noop`.
  - `lnd`: use LND REST.
- `RUNTIME_LLP_LND_REST_BASE_URL` (required when `backend=lnd`, unless `LND_REST_BASE_URL` is set):
  - example: `https://127.0.0.1:8080`
- `RUNTIME_LLP_LND_REST_MACAROON_HEX` (required when `backend=lnd`, unless `LND_REST_MACAROON_HEX` is set):
  - hex macaroon used for the Phase 0 inspection surface (balances + list channels).
- `RUNTIME_LLP_LND_REST_TLS_CERT_BASE64` (optional, or use `LND_REST_TLS_CERT_BASE64`):
  - PEM/DER cert bytes, base64-encoded (adds a custom root for self-hosted LND).
- `RUNTIME_LLP_LND_REST_TLS_VERIFY` (optional, default `true`, or use `LND_REST_TLS_VERIFY`):
  - when `false`, the client accepts invalid TLS certs (dev only).

Notes:

- Runtime supports the `LND_REST_*` environment variables as a fallback for shared deployment
  posture with the control service, but runtime-preferred keys are the `RUNTIME_LLP_*` variants.

## What Runtime Publishes (Phase 0)

LLP pool snapshots (`assets_json`) include a `lightning` object:

- `backend` (`noop|lnd`)
- coarse channel liquidity totals (sats):
  - `channelTotalSats`, `channelOutboundSats`, `channelInboundSats`
- coarse channel health:
  - `channelCount`, `connectedChannelCount`
- `lastError` (if node queries fail)

This is intentionally coarse; it is enough to validate "channel-backed assets exist" and to
instrument `/stats` dashboards.

## Notes

- OpenAgents uses **LND-only** for Phase 0 LLP. Any non-LND backend support in code is legacy
  migration debt and should not be deployed.
- Spark is a **wallet rail** (quick user wallets) used behind the wallet-executor custody boundary.
  LLP liquidity is channel-backed and node-backed (LND Phase 0).
