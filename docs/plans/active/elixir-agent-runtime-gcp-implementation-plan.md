# OpenAgents Elixir Agent Runtime Plan (No Web Rewrite)

Date: 2026-02-18  
Status: Active plan  
Owner: OpenAgents platform/runtime  
Scope: Add an Elixir-based agent runtime to `~/code/openagents` while keeping `apps/openagents.com` (Laravel) as the web/control plane.

## Goal

Ship a production Elixir runtime for long-running autonomous agents (GenServer/Supervisor model), without rewriting `apps/openagents.com`.

The Laravel app remains the user-facing product surface and API contract. Elixir becomes the autonomous execution engine.

## Hard decisions (locked)

1. No Phoenix/Laravel rewrite in this plan.
2. Elixir runtime lives inside the existing `openagents` monorepo under `apps/`.
3. Primary deployment target is GCP to stay co-located with current infra.
4. Runtime must be restart-safe: process memory is a cache, not source of truth.
5. Existing frontend stream contract (AI SDK/Vercel protocol over SSE) is preserved for compatibility.

## Why this plan (current state summary)

Current web runtime (`apps/openagents.com`) already has:

- Working SSE chat and event persistence (`runs`, `messages`, `run_events`).
- Guest/auth semantics tied to Laravel + WorkOS + Sanctum.
- Tooling, L402, profile/admin APIs and web pages coupled to Laravel controllers/models.

This makes a full web rewrite the wrong first move. The highest-value migration is runtime/orchestration only.

References:
- `apps/openagents.com/app/AI/RunOrchestrator.php`
- `apps/openagents.com/app/Http/Controllers/ChatApiController.php`
- `apps/openagents.com/resources/js/pages/chat.tsx`
- `apps/openagents.com/routes/web.php`
- `apps/openagents.com/routes/api.php`

## Monorepo placement (exact paths)

## New app

Create a new OTP/Phoenix app:

- `apps/openagents-runtime/`

Proposed structure:

```text
apps/openagents-runtime/
  README.md
  mix.exs
  mix.lock
  .formatter.exs
  config/
    config.exs
    dev.exs
    test.exs
    runtime.exs
  lib/
    openagents_runtime/
      application.ex
      runtime_supervisor.ex
      agent_supervisor.ex
      agent_registry.ex
      agent_process.ex
      frame_router.ex
      frame_compactor.ex
      memory/
        timeline_store.ex
        l1_compactor.ex
        l2_rollup.ex
        l3_rollup.ex
      tools/
        tool_runner.ex
        tool_task_supervisor.ex
      runs/
        run_store.ex
        run_events.ex
        projections.ex
      integrations/
        laravel_event_mapper.ex
        auth_token_verifier.ex
      telemetry/
        metrics.ex
        tracing.ex
  lib/openagents_runtime_web/
    endpoint.ex
    router.ex
    controllers/
      health_controller.ex
      internal_run_controller.ex
      internal_stream_controller.ex
      internal_agent_controller.ex
  priv/
    repo/migrations/
  test/
    openagents_runtime/
    openagents_runtime_web/
  deploy/
    cloudbuild.yaml
    deploy-production.sh
    jobs/
      migrate.sh
      smoke.sh
    terraform/ (optional phase-2)
```

## Laravel integration code locations

Keep Laravel ownership of public API/UI; add a runtime adapter layer:

- `apps/openagents.com/app/AI/Runtime/RuntimeClient.php`
- `apps/openagents.com/app/AI/Runtime/ElixirRuntimeClient.php`
- `apps/openagents.com/app/AI/Runtime/LegacyLaravelRuntimeClient.php`
- `apps/openagents.com/config/runtime.php`

Touch points:

- `apps/openagents.com/app/Http/Controllers/ChatApiController.php`
- `apps/openagents.com/app/AI/RunOrchestrator.php` (becomes orchestrator facade + fallback)
- `apps/openagents.com/tests/Feature/` (new integration tests with fake runtime)

## Docs/runbooks to add in openagents repo

- `docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md` (this file)
- `apps/openagents-runtime/README.md`
- `apps/openagents-runtime/docs/DEPLOY_GCP.md`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- `apps/openagents-runtime/docs/OPERATIONS.md`
- `docs/PROJECT_OVERVIEW.md` update with new app
- `docs/README.md` update with new runtime docs links

## Target architecture (no rewrite)

## Responsibilities

Laravel (`apps/openagents.com`) keeps:

