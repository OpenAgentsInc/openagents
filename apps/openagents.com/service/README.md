# openagents-control-service (OA-RUST-015..021)

Rust control service scaffold for `apps/openagents.com`.

## What this includes

- Server bootstrap + config loading.
- Baseline routes:
  - `GET /healthz`
  - `GET /readyz`
  - `POST /api/auth/email`
  - `POST /api/auth/verify`
  - `GET /api/auth/session`
  - `GET /api/auth/sessions`
  - `POST /api/auth/sessions/revoke`
  - `POST /api/auth/refresh`
  - `POST /api/auth/logout`
  - `GET /api/me`
  - `GET /api/orgs/memberships`
  - `POST /api/orgs/active`
  - `POST /api/policy/authorize`
  - `GET /api/v1/auth/session`
  - `GET /api/v1/auth/sessions`
  - `POST /api/v1/auth/sessions/revoke`
  - `GET /api/v1/control/status`
  - `GET /api/v1/control/route-split/status`
  - `POST /api/v1/control/route-split/override`
  - `POST /api/v1/control/route-split/evaluate`
  - `POST /api/sync/token`
  - `POST /api/v1/sync/token`
  - `POST /api/runtime/threads/:thread_id/messages` (Codex thread command lane)
  - `GET /sw.js` service worker script host
  - `GET /manifest.json` static manifest
  - `GET /assets/*` versioned static asset host
  - `GET /*` feature-flagged web-shell vs legacy route split entry
- Request middleware foundations:
  - request ID propagation (`x-request-id`)
  - HTTP trace layer

## Environment

- `OA_CONTROL_BIND_ADDR` (default: `127.0.0.1:8787`)
- `OA_CONTROL_LOG_FILTER` (default: `info`)
- `OA_CONTROL_LOG_FORMAT` (`json|pretty`, default: `json`)
- `OA_CONTROL_STATIC_DIR` (default: `../web-shell/dist`)
- `OA_AUTH_PROVIDER_MODE` (`workos|mock`, default: `workos`; `mock` is local/testing only)
- `WORKOS_CLIENT_ID` (required in `workos` mode)
- `WORKOS_API_KEY` (required in `workos` mode)
- `OA_WORKOS_API_BASE_URL` (default: `https://api.workos.com`)
- `OA_AUTH_MOCK_MAGIC_CODE` (default: `123456`)
- `OA_AUTH_CHALLENGE_TTL_SECONDS` (default: `600`)
- `OA_AUTH_ACCESS_TTL_SECONDS` (default: `3600`)
- `OA_AUTH_REFRESH_TTL_SECONDS` (default: `2592000`)
- `OA_SYNC_TOKEN_ENABLED` (`true|false`, default: `true`)
- `OA_SYNC_TOKEN_SIGNING_KEY` / `SYNC_TOKEN_SIGNING_KEY` (required for sync token mint)
- `OA_SYNC_TOKEN_ISSUER` (default: `https://openagents.com`)
- `OA_SYNC_TOKEN_AUDIENCE` (default: `openagents-sync`)
- `OA_SYNC_TOKEN_KEY_ID` (default: `sync-auth-v1`)
- `OA_SYNC_TOKEN_CLAIMS_VERSION` (default: `oa_sync_claims_v1`)
- `OA_SYNC_TOKEN_TTL_SECONDS` (default: `300`)
- `OA_SYNC_TOKEN_MIN_TTL_SECONDS` (default: `60`)
- `OA_SYNC_TOKEN_MAX_TTL_SECONDS` (default: `900`)
- `OA_SYNC_TOKEN_ALLOWED_SCOPES` (default: `runtime.codex_worker_events,runtime.codex_worker_summaries,runtime.run_summaries`)
- `OA_SYNC_TOKEN_DEFAULT_SCOPES` (default: `runtime.codex_worker_events`)
- `OA_ROUTE_SPLIT_ENABLED` (`true|false`, default: `true`)
- `OA_ROUTE_SPLIT_MODE` (`legacy|rust|cohort`, default: `rust`)
- `OA_ROUTE_SPLIT_RUST_ROUTES` (CSV route prefixes, default: `/` for full rust-shell ownership)
- `OA_ROUTE_SPLIT_COHORT_PERCENTAGE` (`0..100`, default: `100`)
- `OA_ROUTE_SPLIT_SALT` (stable cohort hash salt, default: `openagents-route-split-v1`)
- `OA_ROUTE_SPLIT_FORCE_LEGACY` (`true|false`, default: `false`)
- `OA_ROUTE_SPLIT_LEGACY_BASE_URL` (legacy fallback base URL, e.g. `https://legacy.openagents.com`)
- `OA_RUNTIME_SYNC_REVOKE_BASE_URL` (optional runtime base URL for websocket revocation propagation, e.g. `https://openagents-runtime.example.com`)
- `OA_RUNTIME_SYNC_REVOKE_PATH` (default: `/internal/v1/sync/sessions/revoke`)
- `OA_RUNTIME_SIGNATURE_SECRET` (optional shared runtime internal-signature secret; when unset revocation propagation is skipped)
- `OA_RUNTIME_SIGNATURE_TTL_SECONDS` (default: `60`)
- `OA_COMPAT_CONTROL_ENFORCED` (`true|false`, default: `false`)
- `OA_COMPAT_CONTROL_PROTOCOL_VERSION` (default: `openagents.control.v1`)
- `OA_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID` (default: `00000000T000000Z`)
- `OA_COMPAT_CONTROL_MAX_CLIENT_BUILD_ID` (optional upper support window)
- `OA_COMPAT_CONTROL_MIN_SCHEMA_VERSION` (default: `1`)
- `OA_COMPAT_CONTROL_MAX_SCHEMA_VERSION` (default: `1`)

