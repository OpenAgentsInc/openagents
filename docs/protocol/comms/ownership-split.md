# Comms Ownership Split (Runtime vs Laravel)

## Runtime (canonical execution)

- Validate comms integration manifests before execution.
- Execute send-side effects (`comms.send`) through provider adapters.
- Enforce consent/suppression/policy gate decisions.
- Emit receipt-visible outcomes and reason codes.
- Record normalized delivery events into canonical runtime event log.
- Canonical delivery event writer: `runtime.comms_delivery_events` via `/internal/v1/comms/delivery-events`.

## Laravel (control-plane authored state)

- Author integration records and manifest payloads from settings UI.
- Store provider keys encrypted and expose runtime-scoped secret retrieval.
- Receive provider webhooks, verify signatures, normalize payloads, and forward to runtime ingestion.
- Maintain webhook transport/idempotency logs in `comms_webhook_events`.
- Project runtime delivery outcomes into UI read-model `comms_delivery_projections`.
- Provide operator/admin views for integration health and audit trails.

## Projection Write Authority

- Single writer for `comms_delivery_projections`: `App\Services\CommsDeliveryProjectionProjector`.
- Invocation path: `App\Jobs\ForwardResendWebhookToRuntime` after successful runtime ingest response.
- Other Laravel control-plane writers must not mutate projection rows.

## Ownership Table

- Detailed ownership + writer matrix: `docs/protocol/comms/ownership-table.v1.md`

## Shared Contract Boundary

- Manifest shape: `docs/protocol/comms/integration-manifest.schema.v1.json`
- Tool pack contract: `docs/protocol/comms/tool-pack-contract.v1.json`
- Reason taxonomy: `docs/protocol/reasons/runtime-policy-reason-codes.v1.json`
- Security/replay verification matrix: `docs/protocol/comms/security-replay-verification-matrix.v1.md`