- User auth/session (WorkOS, Sanctum, guest bootstrap behavior)
- Public API surface and route contracts
- Web pages, Inertia/React UI
- Product/business APIs (profile, tokens, admin pages, etc.)
- Existing DB read models consumed by UI/API

Elixir (`apps/openagents-runtime`) owns:

- Long-running agent process lifecycle
- Frame-based inference loop
- Async tool execution and cancellation
- Tiered memory compaction and expansion
- Autonomous scheduling (cron-style ticks, map/reduce scans)
- Runtime event emission for UI stream compatibility

## High-level data flow

1. Browser sends message to existing Laravel endpoint (`/api/chat` or `/api/chats/{id}/stream`).
2. Laravel validates auth/session/ownership exactly as today.
3. Laravel calls Elixir internal API to start/continue run.
4. Laravel proxies runtime stream frames to browser as SSE (same AI SDK protocol).
5. Elixir persists canonical runtime events + run state.
6. Laravel read APIs continue returning conversation/runs/events for refresh/history.

## Internal runtime contract (Laravel <-> Elixir)

All runtime APIs are internal-only (`/internal/v1/*`), authenticated via service-to-service identity + signed token.

## Start/continue run

`POST /internal/v1/runs`

Request (shape, not final):

```json
{
  "runId": "uuid",
  "threadId": "conversation-id",
  "userId": 123,
  "autopilotId": "optional",
  "authenticatedSession": true,
  "prompt": "latest user text",
  "messages": [],
  "contextFrames": [],
  "toolPolicy": {
    "allow": [],
    "deny": [],
    "guestMode": false
  }
}
```

Response:

```json
{
  "runId": "uuid",
  "status": "accepted"
}
```

## Stream run events

`GET /internal/v1/runs/{runId}/stream`

Returns SSE frames compatible with current frontend expectations:

- `start`
- `start-step`
- `text-start`
- `text-delta`
- `text-end`
- tool-related frames
- `finish-step`
- `finish`
- `[DONE]`

Laravel should proxy bytes with minimal transformation.

## Cancel run

`POST /internal/v1/runs/{runId}/cancel`

Best effort immediate cancel of model/tool tasks, then emit terminal status event.

## Snapshot/recovery

`GET /internal/v1/runs/{runId}`
`GET /internal/v1/threads/{threadId}/state`

Used by Laravel for recovery, diagnostics, and fallback paths.

## Agent lifecycle endpoints (phase 2+)

- `POST /internal/v1/agents/{agentId}/wake`
- `POST /internal/v1/agents/{agentId}/sleep`
- `POST /internal/v1/agents/{agentId}/frames`

These support autonomous frame ingestion beyond user-request loops.

## Runtime data model strategy

Goal: keep compatibility for Laravel APIs while giving Elixir first-class event sourcing for long-lived agents.

## Preferred approach

Use the same Postgres cluster but a dedicated schema for runtime internals.

- Existing Laravel tables remain source for current API contracts:
  - `threads`, `runs`, `messages`, `run_events`, `autopilots*`
- New Elixir schema for deeper runtime state:
  - `runtime.agent_instances`
  - `runtime.frames`
  - `runtime.frame_chunks_l1`
  - `runtime.frame_chunks_l2`
  - `runtime.frame_chunks_l3`
  - `runtime.tool_tasks`
  - `runtime.checkpoints`
  - `runtime.expansion_jobs`

Elixir writes canonical runtime events and updates Laravel-compatible projections.

## Projection ownership

Elixir is authoritative for runtime execution records once cutover flag is enabled.

Projection rules:

1. Runtime event appended in `runtime.*`.
2. Projection updater writes/updates:
   - `runs`
   - `messages`
   - `run_events`
3. Laravel read endpoints remain unchanged.

This gives migration safety while preserving UI/API behavior.

## Process model in Elixir

## Supervisor tree (initial)

```text
OpenAgentsRuntime.Application
└── OpenAgentsRuntime.RuntimeSupervisor
    ├── Registry (agent registry)
    ├── DynamicSupervisor (AgentProcess workers)
    ├── Task.Supervisor (tool tasks)
    ├── Oban (scheduled compactions and maintenance)
    ├── Repo (Ecto/Postgres)
    ├── Finch (HTTP clients)
    └── Telemetry/metrics exporters
```

## Agent process responsibilities

Each `AgentProcess`:

- Accepts frames/events (user prompt, sensory signal, cron tick, tool result)
- Decides next action:
  - emit assistant message delta
  - call tool(s)
  - schedule subtask/subagent scan
  - compact memory segment
- Periodically checkpoints process state to Postgres
- Can fully recover from checkpoint + event log replay

