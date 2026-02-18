Below is a concrete GitHub issue set (titles + 1-paragraph summaries) that should cover implementing the plan end-to-end. I’ve grouped them by milestone/epic, but each bullet is intended to be a **separate GitHub issue**.

## Status Update (2026-02-18)

Completed and closed:

- Epic 0 (items 1-4): `#1655` to `#1658`
- Epic 1 / A (items 5-8): `#1659` to `#1662`
- Epic 2 / B (items 9-14): `#1663` to `#1668`
- Epic 3 / C (items 15-17): `#1669` to `#1671`

Roadmap follow-ups identified after implementation:

- Add explicit internal auth enforcement plug/middleware so `X-OA-RUNTIME-SIGNATURE` verification is mandatory for all `/internal/v1/*` endpoints (verifier module exists, endpoint enforcement should be explicit).
- Align contract artifacts with implemented endpoint requirements (`thread_id` ownership parameter on stream/snapshot/frame append, optional `tail_ms` stream tail window).
- Add a small contract-convergence pass to keep runtime controller behavior, `RUNTIME_CONTRACT.md`, and `openapi-internal-v1.yaml` lockstep in CI.

Suggested labels (optional): `runtime`, `laravel`, `infra`, `db`, `security`, `streaming`, `tools`, `ds-elixir`, `observability`, `tests`, `docs`.

---

## Epic 0: Project scaffolding and repo wiring

1. **[runtime] Scaffold `apps/openagents-runtime` Phoenix/OTP application in monorepo**
   Create the new Elixir app at `apps/openagents-runtime/` with standard `mix` structure, Phoenix endpoint/router for internal-only APIs, and OTP Application/Supervisor wiring. Ensure the app boots locally with a minimal health route and clean module layout matching the plan’s proposed directories.

2. **[runtime] Add Elixir CI checks (format, tests, warnings-as-errors)**
   Add CI scripts/config so every PR runs `mix format --check-formatted`, `mix test`, and fails on warnings (as feasible). This issue also adds baseline static analysis hooks (Creed/Dialyzer optional) and establishes the “contract docs + tests must stay green” discipline early.

3. **[runtime] Add Dockerfile and Cloud Build pipeline for runtime service**
   Implement `apps/openagents-runtime/Dockerfile` plus `deploy/cloudbuild.yaml` to build/push runtime images tagged by git SHA. Include build args and caching where appropriate, and ensure the container starts with correct `MIX_ENV=prod` config and release mode.

4. **[infra] Create Kubernetes manifests for runtime (StatefulSet, services, PDB, HPA skeleton)**
   Add base manifests for `openagents-runtime` StatefulSet, headless service (for BEAM node naming), ClusterIP service (internal HTTP), PodDisruptionBudget, and initial HPA scaffolding. Keep manifests environment-parameterized (dev/staging/prod) and aligned with your “internal-only” runtime posture.

---

## Epic A: Internal API contract + authentication

5. **[docs/runtime] Write `RUNTIME_CONTRACT.md` and generate internal OpenAPI spec (`/internal/v1/*`)**
   Define request/response shapes for start run, append frame, stream, cancel, and snapshot endpoints. Include the cursor semantics (`seq`, `Last-Event-ID`, `?cursor`), error classes, and idempotency rules (`frameId`). Add an OpenAPI artifact (even if partial) so Laravel/runtime integration stays exact.

6. **[runtime/security] Implement signed internal token verification (`auth_token_verifier.ex`)**
   Implement verification for the internal `X-OA-RUNTIME-SIGNATURE` token (HMAC/JWT—whatever you chose), including `iat/exp/nonce` and claim binding to `run_id/thread_id/user_id` (or guest scope). Reject replayed/expired/mismatched tokens and standardize auth failures as structured errors.

