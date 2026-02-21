# openagents-control-service (OA-RUST-015..018)

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
  - `POST /api/sync/token`
  - `POST /api/v1/sync/token`
  - `GET /manifest.json` static manifest
  - `GET /assets/*` versioned static asset host
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

- `GET /manifest.json` is served with `Cache-Control: no-cache, no-store, must-revalidate`.
- `GET /assets/<hashed-file>` is served with `Cache-Control: public, max-age=31536000, immutable`.
- `GET /assets/<non-hashed-file>` is served with `Cache-Control: public, max-age=60`.

## Observability baseline

- Request correlation IDs are propagated via `x-request-id` middleware and emitted in structured logs.
- Audit events are emitted for sensitive control actions (`auth.challenge.requested`, `auth.verify.completed`, `auth.refresh.completed`, `auth.logout.completed`, `auth.active_org.updated`, `sync.token.issued`).
- Service emits JSON logs by default (`OA_CONTROL_LOG_FORMAT=json`) for machine parsing.