## Important constraint for Cloud Run

Cloud Run instances are not permanent. Therefore:

- GenServer process state cannot be treated as durable.
- Any state needed after restart must be persisted.
- In-memory state is a performance cache and coordination helper only.

This plan enforces that from day one.

## Memory architecture (tiered timeline)

Implement the proposed tier system directly:

- Hot window: raw events for last 20-30 minutes
- L1: model-generated 10-minute compactions
- L2: hourly rollups from L1
- L3: daily rollups from L2

Core operations:

1. `compact_l1` (cron + pressure-triggered)
2. `rollup_l2` (hourly)
3. `rollup_l3` (daily)
4. `expand_chunk` (on-demand downward traversal)
5. `timeline_map_reduce` (subagent retrieval across chunks)

All compactions produce auditable artifacts:

- input chunk ids
- output chunk id
- model/provider metadata
- token/latency stats
- hash of output summary text

## Tool execution model

Tool calls run async under `Task.Supervisor`.

State machine per tool task:

- `queued`
- `running`
- `streaming_output` (optional partial output)
- `succeeded`
- `failed`
- `canceled`
- `timed_out`

Required capabilities:

- start tool execution without blocking agent inference loop
- stream partial progress events
- cancel tool execution from user request or policy engine
- persist deterministic receipts (params hash, output hash, latency, error)

## Compatibility with current Laravel stream/UI

Do not break frontend transport initially.

Elixir emits runtime-native events, then maps to AI SDK stream events via:

- `lib/openagents_runtime/integrations/laravel_event_mapper.ex`

Mapping examples:

- runtime `run.started` -> `{"type":"start"}`
- runtime `run.step_started` -> `{"type":"start-step"}`
- runtime `message.delta` -> `{"type":"text-delta","id":"...","delta":"..."}`
- runtime `run.finished` -> `{"type":"finish", ...}`
- runtime terminal -> `data: [DONE]`

## Deployment options: Fly.io vs GCP

## Fly.io strengths

- Excellent BEAM ergonomics (long-lived processes, easy clustering, region placement).
- Operationally simple for Phoenix/Elixir teams.
- Straightforward websocket support.

## Fly.io drawbacks for OpenAgents now

- Splits infra across clouds (current stack is GCP-centric).
- Adds cross-cloud latency and egress between Laravel, DB, Redis, and runtime.
- Increases operational surface and incident complexity.

## GCP strengths

- Co-locates runtime with existing OpenAgents services, secrets, monitoring, and DB.
- Reuses current Cloud Build, Artifact Registry, Cloud Run conventions.
- Simplifies IAM and service-to-service trust with existing project setup.

## GCP drawbacks

- Cloud Run is not a stable node identity platform for classic distributed Erlang clustering.
- Must design for stateless/process-recoverable runtime semantics.

## Recommendation

Start on GCP (Cloud Run) now.  
Only revisit Fly.io if we later require persistent BEAM cluster semantics that Cloud Run cannot satisfy without too much complexity.

## GCP reference topology (recommended)

Project/region aligned with current app:

- Project: `openagentsgemini`
- Region: `us-central1`

Services/jobs:

1. `openagents-runtime-api` (Cloud Run service)
   - Internal ingress only
   - Handles internal HTTP/SSE API from Laravel
   - Runs Phoenix endpoint + runtime supervisors

2. `openagents-runtime-jobs` (Cloud Run job or secondary service command)
   - Runs migrations, backfills, maintenance one-offs

3. Optional: `openagents-runtime-worker` (Cloud Run service)
   - If we split web ingress from heavy background execution later

Backing services:

- Cloud SQL Postgres (same instance or dedicated DB/schema)
- Memorystore Redis (optional phase-1, recommended phase-2)
- Secret Manager (API keys, DB creds, service auth secrets)
- Cloud Scheduler (if external cron needed, though Oban cron is preferred)

## Networking and security model

## Ingress

`openagents-runtime-api` should be internal/private:

- Cloud Run ingress: internal + load balancer or internal only
- No public internet traffic directly to runtime endpoints

Laravel reaches runtime via private network path.

## Service-to-service auth

Use both:

1. GCP identity-based service invocation (IAM-bound service account)
2. Signed internal runtime token (`X-OA-RUNTIME-SIGNATURE`) with short TTL

This dual layer protects against misconfigured ingress and replay.

## Secrets

Create dedicated secrets:

- `openagents-runtime-db-password`
- `openagents-runtime-internal-signing-key`
- `openagents-runtime-openrouter-api-key` (if needed)
- `openagents-runtime-ai-gateway-api-key` (if needed)
- tool-specific creds as runtime grows

