# Slim OpenClaw Runtime — options for OpenAgents (2026-02-02)

This doc proposes a **slimmer per-user runtime** for Managed OpenClaw, and how to house it inside the main `openagents` monorepo for operational simplicity.

Context:
- Today we used `moltworker` (Cloudflare Worker + Containers + DO) as a convenient sandbox runtime.
- It includes a Cloudflare Access protected admin UI and a bunch of generic “moltbot” plumbing.
- For Managed OpenClaw inside OpenAgents, we want:
  - **OpenAgents-native UI** on `openagents.com`
  - a per-user runtime that is **internal-only** (service token), no end-user Cloudflare Access
  - fewer moving parts, smaller surface area, less rebranding risk

---

## Goals

1) **Minimal API surface**
- Only implement endpoints required by the orchestrator (`apps/api` Rust) per `instance-runtime-api-contract.md`.

2) **Hard internal auth**
- Require `Authorization: Bearer <service_token>` (or `X-OpenAgents-Service-Token`).
- No UI routes.

3) **Per-user isolation**
- Keep the beta isolation choice: 1 runtime worker + 1 container sandbox per user is acceptable.

4) **Persistence**
- Use R2 backup/restore + periodic sync (or the latest mount approach) so paired devices and workspace persist.

5) **Repo hygiene**
- Avoid carrying brand strings (like `clawdbot`) into user-facing surfaces.
- Keep upstream dependencies pinned.

---

## Non-goals (for MVP)

- No end-user direct access to runtime hostnames.
- No multi-tenant runtime sharing.
- No fully general “bring your own worker”.
- No public developer API on the runtime itself.

---

## Placement options in the OpenAgents monorepo

### Option A (recommended): add a new app: `apps/openclaw-runtime/`
**Pros**
- Clean separation from existing Rust API and website.
- Easier to reason about (purpose-built).
- Can keep its own `wrangler.jsonc`, Dockerfile, tests.

**Cons**
- One more deploy pipeline.

Suggested structure:
```
openagents/
  apps/
    api/                 # existing Rust worker at /api
    website/             # existing TanStack Start worker at /
    openclaw-runtime/    # NEW: template runtime worker + container
      src/
      Dockerfile
      wrangler.jsonc
      package.json
      README.md
```

### Option B: embed inside `apps/api` as a “template assets” folder
This would store runtime code as build artifacts/templates but not as a separately deployable project.

**Pros**
- One deploy surface.

**Cons**
- Messy: mixing Rust worker and Node worker code.
- Harder local dev.

### Option C: keep as separate repo (status quo)
**Pros**
- Matches upstream model.

**Cons**
- Operational friction.
- Harder to keep OpenAgents orchestrator and runtime in lock-step.

Recommendation: **Option A**.

---

## Runtime architecture choices

### Runtime Choice 1: “Worker + DO + Container” (like moltworker, but slim)
- Worker handles HTTP and orchestration.
- DO represents the per-user sandbox instance.
- Container runs OpenClaw gateway process.

**Pros**
- Very similar to what we already proved.
- Strong control over process lifecycle.

**Cons**
- Slightly more CF-specific plumbing.

### Runtime Choice 2: “Worker only” (no container)
Run OpenClaw gateway directly in Worker.

**Pros**
- Fewer components.

**Cons**
- Likely not feasible: OpenClaw gateway expects local filesystem, long-running process semantics, possibly binaries.

### Runtime Choice 3: “Container only” (no worker)
Expose a container endpoint, skip the worker layer.

**Pros**
- Conceptually simple.

**Cons**
- Harder auth/routing; Cloudflare’s native controls are Worker-first.

Recommendation: **Choice 1** for MVP.

---

## Proposed slim runtime API

Base: internal-only. No UI.

Endpoints (match `instance-runtime-api-contract.md`):
- `GET /v1/status`
- `POST /v1/restart`
- `POST /v1/backup`
- `GET /v1/devices`
- `POST /v1/devices/{requestId}/approve`

Auth:
- `Authorization: Bearer <service_token>`
  - Reject with 401 JSON.

Response envelope (stable):
```json
{ "ok": true, "data": { ... } }
{ "ok": false, "error": "..." }
```

---

## Deployment model for managed instances

In MVP we can keep a simple provisioning approach:
- The Rust API (or a future provisioning worker) creates:
  - a per-user runtime Worker (script name derived from user id)
  - a per-user Container app (or per-user DO namespace + container binding)
  - an R2 bucket or prefix for persistence

Later optimization:
- Move to **Workers for Platforms (dispatch namespaces)** so we can:
  - share one runtime script and create per-user “instances” cheaply
  - reduce deploy times
  - keep isolation guarantees

---

## Persistence options

### Option P1: R2 FUSE mount + periodic sync (current moltworker model)
- Container mounts R2 using a FUSE approach; cron ensures sync.

### Option P2: Backup/restore on startup + periodic rsync (simpler)
- On boot:
  - restore config/workspace from R2 snapshot
- Every N minutes:
  - rsync back to R2

Recommendation: start with **P2** (simpler, fewer edge cases), migrate to P1 later if needed.

---

## Migration plan from moltworker sandbox

1) Create `apps/openclaw-runtime/` with the slim Worker+Container template.
2) Implement the `/v1/*` endpoints + service-token auth.
3) Update Rust `/api/openclaw/*` runtime client to call the new runtime endpoint paths.
4) Keep moltworker sandbox around only for internal testing until the new runtime is stable.

---

## Operational notes / pitfalls

- **Never log secrets** (service token, provider keys).
- Make backup idempotent and time-bounded.
- Avoid letting the runtime accept arbitrary shell commands.
- Ensure the runtime does not expose admin UI routes.

---

## Recommended decision

- Add **`apps/openclaw-runtime/`** to the OpenAgents monorepo.
- Use the **Worker + DO + Container** pattern.
- Implement only `/v1/*` + service-token auth.
- Start with a **backup/restore + rsync** persistence scheme.