7. **[runtime/security] Enforce confused-deputy protections via DB ownership checks**
   Runtime must not trust request payload identity claims. Add authoritative DB checks that `run_id` + `thread_id` belong to the asserted principal (user or guest scope) and fail closed. This should be enforced on every endpoint that mutates or streams run state.

8. **[runtime/observability] Propagate W3C tracing (`traceparent`) across Laravel → runtime → tools/models**
   Add middleware and internal plumbing so `traceparent/tracestate` and `x-request-id` flow into logs, traces, receipts, and tool calls. Define a consistent trace/span naming scheme for phases (`ingest`, `infer`, `tool`, `persist`, `stream`) so debugging is straightforward.

---

## Epic B: Database schema and event log foundation

9. **[db/runtime] Create baseline runtime schema + migrations (schema `runtime`, sequences, extensions)**
   Create Postgres schema `runtime` and baseline migrations in `priv/repo/migrations`. Include required extensions (if any), and establish naming conventions for tables and indexes to support high-volume append and tail queries.

10. **[db/runtime] Implement run event log tables with monotonic `(run_id, seq)` and transactionally allocated `seq`**
    Create the canonical runtime event store (e.g., `runtime.run_events`) with strictly monotonic `seq` per run, plus required indexes and uniqueness constraints. Ensure `seq` allocation is transactional and never in-memory.

11. **[db/runtime] Implement frames table with idempotent `frameId` semantics**
    Create `runtime.frames` to store ingested frames with `frame_id` as an idempotency key. Runtime should treat duplicates as no-op success while preserving the original canonical frame record.

12. **[db/runtime] Implement lease table `runtime.run_leases` with heartbeat + steal rules**
    Create `runtime.run_leases` with columns to support TTL heartbeat, owner identity, last progress watermark, and safe stealing only when expired + no progress movement. This is the enforcement point for “at most one active executor per run.”

13. **[db/runtime] Implement transactional append boundary and wakeups (`LISTEN/NOTIFY`)**
    Add the mechanism so event insert + `NOTIFY` happen in the same DB transaction. Define notify payload conventions (e.g., `run_id` or `run_id:seq`) and ensure stream readers always re-read committed DB state (NOTIFY is only a wakeup).

14. **[db/runtime] Add optional per-run hash chain fields for tamper-evident logs**
    Extend run events with `prev_hash`/`event_hash` (or equivalent) so each run’s event stream forms a hash chain. Document verification behavior and provide a utility to validate integrity for a given run.

---

## Epic C: Streaming (stream-from-log) and event mapping

15. **[runtime/streaming] Implement `GET /internal/v1/runs/{runId}/stream` with cursor resume semantics**
    Build the stream endpoint to tail durable events location-independently, supporting `Last-Event-ID` and `?cursor`. Ensure SSE `id:` always equals `seq`, guarantee gap-free replay while retained, and return explicit stale-cursor errors when retention floors are exceeded.

16. **[runtime/streaming] Implement efficient stream tailer with NOTIFY wakeups + backoff**
    Avoid poll storms by using `LISTEN/NOTIFY` to wake tail loops and DB tail as source of truth. Add backoff/jitter for reconnect churn and slow consumers, and ensure the implementation doesn’t hold DB connections unnecessarily.

17. **[runtime/integration] Implement Laravel AI SDK SSE mapping (`laravel_event_mapper.ex`)**
    Create a deterministic mapper from runtime-native events to the existing AI SDK/Vercel SSE frame protocol (`start`, `text-delta`, tool frames, `finish`, `[DONE]`). Include golden tests to ensure Laravel’s frontend expectations remain unchanged.

---

## Epic D: Run execution + supervision

18. **[runtime] Implement run executor loop (lease acquire/renew, frame consume, event emit, terminal state)**
    Implement the core executor that acquires the run lease, processes frames in order, emits runtime events and mapped stream frames, and transitions to deterministic terminal states. Ensure lease renewals are frequent and stop on terminal/cancel paths.

