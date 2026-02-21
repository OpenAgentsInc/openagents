# OpenAgents Elixir Agent Runtime Plan (No Web Rewrite)

Date: 2026-02-18  
Status: Active plan (Epic 0-3 implementation complete; next execution wave in progress)  
Owner: OpenAgents platform/runtime  
Scope: Add an Elixir-based agent runtime to `~/code/openagents` while keeping `apps/openagents.com` (Laravel) as the web/control plane.

## Progress snapshot (updated 2026-02-18)

Delivered in `apps/runtime`:

- Runtime app scaffold, CI workflow, Docker/Cloud Build, and GKE manifest skeletons.
- Internal contract docs and OpenAPI artifact.
- Signed token verifier module, ownership guard, and trace propagation plumbing.
- Baseline runtime schema, run event log, frames, leases, append notifications, hash-chain integrity.
- Stream endpoint with cursor resume semantics, wakeup-driven tailing with backoff, and Laravel SSE mapper + golden tests.

Closed issue range for this delivered slice:

- `#1655` through `#1671` (Epic 0, Epic 1/A, Epic 2/B, Epic 3/C).

Roadmap adjustments from implementation review:

1. Add explicit auth enforcement middleware for `/internal/v1/*` so signed token verification is mandatory in request flow, not only available as a module.
2. Keep contract artifacts aligned with implemented runtime behavior:
`thread_id` ownership parameter requirements on stream/snapshot/frame append and `tail_ms` stream-tail control.
3. Add contract-convergence checks in CI to prevent drift between controller behavior, `RUNTIME_CONTRACT.md`, and `openapi-internal-v1.yaml`.

## Goal

Ship a production Elixir runtime for long-running autonomous agents (GenServer/Supervisor model), without rewriting `apps/openagents.com`.

The Laravel app remains the user-facing product surface and API contract. Elixir becomes the autonomous execution engine.

DS-Elixir-specific migration and contract details are in `docs/plans/active/elixir-ds-runtime-integration-addendum.md`.

## Strategic context: how this runtime enables the OpenAgents vision

OpenAgents is explicitly pursuing the "operating system for the AI agent economy" model described in `docs/SYNTHESIS.md`: identity rails, payment rails, market rails, and transparency rails that let agents operate as sovereign economic actors rather than as stateless chat endpoints. That strategy only works if the runtime can sustain long-lived autonomous behavior with durable state transitions, policy-enforced spending, reproducible trajectories, and fault-tolerant recovery across continuous operation. In other words, this is not just about faster chat responses. It is about building a reliable execution substrate for agents that need to coordinate work, spend budget, call tools, participate in markets, and continue operating through failures without violating financial or policy boundaries.

The wedge-to-platform path in `docs/SYNTHESIS.md` starts with Autopilot and expands toward skills and compute marketplaces, treasury controls, and eventually protocol-level agent interoperability. The runtime is the bridge across those stages. Autopilot as "first buyer" creates demand floor by continuously purchasing inference, verification, and compute work; that demand floor only remains trustworthy if runtime execution is deterministic enough to audit, resilient enough to recover, and scalable enough to manage many concurrent agent sessions and sub-tasks. The architecture in this plan is therefore a platform decision, not a local implementation detail: it is the mechanism that turns product usage into reliable market activity and durable protocol network effects.

## Why Elixir/BEAM is the right runtime for this phase

Elixir on the BEAM is selected because OpenAgents needs runtime-level concurrency semantics, not library-level approximations. The target workload is a large number of concurrent, long-lived, stateful agent sessions with asynchronous tool execution, intermittent external API failures, and continuous background compaction/reconciliation work. BEAM's process isolation, message passing, supervisor hierarchies, preemptive scheduling, and process-level introspection directly match this workload shape. The intent is to treat agent failures as routine events in a supervised system rather than exceptional control-flow accidents in a shared-state runtime.

Just as importantly, the BEAM model supports the operational quality required by OpenAgents' economic layer: budget enforcement, payment-linked receipts, and post-failure reconciliation are only credible when runtime failures are contained and recovery paths are first-class. "Let it crash" is not a slogan here; it is a mechanism for keeping the system live while preserving correctness boundaries for run acceptance, event logs, checkpoints, and settlement-relevant projections. This plan adopts Elixir/BEAM because it reduces systemic fragility in exactly the areas where OpenAgents must be strongest to realize the OS vision in production.

## DS-Elixir is part of the runtime architecture, not a later add-on

OpenAgents previously implemented a substantial DSPy-inspired DSE layer in the now-removed `apps/web` stack (`8c460f956` removal commit; key prior integration state in `42700ddef`, plus tool replay hardening in `c79250a58`). That implementation already validated critical behavior-shaping primitives: stable signature IDs, artifact-pinned strategy selection (`direct.v1` and `rlm_lite.v1`), deterministic budget envelopes, canary policy selection, prediction receipts/traces, and compile/promote loops with rollback.

This plan carries those primitives forward into Elixir as DS-Elixir inside `apps/runtime`. The goal is to preserve what worked in DSE while removing the previous coupling constraints. DS-Elixir is therefore a first-class subsystem in this runtime plan because autonomous execution quality depends on explicit contracts for inference behavior, not only on process supervision.

DS-Elixir constraints adopted in this plan:

1. Signature contracts are versioned, typed, and hashable.
2. Compiled artifacts are immutable; promotion/rollback is pointer-based.
3. Inference strategy is explicit and receipt-visible (`direct.v1` or `rlm_lite.v1`).
4. Budget usage is recorded per run for audit and guardrails.
5. Tool replay context is bounded and redacted before reinjection.
6. Compile/eval/canary flows are operational controls, not ad-hoc scripts.

## Hard decisions (locked)

1. No Phoenix/Laravel rewrite in this plan.
2. Elixir runtime lives inside the existing `openagents` monorepo under `apps/`.
3. Primary deployment target is GCP GKE Standard using StatefulSets and stable BEAM node identity.
4. Runtime must be restart-safe: process memory is a cache, not source of truth.
5. Existing frontend stream contract (AI SDK/Vercel protocol over SSE) is preserved for compatibility.
6. DS-Elixir is included in runtime scope for signature execution, receipts, and compile/promotion control.
7. Stream serving is location-independent (`stream-from-log`), never sticky-pod dependent for correctness.
8. Single active executor per run is enforced with a Postgres lease row (`runtime.run_leases`) + TTL heartbeat semantics.

