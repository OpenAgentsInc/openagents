# DS-Elixir Runtime Integration Addendum

Date: 2026-02-18  
Status: Active companion plan  
Depends on: `docs/plans/active/elixir-agent-runtime-gcp-implementation-plan.md`

## Purpose

This addendum specifies how OpenAgents ports proven DSE behavior controls from the removed `apps/web` stack into the new Elixir runtime (`apps/runtime`) as DS-Elixir.

The main runtime plan defines infrastructure and migration flow. This document defines the DS-Elixir contract surface, migration map, and rollout sequencing.

## Why this addendum exists

OpenAgents previously validated a strong DSE architecture pattern before `apps/web` was removed:

- stable signature contracts,
- artifact-pinned strategy execution,
- budgeted inference,
- receipt + trace recording,
- tool replay into context,
- compile/eval/canary/promotion loops.

Those capabilities are essential to autonomous runtime quality and should not be lost during the runtime migration.

Key historical references:

- Removal commit: `8c460f956`
- Prior integrated state: `42700ddef`
- Tool replay hardening: `c79250a58`
- Prior docs:
  - `docs/dse/COMPILER-CONTRACT.md`
  - `docs/dse/TOOLS.md`
  - `docs/autopilot/dse/dse.md`
  - `docs/autopilot/runbooks/DSE_PLAYBOOK.md`
  - `docs/autopilot/runbooks/SELF_IMPROVE_RUNBOOK.md`

## Carry-forward invariants (non-negotiable)

1. Signature IDs are stable and versioned.
2. Compiled artifacts are immutable.
3. Active behavior changes only through pointer updates (promote/rollback).
4. Strategy ID and budget usage must be receipt-visible.
5. Tool calls/results are replayable with bounded/redacted context reinjection.
6. Compile/eval/canary controls are explicit operator workflows.

## Runtime architecture placement

DS-Elixir modules in target runtime app:

- `lib/openagents_runtime/ds/signatures/catalog.ex`
- `lib/openagents_runtime/ds/predict.ex`
- `lib/openagents_runtime/ds/strategies/direct_v1.ex`
- `lib/openagents_runtime/ds/strategies/rlm_lite_v1.ex`
- `lib/openagents_runtime/ds/policy_registry.ex`
- `lib/openagents_runtime/ds/receipts.ex`
- `lib/openagents_runtime/ds/traces.ex`
- `lib/openagents_runtime/ds/tool_replay.ex`
- `lib/openagents_runtime/ds/compile/compile_service.ex`
- `lib/openagents_runtime/ds/compile/dataset_exporter.ex`
- `lib/openagents_runtime/ds/compile/promote_service.ex`

Runtime correctness bindings (from main runtime plan):

1. Single executor per run is lease-controlled (`runtime.run_leases` with TTL heartbeat).
2. Streams are served from event log with monotonic `(run_id, seq)` cursor semantics.
3. Stream wakeups use `LISTEN/NOTIFY`, but durable event log remains source of truth.

## Legacy mapping: `apps/web` DSE -> DS-Elixir

| Legacy component | Prior role | DS-Elixir target |
|---|---|---|
| `apps/web/src/effuse-host/autopilot.ts` | runtime signature predict orchestration + strategy/budget usage | `ds/predict.ex` integrated with `frame_router.ex` |
| `apps/web/src/effuse-host/toolReplay.ts` | bounded/redacted tool replay context | `ds/tool_replay.ex` |
| `apps/web/src/effuse-host/dse.ts` | policy registry + receipts + blob/var layers | `ds/policy_registry.ex`, `ds/receipts.ex`, `ds/traces.ex` |
| `apps/web/src/effuse-host/dseJobs.ts` | compile job specs/search spaces/rewards | `ds/compile/compile_service.ex` + job schema modules |
| `apps/web/src/effuse-host/dseCompile.ts` | compile execution + artifact/report persistence | `ds/compile/compile_service.ex` + `promote_service.ex` |

## Data model mapping

Khala-era DSE collections map to Postgres runtime schema:

- `dse.active` -> `runtime.ds_active_policies`
- `dse.artifacts` -> `runtime.ds_compiled_artifacts`
- `dse.receipts` -> `runtime.ds_predict_receipts`
- `dse.blobs` -> `runtime.ds_traces` and runtime blob storage tables
- `dse.varSpace` -> runtime varspace tables
- `dse.examples` -> `runtime.ds_examples`
- `dse.compileReports` -> `runtime.ds_compile_reports`
- `dse.evalReports` -> `runtime.ds_eval_reports`

## Initial signature portfolio (phase 1)

Port signatures that previously had direct product impact:

1. `@openagents/autopilot/blueprint/SelectTool.v1`
2. `@openagents/autopilot/canary/RecapThread.v1`
3. `@openagents/autopilot/rlm/SummarizeThread.v1`
4. Upgrade/capability detection signature from prior autopilot catalog (exact ID from legacy catalog export)

Execution strategy support required immediately:

- `direct.v1`
- `rlm_lite.v1` (for long-context recap/summarization)