19. **[runtime] Implement `AgentProcess` GenServer + DynamicSupervisor orchestration**
    Add `AgentProcess` as the per-run/per-session execution process and wire it to a DynamicSupervisor. Keep callback handlers short; offload long work; instrument mailbox/reductions; and ensure processes can recover from checkpoints + event replay.

20. **[runtime] Implement cancel semantics end-to-end (`run.cancel_requested`, best-effort tool cancel, terminal outcome)**
    Implement `POST /cancel` to append `cancel_requested`, stop starting new work, and best-effort cancel inflight tasks. Ensure late tool/model returns are either discarded or recorded as non-authoritative and cannot reopen a run.

21. **[runtime] Implement janitor reconciliation for stale runs and lost executors**
    Create a maintenance worker that detects stale leases / stalled progress and appends `run.executor_lost`. It should attempt safe resumption via normal lease acquisition and terminate deterministically if recovery budgets are exceeded.

---

## Epic E: Tools, tasks, and replay

22. **[runtime/tools] Implement tool task state machine + persistence (`runtime.tool_tasks`)**
    Build the durable tool task model (`queued/running/streaming/succeeded/failed/canceled/timed_out`) with deterministic `tool_call_id`, timestamps, outputs, and error classification. This table should support reconciliation and audit, not just runtime convenience.

23. **[runtime/tools] Implement Task.Supervisor execution with streaming progress + cancellation**
    Wire tool execution under `Task.Supervisor` with timeouts, structured progress events, and cancel support. Ensure tool tasks do not block GenServer callbacks and that long-running tools are isolated with proper failure containment.

24. **[runtime/tools] Implement tool replay context builder with redaction and bounded summaries**
    Implement `ds/tool_replay.ex` (or shared module) that creates bounded, deterministic summaries of recent tool activity for reinjection. Enforce redaction/sanitization at the boundary so secrets/PII cannot leak into logs, traces, or replay context.

---

## Epic F: Delegated spending, budgets, and settlement safety

25. **[runtime/spend] Implement SpendAuthorization model + resolution by scope**
    Create `runtime.spend_authorizations` with scope (`autopilot/thread/run`), mode (`interactive/delegated_budget/deny/...`), budgets, constraints, and expiry. Implement resolution logic so runtime can pick the authoritative active authorization for a run without trusting request payloads.

26. **[runtime/spend] Implement reservations ledger (`runtime.spend_reservations`) and atomic reserve/commit/release**
    Add a reservations table and transactional logic so settlement-boundary actions reserve budget atomically before execution, commit on success, and release/reconcile on failure/unknown outcomes. This must be safe under crashes and retries.

27. **[runtime/spend] Enforce settlement idempotency keys + correlation IDs on all settlement-boundary tools**
    For L402/Lightning-adjacent tools, require provider-facing idempotency keys and store settlement correlation identifiers in receipts. Define `safe_retry` vs `dedupe_reconcile_required` behavior and make retries conditional on reconciliation state.

28. **[runtime/policy] Implement budget exhaustion behavior and deterministic policy decisions**
    When reserves fail due to budget/constraints, append `policy.budget_exhausted` (or similar) and emit a receipt-visible `policy_decision`. Ensure the executor does not loop silently; it must either continue with non-settlement work or terminate with an explicit policy error.

---

## Epic G: Provider resilience

29. **[runtime] Add provider circuit breakers + receipt-visible fallback policy**
    Implement per-provider timeout/rate budgets, circuit breaker state (open/half-open/closed), and controlled fallback behavior only when policy allows. Any fallback must be explicit in receipts/traces so behavior is auditable.

---

## Epic H: Projections into Laravel read models

30. **[runtime/db] Implement projection writer into Laravel tables with monotonic watermarks**
    Create the projection pipeline that takes runtime events and writes/updates Laravel-facing `runs/messages/run_events` while preserving idempotency and monotonic watermark progression. The system must be rebuildable and safe under duplicates/out-of-order delivery.

