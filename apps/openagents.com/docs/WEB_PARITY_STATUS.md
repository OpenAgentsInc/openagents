# Laravel web parity status

Updated: 2026-02-16

This document tracks which user-facing and L402-critical behaviors are implemented in the Laravel app at `apps/openagents.com`.

## Scope used for "relevant"

Relevant means production user workflows for:

- chat continuity and tool execution,
- L402 buying flow,
- payment artifacts/observability,
- admin access controls for operational pages.

It does **not** include legacy Effuse-specific implementation internals (pane runtime engine, Convex-specific workers, DSE admin ops pages) that are architectural details from the old stack rather than user product requirements.

## Implemented parity

### Chat surface

- Persistent conversation threads with sidebar history.
- AI SDK Vercel-protocol SSE streaming.
- Tool cards rendered in chat.
- L402 tool cards show concise status summaries and keep detailed payload behind collapsible tool cards.

### L402 tool chain

- `lightning_l402_fetch` now supports two-step approval parity:
  - queues approval intent by default (`approvalRequired=true`),
  - returns `status=approval_requested` and `taskId`.
- `lightning_l402_approve` executes queued intent by `taskId`.
- Deterministic deny paths for missing/expired tasks.
- Endpoint preset support (`endpointPreset`) to mirror demo route behavior.

### L402 policy and execution behavior

- Host allowlist enforcement.
- Per-call spend cap enforcement.
- Quote-vs-cap pre-payment block behavior.
- Credential caching semantics and cache hit/miss reporting.
- Response capture bounds (`response_max_bytes`, preview bytes, SHA256).

### L402 observability pages (pane-equivalent pages)

- `/l402` wallet summary
- `/l402/transactions` list
- `/l402/transactions/{eventId}` detail
- `/l402/paywalls` grouped target view
- `/l402/settlements` settlement totals and activity
- `/l402/deployments` gateway/deployment event feed and config snapshot

### Event receipts + correlation

- `l402_fetch_receipt` events are persisted from both:
  - `lightning_l402_fetch`
  - `lightning_l402_approve`
- Transaction pages read these receipts and enforce per-user scoping.

### Admin access

- `/admin` protected by env/config-controlled admin email allowlist.

## Configuration parity added

- `L402_APPROVAL_TTL_SECONDS` support in config (`config/lightning.php`).
- Existing L402 env keys remain active for allowlist, caps, payer backend, and payload limits.

## Test coverage added/updated

- `tests/Feature/LightningL402ApprovalFlowTest.php`
  - queue intent -> approve -> paid result
  - expired task -> deterministic failure
- `tests/Feature/LightningL402PresetToolTest.php`
  - endpoint preset resolution
- `tests/Feature/L402PagesTest.php`
  - page auth + transaction ownership scoping
- `tests/Feature/L402ReceiptEventsTest.php`
  - receipt capture for `lightning_l402_approve`

## Validation commands

- `php artisan test`
- `npm run types`
- `npm run build`

All pass on current branch.
