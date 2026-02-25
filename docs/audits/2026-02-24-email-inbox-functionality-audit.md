# Email / Inbox Functionality Audit (Post-Remediation)

Date: 2026-02-24

## Scope

- Control service inbox contract + Gmail adapter
- Control service secret-at-rest handling for provider integrations
- Desktop inbox wiring to control-service APIs
- Runtime comms delivery-event ingest path parity
- QA/ops readiness artifacts for Gmail-backed flows

## Executive Status

Gmail-backed inbox is now implemented across control service, desktop, and runtime contract surfaces.

Primary gaps from the earlier audit are closed:

1. Canonical inbox API routes now exist in control service (`/api/inbox/*`).
2. Gmail list/read/send with access-token refresh is implemented.
3. Desktop inbox no longer boots from hard-coded sample threads on the primary path.
4. Integration provider secrets are encrypted at rest when key material is configured.
5. Runtime now implements `POST /internal/v1/comms/delivery-events`, matching control defaults/docs.
6. Local CI includes deterministic non-live inbox/Gmail contract checks.

## Issue Workstream Closure Status

- `#2148` Control-service inbox API contract: implemented
- `#2149` Gmail mailbox adapter + refresh: implemented
- `#2150` Desktop inbox backend wiring: implemented
- `#2151` Secret encryption at rest + migration path: implemented
- `#2152` Desktop `x-client` normalization + compatibility policy: implemented
- `#2153` Control/runtime delivery-event path drift: implemented
- `#2154` Deterministic QA lane + staging checklist updates: implemented
- `#2155` OAuth + secret rotation ops runbook: implemented
- `#2156` Master tracker: complete

## Implemented Surfaces

### Control service

- Inbox routes:
  - `GET /api/inbox/threads`
  - `POST /api/inbox/refresh`
  - `GET /api/inbox/threads/:thread_id`
  - `POST /api/inbox/threads/:thread_id/draft/approve`
  - `POST /api/inbox/threads/:thread_id/draft/reject`
  - `POST /api/inbox/threads/:thread_id/reply/send`
- Gmail adapter:
  - thread list + detail fetch
  - reply send
  - access-token refresh with retry/backoff/timeout
- Observability:
  - inbox list/detail/refresh/approve/reject/send audit events
  - Gmail request failure counters
- OpenAPI:
  - route constants, operation entries, request/response examples

### Domain store

- Integration secret envelope support:
  - encrypted envelope format: `enc:v1:<key_id>:<nonce>:<ciphertext>`
  - decrypt-on-read with key-id validation
  - lazy plaintext migration to encrypted form
- Inbox persistence:
  - per-thread state projection (approval/decision/draft preview/source)
  - inbox audit record stream

### Desktop

- `X-Client` auth header switched to canonical `autopilot-desktop`
- Compatibility alias window documented (legacy `openagents-expo` through June 30, 2026)
- Inbox actions now call backend APIs:
  - refresh
  - select thread (detail load)
  - approve draft
  - reject draft
- Local hard-coded seed threads removed from primary inbox flow

### Runtime

- Added `POST /internal/v1/comms/delivery-events` route implementation
- Idempotent replay behavior:
  - first event: `202` accepted
  - duplicate event id: `200` idempotent replay

## Verification Evidence (Deterministic / Non-Live)

- Control service:
  - `inbox_routes_fetch_gmail_threads_and_support_actions`
  - `inbox_threads_fail_when_refresh_token_is_missing`
- Runtime:
  - `comms_delivery_events_endpoint_accepts_and_deduplicates`
- Desktop contract parsing:
  - `extract_inbox_snapshot_parses_contract_shape`

Local CI lane:

- `./scripts/local-ci.sh inbox-gmail`

## What Is Still Required To Function With A Real Gmail Account

The code path is complete; production success now depends on environment/operator setup:

1. Provision Google OAuth credentials for each environment (dev/staging/prod).
2. Register exact redirect URIs in Google Cloud Console:
   - `http://localhost:8080/settings/integrations/google/callback`
   - `https://staging.openagents.com/settings/integrations/google/callback`
   - `https://openagents.com/settings/integrations/google/callback`
3. Set control-service env vars:
   - `GOOGLE_OAUTH_CLIENT_ID`
   - `GOOGLE_OAUTH_CLIENT_SECRET`
   - `GOOGLE_OAUTH_REDIRECT_URI`
   - `GOOGLE_OAUTH_SCOPES` (must include `gmail.readonly` and `gmail.send`)
   - `GOOGLE_OAUTH_TOKEN_URL`
4. Set provider-secret encryption env vars:
   - `OA_INTEGRATION_SECRET_ENCRYPTION_KEY` (32-byte base64/base64url)
   - `OA_INTEGRATION_SECRET_KEY_ID`
5. Deploy control service and runtime with aligned comms path (`/internal/v1/comms/delivery-events`).
6. Run staging smoke checklist (connect -> refresh -> detail -> approve/reject -> send).

## Canonical Runbook

- `apps/openagents.com/docs/GMAIL_INBOX_OAUTH_AND_SECRET_ROTATION_RUNBOOK.md`