31. **[runtime/db] Implement full reprojection job + projection reconciler**
    Provide a job that can rebuild Laravel-facing read models from runtime event logs, plus a reconciler that detects drift and repairs it. This is your safety net for dual-writer cutover and future schema evolution.

---

## Epic I: Tiered memory system

32. **[runtime/memory] Implement timeline storage + retention classes for raw and compacted data**
    Create tables and APIs for hot raw windows plus L1/L2/L3 chunk storage, with retention policies per event class. Define which events are long-lived vs which can be aged out after compaction and checkpoint safety.

33. **[runtime/memory] Implement L1 compaction job with auditable compaction artifacts**
    Implement `compact_l1` as a scheduled/pressure-triggered job that produces L1 summaries with provenance: input chunk IDs, output chunk ID, model metadata, token stats, and output hash. Store large artifacts in GCS when needed and keep DB metadata bounded.

34. **[runtime/memory] Implement L2/L3 rollups + on-demand expansion APIs**
    Implement hourly/daily rollups and the ability to expand back down when needed for reasoning. Ensure the expansion path is deterministic, bounded, and does not accidentally trigger unbounded spend without the appropriate authorization mode.

---

## Epic J: DS-Elixir core (signatures, predict, receipts)

35. **[ds-elixir] Implement signature catalog with stable IDs, schema hashes, and prompt/program hashes**
    Create `ds/signatures/catalog.ex` to define versioned signature contracts and hashing rules. This is the anchor for artifact compatibility, strategy selection, and reproducible receipts.

36. **[ds-elixir] Implement predict pipeline with `direct.v1` strategy**
    Implement the simplest predict path that produces receipts with `signature_id`, params/prompt/output hashes, budget counters, timing, and policy linkage. This should be the first DS capability you can canary safely.

37. **[ds-elixir] Implement `rlm_lite.v1` strategy with trace capture and bounded tool replay injection**
    Implement the RLM-lite loop with explicit iteration budgets, trace references, and safe replay summaries. Ensure traces are pointer-based with large payloads offloaded to GCS.

38. **[ds-elixir] Implement DS policy registry + active artifact pointers + canary selection**
    Create `ds/policy_registry.ex` and storage for active pointers to compiled artifacts. Implement canary selection logic that is receipt-visible and rollbackable by pointer mutation only.

---

## Epic K: DS-Elixir compile/eval/promote loop

39. **[ds-elixir] Implement dataset exporter from receipts/traces**
    Build `dataset_exporter.ex` to produce hash-stable datasets from real receipts/traces (train/holdout/test splits). This is required before compile/eval becomes operationally meaningful.

40. **[ds-elixir] Implement compile runner + compile/eval report persistence**
    Implement the compile job schema, search space evaluation, and report tables (`runtime.ds_compile_reports`, `runtime.ds_eval_reports`). Ensure job specs are hash-stable and artifacts are immutable.

41. **[ds-elixir] Implement promote/rollback APIs with audit logging**
    Implement pointer updates for canary/promote/rollback with audit log records. Ensure changes are additive, reversible, and visible in runtime receipts for every affected prediction.

---

## Epic L: Laravel integration and cutover

42. **[laravel] Implement `RuntimeClient` interface + `ElixirRuntimeClient` + config flags**
    Add the Laravel runtime abstraction, configuration file, and concrete HTTP/SSE client that speaks `/internal/v1/*`. Include timeouts, auth signing, retries (bounded), and structured error handling.

43. **[laravel] Update Chat endpoints/orchestrator to use RuntimeClient and proxy SSE bytes**
    Refactor `ChatApiController` and `RunOrchestrator` into a facade that delegates execution to the runtime client while preserving auth semantics. Ensure SSE proxying is minimal transformation, supports reconnect, and preserves frontend wire protocol.