## Run locally

From repository root:

```bash
cargo run --manifest-path apps/openagents.com/service/Cargo.toml
```

Health checks:

```bash
curl -sS http://127.0.0.1:8787/healthz | jq
curl -sS http://127.0.0.1:8787/readyz | jq
```

Local/testing mock-mode auth smoke test:

```bash
OA_AUTH_PROVIDER_MODE=mock cargo run --manifest-path apps/openagents.com/service/Cargo.toml

curl -c /tmp/oa.cookie -H 'content-type: application/json' \
  -d '{"email":"you@example.com"}' \
  http://127.0.0.1:8787/api/auth/email | jq

VERIFY_RESPONSE="$(curl -sS -b /tmp/oa.cookie -H 'content-type: application/json' -H 'x-client: autopilot-ios' \
  -d '{"code":"123456"}' \
  http://127.0.0.1:8787/api/auth/verify)"
echo "${VERIFY_RESPONSE}" | jq
ACCESS_TOKEN="$(echo "${VERIFY_RESPONSE}" | jq -r '.token')"

curl -sS -H "authorization: Bearer ${ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"scopes":["runtime.codex_worker_events"]}' \
  http://127.0.0.1:8787/api/sync/token | jq

curl -i http://127.0.0.1:8787/manifest.json
curl -i http://127.0.0.1:8787/assets/app-<contenthash>.js
```