## Correctness dependencies (day-one invariants)

1. Event log is source of truth for run state and stream reconstruction.
2. Any runtime pod can serve `GET /internal/v1/runs/{runId}/stream` by tailing durable events.
3. At most one active executor exists per run via lease acquisition/renewal on `runtime.run_leases`.
4. BEAM distribution is optional for correctness; it is an optimization for coordination/fanout.

## Day-1 critical decisions checklist

- `stream-from-log` selected as migration invariant.
- lease-row single executor guarantee selected.
- frame-only ingestion contract selected (no full history payload from Laravel).
- dual-writer overlap minimized by projection ownership and reprojection tooling.

## BEAM lessons explicitly adopted in this plan

The following BEAM/OTP lessons are now design constraints, not optional implementation details:

1. Process-per-session/per-agent isolation.
2. Message passing between processes; no shared mutable in-memory session state.
3. Supervisor-driven recovery ("let it crash") for non-deterministic AI/tool failures.
4. Preemptive scheduling advantage is preserved by avoiding CPU-heavy work on scheduler threads.
5. Runtime introspection is first-class: process mailboxes, reductions, memory, restart counts, queue depth.

Practical interpretation for OpenAgents:

- AI and tool failures are expected conditions, not exceptional architecture events.
- Recovery policy must be in supervision strategy and persisted checkpoints, not only try/catch logic.
- Any state required for correctness must survive process/node restart.

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

- `apps/runtime/`

Proposed structure:

```text
apps/runtime/
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
      ds/
        signatures/
          catalog.ex
        predict.ex
        strategies/
          direct_v1.ex
          rlm_lite_v1.ex
        policy_registry.ex
        receipts.ex
        traces.ex
        tool_replay.ex
        compile/
          compile_service.ex
          dataset_exporter.ex
          promote_service.ex
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
- `apps/runtime/README.md`
- `apps/runtime/docs/DEPLOY_GCP.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `apps/runtime/docs/OPERATIONS.md`
- `apps/runtime/docs/DS_ELIXIR_RUNTIME_CONTRACT.md`
- `apps/runtime/docs/DS_ELIXIR_OPERATIONS.md`
- `docs/PROJECT_OVERVIEW.md` update with new app
- `docs/README.md` update with new runtime docs links
- `docs/plans/active/elixir-ds-runtime-integration-addendum.md`

## Target architecture (no rewrite)

## Responsibilities

Laravel (`apps/openagents.com`) keeps:

- User auth/session (WorkOS, Sanctum, guest bootstrap behavior)
- Public API surface and route contracts
- Web pages, Inertia/React UI
- Product/business APIs (profile, tokens, admin pages, etc.)
- Existing DB read models consumed by UI/API

Elixir (`apps/runtime`) owns:

- Long-running agent process lifecycle
- Frame-based inference loop
- Async tool execution and cancellation
- Tiered memory compaction and expansion
- Autonomous scheduling (cron-style ticks, map/reduce scans)
- DS-Elixir signature execution (strategy selection, budgets, receipts, traces)
- DS-Elixir artifact policy registry and canary selection
- DS-Elixir compile/eval/promote/rollback controls
- Runtime event emission for UI stream compatibility

## High-level data flow

1. Browser sends message to existing Laravel endpoint (`/api/chat` or `/api/chats/{id}/stream`).
2. Laravel validates auth/session/ownership exactly as today.
3. Laravel calls Elixir internal API to start run (if needed) and append a frame into the run event log.
4. Elixir acquires/renews run executor lease and executes DS-Elixir signatures for routing/tool/memory decisions.
5. Laravel proxies runtime stream frames to browser as SSE (same AI SDK protocol), and stream data is served from durable log tailing (not pod affinity).
6. Elixir persists canonical runtime events + run state + DS receipts/traces.
7. Laravel read APIs continue returning conversation/runs/events for refresh/history.

## DS-Elixir execution loop (inside a run)

1. Accept frame/event and append durable runtime event.
2. Choose relevant signature(s) for the frame (for example: tool selection, recap, upgrade detection).
3. Resolve active compiled artifact for signature (or fall back to signature defaults).
4. Execute predict with explicit strategy (`direct.v1` or `rlm_lite.v1`) and budgets.
5. Run tools asynchronously when selected; persist tool receipts/events.
6. Build bounded/redacted tool replay context for subsequent inference.
7. Persist predict receipt and optional trace handle.
8. Continue until deterministic terminal run state is emitted.

## Run lifecycle state machine and hard execution caps (required)

Run lifecycle states:

- `accepted`
- `running`
- `waiting_tool`
- `cancel_requested`
- terminal: `completed | failed | canceled`

Terminal reason classes (minimum):

- `completed`
- `canceled_by_user`
- `max_steps_exceeded`
- `max_wall_clock_exceeded`
- `max_tokens_exceeded`
- `max_model_calls_exceeded`
- `max_tool_calls_exceeded`
- `policy_budget_exhausted`
- `internal_error`

Deterministic hard caps (policy configurable, must be enforced):

1. max steps per run
2. max wall-clock duration per run
3. max model tokens per run
4. max model calls per run
5. max tool calls per run

Cap breach behavior:

- append explicit cap-breach event,
- stop new work immediately,
- transition run to deterministic terminal state with reason class.

## Run-level concurrency control (single executor guarantee)

To prevent double execution and duplicate side effects:

1. Runtime acquires lease in `runtime.run_leases` keyed by `run_id`:
   - acquire if absent, or
   - steal only when `lease_expires_at < now()`.
2. Active executor heartbeats lease on short interval while processing frames.
3. If lease is held by a live executor, runtime returns `already_running` (or stream-attach response) and does not execute tools/model calls.
4. Executor releases lease on terminal run states; crash recovery relies on lease TTL expiration.

Implementation note:

- This plan chooses lease rows over session-bound advisory locks to avoid Ecto connection-pool edge cases for long-lived run execution.
- Recommended acquisition pattern:
  - `INSERT ... ON CONFLICT (run_id) DO UPDATE ... WHERE run_leases.lease_expires_at < now()`
  - or equivalent `SELECT ... FOR UPDATE SKIP LOCKED` lease transaction.

