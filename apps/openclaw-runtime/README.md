# OpenClaw Runtime (Slim)

Internal per-user runtime for OpenAgents. This Worker is **not** user-facing.

## Local dev

```bash
npm install
npm run dev
```

## Deploy

```bash
npm run deploy
```

## Required secrets

- `OPENAGENTS_SERVICE_TOKEN` (service token for `/v1/*` endpoints)

## Optional configuration

- `OPENCLAW_INSTANCE_ID` (used for R2 backup key prefix)
- `OPENCLAW_GATEWAY_TOKEN` (optional gateway auth token)
- `OPENCLAW_INSTANCE_TYPE` (reported via `/v1/status`)
- `OPENCLAW_VERSION` (reported via `/v1/status` if set)
- `OPENCLAW_BIND_MODE` (default: `127.0.0.1`)

## API

Base path: `/v1`

- `GET /v1/status`
- `POST /v1/gateway/restart`
- `POST /v1/storage/backup`
- `GET /v1/devices`
- `POST /v1/devices/:requestId/approve`

All requests require the service token.
