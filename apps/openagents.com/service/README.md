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
  - `POST /api/auth/refresh`
  - `POST /api/auth/logout`
  - `GET /api/me`
  - `GET /api/orgs/memberships`
  - `POST /api/orgs/active`
  - `POST /api/policy/authorize`
  - `GET /api/v1/auth/session`
  - `GET /api/v1/control/status`
  - `GET /api/v1/control/route-split/status`
  - `POST /api/v1/control/route-split/override`
  - `POST /api/v1/control/route-split/evaluate`
  - `POST /api/sync/token`
  - `POST /api/v1/sync/token`
  - `POST /api/runtime/threads/:thread_id/messages` (Codex thread command lane)
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
- `OA_AUTH_PROVIDER_MODE` (`auto|workos|mock`, default: `auto`)
- `WORKOS_CLIENT_ID` (used when WorkOS provider is active)
- `WORKOS_API_KEY` (used when WorkOS provider is active)
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

Mock-mode auth smoke test:

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
- `GET /assets/<hashed-file>` is served with `Cache-Control: public, max-age=31536000, immutable`.
- `GET /assets/<non-hashed-file>` is served with `Cache-Control: public, max-age=60`.

## Observability baseline

- Request correlation IDs are propagated via `x-request-id` middleware and emitted in structured logs.
- Audit events are emitted for sensitive control actions (`auth.challenge.requested`, `auth.verify.completed`, `auth.refresh.completed`, `auth.logout.completed`, `auth.active_org.updated`, `sync.token.issued`).
- Service emits JSON logs by default (`OA_CONTROL_LOG_FORMAT=json`) for machine parsing.

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