Lease tuning (initial defaults):

- heartbeat interval: 3s
- lease TTL: 20s
- steal gate: lease expired and no progress watermark movement in the steal window
- do not steal when progress watermark is moving, even if clock skew suggests expiry

This gives at-most-one active executor per run without requiring sticky routing or cluster-wide process discovery for correctness.

## Internal runtime contract (Laravel <-> Elixir)

All runtime APIs are internal-only (`/internal/v1/*`), authenticated via service-to-service identity + signed token.

Trace propagation contract (required):

- Laravel forwards W3C `traceparent` (and `tracestate` when present) plus `x-request-id`.
- Runtime preserves/propagates these ids through:
  - run/frame/event writes,
  - model/tool calls,
  - DS receipts/traces.
- All logs and metrics must be joinable by trace/request id across browser -> Laravel -> runtime -> tool/model boundaries.

Confused-deputy prevention (required):

1. Runtime does not trust inbound `userId`/`threadId` claims blindly.
2. Runtime validates that `run_id` and `thread_id` belong to the claimed user (or guest scope) in durable DB state.
3. Signed internal token claims include `run_id`, `thread_id`, `user_id` (or guest scope id), and `exp`.
4. Runtime rejects claim/payload/DB mismatches as authorization failures.

## Start run

`POST /internal/v1/runs`

Purpose: create or upsert run envelope metadata and return acceptance status.

Request (shape, not final):

```json
{
  "runId": "uuid",
  "threadId": "conversation-id",
  "userId": 123,
  "autopilotId": "optional",
  "authenticatedSession": true,
  "authorizationRef": {
    "autopilotId": "ap_...",
    "mode": "delegated_budget"
  },
  "toolPolicy": {
    "allow": [],
    "deny": [],
    "guestMode": false
  }
}
```

`authorizationRef` identifies the active spending authorization context. Runtime resolves and validates authoritative records from DB; it does not treat request payload as proof of permission.

Response:

```json
{
  "runId": "uuid",
  "status": "accepted"
}
```

## Append frame (primary ingestion contract)

`POST /internal/v1/runs/{runId}/frames`

Purpose: append one idempotent frame/event to the run log. Laravel sends minimal payload only; runtime loads historical context from durable runtime state + tiered memory.

Request (shape, not final):

```json
{
  "frameId": "uuid-or-deterministic-id",
  "type": "user_message",
  "payload": {
    "text": "latest user text"
  },
  "source": {
    "kind": "laravel_api",
    "requestId": "req_..."
  }
}
```

Rules:

- `frameId` is idempotency key for retries.
- Runtime rejects duplicate frame appends as no-op success.
- Laravel must not send full `messages` or `contextFrames` arrays.

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

Streaming architecture (day one):

- Stream endpoint is location-independent (`stream-from-log`).
- Any pod can serve stream by tailing persisted run events.
- No sticky session routing is required for correctness.
- Reconnect uses event cursor/watermark so clients resume without replay ambiguity.
- Stream wakeups use Postgres `LISTEN/NOTIFY` on event append; DB log tail remains source of truth.

Stream cursor contract (required):

1. Every run event has strictly monotonic `(run_id, seq)` ordering.
2. Stream resume supports both:
   - `Last-Event-ID: <seq>`
   - query param `?cursor=<seq>`
3. Server guarantees gap-free replay from cursor+1 while retained events exist.
4. If cursor points before retention floor, server returns explicit stale-cursor response with restart instructions.
5. SSE `id:` field is always set to event `seq`.

Event append transaction boundary (required):

1. Event insert, `seq` allocation, and `NOTIFY` are emitted in the same DB transaction.
2. `seq` allocation is transactional (sequence or per-run allocator), not in-memory.
3. Projection watermarks advance only from committed events.
4. `NOTIFY` is a wakeup signal only; stream readers always re-read committed DB state.

Tamper-evident event integrity (required):

1. each run event stores `prev_hash` and `event_hash` (per-run hash chain).
2. checkpoints store chain head hash at checkpoint time.
3. replay/projection paths can verify chain integrity on demand.
4. integrity mismatch emits explicit audit/integrity incident events.

Laravel should proxy bytes with minimal transformation.

## Cancel run

`POST /internal/v1/runs/{runId}/cancel`

Cancellation contract:

1. Append durable `run.cancel_requested` event immediately.
2. Executor stops starting new model/tool work after cancel is observed.
3. In-flight work receives best-effort cancel/timeout signals.
4. If in-flight work still returns, result is marked late and is either discarded or recorded as non-authoritative (must not reopen run).
5. Run ends in deterministic terminal state (`canceled` or `failed`) with reason classification.

## Snapshot/recovery

`GET /internal/v1/runs/{runId}`
`GET /internal/v1/threads/{threadId}/state`

Used by Laravel for recovery, diagnostics, and fallback paths.

## Crash recovery reconciliation loop (janitor)

Run continuously as maintenance worker:

1. Detect runs in `running` with stale lease heartbeat/progress.
2. Append `run.executor_lost` event with recovery metadata.
3. Attempt safe resume via normal lease acquisition path.
4. If recovery budget/time is exceeded, append deterministic terminal failure with explicit error class.

This turns pod death or node eviction into a normal recoverable lifecycle outcome.

## Agent lifecycle endpoints (phase 2+)

- `POST /internal/v1/agents/{agentId}/wake`
- `POST /internal/v1/agents/{agentId}/sleep`
- `POST /internal/v1/agents/{agentId}/frames`

These support autonomous frame ingestion beyond user-request loops.

## Runtime data model strategy

Goal: keep compatibility for Laravel APIs while giving Elixir first-class event sourcing for long-lived agents.

## Preferred approach

Use the same Postgres cluster but a dedicated schema for runtime internals.

- Existing Laravel tables remain source for current API contracts in legacy mode.
- In elixir mode, Laravel-facing tables become read models projected from runtime events:
  - `threads`, `runs`, `messages`, `run_events`, `autopilots*`
