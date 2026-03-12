# Autopilot Earnings Operator Runbook

## Audience

Internal operators responsible for keeping autonomous earnings runs healthy, payout-correct, and rollback-safe.

## Operational Objectives

- Preserve authoritative payout correctness.
- Keep scheduler and recovery paths deterministic.
- Keep swap exposure and Blink execution behavior within policy.
- Maintain clear incident response with fast containment.
- Keep revenue-lane posture explicit (compute lane active; liquidity solver lane disabled unless explicitly rolled out).

## Lane Scope and Default Posture

Autopilot Earn is a multi-lane provider economy, but current operations are compute-lane first.

- Active lane: NIP-90 compute provider earnings.
- Future lane: Hydra liquidity solver earnings (capital + execution) in an OpenAgents-native solver market.
- Default safety rule: liquidity solver mode remains off unless a dedicated rollout explicitly enables it.

## Daily Checks

1. Scheduler health
- Sample active goals with `openagents_goal_scheduler` `status`.
- Verify goals have sane `next_run_epoch_seconds`, no stuck `Running` state, and expected missed-run policy.

2. Lane posture
- Confirm current cohorts are operating compute-lane goals only.
- Treat any unexpected liquidity-solver activation as a containment incident until explicitly authorized.

3. Payout correctness
- Confirm latest run audits include payout evidence and wallet reconciliation with no synthetic-pointer mismatches.
- Spot-check wallet receive totals against reported earned delta.

4. Swap risk posture
- Check recent swap quote audits and execution receipts.
- Track provider/source consistency (`blink_infrastructure` expected for production).
- Flag repeated failures, high fee/low output anomalies, or stale quote execution attempts.

5. Recovery posture
- Ensure startup recovery reports are healthy (`recover_startup` as needed).
- Confirm catch-up backlog is draining for `catch_up` goals.

## Seed-Demand Operations

Seed-demand jobs are allowed for first-run earning reliability, but must stay auditable.

Rules:

- Seed jobs must produce real wallet payment confirmations.
- Seed/quest labeling must remain explicit in operational reporting.
- Never count synthetic-only history updates as successful payout evidence.

Operator checks:

- For sampled seed-demand goals, confirm:
  - run audit payout evidence exists,
  - payment pointers map to wallet receive payment IDs,
  - authoritative goal completion is consistent with wallet-confirmed delta.

## Payout Verification Procedure

When verifying a run:

1. Query scheduler status with `goal_id`.
2. Inspect `latest_run_audit`:
- `lifecycle_status`
- `condition_*` fields
- `attempts[*].tool_invocations`
- `payout_evidence`
- `swap_quote_evidence` and `swap_execution_evidence`
3. Inspect reconciliation summary:
- `earned_wallet_delta_sats`
- `wallet_delta_excluding_swaps_sats`
- swap conversion/fee totals
- unattributed receive sats
4. Confirm no mismatches indicating synthetic payout pointers or missing wallet receipt linkage.

## Swap-Risk Monitoring

Monitor these indicators:

- Blink execution failure ratio increase over baseline.
- Execution failure ratio (`FAILURE` or persistent `PENDING`).
- Quotes expiring before execution.
- Average fee sats/slippage moving outside expected envelope.

Risk controls:

- Tighten swap policy limits for affected goals (`max_fee_sats`, `max_slippage_bps`, per-swap caps).
- Temporarily disable swap-heavy goals via kill switch when risk spikes.
- Re-enable only after provider path and receipt integrity stabilize.

## Incident Response

### Incident A: False-success payout signal

Symptoms:

- Goal marked successful without matching wallet evidence.

Immediate actions:

1. Enable goal kill switch (`set_kill_switch`) for affected goal(s).
2. Snapshot `latest_run_audit` and reconciliation payload.
3. Identify mismatch source:
- synthetic payout pointers,
- missing wallet payment IDs,
- wallet source errors.
4. File corrective issue and block further rollout changes until fixed.

### Incident B: Swap instability

Symptoms:

- Elevated swap failure, stale quote churn, abnormal fee/slippage.

Immediate actions:

1. Kill switch goals with active swap churn.
2. Disable OS adapter if repeated autonomous scheduling amplifies failures.
3. Restrict policy limits and resume in controlled mode.
4. Track stabilization over multiple runs before restoring full schedule.

### Incident C: Scheduler/recovery regression

Symptoms:

- Goals not triggering, replay backlog growing, or recovery repeatedly failing.

Immediate actions:

1. Run `recover_startup` and `reconcile_os_adapters`.
2. Check `pending_catchup_runs`, `last_recovery_epoch_seconds`, adapter reconcile state.
3. Temporarily force critical goals to manual `run_now` until scheduler lane is stable.

## Containment and Rollback Criteria

Containment triggers:

- any verified false-success payout case,
- sustained swap failure/fallback spike,
- inability to recover scheduler queue on restart.

Rollback actions:

1. Engage kill switch for affected goals.
2. Disable automated schedules for affected cohort.
3. Revert to manual-run operations while root cause is addressed.

## Release Gate for Ops Readiness

Run and require green before rollout expansion:

- `scripts/lint/autopilot-earnings-epic-test-gate.sh`

Keep a retained copy of:

- scheduler status snapshots,
- run audit receipts,
- incident timeline notes,
- rollback/restore actions.

## Runtime Log Capture

Autopilot writes durable per-launch session logs by default to:

- `~/.openagents/logs/autopilot/sessions/<session-id>.jsonl`
- `~/.openagents/logs/autopilot/latest.jsonl`

Optional development override:

- `OPENAGENTS_AUTOPILOT_LOG_DIR`

Use these files for postmortem inspection when Mission Control output or terminal
history is no longer available. The JSONL stream includes both selected tracing
events from the buyer/provider/payment flow and synthesized Mission Control
operator lines.

Recommended operator checks:

1. Confirm the current session log exists:
   `ls ~/.openagents/logs/autopilot/sessions`
2. Inspect the latest session:
   `tail -n 100 ~/.openagents/logs/autopilot/latest.jsonl`
3. When debugging payout flow, search for:
   `payment-required`, `buyer payment settled`, `provider settlement confirmed`, `job failed`

## Reciprocal Loop Verification

For two-key/two-wallet bilateral 10-sat loop setup, pane operation flow, and pass/fail troubleshooting:

- `AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`