## DS-Elixir runtime contracts

### Signature contract

- `signature_id`
- input schema hash
- output schema hash
- prompt/program hash
- default params (strategy, decode policy, budgets)
- constraints (timeout/retry/tool allowlist)

### Compiled artifact contract

- `compiled_id` (immutable)
- `signature_id`
- compatibility hashes
- selected params/strategy
- provenance (`job_hash`, `dataset_hash`, compiler version)
- created timestamp + report refs

### Predict receipt contract

- `signature_id`, `compiled_id`, `strategy_id`
- `params_hash`, `prompt_hash`, `output_hash`
- latency and budget counters
- `authorization_id`, `authorization_mode`
- `policy_decision`
- `budget_before`, `reserved`, `spent`, `budget_after`
- terminal status/error class
- optional `trace_ref` for RLM-like runs
- trace/request linkage (`trace_id`, `x_request_id`) for end-to-end correlation

### Cross-language contract packaging

Contracts above must be published as canonical versioned JSON schemas consumed by both:

- Elixir runtime (writer/enforcer),
- Laravel/control-plane surfaces (reader/validator).

Schema policy:

1. Additive evolution by default.
2. Explicit major version bump on breaking contract changes.
3. Contract docs remain language-agnostic even when implementation is Elixir-first.
4. Event payload contracts include `event_version` and upcaster coverage requirements for replay compatibility.

## Tool replay contract

Tool replay must preserve safety and usefulness:

- include only recent bounded events,
- redact sensitive keys and secrets,
- truncate large payloads,
- emit deterministic summary lines by tool/state,
- inject replay summary as system context on next frame.

This is required to prevent tool-result amnesia in multi-step autonomous loops.

## Payment and settlement idempotency contract

For L402/Lightning-adjacent tools, DS-Elixir runtime must enforce:

1. Deterministic `tool_call_id` per execution intent.
2. Settlement boundary classification (`safe_retry` vs `dedupe_reconcile_required`).
3. Provider-level idempotency keys for settlement-affecting operations.
4. Receipt linkage fields required for post-crash reconciliation without duplicate spend attempts.

Authorization modes:

- `interactive`
- `delegated_budget`
- `deny`
- optional `delegated_budget_with_threshold`

SpendAuthorization requirement:

- settlement-adjacent execution must use control-plane-issued budget envelope authorization.
- authorization scope: `{autopilot_id?, thread_id?, run_id?}`
- authorization mode determines whether in-the-moment approval is required.
- runtime must enforce without user interaction when mode is delegated.
- authorization validation and consumption must be receipt-visible and auditable.

Revocation and reservation reconciliation requirements:

1. authorization revoke/expire must block new settlement reserves immediately.
2. stuck reservations must be reconciled by janitor flow with deterministic release/commit outcome.
3. unknown settlement outcomes require reconcile-before-retry when `dedupe_reconcile_required`.

Required receipt linkage fields:

- `authorization_id`
- `authorization_mode`
- `policy_decision`
- `budget_before`, `reserved`, `spent`, `budget_after`

Background budget attribution rule:

- DS-driven maintenance work (compaction/map-reduce/eval prep) defaults to system maintenance budget unless explicitly marked billable to user/autopilot scope.

## Compile/eval/promotion loop in runtime

1. Export examples from real receipts/traces.
2. Split datasets (`train`, `holdout`, optional `test`).
3. Compile candidate artifacts over defined search spaces.
4. Record compile/eval reports with hash-stable provenance.
5. Set canary policy pointer for percentage rollout.
6. Promote or rollback by pointer update only.

Operational posture:

- compile is explicit operator workflow,
- runtime predict uses active pointer only,
- rollback is one pointer mutation.

## Large trace/artifact storage seam

Keep relational metadata in Postgres, but store large trace/artifact payloads in object storage (GCS) behind immutable object keys and hash-addressed references.

This prevents unbounded Postgres growth and keeps query paths for runtime execution fast.

## Rollout sequencing

1. Wire DS-Elixir predict into run loop behind feature flags.
2. Enable `direct.v1` signatures first for parity.
3. Enable `rlm_lite.v1` only for recap/summarization cohort.
4. Turn on receipt + trace capture and operator retrieval endpoints.
5. Enable compile/promotion controls after parity and stability gates.

## Acceptance criteria

1. Signature parity for initial portfolio in canary traffic.
2. Receipt completeness for every DS-Elixir predict run.
3. Tool replay context visible and bounded in traces.
4. Canary promotion/rollback exercised in staging and production.
5. No regression to Laravel SSE contract during DS-Elixir rollout.
6. Canonical JSON schemas published and consumed by both Elixir and Laravel surfaces.
7. Replay harness supports deterministic frozen-IO replays for incident debugging and dataset provenance.

## Open decisions to resolve early

1. Whether DS artifacts live in runtime Postgres only, or mirrored for external analytics.
2. Whether operator compile APIs remain internal-only or require a separate control plane.
3. Long-term placement of heavy compile jobs (runtime pods vs dedicated worker pool).
