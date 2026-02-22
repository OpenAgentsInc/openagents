# Middleware Parity (OA-WEBPARITY-006)

This service now carries reusable middleware layers that mirror Laravel middleware contracts used by `apps/openagents.com`.

## Implemented layers

- `auth_session_gate`
  - Extracts bearer token, resolves active session, and enforces authenticated access.
  - Injects authenticated `SessionBundle` into request extensions for downstream layers/handlers.

- `workos_session_gate`
  - Enforces WorkOS-linked identity when `OA_AUTH_PROVIDER_MODE=workos`.
  - Preserves local-test bypass semantics via `OA_AUTH_LOCAL_TEST_LOGIN_ENABLED` for `test_local_*` identities.

- `admin_email_gate`
  - Enforces allowlist-based admin access (`OA_ADMIN_EMAILS`).
  - Used for operator-only control mutations.

- `throttle_auth_email_gate`
  - In-memory throttle equivalent for auth challenge requests.
  - Current policy: `30` requests / `60` seconds per identity key.

- `throttle_thread_message_gate`
  - In-memory throttle for thread command mutation lane.
  - Current policy: `60` requests / `60` seconds per identity key.

- `runtime_internal_request_gate`
  - Laravel-compatible runtime-internal signature enforcement:
    - `x-oa-internal-key-id`
    - `x-oa-internal-timestamp`
    - `x-oa-internal-nonce`
    - `x-oa-internal-body-sha256`
    - `x-oa-internal-signature`
  - Includes timestamp TTL checks and nonce replay detection.
  - Implemented as reusable layer and validated in tests; route adoption is tracked by OA-WEBPARITY-033.

## Route usage in Rust service

- Protected API domain routes are wrapped by:
  - `auth_session_gate`
  - `workos_session_gate`

- `POST /api/v1/control/route-split/override` additionally requires:
  - `admin_email_gate`

- Throttle-protected routes:
  - `POST /api/auth/email`
  - `POST /api/runtime/threads/:thread_id/messages`

## Environment

- `OA_ADMIN_EMAILS` (CSV admin allowlist)
- `OA_AUTH_LOCAL_TEST_LOGIN_ENABLED` (`true|false`)
- `OA_RUNTIME_INTERNAL_SHARED_SECRET`
- `OA_RUNTIME_INTERNAL_KEY_ID`
- `OA_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS`

## Verification

Middleware parity is covered by service tests, including:

- auth email throttle limit
- thread message throttle limit
- admin gate rejection for non-admin users
- runtime-internal signature + nonce replay rejection
