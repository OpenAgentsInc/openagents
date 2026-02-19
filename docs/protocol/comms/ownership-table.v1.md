# Comms Ownership Table v1

Canonical ownership and write authority for comms entities.

## Ownership Matrix

| Entity | Store | Primary Writer | Secondary Writers | Reader Surfaces | Notes |
| --- | --- | --- | --- | --- | --- |
| Integration credentials/status (`user_integrations`) | Laravel DB | Laravel settings flows (`IntegrationController`, `IntegrationSecretLifecycleService`) | none | Laravel settings UI, internal secret fetch | Control-plane authored state only. |
| Integration lifecycle audit (`user_integration_audits`) | Laravel DB | Laravel lifecycle + projection projector | none | Laravel settings UI | Audit trail for authored and projected state changes. |
| Incoming webhook envelope (`comms_webhook_events`) | Laravel DB | Laravel webhook controller (`ResendWebhookController`) | Forwarding job status updates (`ForwardResendWebhookToRuntime`) | Operator/debug surfaces | Transport/idempotency/retry bookkeeping, not canonical delivery outcomes. |
| Canonical delivery execution events (`runtime.comms_delivery_events`) | Runtime DB (`runtime` schema) | Runtime internal ingest (`POST /internal/v1/comms/delivery-events`) | none | Runtime tooling, replay/receipts context | Runtime is canonical writer for execution outcomes. |
| UI delivery read model (`comms_delivery_projections`) | Laravel DB | `CommsDeliveryProjectionProjector` invoked from runtime-forward success path | none | Laravel settings UI (`settings/integrations`) | Explicit projection table. Single writer enforced by code path + unique scope key. |

## Single-Writer Rule

`comms_delivery_projections` is **write-authorized only** through:

- `App\Services\CommsDeliveryProjectionProjector::projectFromRuntimeDelivery()`
- Called by `App\Jobs\ForwardResendWebhookToRuntime` after successful runtime ingest.

Other control-plane services must not update delivery projection rows.

## End-to-End Contracted Flow

1. Authored intent (Laravel): user connects Resend integration (`user_integrations`).
2. Runtime execution outcome: webhook normalized and forwarded; runtime ingests canonical event (`runtime.comms_delivery_events`).
3. UI projection: Laravel projector updates `comms_delivery_projections`; settings UI reads projection row.

Reference verification test:
- `apps/openagents.com/tests/Feature/Settings/CommsOwnershipProjectionFlowTest.php`