- New Elixir schema for deeper runtime state:
  - `runtime.agent_instances`
  - `runtime.frames`
  - `runtime.run_leases`
  - `runtime.spend_authorizations`
  - `runtime.spend_reservations`
  - `runtime.frame_chunks_l1`
  - `runtime.frame_chunks_l2`
  - `runtime.frame_chunks_l3`
  - `runtime.tool_tasks`
  - `runtime.checkpoints`
  - `runtime.expansion_jobs`
  - `runtime.ds_signatures`
  - `runtime.ds_active_policies`
  - `runtime.ds_compiled_artifacts`
  - `runtime.ds_predict_receipts`
  - `runtime.ds_traces`
  - `runtime.ds_examples`
  - `runtime.ds_compile_reports`
  - `runtime.ds_eval_reports`

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

DS-Elixir artifacts and receipts remain runtime-internal sources of truth. Laravel projections should expose only summary/debug fields needed by existing product surfaces.

Projection writer requirements:

1. Idempotent writes using deterministic projection ids/keys.
2. Monotonic projection watermarks to prevent out-of-order rewinds.
3. Rebuildable read models via full reprojection jobs.

Potential simplifier (phase-2+):

- evaluate DB views/materialized views to reduce ORM coupling and dual-writer friction.

## Event log storage hygiene (required)

Because runtime event log is source of truth, storage discipline is mandatory:

1. Baseline indexes:
   - `(run_id, seq)` unique
   - `(thread_id, created_at)`
   - `(run_id, created_at)`
2. Growth plan:
   - partition event tables (time or hash strategy) before large-scale traffic,
   - keep hot partitions small enough for fast tail queries.
3. Retention policy:
   - retain canonical control/receipt events long-term,
   - age out raw high-volume deltas once compacted and checkpoint-safe,
   - enforce clear retention windows per event class.

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

Oban/load isolation rule:

- Run-execution hot paths and maintenance jobs must not compete unchecked.
- Separate Oban queues for compaction/reprojection/maintenance with strict concurrency caps.
- Prefer separate DB pool sizing (or separate repo) for heavy maintenance jobs when load increases.

## OTP primitive mapping (agent runtime)

| Runtime concern | OTP primitive | OpenAgents implementation target |
|---|---|---|
| Isolated agent state | GenServer process | `AgentProcess` per active runtime session |
| Agent discovery | Registry | `agent_registry.ex` keyed by agent/thread id |
| Failure containment | Supervisor trees | `:one_for_one` for single agent/tool failures |
| Background orchestration | Oban + supervisors | compaction, rollups, replay, reconciliation |
| Broadcast/event fanout | Phoenix.PubSub / `:pg` (phase-2) | stream fanout to Laravel proxy + internal observers |
| In-memory fast lookup | ETS (phase-2 selective) | cache-only indexes with DB as source of truth |

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

## Let-it-crash policy boundaries

We adopt "let it crash" for runtime process correctness, but with explicit boundaries to prevent user-facing data loss:

- Crash-allowed components:
  - per-agent inference process
  - per-tool task process
  - per-run transient stream process
- Must-never-lose boundaries:
  - run acceptance record
  - frame/event append log
  - checkpoint state and projection watermark
- Required guarantees:
  - idempotent run/frame ingestion (client retries safe)
  - restart from last durable checkpoint + replay tail events
  - deterministic terminal status (`completed|failed|canceled`) per run id

## Backpressure and mailbox controls (required)

To avoid hidden overload pathologies in high-concurrency agent workloads:

- Set explicit mailbox and queue-size thresholds for agent and tool processes.
- Emit telemetry and shed load/slow-path when thresholds are exceeded.
- Bound concurrent tool tasks per agent and globally.
- Separate latency-sensitive stream processes from heavy maintenance queues.
- Add admission control for new runs when runtime is degraded.

## Admission control limits (required)

Define explicit caps (policy/tier configurable):

1. max concurrent runs per user/autopilot
2. max tool tasks per run and global max tool tasks
3. max event emission rate per run (deltas/sec)
4. max payload bytes for frame append
5. max payload bytes for tool result persistence
6. max concurrent stream consumers per run

On limit breach, return explicit throttle/reject outcomes; do not allow uncontrolled queue growth.

## CPU and blocking work policy

BEAM preemption is valuable only if we avoid scheduler starvation:

- No heavy CPU parsing/transforms inside hot GenServer callbacks.
- Offload expensive work to supervised tasks or native services with timeouts.
- Keep callback handlers short; push long operations to async continuation steps.
- Track scheduler utilization and long reductions as production alerts.

## Important constraints for GKE + BEAM clustering

Even with stable StatefulSet identities, pod restarts and node drains are normal in Kubernetes. Therefore:

- GenServer process state cannot be treated as durable.
- Any state needed after restart must be persisted.
- In-memory state is a performance cache and coordination helper only.
- Cluster membership must converge automatically after pod reschedules.
- Network partitions (netsplits) are expected failure modes, not edge cases.

This plan enforces those constraints from day one.

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

## Budget attribution policy for background work (required)

Background runtime jobs (compaction, rollups, timeline map-reduce, maintenance inference) must have explicit budget attribution:

1. default attribution: `maintenance_budget` (system-funded), not user delegated spend authorization.
2. user/autopilot budget attribution is allowed only when operation is explicitly user-initiated and marked as billable.
3. every background receipt must record:
   - `payer_type` (`system` | `user` | `autopilot`)
   - `authorization_id` when applicable
   - budget deltas and decision path

This prevents silent user budget drain from autonomous maintenance work.

## DS-Elixir contract layer (required)

Core DS-Elixir runtime contracts in this plan:

- Signature contract:
  - `signature_id` (stable versioned ID)
  - input/output schema hash
  - prompt/program hash
  - default strategy + budgets + constraints
- Compiled artifact:
  - immutable `compiled_id`
  - signature compatibility hashes
  - chosen params/strategy
  - provenance (`job_hash`, `dataset_hash`, compiler version)
- Predict receipt:
  - `signature_id`, `compiled_id`, `strategy_id`
  - `params_hash`, `prompt_hash`, `output_hash`
  - budget limits/usage and latency counters
  - `authorization_id`, `authorization_mode`
  - budget accounting deltas (`budget_before`, `reserved`, `spent`, `budget_after`)
  - `policy_decision` (`allowed`, `denied_over_budget`, `denied_tool_not_allowed`, `needs_interactive_approval`)
  - error class and optional trace reference