## Test

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```

## Static cache policy

- Build the web-shell static dist with: `apps/openagents.com/web-shell/build-dist.sh`
- `GET /manifest.json` is served with `Cache-Control: no-cache, no-store, must-revalidate`.
- `GET /sw.js` is served with `Cache-Control: no-cache, no-store, must-revalidate`.
- `GET /assets/<hashed-file>` is served with `Cache-Control: public, max-age=31536000, immutable`.
- `GET /assets/<non-hashed-file>` is served with `Cache-Control: public, max-age=60`.
- SW pinning/rollback release order and recovery runbook:
  - `apps/openagents.com/service/docs/SW_ASSET_PINNING_ROLLBACK_RUNBOOK.md`

## Compatibility negotiation policy

- Canonical policy and failure-code matrix:
  - `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
  - `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
- Khala WS mapping reference:
  - `docs/protocol/OA_SYNC_WS_MAPPING.md`
- Control API compatibility headers:
  - `x-oa-client-build-id`
  - `x-oa-protocol-version`
  - `x-oa-schema-version`
- When control compatibility enforcement is enabled, unsupported requests return `426 Upgrade Required` with deterministic `compatibility` failure payloads.

## Observability baseline

- Request correlation IDs are propagated via `x-request-id` middleware and emitted in structured logs.
- Audit events are emitted for sensitive control actions (`auth.challenge.requested`, `auth.verify.completed`, `auth.refresh.completed`, `auth.logout.completed`, `auth.active_org.updated`, `auth.sessions.listed`, `auth.sessions.revoked`, `sync.token.issued`).
- Service emits JSON logs by default (`OA_CONTROL_LOG_FORMAT=json`) for machine parsing.

## Session model guarantees

- Refresh token rotation is mandatory (`rotate_refresh_token=false` is rejected).
- Refresh tokens are single-use. Reuse of a revoked/rotated refresh token triggers replay defense and revokes the active session.
- Session records are device-scoped (`x-device-id` / `device_id`) and auditable via `GET /api/auth/sessions`.
- Device-scoped and global revocation are supported via `POST /api/auth/sessions/revoke`.
- When runtime revocation config is set, session invalidation signals are propagated to runtime (`/internal/v1/sync/sessions/revoke`) for live websocket eviction.

## Route split and rollback

- Route targeting is deterministic per request/cohort key (`x-oa-route-key`) using configured route prefixes and split mode.
- Rust shell is the default web route target for configured surfaces (`OA_ROUTE_SPLIT_MODE=rust` + `OA_ROUTE_SPLIT_RUST_ROUTES=/`).
- `/chat/*` remains pinned to Rust shell (`pilot_route_rust_only`) and is never redirected to legacy.
- Auth/onboarding entry routes are Rust-shell prefixes during OA-RUST-061 rollout:
  - `/login`
  - `/register`
  - `/authenticate`
  - `/onboarding/*`
- Account/settings/admin route groups are expected Rust-shell prefixes during OA-RUST-059 rollout:
  - `/account/*`
  - `/settings/*`
  - `/admin/*`
- Billing/lightning operator routes are expected Rust-shell prefixes during OA-RUST-060 rollout:
  - `/l402/*`
  - `/billing/*` (alias)
- Fast rollback is supported with authenticated override:
  - `POST /api/v1/control/route-split/override` with body `{"target":"legacy"}` forces legacy immediately.
  - `POST /api/v1/control/route-split/override` with body `{"target":"clear"}` returns to configured mode.
- Route split decisions emit auditable events as `route.split.decision`.
- Codex pilot route checklist/run notes: `apps/openagents.com/docs/20260221-codex-thread-rust-pilot.md`
- Auth/onboarding rollout checklist: `apps/openagents.com/docs/20260221-route-group-rollout-auth-onboarding.md`
- Account/settings/admin rollout checklist: `apps/openagents.com/docs/20260221-route-group-rollout-account-settings-admin.md`
- Billing/lightning rollout checklist: `apps/openagents.com/docs/20260221-route-group-rollout-billing-lightning.md`
- Default router cutover checklist: `apps/openagents.com/docs/20260221-route-cutover-default-rust.md`

## Canary Runbook

- Full staged canary + rollback SOP: `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Deploy smoke check: `OPENAGENTS_BASE_URL=https://<target-host> apps/openagents.com/service/deploy/smoke-health.sh`

## Schema Evolution Policy

- Zero-downtime expand/migrate/contract policy and mixed-version sequencing:
  - `docs/SCHEMA_EVOLUTION_PLAYBOOK.md`
- Apply this playbook for any control/runtime schema or proto contract rollout where old+new binaries may overlap.
