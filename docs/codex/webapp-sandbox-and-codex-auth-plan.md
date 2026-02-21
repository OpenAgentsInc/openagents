# Multi-Backend Sandbox + Codex Auth Plan

Status: Active (backend architecture companion)
Date: 2026-02-19
Primary companion: `docs/codex/unified-runtime-desktop-plan.md`
Related ADR: `docs/plans/archived/adr-legacy-2026-02-21/ADR-0029-khala-sync-layer-and-codex-agent-mode.md`

## Why this document exists

`docs/codex/unified-runtime-desktop-plan.md` defines the canonical Codex product direction: desktop-first execution with runtime as durable authority.

This document defines the hosted sandbox side of that same direction:

- support multiple sandbox backends,
- keep one runtime contract,
- and define what "OpenAgents-hosted sandbox infra" means on Google Cloud.

## Product stance

1. Desktop-first remains default for Codex execution.
2. Hosted sandboxes are a parallel execution lane, not a separate product.
3. Hosted lane must be backend-portable:
   - Cloudflare Sandbox
   - Daytona
   - OpenAgents GCP-native sandbox infra
4. Web/mobile surfaces always administer Codex through Laravel + runtime contracts, regardless of backend.

## Current infrastructure baseline (already in repo/docs)

OpenAgents already runs substantial GCP infrastructure:

- Laravel web control plane on Cloud Run (`apps/openagents.com/docs/GCP_DEPLOY_PLAN.md`)
- Runtime deploy on GKE (`apps/runtime/docs/DEPLOY_GCP.md`)
- Runtime ingress hardening and network policy (`apps/runtime/docs/NETWORK_POLICY.md`)
- Existing Cloud Run and GCE Lightning/Bitcoin infra (`docs/lightning/status/20260215-current-status.md`)

This means we can define a first-party sandbox backend without introducing a new cloud footprint.

## Goals

1. Allow a user's Codex agent to run in its own isolated environment.
2. Preserve runtime as durable event + ownership authority.
3. Make backend choice an operator policy decision, not a product fork.
4. Keep Codex auth behavior consistent across backends.
5. Keep contract parity for web and (next) mobile.

## Non-goals

- Replacing desktop-first with hosted-only execution.
- Coupling web/mobile directly to vendor-specific sandbox APIs.
- Reintroducing legacy worker-era architecture as the primary control plane.

## Canonical control plane and contract

Control-path remains:

`Web/Mobile -> apps/openagents.com (Laravel) -> apps/runtime (internal contract) -> sandbox backend adapter`

Runtime Codex worker contract remains authoritative:

- `POST /internal/v1/codex/workers`
- `GET /internal/v1/codex/workers/{worker_id}/snapshot`
- `POST /internal/v1/codex/workers/{worker_id}/requests`
- `GET /internal/v1/codex/workers/{worker_id}/stream`
- `POST /internal/v1/codex/workers/{worker_id}/stop`

Backend-specific behavior must be hidden behind runtime adapter boundaries.

## Khala Self-Hosted Sync Boundary

If Khala is used for cross-client reactive sync, it sits beside this control
path as a projection layer:

`runtime event log -> runtime projector -> Khala read models -> web/mobile/desktop subscriptions`

Normative rules:

1. Runtime/Postgres is authoritative for worker lifecycle and stream history.
2. Khala stores derived read models only (summaries, status, notifications).
3. Runtime is the single writer for Khala projection docs.
4. Laravel is auth authority and mints Khala auth JWTs for clients.

See `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`.

## Layer-0 Schema Authority

For hosted sandbox backends, shared contracts must be proto-first and generated across languages from `proto/`.

- Canonical contract root: `proto/`
- Versioned package root: `proto/openagents/protocol/v1/*`
- Governance baseline: `proto/README.md`
- Mapping guidance for JSON/SSE: `docs/protocol/LAYER0_PROTOBUF_MAPPING.md`

This follows the neutral-schema direction captured in `docs/local/convo.md` (proto3 + Buf + JSON wire compatibility) and prevents source-of-truth drift between Laravel/Elixir/TS/Rust.

## Planned Proto Definitions (Hosted Sandbox + Codex)

General additions needed to represent backend-portable hosted execution:

1. `proto/openagents/protocol/v1/codex_workers.proto`
   - worker create/snapshot/request/stop request+response envelopes
   - backend and execution mode metadata
2. `proto/openagents/protocol/v1/codex_events.proto`
   - durable worker event envelope (`worker_id`, `seq`, `event_type`, `event_version`, `oneof payload`)
   - codified event payloads for lifecycle and stream parity