## CI/CD and build pipeline in monorepo

## Build config

Add:

- `apps/openagents-runtime/Dockerfile`
- `apps/openagents-runtime/deploy/cloudbuild.yaml`

Cloud Build submit path:

```bash
gcloud builds submit \
  --config apps/openagents-runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/openagents-runtime
```

## Deploy scripts

Add:

- `apps/openagents-runtime/deploy/deploy-production.sh`
- `apps/openagents-runtime/deploy/apply-production-env.sh` (same pattern as Laravel app)
- `apps/openagents-runtime/deploy/smoke/health.sh`
- `apps/openagents-runtime/deploy/smoke/stream.sh`

## Runtime migration command

Cloud Run job command:

- `bin/openagents_runtime eval "OpenAgentsRuntime.Release.migrate()"`

Release helper module required:

- `lib/openagents_runtime/release.ex`

## Rollout strategy (phased, with flags)

## Phase 0: Scaffold runtime app

Deliverables:

- New Elixir app compiles/tests locally
- Health endpoints
- Runtime contract doc
- No production traffic

Exit criteria:

- `mix test` green
- container builds and starts on Cloud Run
- health smoke passes

## Phase 1: Shadow mode (no user impact)

Behavior:

- Laravel continues current runtime path.
- Laravel asynchronously mirrors eligible chat requests to Elixir shadow endpoint.
- Compare event outputs and timing offline.

Deliverables:

- Diff tooling for `run_events` parity
- Latency and error telemetry comparisons

Exit criteria:

- 95%+ semantic parity for sampled runs
- no elevated error rates in Laravel path

## Phase 2: Canary mode (small traffic %)

Behavior:

- Feature flag routes selected users/autopilots to Elixir runtime.
- Laravel fallback to legacy orchestrator on runtime failure.

Flags:

- `RUNTIME_DRIVER=legacy|elixir`
- per-user/per-autopilot override table in Laravel

Exit criteria:

- runtime error budget stable
- no regression in stream UX
- recovery paths validated in production

## Phase 3: Default-on

Behavior:

- Elixir runtime default for authenticated users.
- Legacy Laravel runtime retained as emergency rollback path.

Exit criteria:

- 2-4 weeks stable SLO
- incident playbooks exercised

## Phase 4: Autonomous frame features

Behavior:

- Sensory subscriptions and periodic frame ingestion enabled
- tiered memory compaction on production schedules
- timeline map/reduce enabled behind targeted flags

## Laravel change plan (specific)

## New config

`apps/openagents.com/config/runtime.php`

Keys:

- `driver` (`legacy`/`elixir`)
- `elixir.base_url`
- `elixir.timeout_ms`
- `elixir.connect_timeout_ms`
- `elixir.signing_secret`
- `elixir.require_iam_identity` (bool)

## Runtime client abstraction

Interface:

- `startRun(...)`
- `streamRun(...)`
- `cancelRun(...)`
- `getRun(...)`

Concrete clients:

- `LegacyLaravelRuntimeClient` (current behavior)
- `ElixirRuntimeClient` (HTTP/SSE to runtime service)

## Controller/orchestrator changes

`ChatApiController`:

- keep auth/guest/session ownership checks unchanged
- delegate runtime execution to runtime client via orchestrator

`RunOrchestrator`:

- reduced to projection/fallback compatibility layer over runtime client

## DB ownership changes

Initially:

- Laravel still writes existing tables for legacy mode.
- Elixir writes runtime tables + projection tables for elixir mode.

Eventually:

- Elixir becomes sole writer for runtime projections in elixir mode.

## Elixir implementation milestones (specific)

## Milestone A: Runtime API skeleton

- Phoenix endpoint/router/controllers
- health/readiness
- signed request verification

## Milestone B: Run acceptance + stream

- start/continue run endpoints
- SSE stream output mapper compatible with AI SDK protocol

## Milestone C: Agent process supervision

- DynamicSupervisor and per-agent process startup
- frame ingestion loop
- checkpoint persistence

## Milestone D: Tool async model

- non-blocking tool tasks
- progress events
- cancellation path

## Milestone E: Tiered memory

- L1/L2/L3 compaction jobs
- chunk expansion APIs
- retention policies

## Milestone F: Timeline map/reduce

- spawn subagent retrieval workers
- aggregate semantic outputs
- merge back into main agent context

## Observability and SLOs

## Required telemetry dimensions

