# openagents-control-service (OA-RUST-015 skeleton)

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
  - `GET /api/v1/auth/session`
  - `GET /api/v1/control/status` (OA-RUST-017 placeholder)
  - `POST /api/v1/sync/token` (OA-RUST-018 placeholder)
  - `GET /assets/*` static host skeleton
- Request middleware foundations:
  - request ID propagation (`x-request-id`)
  - HTTP trace layer

## Environment

- `OA_CONTROL_BIND_ADDR` (default: `127.0.0.1:8787`)
- `OA_CONTROL_LOG_FILTER` (default: `info`)
- `OA_CONTROL_STATIC_DIR` (default: `../web-shell/dist`)
- `OA_AUTH_PROVIDER_MODE` (`auto|workos|mock`, default: `auto`)
- `WORKOS_CLIENT_ID` (used when WorkOS provider is active)
- `WORKOS_API_KEY` (used when WorkOS provider is active)
- `OA_WORKOS_API_BASE_URL` (default: `https://api.workos.com`)
- `OA_AUTH_MOCK_MAGIC_CODE` (default: `123456`)
- `OA_AUTH_CHALLENGE_TTL_SECONDS` (default: `600`)
- `OA_AUTH_ACCESS_TTL_SECONDS` (default: `3600`)
- `OA_AUTH_REFRESH_TTL_SECONDS` (default: `2592000`)

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

curl -b /tmp/oa.cookie -H 'content-type: application/json' -H 'x-client: autopilot-ios' \
  -d '{"code":"123456"}' \
  http://127.0.0.1:8787/api/auth/verify | jq
```

## Test

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```