3. `proto/openagents/protocol/v1/codex_sandbox.proto`
   - backend enum (`CLOUDFLARE_SANDBOX`, `DAYTONA`, `OPENAGENTS_GCP`)
   - isolation and placement metadata for sandbox instances
4. `proto/openagents/protocol/v1/codex_auth.proto`
   - device-code and callback status envelopes
   - token reference metadata for hydration (no secret payload material)
5. `proto/openagents/protocol/v1/codex_admin.proto`
   - optional admin/ops messages for backend health, routing decision reason, and teardown state

## Backend abstraction

Runtime should treat hosted sandbox providers through one adapter contract:

1. `provision(worker)`
Create or reattach isolated environment for a worker.
2. `start_codex(worker)`
Launch Codex/OpenCode app-server in that environment.
3. `request(worker, envelope)`
Send JSON-RPC-style request.
4. `stream(worker, cursor)`
Surface worker events via runtime durable stream semantics.
5. `snapshot(worker)`
Report health/status/resources.
6. `stop(worker)`
Graceful shutdown + durability markers.
7. `teardown(worker)`
Hard delete/reclaim.

Backend values (initial set):

- `cloudflare_sandbox`
- `daytona`
- `openagents_gcp`

## Backend options

### 1) Cloudflare Sandbox backend

Best when:

- fast container startup and edge-adjacent execution matter,
- sandbox APIs are preferred over managing infra directly.

Tradeoffs:

- runtime/control-plane integration still needs adapter and proxy ownership,
- storage/auth persistence must stay outside ephemeral containers.

### 2) Daytona backend

Best when:

- managed dev-workspace lifecycle is desired,
- we want standardized workspace semantics across providers.

Tradeoffs:

- vendor API availability/latency profile must be monitored,
- runtime still owns durable event ledger and user ownership checks.

### 3) OpenAgents GCP backend (our own infra)

Best when:

- we want first-party control over isolation, network, and cost profile,
- we need tight integration with existing GCP runtime + ops posture.

Tradeoffs:

- operational ownership shifts to us (scheduling, hardening, scaling),
- requires stronger SRE/process discipline than managed backends.

## Defining "our own infra" on Google Cloud

OpenAgents GCP sandbox backend means:

1. **Control plane stays the same**
   - Laravel remains public API/UI authority.
   - Runtime remains internal execution authority.

2. **Sandbox orchestration lives with runtime**
   - Runtime schedules sandbox workloads and tracks worker ownership.
   - Runtime writes canonical worker events/status to `runtime.codex_*` tables.

3. **Isolated execution on GKE**
   - Dedicated sandbox node pool(s) separate from core runtime pods.
   - One isolated sandbox runtime per worker/session boundary.
   - Enforce pod/namespace isolation and deny-by-default networking.

4. **Image and artifact supply chain on GCP**
   - Sandbox images built via Cloud Build.
   - Images stored in Artifact Registry.
   - Signed/tagged images promoted by environment.

5. **Durability outside sandbox containers**
   - Workspace and Codex home material persisted outside the container lifecycle.
   - Runtime stores durable references (`workspace_ref`, `codex_home_ref`) and event history.

6. **Secret and identity model**
   - Secret Manager + Workload Identity for backend service auth.
   - No long-lived credentials baked into sandbox images.

7. **Observability + guardrails**
   - Runtime tracing/metrics policy from existing observability docs.
   - Network policy boundaries from existing runtime hardening docs.

## Isolation model for a user's Codex agent

Minimum isolation contract:

1. Environment is worker-scoped (default one environment per `worker_id`).
2. Worker is principal-owned (`x-oa-user-id` or `x-oa-guest-scope`).
3. Runtime enforces ownership at every read/write/stream/stop boundary.
4. Cross-worker filesystem/process access is denied.
5. Outbound network egress is policy-governed (allowlist/proxy path where required).

Recommended default mapping:

- `worker_id` is the hosted execution unit.
- `thread_id` is logical chat/session context.
- One user can have multiple workers; each worker remains isolated.

## Codex auth across backends

Codex/OpenCode auth flow should not depend on backend vendor.

Baseline:

- Device-code flow first (`method: 1`) for backend-neutral reliability.
- Browser PKCE redirect flow is optional and gated on OpenCode support for non-localhost callback handling.

Durability requirements:
- Persist auth payloads outside ephemeral sandbox containers.
- Rehydrate auth state on sandbox start.
- Keep auth data encrypted at rest and scoped to worker ownership.

## Codex + Khala MCP Operational Posture

For cloud coding-agent workflows (Codex/Jules/Devin/Cursor cloud agents) that
interact with Khala projects:

1. Default CLI mode is anonymous local agent mode:
   - `KHALA_AGENT_MODE=anonymous npx khala dev --once`
2. MCP production access stays disabled by default.
3. If production access is required, it must be explicitly enabled and scoped.
4. For self-hosted deployments, MCP should be configured with explicit env file
   (`KHALA_SELF_HOSTED_URL`, `KHALA_SELF_HOSTED_ADMIN_KEY`).

This keeps Codex automation useful for iteration while reducing accidental
production mutation risk.

Reference for OpenCode auth flow constraints:

- `docs/plans/archived/codex/opencode-codex-auth.md`

## Required runtime contract extensions for hosted sandboxes

To make hosted backends first-class in current runtime contract, add:

1. Worker backend metadata in create/snapshot payloads:
   - `backend`
   - `execution_mode` (`desktop` or `sandbox`)
2. Async event ingest endpoint:
   - `POST /internal/v1/codex/workers/{worker_id}/events`
3. Heartbeat semantics for hosted workers.
4. Laravel stream proxy for worker SSE:
   - `GET /api/runtime/codex/workers/{workerId}/stream`

## GCP implementation shape (proposed)

### Components

1. `openagents-web` (Cloud Run)
   - user-facing admin/API for Codex worker operations.
2. `runtime` (GKE)
   - adapter routing, ownership checks, durable events.
3. `openagents-sandbox-*` workloads (GKE)
   - isolated execution workloads for Codex workers.
4. Shared GCP services
   - Artifact Registry (images)
   - Secret Manager (secrets)
   - Cloud Logging/Monitoring
   - existing DB/storage/network foundations already used by web/runtime/lightning.

### Request flow (hosted worker)

1. User asks to start hosted Codex worker in web UI.
2. Laravel calls runtime `POST /internal/v1/codex/workers`.
3. Runtime selects backend (policy + availability).
4. Backend adapter provisions isolated environment.
5. Runtime appends `worker.started` and subsequent events durably.
6. Web/mobile observe through runtime-backed snapshot/stream APIs.

### Stop flow

1. User/admin triggers stop.
2. Laravel calls runtime stop endpoint.
3. Runtime requests graceful backend shutdown.
4. Runtime appends `worker.stopped`.
5. Teardown policy decides immediate destroy vs timed retention.

## Backend selection policy

Selection should be deterministic and policy-driven:

1. If user is desktop-linked and policy is `desktop_preferred`, use desktop worker.
2. If hosted required, select backend based on:
   - policy allowlist,
   - region/latency target,
   - cost/SLO,
   - backend health.
3. Persist chosen backend in worker metadata for auditability and reproducibility.

## Delivery phases

### Phase 0: Contract + docs alignment

- Keep unified plan as canonical product direction.
- Keep this document as canonical hosted-backend architecture.
- Extend runtime contract docs/OpenAPI for backend metadata + events.

### Phase 1: Runtime backend abstraction

- Add backend enum + adapter routing in runtime Codex workers domain.
- Keep `in_memory` for tests/dev.
- Add conformance tests per adapter boundary.

### Phase 2: First hosted adapter

- Implement one production adapter (recommended first: `openagents_gcp` because infra already exists and is operator-controlled).
- Add lifecycle smoke tests for create/request/stream/stop.

### Phase 3: Additional adapters

- Add Cloudflare and Daytona adapters behind same runtime contract.
- Add policy-based backend routing and explicit fallback logic.

### Phase 4: UX parity

- Web admin panel: backend visibility, worker state, and live stream.
- Mobile read/admin parity on same Laravel routes.

## Verification gates

1. Runtime contract tests pass for all adapters.
2. Laravel feature tests cover worker lifecycle + stream proxy.
3. Backend smoke run proves:
   - isolated environment creation,
   - Codex request/response path,
   - durable event streaming,
   - clean stop/teardown.
4. Security checks prove:
   - owner isolation,
   - deny-by-default network posture,
   - secret handling/redaction policy.

## Operational references

- Web deploy topology: `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md`
- Web production env/secrets: `apps/openagents.com/docs/PRODUCTION_ENV_AND_SECRETS.md`
- Runtime deploy on GKE: `apps/runtime/docs/DEPLOY_GCP.md`
- Runtime network hardening: `apps/runtime/docs/NETWORK_POLICY.md`
- Runtime observability: `apps/runtime/docs/OBSERVABILITY.md`
- Existing GCP Lightning/Bitcoin estate: `docs/lightning/status/20260215-current-status.md`
- Canonical Codex direction: `docs/codex/unified-runtime-desktop-plan.md`
- OpenCode auth mechanics (archived deep-dive): `docs/plans/archived/codex/opencode-codex-auth.md`
