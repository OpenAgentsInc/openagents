# Domain Persistence (OA-WEBPARITY-010)

Rust persistence module for cross-domain Laravel parity groundwork.

Module:
- `apps/openagents.com/service/src/domain_store.rs`

Config:
- `OA_DOMAIN_STORE_PATH` (optional JSON snapshot path)

## Covered Domain Families

- Autopilot:
  - `autopilots`
  - `autopilot_profiles`
  - `autopilot_policies`
  - `autopilot_runtime_bindings`
- L402:
  - `l402_credentials`
  - `l402_paywalls`
  - `user_spark_wallets`
- Integrations/comms:
  - `user_integrations`
  - `user_integration_audits`
  - `comms_webhook_events`
  - `comms_delivery_projections`
- Social:
  - `shouts`
  - `whispers`

## Semantics implemented

- Durable snapshot persistence with atomic write/rename.
- Owner boundary checks for autopilot and paywall mutation paths.
- Autopilot handle uniqueness and owner-scoped resolution (`id` or `handle`).
- Integration secret lifecycle actions (`secret_created`, `secret_updated`, `secret_rotated`, `secret_revoked`) with audit rows.
- Webhook ingest idempotency via `idempotency_key`.
- Social listing semantics:
  - shouts support `zone`, `before_id`, and `since` filtering
  - whispers support actor-scoped list and recipient-only read mark

## Current scope boundaries

- This module is persistence/repository groundwork for parity issues.
- API handler parity for these domains is intentionally deferred to later OA-WEBPARITY issues.
- Spacetime live delivery remains WS-only; this module introduces no SSE behavior.
