# openagents-control-service (OA-RUST-015..021)

Rust control service scaffold for `apps/openagents.com`.

## What this includes

- Server bootstrap + config loading.
- Baseline routes:
  - `GET /` landing page (desktop download only)
  - `GET /download-desktop` redirect to configured desktop download URL
  - `GET /healthz`
  - `GET /readyz`
  - `POST /api/auth/email`
  - `POST /api/auth/register`
  - `POST /api/auth/verify`
  - `GET /api/auth/session`
  - `GET /api/auth/sessions`
  - `POST /api/auth/sessions/revoke`
  - `POST /api/auth/refresh`
  - `POST /api/auth/logout`
  - `GET /api/me`
  - `GET /api/tokens`
  - `POST /api/tokens`
  - `DELETE /api/tokens/current`
  - `DELETE /api/tokens/:token_id`
  - `DELETE /api/tokens`
  - `POST /api/khala/token`
  - `GET /api/settings/profile`
  - `PATCH /api/settings/profile`
  - `DELETE /api/settings/profile`
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
  - `GET /api/runtime/threads` (Codex thread projection list)
  - `GET /api/runtime/threads/:thread_id/messages` (Codex thread message projection read)
  - `POST /api/runtime/threads/:thread_id/messages` (Codex thread command lane)
  - `GET /openapi.json` Rust-generated OpenAPI document
- Request middleware foundations:
  - request ID propagation (`x-request-id`)
  - HTTP trace layer
  - auth + WorkOS session gates
  - admin gate allowlist
  - in-memory throttle gates
  - runtime-internal signature/replay guard layer

## Environment

- `OA_CONTROL_BIND_ADDR` (default: `127.0.0.1:8787`)
- `OA_CONTROL_LOG_FILTER` (default: `info`)
- `OA_CONTROL_LOG_FORMAT` (`json|pretty`, default: `json`)
- `OA_CONTROL_STATIC_DIR` (default: `apps/openagents.com/service/static`)
- `OA_DESKTOP_DOWNLOAD_URL` (default: `https://github.com/OpenAgentsInc/openagents/releases/latest`)
- `OA_AUTH_PROVIDER_MODE` (`workos|mock`, default: `workos`; `mock` is local/testing only)
- `WORKOS_CLIENT_ID` (required in `workos` mode)
- `WORKOS_API_KEY` (required in `workos` mode)
- `OA_WORKOS_API_BASE_URL` (default: `https://api.workos.com`)
- `OA_AUTH_MOCK_MAGIC_CODE` (default: `123456`)
- `OA_AUTH_LOCAL_TEST_LOGIN_ENABLED` (`true|false`, default: `false`; allows `test_local_*` WorkOS bypass lane in local testing)
- `OA_AUTH_LOCAL_TEST_LOGIN_ALLOWED_EMAILS` (CSV allowlist for `/internal/test-login`; empty denies all)
- `OA_AUTH_LOCAL_TEST_LOGIN_SIGNING_KEY` (required to validate signed `/internal/test-login` URLs)
- `OA_AUTH_API_SIGNUP_ENABLED` (`true|false`, default: `false`; enables local/testing-only `POST /api/auth/register`)
- `OA_AUTH_API_SIGNUP_ALLOWED_DOMAINS` (CSV email-domain allowlist for API signup, empty means allow all)
- `OA_AUTH_API_SIGNUP_DEFAULT_TOKEN_NAME` (default: `api-bootstrap`)
- `OA_ADMIN_EMAILS` (CSV admin allowlist for admin middleware parity routes)
- `OA_AUTH_STORE_PATH` (optional filesystem path for durable auth/session/token store snapshots)
- `OA_AUTH_CHALLENGE_TTL_SECONDS` (default: `600`)
- `OA_AUTH_ACCESS_TTL_SECONDS` (default: `3600`)
- `OA_AUTH_REFRESH_TTL_SECONDS` (default: `2592000`)
- `OA_KHALA_TOKEN_ENABLED` (`true|false`, default: `true`)
- `OA_KHALA_TOKEN_SIGNING_KEY` / `KHALA_TOKEN_SIGNING_KEY` (required for khala token mint)
- `OA_KHALA_TOKEN_ISSUER` (default: `https://openagents.com`)
- `OA_KHALA_TOKEN_AUDIENCE` (default: `openagents-khala`)
- `OA_KHALA_TOKEN_SUBJECT_PREFIX` (default: `user`)
- `OA_KHALA_TOKEN_KEY_ID` (default: `khala-auth-v1`)
- `OA_KHALA_TOKEN_CLAIMS_VERSION` (default: `oa_khala_claims_v1`)
- `OA_KHALA_TOKEN_TTL_SECONDS` (default: `300`)
- `OA_KHALA_TOKEN_MIN_TTL_SECONDS` (default: `60`)
- `OA_KHALA_TOKEN_MAX_TTL_SECONDS` (default: `900`)
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
- `OA_RUNTIME_INTERNAL_SHARED_SECRET` (optional shared secret for runtime-internal middleware parity checks)
- `OA_RUNTIME_INTERNAL_KEY_ID` (default: `runtime-internal-v1`)
- `OA_RUNTIME_INTERNAL_SIGNATURE_TTL_SECONDS` (default: `60`)
- `OA_CODEX_THREAD_STORE_PATH` (optional filesystem path for durable Codex thread/message projection store snapshots)
- `OA_DOMAIN_STORE_PATH` (optional filesystem path for durable cross-domain persistence snapshots: autopilot/l402/integrations/comms/social)
- `OA_MAINTENANCE_MODE_ENABLED` (`true|false`, default: `false`)
- `OA_MAINTENANCE_BYPASS_TOKEN` (optional; required to enable operator bypass flow)
- `OA_MAINTENANCE_BYPASS_COOKIE_NAME` (default: `oa_maintenance_bypass`)
- `OA_MAINTENANCE_BYPASS_COOKIE_TTL_SECONDS` (default: `900`, min: `60`)
- `OA_MAINTENANCE_ALLOWED_PATHS` (CSV, default: `/healthz,/readyz`; supports `*` suffix for prefix patterns)
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

