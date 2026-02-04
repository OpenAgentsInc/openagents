# Split plan: Autopilot (Cloudflare Agents SDK) + OpenClaw runtime

## Goal

Give every user a durable **Autopilot** (Cloudflare Agents SDK agent) that can:

- run the “OpenAgents-native” product experience (memory, approvals, progress streaming, billing-aware tool routing)
- **provision and attach** one or more OpenClaw Gateways (OpenClaw instances) when the user upgrades / needs it
- keep OpenClaw running on the infrastructure it expects (gateway process + CLI + filesystem) while Autopilot remains web-native

Canonical OpenClaw + Cloudflare plan lives in `apps/web/docs/openclaw-on-openagents-com.md`.

## Repo naming (keep as-is)

Do **not** rename the existing Cloudflare worker app right now:

- Folder stays `apps/agent-worker/`
- Worker name stays `openagents-agent-worker`

## Non-goals (for this split)

- Re-implement OpenClaw Gateway semantics inside Autopilot (we keep the gateway process inside containers).
- Expose OpenClaw gateway tokens or runtime control endpoints to the browser.

## Components and responsibility boundaries

### 1) Autopilot (Agents SDK agent; DO-backed)

Autopilot is the **durable product brain** for a user:

- owns user memory and “projects/threads” state
- owns tool routing and approval gating
- can schedule background work (alarms/queues/workflows later)
- decides when to spin up / attach an OpenClaw instance

Implementation direction:

- Use Cloudflare Agents SDK patterns (eventually `AIChatAgent` + `routeAgentRequest`) for resumable streaming and state sync.
- Key the Autopilot agent by **user id** (one durable Autopilot per user).

### 2) OpenClaw runtime (containers; per instance)

OpenClaw runtime is the **execution substrate** OpenClaw expects:

- Cloudflare Sandbox container(s) running the gateway process (`openclaw gateway`)
- filesystem, CLI execution, background processes, R2 backups
- exposes a small internal-only HTTP surface (`/v1/*`) that proxies into the gateway (`/tools/invoke`, `/v1/responses`, etc.)

This stays in `apps/openclaw-runtime` and should evolve toward true multi-tenancy:

- runtime accepts a tenant/instance key and uses it as the sandbox id (instead of one fixed id).

### 3) Stable control plane API (Rust worker)

`apps/api` remains the stable surface that enforces:

- auth (WorkOS identity, internal key, service-to-service token)
- billing/credits gating
- Convex reads/writes for instance metadata + encrypted secrets
- runtime proxying (service token → runtime `/v1/*`)

Autopilot and the web app should call `apps/api`, not `apps/openclaw-runtime` directly.

### 4) Web UI (TanStack Start)

`apps/web` is the product UI and should be able to:

- connect to Autopilot (via Agents SDK client, or current proxy endpoints)
- show approvals/progress/state
- “Attach OpenClaw” / “Create OpenClaw” UX that triggers Autopilot actions

## Data model changes (to support “one user → many OpenClaw instances”)

Current provisioning is “record-only” (writes a single `openclaw_instances` row for the user). `apps/web/docs/zero-to-openclaw-30s.md`

To support multiple instances per user, we need:

- an instance key (e.g. `instance_id` or `slug`) in `openclaw_instances`
- a unique index like `(user_id, instance_id)`
- per-instance secrets (service token, gateway token) stored encrypted
- per-instance runtime identity (sandbox id / container app id / runtime_url)

## Plan (milestones + acceptance criteria)

### Milestone A — Autopilot becomes the owner of “upgrade + attach”

- Use the existing `apps/agent-worker` as the durable Autopilot orchestrator (still a Worker + DO).
- Autopilot exposes internal endpoints (server-to-server) to:
  - create/list/update OpenClaw instances for a user
  - attach/detach an OpenClaw instance to a project/thread
  - return the “effective toolset” given user plan + attachments

Acceptance:
- One Autopilot per user exists and persists state across requests.
- UI can call “Attach OpenClaw” and receive a durable state update.

### Milestone B — Real per-instance sandbox selection in `apps/openclaw-runtime`

- Stop using a single fixed sandbox id (today’s default is effectively global).
- Make runtime choose sandbox id from a request-level tenant/instance key:
  - example: `sandbox_id = normalize(userId + ':' + instanceId)`

Acceptance:
- Two different users (or two instanceIds for one user) do not share the same sandbox.
- Backups and device/pairing state are isolated per sandbox.

### Milestone C — “Provision” becomes real provisioning (not record-only)

- Provision path creates/initializes a sandbox for the instance and validates readiness:
  - generate and store service token (API↔runtime)
  - optionally generate/store gateway token (runtime↔gateway)
  - ensure gateway process is up (status endpoint) and R2 prefix/bucket is configured

Acceptance:
- `Provision OpenClaw` results in a running gateway for that instance.
- `Delete OpenClaw` tears down the instance (or at least disables + schedules cleanup).

### Milestone D — OpenAgents-specific tools live in Autopilot

Add OpenAgents tools to Autopilot (examples):

- payments/credits checks + spend approvals
- community actions (posting, moderation queue, notifications)
- OpenAgents-only integrations (marketplace, receipts, budgets)

Autopilot calls OpenClaw through the stable API:

- OpenClaw tools are invoked via `apps/api` → `apps/openclaw-runtime` → gateway.

Acceptance:
- Autopilot can run a mixed tool sequence (OpenAgents + OpenClaw) with a single approval model.

## Upgrade path (what the user experiences)

- Free/default: Autopilot-only (durable chat + OpenAgents tools).
- Upgrade: Autopilot provisions one or more OpenClaw instances and attaches them to projects/threads.
- Optional: user can create multiple instances (e.g. “work” vs “personal”, or separate channel/device domains).
