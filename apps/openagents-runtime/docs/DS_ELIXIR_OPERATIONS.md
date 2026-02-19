# DS-Elixir Operations

Date: 2026-02-19  
Status: Active runbook (implemented surface)

This runbook covers day-to-day operations for DS-Elixir execution, spend enforcement, pointer rollout, and replay workflows in `apps/openagents-runtime`.

## 1) Operational goals

1. Keep predict/tool behavior deterministic and auditable under restarts.
2. Prevent uncontrolled spending during unattended/autopilot execution.
3. Roll out behavior changes via pointer mutation with fast rollback.
4. Preserve replayability for incidents, canary evaluation, and compile dataset export.

## 2) Pre-flight checks

From `apps/openagents-runtime/`:

```bash
mix format --check-formatted
mix compile --warnings-as-errors
mix runtime.contract.check
mix test test/openagents_runtime/ds
mix test test/openagents_runtime/spend
mix test test/openagents_runtime/tools/tool_runner_execution_test.exs
```

If any of these fail, do not change active DS pointers in production.

## 3) Inspecting DS state

Use `psql` against runtime schema to verify pointer, compile, and spend status.

```sql
-- Active pointer and rollout configuration
SELECT signature_id, canary_percent, rollout_seed, updated_at
FROM runtime.ds_artifact_pointers
ORDER BY signature_id;

-- Recent pointer mutations
SELECT id, signature_id, action, actor, reason, inserted_at
FROM runtime.ds_pointer_audits
ORDER BY id DESC
LIMIT 50;

-- Recent compile reports
SELECT report_id, signature_id, status, job_hash, dataset_hash, inserted_at
FROM runtime.ds_compile_reports
ORDER BY inserted_at DESC
LIMIT 50;

-- Recent spend authorizations and budget posture
SELECT authorization_id, mode, run_id, thread_id, autopilot_id,
       max_total_sats, spent_sats, reserved_sats, expires_at, revoked_at
FROM runtime.spend_authorizations
ORDER BY inserted_at DESC
LIMIT 50;

-- Reconcile-required reservations that block retries
SELECT authorization_id, run_id, tool_call_id, amount_sats, retry_class, state, failure_reason, reserved_at
FROM runtime.spend_reservations
WHERE state = 'reconcile_required'
ORDER BY reserved_at ASC;
```

## 4) Predict execution checks

Predict behavior should always emit receipt-visible policy and budget context.

Runtime modules involved:

- `OpenAgentsRuntime.DS.Predict`
- `OpenAgentsRuntime.DS.PolicyEvaluator`
- `OpenAgentsRuntime.DS.Receipts`
- `OpenAgentsRuntime.DS.Traces`

Expected checks:

1. Every predict call has `receipt_id`, `signature_id`, `compiled_id`, `strategy_id`.
2. Policy envelope includes `authorization_id`, `authorization_mode`, `decision`, `reason_code`, `evaluation_hash`.
3. Budget envelope includes `spent_sats`, `reserved_sats`, and `remaining_sats` where applicable.
4. Trace references are present for `rlm_lite.v1`.

## 5) Spend enforcement and settlement reconciliation

Settlement-boundary tools must run through reserve/commit/release/reconcile.

Execution path:

- `ToolRunner` -> `Reservations.reserve/5` -> provider call -> `Reservations.commit/4` or `Reservations.release/4`
- Unknown settlement outcomes -> `Reservations.mark_reconcile_required/4`
- Reconcile before retry for `dedupe_reconcile_required`

Routine reconciliation command (IEx):

```elixir
iex -S mix
alias OpenAgentsRuntime.Spend.Reservations
{:ok, reconciled} = Reservations.recover_stuck()
IO.inspect(reconciled, label: "recovered_stuck_reservations")
```

Budget exhaustion expected behavior:

1. Runtime emits `policy.decision` with `reason_code = policy_denied.budget_exhausted`.
2. Runtime emits `policy.budget_exhausted` event.
3. Settlement-boundary tool execution does not continue.

## 6) Pointer rollout, canary, promotion, rollback

Canary and promotion controls:

- Selection: `OpenAgentsRuntime.DS.PolicyRegistry.active_artifact/2`
- Mutation/audit: `OpenAgentsRuntime.DS.Compile.PromoteService`

Operational rules:

1. Update canary percent first, observe receipts and error rates.
2. Promote only after canary confidence threshold is met.
3. Roll back by pointer mutation; never mutate historical compiled artifact payloads.
4. Every mutation must produce a `runtime.ds_pointer_audits` record.

Manual IEx flow:

```elixir
iex -S mix
alias OpenAgentsRuntime.DS.Compile.PromoteService

# promote
{:ok, result} =
  PromoteService.promote(
    "@openagents/autopilot/blueprint/SelectTool.v1",
    "compiled:@openagents/autopilot/blueprint/SelectTool.v1:direct.v1",
    actor: "ops",
    reason: "canary pass"
  )

IO.inspect(result, label: "promotion")

# rollback
{:ok, rollback_result} =
  PromoteService.rollback(
    "@openagents/autopilot/blueprint/SelectTool.v1",
    actor: "ops",
    reason: "rollback after incident"
  )

IO.inspect(rollback_result, label: "rollback")
```

## 7) Compile/eval workflow

Compile pipeline:

1. Export dataset from receipts/traces (`DatasetExporter.export/3`).
2. Run compile/eval (`CompileService.compile/2`).
3. Review compile/eval reports and split metrics.
4. Set canary pointer and monitor.
5. Promote or roll back through pointer-only mutations.

Recommended validation before promotion:

```bash
mix test test/openagents_runtime/ds/compile/dataset_exporter_test.exs
mix test test/openagents_runtime/ds/compile/compile_service_test.exs
mix test test/openagents_runtime/ds/compile/promote_service_test.exs
```

## 8) Replay workflow for incidents

Minimum replay packet:

- ordered run events `(run_id, seq)`
- predict receipts for affected run
- trace references (inline payload or external artifact URI)
- settlement reservation records for tool calls

Replay procedure:

1. Pull event stream in seq order from `runtime.run_events`.
2. Verify policy decisions by recalculating `evaluation_hash` via `PolicyEvaluator.replay/3`.
3. Rebuild DS examples via `DatasetExporter` for forensic comparison.
4. If settlement outcomes are unknown, reconcile reservation state before any retry.

Replay acceptance criteria:

1. Reason codes and evaluation hashes match original receipts.
2. Tool settlement outcomes are single-accounted (no duplicate commit).
3. Pointer state at incident timestamp is recoverable from pointer audit trail.

## 9) Alerts and incident triggers

Primary alert indicators:

- Spike in `policy_denied.budget_exhausted` without expected budget changes.
- Increase in `reconcile_required` reservations.
- Provider circuit-breaker open rates and fallback exhaustion.
- Pointer churn outside release windows.

Related runbooks:

- `apps/openagents-runtime/docs/OPERATIONS.md`
- `apps/openagents-runtime/docs/OPERATIONS_ALERTING.md`
- `apps/openagents-runtime/docs/OBSERVABILITY.md`
- `apps/openagents-runtime/docs/REPROJECTION.md`

## 10) Security and data-handling requirements

1. Do not persist raw secrets or auth headers in traces/receipts.
2. Use sanitization middleware outputs only.
3. Restrict DS operator actions to audited internal paths.
4. Treat external trace artifacts as immutable and access-controlled.

Security policy reference:

- `apps/openagents-runtime/docs/SANITIZATION_POLICY.md`