Operator CLI replacements (OA-WEBPARITY-055):

```bash
cargo run --manifest-path apps/openagents.com/service/Cargo.toml \
  --bin openagents-control-ops -- demo:l402 --token <ACCESS_TOKEN>
```

Command mapping and flags:
- `apps/openagents.com/docs/20260222-oa-webparity-055-rust-cli-replacements.md`

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

VERIFY_RESPONSE="$(curl -sS -b /tmp/oa.cookie -H 'content-type: application/json' -H 'x-client: autopilot-desktop' \
  -d '{"code":"123456"}' \
  http://127.0.0.1:8787/api/auth/verify)"
echo "${VERIFY_RESPONSE}" | jq
ACCESS_TOKEN="$(echo "${VERIFY_RESPONSE}" | jq -r '.token')"

curl -sS -H "authorization: Bearer ${ACCESS_TOKEN}" \
  -H 'content-type: application/json' \
  -d '{"scopes":["runtime.codex_worker_events"]}' \
  http://127.0.0.1:8787/api/sync/token | jq
```

## Test

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```

Browser-level HTMX smoke suite:

```bash
BASE_URL=http://127.0.0.1:8787 apps/openagents.com/service/scripts/htmx_browser_smoke.sh
```

See run modes and env vars:

- `apps/openagents.com/service/docs/HTMX_BROWSER_SMOKE.md`

HTMX route-group staged canary:

```bash
BASE_URL=https://staging.openagents.com \
CONTROL_ACCESS_TOKEN=<admin-token> \
apps/openagents.com/service/scripts/htmx-route-group-canary.sh
```

Runbook:

- `apps/openagents.com/service/docs/HTMX_ROUTE_GROUP_ROLLOUT.md`

## Landing mode notes

- Web behavior is intentionally limited to the landing page and desktop-download redirect.
- Desktop download destination is controlled by `OA_DESKTOP_DOWNLOAD_URL`.
- `OA_CONTROL_STATIC_DIR` remains available for readiness checks and optional static overlays.

## API envelope and error matrix

- Shared Rust API envelope helpers:
  - `apps/openagents.com/service/src/api_envelope.rs`
- Canonical error-code/status matrix:
  - `apps/openagents.com/service/docs/API_ENVELOPE_ERROR_MATRIX.md`

## Middleware parity matrix

- Canonical middleware parity mapping:
  - `apps/openagents.com/service/docs/MIDDLEWARE_PARITY.md`

## OpenAPI pipeline

- Rust route-contract source:
  - `apps/openagents.com/service/src/openapi.rs`
- Runtime document route:
  - `GET /openapi.json`
- Snapshot generation:
  - `apps/openagents.com/service/scripts/generate-openapi-json.sh`
- Snapshot verification:
  - `apps/openagents.com/service/scripts/verify-openapi-json.sh`
- Committed snapshot:
  - `apps/openagents.com/service/openapi/openapi.json`
- CI gate: removed (workflow automation disabled by invariant).

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
- Compatibility rejection response headers:
  - `x-oa-compatibility-code`
  - `x-oa-compatibility-upgrade-required`
  - `x-oa-compatibility-protocol-version`
  - `x-oa-compatibility-min-client-build-id`
  - `x-oa-compatibility-max-client-build-id` (when configured)
  - `x-oa-compatibility-min-schema-version`
  - `x-oa-compatibility-max-schema-version`

## Observability baseline