Canonical contract requirement:

- Define these as canonical JSON schemas shared by Elixir runtime and Laravel integration surfaces (language-agnostic contract source of truth).

Design rules:

1. Artifact mutation is forbidden; only active pointer changes.
2. Any strategy/budget decision affecting behavior must be receipt-visible.
3. Compile/promotion uses deterministic job and dataset hashes.

Large artifact/trace storage seam:

- Keep metadata and pointers in Postgres.
- Store large trace/artifact payloads in object storage (GCS) with immutable object keys and content hashes.
- Keep DB payload size bounded to prevent table bloat and degraded query paths.

Event evolution requirements:

- every runtime event carries `event_type` + `event_version`.
- replay path includes upcasters to map older versions into current in-memory shapes.
- breaking event payload changes require version bump + tested upcaster coverage.

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
- persist bounded/redacted tool replay summaries for next-frame context injection

Authorization modes for settlement-boundary tools (required):

- `interactive`: runtime requires explicit in-the-moment approval before execution.
- `delegated_budget`: runtime may execute while user is AFK if all budget/constraint checks pass.
- `deny`: runtime must never execute settlement-boundary actions.
- `delegated_budget_with_threshold` (optional): delegated under threshold; interactive required above threshold.

Interactive approval is optional mode, not a global default.

SpendAuthorization (Budget Envelope) primitive (required):

Control-plane issued authorization envelope used for unattended execution:

- `authorization_id`
- `owner_user_id`
- scope: `{autopilot_id?, thread_id?, run_id?}`
- `mode`: `interactive | delegated_budget | deny | delegated_budget_with_threshold`
- `expires_at`
- budgets:
  - `max_total_sats`
  - `max_per_call_sats`
  - optional `max_per_day_sats`
- constraints:
  - tool allowlist/denylist
  - domain/provider allowlist for settlement-boundary tools
  - optional model/provider allowlists
  - optional time-window constraints
- accounting:
  - `reserved_sats`
  - `spent_sats`
  - derived `remaining_sats`

Runtime must validate and enforce SpendAuthorization without user interaction when mode allows delegation.

Settlement-boundary execution flow (required):

1. Identify `settlement_boundary=true` for the tool intent.
2. Resolve active `SpendAuthorization` by scope (`run_id`, `thread_id`, `autopilot_id`).
3. Perform atomic reserve:
   - reserve succeeds only if `spent + reserved + requested_amount <= budget_limit`.
4. Execute tool with deterministic `tool_call_id` and provider idempotency key.
5. On success: commit reservation (`spent += amount`, `reserved -= amount`) with settlement correlation ids.
6. On failure/cancel/unknown outcome: release reservation or reconcile deterministically before retry.

Financial/idempotency requirements (payments, L402, settlement-adjacent tools):

- every tool task must have deterministic `tool_call_id`
- every task declares settlement boundary: `safe_retry` or `dedupe_reconcile_required`
- settlement-affecting tools must use idempotency keys at provider boundary
- receipts must include `authorization_id`, `authorization_mode`, and settlement correlation ids
- policy decision is persisted (`allowed`, `denied_over_budget`, `denied_tool_not_allowed`, `needs_interactive_approval`)

Budget exhaustion behavior (required):

1. Append `policy.budget_exhausted` when authorization budget cannot reserve required amount.
2. Stop starting settlement-boundary tools for the affected run authorization context.
3. Either:
   - continue non-settlement work, or
   - terminate run with explicit policy error classification.
4. Never silently loop retries against exhausted budget.

SpendAuthorization revocation and reservation reconciliation (required):

1. revocation is control-plane authoritative and must be observed on every reserve/commit check.
2. revoked/expired authorization immediately blocks new settlement-boundary reserves.
3. janitor reconciles stuck reservations (reserved with no terminal commit/release due to crash/timeout).
4. `dedupe_reconcile_required` outcomes must run reconcile-before-retry, never blind retry.
5. unknown settlement outcomes must remain blocked until reconciliation determines final state.

Provider circuit breaker and fallback policy (required):

1. enforce per-provider timeout/rate budgets
2. open circuit on sustained provider failure rates
3. allow fallback provider/model only when:
   - policy allows fallback, and
   - fallback is receipt-visible (`provider_fallback`, original provider error class)
4. prohibit silent fallback that hides provider degradation

Sanitization policy for traces and tool replay (required):

1. Never persist secrets in events/receipts/traces:
   - authorization headers
   - cookies/session tokens
   - API keys/private tokens
2. Apply PII handling policy for logs and object blobs:
   - redact/hash direct identifiers unless required for explicit business record
   - enforce retention class for sensitive fields
3. Enforce sanitization in shared boundaries:
   - tool runner adapters
   - HTTP client middleware
   - trace serialization pipeline
4. Tool replay context is built only from sanitized payloads.

Tool runner isolation and egress controls (required):

1. run settlement/network-capable tools in isolated worker pool/process boundary.
2. use separate service account and minimal IAM scopes for tool workers.
3. enforce network egress allowlists at network policy/firewall layer, not only app logic.
4. secrets are never returned in tool outputs, logs, or replay payloads; enforce via shared middleware.

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

## Chosen deployment architecture: GKE Standard on GCP

This plan standardizes on GKE Standard as the correct runtime substrate for proper BEAM handling.

Why this is the chosen architecture:

- StatefulSets provide stable pod identity and DNS names for Erlang node naming.
- Headless services and Kubernetes-native discovery support reliable BEAM clustering.
- Long-lived BEAM processes run without request-timeout coupling.
- We stay co-located with current GCP infra (Cloud SQL, Secret Manager, observability stack, existing project IAM).

## BEAM cluster requirements (day one)

Note: cluster/distribution features support coordination and fanout, but correctness does not depend on cross-node process discovery.

1. StatefulSet deployment for runtime nodes (minimum 3 replicas in production).
2. Headless service for node discovery.
3. `libcluster` Kubernetes DNS strategy for automatic cluster formation/healing.
4. Erlang distribution configuration with explicit node naming and cookie management.
5. Pod anti-affinity and PodDisruptionBudget to reduce correlated restarts.
6. Graceful termination hooks (`SIGTERM` drain + checkpoint flush).
7. Narrow Erlang distribution port range (`inet_dist_listen_min/max`) for enforceable NetworkPolicy.
8. Netsplit posture: correctness remains event-log + lease-based executor control even during cluster partitions.

