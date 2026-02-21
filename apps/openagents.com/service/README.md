# openagents-control-service (OA-RUST-015 skeleton)

Rust control service scaffold for `apps/openagents.com`.

## What this includes

- Server bootstrap + config loading.
- Baseline routes:
  - `GET /healthz`
  - `GET /readyz`
  - `GET /api/v1/auth/session` (OA-RUST-016 placeholder)
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

## Test

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```
