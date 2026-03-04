# Autopilot Earnings Rollout Plan

Date: 2026-03-02

## Purpose

Define production rollout controls for autonomous earnings goals:

- feature flag gating,
- staged cohort enablement,
- rollout metrics tracking,
- explicit rollback conditions,
- post-launch hardening checklist.

Autopilot Earn is modeled as multi-lane (compute provider now, liquidity solver later), but this rollout plan currently governs the compute lane.

## Rollout Control Surface

Rollout is controlled through `openagents_goal_scheduler` action `set_rollout`.

Default lane posture:

- compute lane may be staged through normal rollout controls,
- liquidity solver lane remains disabled unless a dedicated future rollout introduces explicit solver controls.

Key fields:

- `rollout_enabled` (`bool`): global feature flag.
- `rollout_stage` (`disabled|internal_dogfood|canary|general_availability`): release stage.
- `rollout_cohorts` (`string[]`): allowlisted cohorts for staged rollout.
  - `"*"` allows all cohorts inside staged modes.
- rollback thresholds:
  - `max_false_success_rate_bps`
  - `max_abort_rate_bps`
  - `max_error_rate_bps`
  - `max_avg_payout_confirm_latency_seconds`
- hardening checklist booleans:
  - `hardening_authoritative_payout_gate_validated`
  - `hardening_scheduler_recovery_drills_validated`
  - `hardening_swap_risk_alerting_validated`
  - `hardening_incident_runbook_validated`
  - `hardening_test_matrix_gate_green`

Rollout status is visible from `openagents_goal_scheduler` action `status`:

- `rollout.config`
- `rollout.gate`
- `rollout.metrics`
- `rollout.health`

## Stage Policy

1. `disabled`
- No autonomous goal loop execution.

2. `internal_dogfood`
- Enabled for internal cohorts only.
- Require checklist booleans to be actively maintained.

3. `canary`
- Enabled for explicit canary cohorts.
- Monitor metrics continuously and stop expansion on threshold breach.

4. `general_availability`
- Fully enabled once canary remains healthy and checklist is complete.

Lane policy note:

- Stage transitions in this document apply to compute-lane earnings.
- Liquidity-solver rollout must be separately staged with capital-risk-specific controls and explicit user opt-in requirements.

## Metric Definitions

Tracked from run-audit receipts:

- Completion rate: `succeeded_runs / total_runs` (`completion_rate_bps`).
- False-success rate: `false_success_runs / succeeded_runs` (`false_success_rate_bps`).
- Payout-confirm latency: average seconds from `run.started_at` to first payout evidence (`avg_payout_confirm_latency_seconds`).
- Abort/error distribution:
  - `aborted_runs`
  - `failed_runs`
  - `error_attempts`
  - `abort_error_distribution` map for operational buckets.

## Rollback Conditions

Rollback is required if any threshold is exceeded in `rollout.health`:

- `false_success_rate_bps > max_false_success_rate_bps`
- `abort_rate_bps > max_abort_rate_bps`
- `error_rate_bps > max_error_rate_bps`
- `avg_payout_confirm_latency_seconds > max_avg_payout_confirm_latency_seconds`

Immediate rollback actions:

1. Set `rollout_enabled=false`.
2. Enable goal kill switch for impacted cohorts/goals.
3. Freeze expansion to new cohorts.
4. Retain run audits + reconciliation snapshots for incident review.

## Hardening Checklist (Post-Launch)

All must be `true` before broad GA:

- authoritative payout gate validated in production-like traffic,
- scheduler recovery drills validated,
- swap risk alerting validated,
- incident runbook validated with drill evidence,
- deterministic test matrix gate green.

## Epic Close Criteria

The rollout issue is closed only when:

1. Stage is `general_availability`.
2. Rollout health remains green over sustained production window.
3. Hardening checklist is fully complete.
4. No unresolved false-success incidents remain.