Recommended initial range:

- `inet_dist_listen_min=9100`
- `inet_dist_listen_max=9155`
- `epmd` on `4369` allowed only within runtime workload selectors

## Kubernetes BEAM cluster spec (concrete)

1. StatefulSet + headless service:
   - StatefulSet name: `runtime`
   - Headless service: `runtime-headless`
   - Pod DNS pattern: `runtime-<ordinal>.runtime-headless.runtime.svc.cluster.local`

2. Node identity:
   - Set node name from pod DNS (long names).
   - Use release env to configure node and cookie at startup.

3. Discovery:
   - `libcluster` with Kubernetes DNS strategy against headless service records.
   - Auto-heal cluster membership on pod replace/reschedule.

4. Network boundaries:
   - NetworkPolicy allowing Erlang distribution traffic only within runtime namespace/workload selectors.
   - Explicit allowlist for narrowed Erlang dist port range (not broad ephemeral ranges).
   - Internal HTTP API exposed through separate ClusterIP service.

5. Scheduling guarantees:
   - required pod anti-affinity across nodes
   - PDB to preserve quorum/availability during voluntary disruptions
   - topology spread constraints to avoid single-node concentration

## GKE profile (recommended)

- GKE mode: Standard
- Region: `us-central1`
- Runtime namespace: `runtime`
- Node pools:
  - `runtime-general` for API/runtime nodes
  - optional `runtime-jobs` pool for heavy compaction/reprojection jobs
- Autoscaling:
  - HPA for runtime API StatefulSet targets
  - Cluster Autoscaler enabled for node pools

## Runtime upgrade strategy (hot code vs rolling deploy)

BEAM supports hot code upgrades, but this plan does not require hot upgrades on day one.

Day-one production strategy:

- Use Kubernetes rolling updates with controlled StatefulSet rollout policies.
- Preserve runtime continuity through checkpoint + replay, not in-place VM code swaps.
- Add state-version fields in checkpoints/events to support code evolution safely.

Future optional phase:

- Evaluate full OTP release upgrades only after runtime contracts stabilize and operational burden is justified.

## GCP reference topology (recommended)

Project/region aligned with current app:

- Project: `openagentsgemini`
- Region: `us-central1`

GKE workloads:

1. `runtime` StatefulSet
   - Internal API + runtime supervisors
   - Stable pod identities for BEAM node membership
2. `runtime-migrate` Kubernetes Job
   - One-off DB migrations per release
3. `runtime-maintenance` CronJobs
   - Compaction backfills, projection reconciliation, periodic integrity scans
4. Optional `runtime-worker` Deployment/StatefulSet
   - If we split API-facing workload from heavy autonomous/background workloads

Backing services:

- Cloud SQL Postgres (same instance or dedicated DB/schema)
- Memorystore Redis (optional phase-1, recommended phase-2)
- Secret Manager (API keys, DB creds, service auth secrets)
- Cloud Scheduler (only if triggering external admin workflows; internal periodic work uses Kubernetes CronJobs and/or Oban)

## Networking and security model

## Ingress

`runtime` runtime API should be internal/private:

- Internal L7/L4 load balancer fronting GKE service
- VPC-private service routing from Laravel web service(s) to runtime service
- No public internet traffic directly to runtime endpoints

Laravel reaches runtime via private network path.

## Service-to-service auth

Use both:

1. GCP identity-based service invocation (IAM-bound service account)
2. Signed internal runtime token (`X-OA-RUNTIME-SIGNATURE`) with short TTL

Signed token claim requirements:

- include `run_id`, `thread_id`, `user_id`/guest scope id, `iat`, `exp`, and nonce
- reject expired, replayed, or claim-mismatched tokens
- bind signature verification to trusted Laravel service account identity

This dual layer protects against misconfigured ingress and replay.

## Secrets

Create dedicated secrets:

- `runtime-db-password`
- `runtime-internal-signing-key`
- `runtime-openrouter-api-key` (if needed)
- `runtime-ai-gateway-api-key` (if needed)
- tool-specific creds as runtime grows

## CI/CD and build pipeline in monorepo

## Build config

Add:

- `apps/runtime/Dockerfile`
- `apps/runtime/deploy/cloudbuild.yaml`

Cloud Build submit path:

```bash
gcloud builds submit \
  --config apps/runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/runtime
```

## Deploy scripts

Add:

- `apps/runtime/deploy/deploy-production.sh`
- `apps/runtime/deploy/apply-production-env.sh` (same pattern as Laravel app)
- `apps/runtime/deploy/smoke/health.sh`
- `apps/runtime/deploy/smoke/stream.sh`

## Runtime migration command

Kubernetes Job command:

- `bin/openagents_runtime eval "OpenAgentsRuntime.Release.migrate()"`

Release helper module required:

- `lib/openagents_runtime/release.ex`

## Zero-downtime schema evolution runbook

1. Additive schema changes first; no destructive migrations in initial deploy step.
2. Ship code that can read old+new shapes before writing new-only paths.
3. Run backfills asynchronously; avoid long blocking migrations.
4. Drop/rename fields only after old read/write paths are removed.
5. When `event_version` changes:
   - upcaster coverage is mandatory in CI,
   - replay tests must pass across previous and current versions.
6. Reprojection jobs must execute against mixed-schema windows during rolling deploys.

## Laravel + runtime migration coordination (shared Postgres)

Because Laravel and runtime share one Postgres cluster, migration order must be explicit:

1. deploy additive schema migrations first (compatible with both current Laravel and runtime).
2. deploy runtime and Laravel application versions that tolerate old+new schema during compatibility window.
3. enable new write paths behind flags only after both services are on compatible versions.
4. run backfills/reprojections.
5. remove old fields/indexes only after both services and projections no longer depend on them.

Define one authoritative migration pipeline owner per release to avoid cross-service race conditions.

## Rollout strategy (phased, with flags)

## Phase 0: Scaffold runtime app

Deliverables:

- New Elixir app compiles/tests locally
- Health endpoints
- Runtime contract doc
- No production traffic

Exit criteria:

