# Slim OpenClaw Runtime (Monorepo App) — Spec + Coding Agent Instructions

Status: **Decided** (no open architecture choices)

We will implement a purpose-built per-user runtime template as a new app inside the OpenAgents monorepo.

- **Repo placement:** `openagents/apps/openclaw-runtime/` (Option A)
- **Runtime architecture:** **Worker + Durable Object + Container** (Choice 1)
- **Auth:** internal-only via service token (no end-user Cloudflare Access)
- **UI:** none (runtime must not ship a dashboard)

This document is the implementation spec + step-by-step instructions for the coding agent.

---

## 0) Objective

Replace reliance on the current `moltworker` sandbox (which includes a Cloudflare Access UI and broader “moltbot” scope) with a **slimmer runtime** that:

- exposes only the endpoints OpenAgents needs (per `instance-runtime-api-contract.md`)
- is callable only by OpenAgents control plane (service token)
- runs OpenClaw gateway in a container sandbox
- persists state via R2 backup/restore + periodic sync

This runtime is a **template** for the per-user “instance runtime” created during provisioning.

---

## 1) Read these docs first

The coding agent must read (in order):

1. `~/code/openagents/docs/openclaw-slim-runtime-options.md` (this file)
2. `/home/christopherdavid/.openclaw/workspace/instance-runtime-api-contract.md`
3. `/home/christopherdavid/.openclaw/workspace/PRD-openagents-managed-openclaw.md`
4. `/home/christopherdavid/.openclaw/workspace/openclaw-api-structure-rust.md`

---

## 2) Monorepo structure (create these files)

Create a new app:

```
openagents/
  apps/
    openclaw-runtime/
      README.md
      package.json
      tsconfig.json
      wrangler.jsonc
      Dockerfile
      src/
        index.ts
        routes/
          v1.ts
        auth/
          serviceToken.ts
        sandbox/
          sandboxDo.ts
          process.ts
          r2.ts
          backup.ts
          files.ts
        types.ts
      test/
        serviceToken.test.ts
        v1.contract.test.ts
        files.identity.test.ts
```

Guidelines:
- keep the runtime small: **no UI**, no Vite build, no static assets.
- prefer minimal dependencies (Hono is fine).

---

## 3) Runtime API (MUST match contract)

Base path: `/v1`

Endpoints:
- `GET /v1/status`
- `POST /v1/gateway/restart`
- `POST /v1/storage/backup`
- `GET /v1/devices`
- `POST /v1/devices/:requestId/approve`
- **Workspace sync (identity docs):**
  - `GET /v1/files/export?mode=identity|workspace`
  - `POST /v1/files/import?mode=identity|workspace&strategy=merge|replace` (multipart zip)
  - `GET /v1/files/read?path=...` (allowlisted)
  - `PUT /v1/files/write?path=...` (allowlisted)

All endpoints:
- require service-token auth
- return JSON envelope for JSON endpoints:
  - `{ "ok": true, "data": ... }`
  - `{ "ok": false, "error": { "code": "...", "message": "...", "details": { ... } } }`

**Exception:** `/v1/files/export` returns `application/zip` (binary), not the JSON envelope.

No other routes should be exposed.

### 3.1 Auth
Implement:
- `X-OpenAgents-Service-Token: <token>` (primary)
- `Authorization: Bearer <OPENAGENTS_SERVICE_TOKEN>` (compat)

Reject with 401:
```json
{ "ok": false, "error": { "code": "unauthorized", "message": "unauthorized", "details": null } }
```

---

## 4) Worker + DO + Container layout

### 4.1 Worker responsibilities
- Parse request + enforce auth.
- Route to DO instance (single DO per Worker instance).
- DO manages lifecycle of the container process.

### 4.2 Durable Object responsibilities
- Own the sandbox identity and hold any in-memory state.
- Ensure the OpenClaw gateway process exists and is reachable.
- Provide helpers:
  - start process
  - stop/restart
  - run CLI commands to list/approve devices
  - backup/sync to R2

### 4.3 Container responsibilities
- Run the OpenClaw gateway process.
- Maintain local filesystem state in container.
- Backup/restore to the mounted data directory.

---

## 5) Persistence (MVP scheme)

We will use a simple **backup/restore + rsync** scheme (not FUSE mounting as a hard requirement).

### 5.1 Storage layout
- Local config/workspace inside container:
  - config: `/root/.clawdbot/` (internal naming tolerated)
  - workspace: `/root/clawd/`
- Worker-visible mount point (in container): `/data/openclaw/`
- R2 object prefix (bucket or prefix):
  - `openclaw/<instanceId>/...`

### 5.2 Restore on startup
On gateway start:
- if an R2 backup exists, restore into `/root/.clawdbot/` and `/root/clawd/`.

### 5.3 Periodic backup
- Use Wrangler cron trigger to run backup every 5 minutes.
- Backup writes:
  - `/root/.clawdbot/**`
  - `/root/clawd/skills/**` at minimum
  - optional: selected workspace data

---

## 6) Configuration (bindings + env)

### 6.1 Wrangler bindings
In `wrangler.jsonc`:
- `durable_objects`: bind DO class, e.g. `Sandbox`
- `containers`: bind container class, e.g. `Sandbox`
- `r2_buckets`: bind e.g. `OPENCLAW_BUCKET`
- `triggers.crons`: `*/5 * * * *`

### 6.2 Secrets
- `OPENAGENTS_SERVICE_TOKEN` (required)
- Provider keys for the gateway (OpenAI/OpenRouter) should be passed through if needed.

Note: avoid putting secrets in logs.

---

## 7) Implementation order (coding agent checklist)

1) Scaffold `apps/openclaw-runtime/` with Worker + DO skeleton and `wrangler.jsonc`.
2) Implement `auth/serviceToken.ts` and unit tests.
3) Implement `/v1/status` end-to-end (Worker → DO → container readiness).
4) Implement device list/approve using CLI inside container.
5) Implement restart.
6) Implement backup endpoint + cron backup.
7) Add contract tests that assert request/response shapes match `instance-runtime-api-contract.md`.
8) Ensure **no Cloudflare Access** middleware exists in this runtime.
9) Implement workspace file sync endpoints + allowlists:
   - identity export/import zip
   - read/write allowlisted identity files
10) Add tests for zip-slip/path traversal protection.

---

## 8) Integration note: how OpenAgents will use this

- `apps/api` (Rust) calls per-user runtime endpoints with the service token.
- Website UI never calls runtime directly.
- Provisioning can deploy this runtime per user (MVP) or later use Workers for Platforms.

---

## 9) Definition of done

- New app exists: `apps/openclaw-runtime/`.
- Local dev works: `pnpm -C apps/openclaw-runtime dev` (or equivalent).
- Deployment works to a test worker name.
- All `/v1/*` endpoints work with service token.
- No UI routes exist.
- Cron backup runs without errors when R2 is configured.

---

## Work log (2026-02-02)
- Added `apps/openclaw-runtime/` scaffold (Worker + container) with `wrangler.jsonc`, `Dockerfile`, `tsconfig.json`, and README.
- Implemented `/v1/*` routes with service-token auth and contract-compliant response envelopes.
- Added sandbox helpers for gateway lifecycle, device CLI actions, and version reporting.
- Implemented backup/restore to R2 using rsync + tar archives and cron-triggered backups.
- Added unit tests for auth and response envelope helpers.
- Aligned endpoint paths + auth/error envelope with `instance-runtime-api-contract.md`.
- Installed dependencies, ran `npm test`, and deployed via `wrangler deploy` (workers.dev enabled by default warning).
