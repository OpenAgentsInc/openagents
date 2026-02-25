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

## Automated LLP Snapshot Worker (MVP-0)

Runtime can materialize LLP snapshots on a fixed cadence for configured pools:

- `RUNTIME_LIQUIDITY_POOL_SNAPSHOT_WORKER_ENABLED` (default `true`)
- `RUNTIME_LIQUIDITY_POOL_SNAPSHOT_POOL_IDS` (default `llp-main`, CSV)
- `RUNTIME_LIQUIDITY_POOL_SNAPSHOT_INTERVAL_SECONDS` (default `60`)
- `RUNTIME_LIQUIDITY_POOL_SNAPSHOT_JITTER_SECONDS` (default `5`)
- `RUNTIME_LIQUIDITY_POOL_SNAPSHOT_RETENTION_COUNT` (default `120`)

Behavior:

- Generates `partitionKind=llp` snapshots in the background.
- Snapshot generation continues even if LND is unavailable; `assets_json.lightning.lastError` is
  populated.
- Old snapshot rows are pruned to the configured retention count per pool/partition.

## Liquidity Status Endpoint

Runtime exposes `GET /internal/v1/liquidity/status` for operator checks and dashboard integration.

Response includes:

- `wallet_executor_configured` / `wallet_executor_reachable`
- `receipt_signing_enabled`
- `quote_ttl_seconds`
- optional `wallet_status` (passthrough status payload from wallet executor)
- optional `error_code` / `error_message` when wallet executor is unavailable/misconfigured

Liquidity smoke verification command:

```bash
cargo run -p openagents-runtime-service --bin vignette-liquidity-pool-mvp0 -- --liquidity-smoke-only
```

## Quote/Pay Idempotency Semantics

For `POST /internal/v1/liquidity/quote_pay`:

- Same `idempotency_key` + same request fingerprint => returns original quote response.
- Same `idempotency_key` + different request fingerprint => conflict (`409`).

For `POST /internal/v1/liquidity/pay`:

- First caller for `quote_id` executes the external wallet lane.
- Concurrent caller while payment is `in_flight` => conflict (`409`).
- Calls after finalization replay deterministic stored payment + canonical receipt (no double spend).

## Notes

- OpenAgents uses **LND-only** for Phase 0 LLP. Any non-LND backend support in code is legacy
  migration debt and should not be deployed.
- Spark is a **wallet rail** (quick user wallets) used behind the wallet-executor custody boundary.
  LLP liquidity is channel-backed and node-backed (LND Phase 0).

## CEP Credit Policy Configuration (MVP-1)

Runtime credit-envelope policy is fully configurable via environment variables. Defaults are safe
for staging bootstrap and can be tuned per deployment.

- `RUNTIME_CREDIT_MAX_SATS_PER_ENVELOPE` (default `100000`)
- `RUNTIME_CREDIT_MAX_OUTSTANDING_ENVELOPES_PER_AGENT` (default `3`)
- `RUNTIME_CREDIT_MAX_OFFER_TTL_SECONDS` (default `3600`)
- `RUNTIME_CREDIT_UNDERWRITING_HISTORY_DAYS` (default `30`)
- `RUNTIME_CREDIT_UNDERWRITING_BASE_SATS` (default `2000`)
- `RUNTIME_CREDIT_UNDERWRITING_K` (default `150.0`)
- `RUNTIME_CREDIT_UNDERWRITING_DEFAULT_PENALTY_MULTIPLIER` (default `2.0`)
- `RUNTIME_CREDIT_MIN_FEE_BPS` (default `50`)
- `RUNTIME_CREDIT_MAX_FEE_BPS` (default `2000`)
- `RUNTIME_CREDIT_FEE_RISK_SCALER` (default `400.0`)
- `RUNTIME_CREDIT_HEALTH_WINDOW_SECONDS` (default `21600`)
- `RUNTIME_CREDIT_HEALTH_SETTLEMENT_SAMPLE_LIMIT` (default `200`)
- `RUNTIME_CREDIT_HEALTH_LN_PAY_SAMPLE_LIMIT` (default `200`)
- `RUNTIME_CREDIT_CIRCUIT_BREAKER_MIN_SAMPLE` (default `5`)
- `RUNTIME_CREDIT_LOSS_RATE_HALT_THRESHOLD` (default `0.5`)
- `RUNTIME_CREDIT_LN_FAILURE_RATE_HALT_THRESHOLD` (default `0.5`)
- `RUNTIME_CREDIT_LN_FAILURE_LARGE_SETTLEMENT_CAP_SATS` (default `5000`)

Operator introspection:

- `GET /internal/v1/credit/health` returns:
  - breaker status (`halt_new_envelopes`, `halt_large_settlements`)
  - settlement/LN sample counts used in breaker decisions
  - effective policy values under `policy`

## CEP API Surface (MVP-1)

Authoritative internal runtime endpoints:

- `POST /internal/v1/credit/intent`
- `POST /internal/v1/credit/offer`
- `POST /internal/v1/credit/envelope`
- `POST /internal/v1/credit/settle`
- `GET /internal/v1/credit/health`
- `GET /internal/v1/credit/agents/{agent_id}/exposure`

Idempotency/conflict semantics:

- `credit/intent`: idempotent by `idempotency_key` + request fingerprint; payload drift returns `409`.
- `credit/offer`: deterministic offer id from request fingerprint; policy/scope drift returns `409`.
- `credit/envelope`: deterministic envelope id from `(offer_id, provider_id)`; consumed/invalid offer returns `409`.
- `credit/settle`: idempotent per `(envelope_id, request fingerprint)` with deterministic replay; conflicting settle payload returns `409`.

Required settlement linkage fields (`credit/settle`):

- `verification_passed`
- `verification_receipt_sha256`
- `provider_invoice`
- `provider_host`
- `policy_context` (scope/job/policy linkage payload)

Served OpenAPI contract:

- `GET /internal/v1/openapi.json` (runtime-served JSON projection of `apps/runtime/docs/openapi-internal-v1.yaml`)
- CEP settle response includes deterministic `settlement_id` in addition to envelope/outcome/receipt fields.
- Hydra MVP-2 observability endpoint:
  - `GET /internal/v1/hydra/observability`

MVP-1 verification commands:

```bash
cargo test -p openagents-runtime-service --no-fail-fast
cargo run -p openagents-runtime-service --bin vignette-neobank-pay-bolt11
./scripts/vignette-hydra-mvp2.sh
```

Harness artifact:

- `output/vignettes/neobank-pay-bolt11/<run_id>/summary.json`