- `mix test` green
- container builds and deploys to development GKE namespace
- health smoke passes

## Phase 1: Shadow mode (no user impact)

Behavior:

- Laravel continues current runtime path.
- Laravel asynchronously mirrors eligible chat requests to Elixir shadow endpoint.
- Compare semantic behavior and timing offline (not literal text equality).

Deliverables:

- Semantic diff tooling for runtime parity, including:
  - event ordering invariants
  - emitted frame/event type coverage
  - tool intent presence/absence
  - terminal state and error class parity
  - budget counters and receipt completeness
- Latency and error telemetry comparisons

Exit criteria:

- 95%+ semantic parity for sampled runs across defined invariants
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

## Elixir engineering policy for agent-generated code

To leverage Elixir's strengths in local reasoning and iterative feedback loops:

- Keep runtime state immutable at module boundaries (explicit input/output transforms).
- Require `@moduledoc` and `@doc` for public runtime modules and internal API handlers.
- Add doctests for critical contract examples (stream event shapes, checkpoint formats).
- Enforce compiler hygiene in CI (`--warnings-as-errors`).
- Prefer explicit deprecation over breaking internal contracts used by Laravel adapter code.
- Keep public runtime contract versioned (`/internal/v1`) and additive by default.

This reduces context ambiguity for human and agent contributors and keeps migration velocity high without breaking production paths.

## Elixir implementation milestones (specific)

## Milestone A: Runtime API skeleton

- Phoenix endpoint/router/controllers
- health/readiness
- signed request verification

## Milestone B: Run acceptance + stream

- start run + frame append endpoints
- SSE stream output mapper compatible with AI SDK protocol
- stream cursor contract (`seq`, `Last-Event-ID`, `?cursor`) + LISTEN/NOTIFY wakeups

## Milestone C: Agent process supervision

- DynamicSupervisor and per-agent process startup
- frame ingestion loop
- checkpoint persistence
- lease acquisition/heartbeat/release for single-executor guarantee

## Milestone D: Tool async model

- non-blocking tool tasks
- progress events
- cancellation path
- financial idempotency (`tool_call_id`, settlement boundary, reconciliation ids)

## Milestone E: Tiered memory

- L1/L2/L3 compaction jobs
- chunk expansion APIs
- retention policies

## Milestone F: Timeline map/reduce

- spawn subagent retrieval workers
- aggregate semantic outputs
- merge back into main agent context

## Milestone G: DS-Elixir runtime predict core

- signature catalog + typed contracts
- strategy-pinned execution (`direct.v1`, `rlm_lite.v1`)
- policy registry lookup for active artifacts
- receipt/trace persistence and retrieval APIs
- tool replay context injection into frame builder

## Milestone H: DS-Elixir compile/eval/promote loop

- dataset export from runtime receipts/traces
- compile job runner with hash-stable job specs
- compile/eval report persistence
- canary rollout pointer controls
- promote/rollback APIs with audit log

## Observability and SLOs

## Required telemetry dimensions

- `run_id`, `thread_id`, `user_id`, `autopilot_id`
- `trace_id`, `span_id`, `x_request_id`
- runtime driver (`legacy|elixir`)
- model/provider
- `signature_id`, `compiled_id`, `strategy_id`
- `params_hash`, `prompt_hash`, `output_hash`
- budget profile + budget usage counters (`lm_calls`, `tool_calls`, `rlm_iterations`, `sub_lm_calls`)
- tool name/status
- terminal reason class
- compaction level (`raw|l1|l2|l3`)
- phase (`ingest|infer|tool|persist|stream`)
- process identifiers (`agent_pid`, logical process key)
- supervisor restart reason/category
- mailbox pressure level

Telemetry cardinality guardrails:

1. High-cardinality ids are allowed in logs and traces.
2. Metrics labels must remain bounded; do not use unbounded ids (`run_id`, `thread_id`, `user_id`) as metric labels.
3. Dashboard aggregations should be based on bounded dimensions (driver, provider, strategy, status class, queue).

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
6. BEAM process health
   - mailbox length percentile
   - restart counts by child spec
   - reductions/scheduler utilization
   - top memory-consuming process groups

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
- doctests for public runtime modules and contracts
- supervisor/restart behavior tests (crash injection)
- mailbox/backpressure behavior tests under load

Commands:

- `cd apps/runtime && mix deps.get`
- `cd apps/runtime && mix test`
- `cd apps/runtime && mix format --check-formatted`
- `cd apps/runtime && mix credo` (if enabled)
- `cd apps/runtime && mix dialyzer` (phase 2+)
- `cd apps/runtime && mix test --warnings-as-errors`

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

## Replay harness (required)

Provide an offline replay path for debugging and DS dataset generation:

1. reconstruct run execution from durable `(run_id, seq)` event stream.
2. support deterministic replay mode using recorded model/tool outputs ("frozen IO").
3. support live-replay mode where policies/run logic are re-evaluated against historical events.
4. export replay slices into DS compile/eval datasets with provenance linkage.

This is required for incident debugging, regression analysis, and reliable self-improvement workflows.

## Load test plan (production-shape)

Required scenarios before broad cutover:

1. high concurrent SSE streams, including slow clients and reconnect churn
2. burst frame ingestion across many concurrent runs
3. tool fanout bursts plus cancel storms
4. provider timeout/rate-limit incidents validating circuit breakers and receipt-visible fallback behavior
5. pod kill/node drain during active runs validating lease takeover, stream cursor resume, and janitor reconciliation

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

## Risk: Kubernetes pod churn kills in-memory agent processes

Mitigation:

- checkpoint often
- replay from events on restart
- never rely on memory-only state

## Risk: SSE proxy complexity/regressions

Mitigation:

- keep event mapping deterministic
- serve streams from durable log tail (location-independent)
- parity tests against existing `ChatStreamingTest` expectations
- semantic shadow diffing before canary

## Risk: multi-pod routing causes stream gaps or duplicate execution

Mitigation:

- stream-from-log endpoint design (any pod can serve stream)
- lease-row single executor per run
- do not rely on sticky sessions for correctness
- reconnect resumes from stream cursor/watermark

## Risk: Laravel SSE serving becomes control-plane bottleneck

Mitigation:

- capacity-model concurrent SSE connections explicitly in web tier
- if PHP-FPM worker saturation appears, move SSE serving to long-lived worker mode (Octane/RoadRunner/Swoole) or dedicated stream service
- keep frontend wire protocol unchanged regardless of serving topology

## Risk: delegated budgets allow unattended spending if misconfigured

Mitigation:

- conservative default budgets and per-tool caps
- required authorization expirations
- strict allowlists for settlement-boundary providers/domains
- clear Laravel audit UI for active authorizations and spend/reserve state
- immediate revoke path in control plane and fast runtime revocation observation

## Risk: stuck reservations or unknown settlement outcomes lock budget or trigger duplicate spend

Mitigation:

- reservation janitor with deterministic release/reconcile rules
- explicit unknown-outcome state requiring reconcile-before-retry
- idempotency keys + settlement correlation ids in every settlement-boundary receipt
- operator tooling to inspect and resolve aged reservations

## Risk: dual writer inconsistency (Laravel + Elixir)

Mitigation:

- strict ownership per runtime driver mode
- idempotent writes with deterministic ids
- monotonic projection watermarks
- projection reconciler jobs
- full reprojection job path for recovery

## Risk: runtime service becomes public attack surface

Mitigation:

- internal ingress only
- IAM service auth
- HMAC/JWT request signing
- short-lived nonces

## Risk: upstream model/tool provider incident degrades runtime SLO

Mitigation:

- per-provider timeout/rate budgets
- circuit breakers with explicit open/half-open/closed states
- receipt-visible fallback behavior only when policy allows
- load-shedding before provider saturation cascades into run backlog

## Capacity and scaling notes

Initial GKE sizing (starting point, tune with load tests):

- `runtime` StatefulSet
  - Replicas: 3
  - CPU request/limit: `1000m/2000m`
  - Memory request/limit: `2Gi/4Gi`
  - Pod anti-affinity: required across nodes
  - PodDisruptionBudget: `minAvailable: 2`
- HPA target:
  - CPU 60-70% target range
  - custom metrics target for mailbox pressure and run queue depth (phase-2)

Scaling policy:

- scale out first by replicas
- scale up node/pod resources when scheduler pressure or GC latency indicates
- isolate heavy maintenance jobs onto separate queue/pool before increasing core API pod limits

## Concrete execution checklist

1. Scaffold `apps/runtime` with Phoenix/OTP.
2. Add runtime API contract doc and generated OpenAPI for internal endpoints.
3. Add deploy assets under `apps/runtime/deploy`.
4. Deploy runtime service to GCP in internal-only mode.
5. Add Laravel runtime client + config flags.
6. Implement frame-only ingestion contract (`/runs/{runId}/frames`) with idempotent `frameId`.
7. Implement stream-from-log endpoint with resume cursor/watermark support.
8. Implement lease-row single executor guarantee on run execution.
9. Implement shadow traffic mirror + semantic parity diff pipeline.
10. Enable canary by user/autopilot flags.
11. Promote to default-on when SLO and parity pass.
12. Enable autonomous frame + tiered memory features gradually.
13. Implement DS-Elixir signature catalog + predict runtime for initial signatures.
14. Implement DS-Elixir receipts/traces + tool replay context pipeline.
15. Implement DS-Elixir compile/eval/promote/canary controls.
16. Publish canonical cross-language DS JSON schema docs and object-storage seam for large traces/artifacts.
17. Enforce confused-deputy guards (token claim binding + DB ownership checks).
18. Implement SpendAuthorization issuance/validation/lifecycle with delegated AFK-safe modes.
19. Enforce transactional event append boundary (`insert + seq + NOTIFY` in one commit path).
20. Add admission-control limits and telemetry cardinality guardrails in production configs.
21. Run production-shape load tests (SSE/slow clients, burst ingestion, cancel storms, pod-kill recovery).
22. Enforce run state machine caps (steps/wall-clock/tokens/model calls/tool calls) with deterministic terminal reasons.
23. Implement maintenance budget attribution and payer visibility in receipts.
24. Implement per-run tamper-evident event hash chain and integrity verification hooks.
25. Isolate tool runner execution and enforce network egress allowlists.
26. Ship offline replay harness with frozen IO mode for incident/debug and DS datasets.
27. Adopt coordinated Laravel/runtime migration pipeline ownership for shared Postgres releases.

## Delegated spending doc checklist

- [x] Authorization modes section added.
- [x] SpendGrant language replaced with SpendAuthorization budget envelope model.
- [x] Atomic reserve/commit/release settlement flow defined.
- [x] Internal runtime contract updated with `authorizationRef` (policy reference).
- [x] Receipt fields include authorization linkage, policy decision, and budget deltas.
- [x] Deterministic budget exhaustion behavior defined.
- [x] Risks/mitigations updated for delegation misconfiguration.

## Non-goals

- Rewriting `apps/openagents.com` to Phoenix.
- Replacing WorkOS/Sanctum session and guest auth flows.
- Replacing existing public API route structure in this phase.
- Building multi-region active/active BEAM federation on day one.

## Decision log

- 2026-02-18: Chose runtime-only migration (no web rewrite).
- 2026-02-18: Chose monorepo placement `apps/runtime`.
- 2026-02-18: Chose GKE Standard + StatefulSet BEAM clustering as primary production architecture.
- 2026-02-18: Preserved SSE AI SDK compatibility as migration invariant.
- 2026-02-18: Added DS-Elixir as first-class runtime subsystem to preserve proven DSE behavior controls.
- 2026-02-18: Chose stream-from-log as day-one multi-pod streaming invariant.
- 2026-02-18: Chose Postgres lease-row TTL model for single active run executor guarantee.
- 2026-02-18: Chose semantic (invariant-based) shadow diffing over literal text diffing.
- 2026-02-18: Added confused-deputy protections with token claim binding and DB ownership checks.
- 2026-02-18: Added SpendAuthorization budget envelope with delegated and interactive modes.
- 2026-02-18: Added transactional append boundary (`event + seq + notify`) as correctness invariant.
- 2026-02-18: Added run state machine caps with deterministic terminal reason classes.
- 2026-02-18: Added maintenance-budget attribution rules for background compaction/map-reduce work.
- 2026-02-18: Added tamper-evident event hash chain, tool-runner isolation controls, and replay harness requirement.