- `run_id`, `thread_id`, `user_id`, `autopilot_id`
- runtime driver (`legacy|elixir`)
- model/provider
- tool name/status
- compaction level (`raw|l1|l2|l3`)
- phase (`ingest|infer|tool|persist|stream`)

## Minimum dashboards

1. Runtime health
   - request rate, p95 latency, 5xx rate
2. Run lifecycle
   - started/completed/failed/canceled counts
3. Stream integrity
   - streams opened, `[DONE]` delivered, client disconnects
4. Tool lifecycle
   - queue depth, task duration, failure rate, cancel rate
5. Memory compaction
   - backlog, chunk counts, compaction latency

## SLO targets (initial)

- Run start accepted: 99.9%
- Stream starts under 2s: 99%
- Run completion without internal error: 99%
- Compaction job success: 99%

## Testing strategy

## Elixir app tests

- unit tests for agent/frame/memory/tool modules
- integration tests for runtime API contracts
- property tests for event ordering/idempotency (recommended)

Commands:

- `cd apps/openagents-runtime && mix deps.get`
- `cd apps/openagents-runtime && mix test`
- `cd apps/openagents-runtime && mix format --check-formatted`
- `cd apps/openagents-runtime && mix credo` (if enabled)
- `cd apps/openagents-runtime && mix dialyzer` (phase 2+)

## Laravel integration tests

Add tests validating:

- fallback behavior on runtime errors
- SSE protocol frame continuity through proxy
- cancel behavior
- ownership/auth semantics unaffected by runtime driver changes

Commands:

- `cd apps/openagents.com && php artisan test`

## End-to-end smoke suite

- authenticated stream start/finish
- guest chat stream
- tool call path
- forced runtime restart + resume
- cancel in-progress run

## Rollback strategy

Fast rollback is flag-based, not deploy-based.

1. Set `RUNTIME_DRIVER=legacy` in Laravel service env.
2. Keep runtime service running for diagnostics.
3. If needed, disable Laravel calls to runtime entirely.

Data safety:

- Elixir writes append-only runtime events.
- Projection failures do not delete runtime history.
- Backfill/reprojection jobs can rebuild Laravel-facing projections.

## Risks and mitigations

## Risk: Cloud Run restarts kill in-memory agent processes

Mitigation:

- checkpoint often
- replay from events on restart
- never rely on memory-only state

## Risk: SSE proxy complexity/regressions

Mitigation:

- keep event mapping deterministic
- parity tests against existing `ChatStreamingTest` expectations
- shadow mode diffing before canary

## Risk: dual writer inconsistency (Laravel + Elixir)

Mitigation:

- strict ownership per runtime driver mode
- idempotent writes with deterministic ids
- projection reconciler jobs

## Risk: runtime service becomes public attack surface

Mitigation:

- internal ingress only
- IAM service auth
- HMAC/JWT request signing
- short-lived nonces

## Capacity and scaling notes

Initial Cloud Run sizing (starting point, tune with load tests):

- `openagents-runtime-api`
  - CPU: 2
  - Memory: 2Gi
  - Min instances: 1
  - Max instances: 10
  - Concurrency: 100 (reduce if tail latency rises)
  - Timeout: set for long stream windows (within Cloud Run limits)

If runtime workloads outgrow Cloud Run semantics:

Phase-2 platform option:

- Move runtime to GKE Autopilot (or Compute Engine managed cluster) while preserving same internal HTTP contract, so Laravel integration remains unchanged.

## Concrete execution checklist

1. Scaffold `apps/openagents-runtime` with Phoenix/OTP.
2. Add runtime API contract doc and generated OpenAPI for internal endpoints.
3. Add deploy assets under `apps/openagents-runtime/deploy`.
4. Deploy runtime service to GCP in internal-only mode.
5. Add Laravel runtime client + config flags.
6. Implement shadow traffic mirror and parity diff pipeline.
7. Enable canary by user/autopilot flags.
8. Promote to default-on when SLO and parity pass.
9. Enable autonomous frame + tiered memory features gradually.

## Non-goals

- Rewriting `apps/openagents.com` to Phoenix.
- Replacing WorkOS/Sanctum session and guest auth flows.
- Replacing existing public API route structure in this phase.
- Building full distributed Erlang cluster semantics on day one.

## Decision log

- 2026-02-18: Chose runtime-only migration (no web rewrite).
- 2026-02-18: Chose monorepo placement `apps/openagents-runtime`.
- 2026-02-18: Chose GCP-first deployment strategy, Fly.io deferred.
- 2026-02-18: Preserved SSE AI SDK compatibility as migration invariant.