- Request correlation IDs are propagated via `x-request-id` middleware and emitted in structured logs.
- Audit events are emitted for sensitive control actions (`auth.challenge.requested`, `auth.challenge.failed`, `auth.verify.completed`, `auth.verify.failed`, `auth.refresh.completed`, `auth.refresh.failed`, `auth.logout.completed`, `auth.logout.failed`, `auth.active_org.updated`, `auth.sessions.listed`, `auth.sessions.revoked`, `sync.token.issued`, `compatibility.rejected`).
- Service emits JSON logs by default (`OA_CONTROL_LOG_FORMAT=json`) for machine parsing.

## Session model guarantees

- Refresh token rotation is mandatory (`rotate_refresh_token=false` is rejected).
- Refresh tokens are single-use. Reuse of a revoked/rotated refresh token triggers replay defense and revokes the active session.
- Session records are device-scoped (`x-device-id` / `device_id`) and auditable via `GET /api/auth/sessions`.
- Device-scoped and global revocation are supported via `POST /api/auth/sessions/revoke`.
- When runtime revocation config is set, session invalidation signals are propagated to runtime (`/internal/v1/sync/sessions/revoke`) for live websocket eviction.

Auth persistence and token-domain storage notes:
- `apps/openagents.com/service/docs/AUTH_PERSISTENCE.md`
- `apps/openagents.com/service/docs/CODEX_THREAD_PERSISTENCE.md`
- `apps/openagents.com/service/docs/DOMAIN_PERSISTENCE.md`

Rust ownership migration/backfill runbook + scripts:
- `apps/openagents.com/service/docs/RUST_OWNERSHIP_BACKFILL_RUNBOOK.md`
- `apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh`
- `apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh`
- `apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh`
- `apps/openagents.com/service/scripts/seed-parity-fixtures.sh`

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
  - Global override:
    - `POST /api/v1/control/route-split/override` with `{"target":"legacy"}` forces legacy immediately.
    - `POST /api/v1/control/route-split/override` with `{"target":"clear"}` returns global routing to configured mode.
  - Per-domain override:
    - `POST /api/v1/control/route-split/override` with `{"target":"legacy","domain":"billing_l402"}` overrides one route group.
    - `POST /api/v1/control/route-split/override` with `{"target":"clear","domain":"billing_l402"}` clears one route-group override.
    - `POST /api/v1/control/route-split/override` with `{"target":"rollback","domain":"billing_l402"}` applies the configured rollback target for that route group.
- Route-split status now includes per-domain rollback matrix and active domain overrides.
- Route split decisions emit auditable events as `route.split.decision`.
- Parity execution checklist (frozen baseline): `apps/openagents.com/docs/20260222-web-parity-charter-checklist.md`
- Web parity program record: `apps/openagents.com/docs/20260222-laravel-rust-wgpui-full-parity-master-plan.md`
- Production canary/rollback report: `apps/openagents.com/docs/20260222-oa-webparity-058-production-canary-rollback-drill.md`
- Production Rust-only route flip report: `apps/openagents.com/docs/20260222-oa-webparity-059-production-rust-route-flip.md`
- Laravel serving retirement report: `apps/openagents.com/docs/20260222-oa-webparity-060-retire-laravel-serving-path.md`
- Domain rollback matrix: `apps/openagents.com/service/docs/ROUTE_SPLIT_ROLLBACK_MATRIX.md`

## Canary Runbook

- Full staged canary + rollback SOP: `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Staging rollout SOP: `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`
- Maintenance-mode cutover SOP: `apps/openagents.com/service/docs/MAINTENANCE_MODE_CUTOVER_RUNBOOK.md`
- Deploy smoke check: `OPENAGENTS_BASE_URL=https://<target-host> apps/openagents.com/service/deploy/smoke-health.sh`
- Control/API/static-host smoke suite:
  - `OPENAGENTS_BASE_URL=https://<target-host> apps/openagents.com/service/deploy/smoke-control.sh`
  - Optional authenticated checks: set `OPENAGENTS_CONTROL_ACCESS_TOKEN=<token>` for session/token endpoint coverage.
  - Optional maintenance bypass checks: set `OPENAGENTS_MAINTENANCE_BYPASS_TOKEN=<token>` to run smoke checks during maintenance windows.

Staging deploy (canonical; 100% traffic):

```bash
TAG="$(git rev-parse --short HEAD)"
gcloud run deploy openagents-control-service-staging \
  --project openagentsgemini \
  --region us-central1 \
  --image "us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:${TAG}" \
  --quiet
```

Optional staging helper (no-traffic revision + local verification gates):

- `apps/openagents.com/service/deploy/deploy-staging.sh` (see `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md` for traffic shift + smoke checks)

## Schema Evolution Policy

- Zero-downtime expand/migrate/contract policy and mixed-version sequencing:
  - `docs/core/SCHEMA_EVOLUTION_PLAYBOOK.md`
- Apply this playbook for any control/runtime schema or proto contract rollout where old+new binaries may overlap.