44. **[laravel] Implement shadow-mode mirroring to runtime + semantic diff pipeline**
    Add the ability to mirror eligible requests to runtime without user impact and compare semantic invariants (ordering, tool intent, terminal states, budget counters, receipt completeness). Store diffs for offline review and add pass/fail gates for canary readiness.

45. **[laravel] Implement canary routing controls (per user/autopilot) and instant rollback switch**
    Add DB-backed overrides and env flags to route specific cohorts to `elixir` driver and fall back to `legacy` instantly. Ensure the rollback path is safe and does not corrupt projections.

46. **[laravel/tests] Add integration tests for runtime mode (SSE continuity, cancel, fallback)**
    Add feature tests that verify streaming frame continuity, cancel behavior, and fallback on runtime errors. Include tests for ownership/auth invariants remaining unchanged across driver modes.

---

## Epic M: Observability, ops, and performance validation

47. **[runtime/observability] Implement telemetry (metrics/traces/logs) with cardinality guardrails**
    Implement telemetry emissions for run lifecycle, stream integrity, tool lifecycle, BEAM process health, and spend enforcement decisions. Enforce the rule that high-cardinality identifiers belong in logs/traces, not metrics labels.

48. **[ops] Create dashboards + alert rules for runtime health, streams, tools, leases, and spend**
    Define the minimum dashboards and alerting thresholds (p95 latency, 5xx, stream `[DONE]` rate, lease steal counts, tool failure spikes, circuit breaker open rate). Ensure runbooks reference these panels.

49. **[tests/load] Implement production-shape load tests (SSE slow clients, burst frames, cancel storms, pod kills)**
    Build a load test suite that targets the real shape: many concurrent SSE connections (including slow consumers), burst ingestion, tool fanout, cancel storms, and chaos (pod kill/node drain). Validate lease takeover, cursor resume, janitor reconciliation, and circuit breaker behavior under incident conditions.

50. **[deploy] Implement migration Job + smoke tests (health, stream, tool path) in `deploy/jobs`**
    Add a Kubernetes Job for migrations and smoke scripts that validate health endpoints, stream functionality, and at least one tool execution path. Make this runnable in CI/CD as a post-deploy gate.

---

## Epic N: Security hardening

51. **[runtime/security] Implement strict sanitization middleware for tools/models/events/traces**
    Implement a single, enforceable sanitization layer that strips secrets (headers, cookies, keys) and applies PII policy to logs and stored artifacts. Ensure tool replay uses only sanitized payloads and enforce this with tests.

52. **[infra/security] Add NetworkPolicies for Erlang distribution ports and internal HTTP only**
    Implement Kubernetes NetworkPolicies that allow BEAM distribution only within the runtime workload selectors and within the constrained port range. Ensure runtime HTTP endpoints are reachable only from the Laravel service(s) / trusted namespaces.

---

## Epic O: Documentation

53. **[docs] Write `DEPLOY_GCP.md` and `OPERATIONS.md` for runtime**
    Document deploy flow, migration jobs, secrets, rollback, incident response (leases, streams, stuck reservations), and day-to-day operations. Include exact commands and expected outputs so operators can run it under stress.

54. **[docs] Write DS-Elixir contract + operations docs (`DS_ELIXIR_RUNTIME_CONTRACT.md`, `DS_ELIXIR_OPERATIONS.md`)**
    Document canonical JSON schemas, hashing rules, artifact immutability, pointer promotion/rollback, receipts/traces layout, and compile/eval workflow. Include how to inspect a run and how to reproduce via replay.

55. **[docs] Update repo overview docs (`docs/PROJECT_OVERVIEW.md`, `docs/README.md`) to include runtime**
    Add the new app to repo documentation, link the runtime docs, and describe the control-plane/runtime split so new contributors don’t accidentally put runtime concerns back into Laravel.

---

If you want this turned into something you can paste directly into GitHub (e.g., with issue body templates, labels, and checklists), tell me whether you prefer **one epic issue per section with subtask checklists** or **all issues as first-class tickets** (the list above assumes first-class tickets).
