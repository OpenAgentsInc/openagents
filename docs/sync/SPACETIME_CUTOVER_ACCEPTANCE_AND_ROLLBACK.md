# Spacetime Cutover Acceptance and Rollback Runbook

Date: 2026-02-25
Status: Active
Owner lanes: Runtime, Control, Desktop, Protocol, Infra

## Purpose

Define hard go/no-go criteria, rollback triggers, and operator procedures for retained Spacetime sync transport rollouts.

This runbook is mandatory evidence for:

1. Staging canary promotion.
2. Production phased rollout.
3. residual legacy-lane cleanup milestones.

## Preconditions

1. `docs/adr/ADR-0009-spacetime-sync-canonical-transport.md` is accepted.
2. `docs/plans/rust-migration-invariant-gates.md` reflects Spacetime canonical transport constraints.
3. Sync v2 contract and compatibility fixtures are merged.
4. Desktop/runtime/control builds are green with Spacetime lane enabled behind feature policy.

## Go/No-Go Acceptance Gates

All gates must be green for the full evaluation window before promotion:

1. Replay correctness
   - No replay-order divergence against baseline parity harness.
   - No unresolved duplicate-apply regressions.
2. Resume/stale-cursor behavior
   - Reconnect/resume succeeds for restart/network-flap scenarios.
   - Stale recovery path produces deterministic client action and successful rebootstrap.
3. Auth/scope correctness
   - No unauthorized reducer or subscribe success events.
   - Token expiry/refresh behavior within expected error budget.
4. SLO posture
   - p95 sync delivery latency <= pre-cutover baseline + agreed migration buffer.
   - reconnect storm recovery remains bounded within runbook target.
5. Rollback discipline
   - No uncontrolled retry/rollback oscillation.
   - cohort rollback decisions remain explicit and operator-approved.

## Promotion Evidence Checklist

Required artifacts:

1. Replay/resume parity harness report.
2. Replay/resume test report.
3. Auth/scope security test report.
4. Staging canary summary with cohort progression evidence.
5. Chaos drill summary (restart, temporary partition, token-expiry storm).
6. Cutover-state announcement artifact confirming Spacetime default mode.
7. On-call signoff and rollback rehearsal confirmation.

Staging canary command:

```bash
./scripts/spacetime/run-staging-canary-rollout.sh
```

Attach `output/canary/spacetime/staging-<timestamp>/SUMMARY.md` with `gate-results.jsonl` and `cohort-results.jsonl`.

Production phased rollout command:

```bash
./scripts/spacetime/run-production-phased-rollout.sh
```

Attach `output/canary/spacetime/production-<timestamp>/SUMMARY.md` with `gate-results.jsonl`, `cohort-results.jsonl`, and `slo-results.jsonl`.

Cutover state announcement command:

```bash
./scripts/spacetime/announce-cutover-state.sh
```

Attach `output/canary/spacetime/cutover-state-<timestamp>/SUMMARY.md` and `result.json`.

Chaos drill command:

```bash
./scripts/spacetime/run-chaos-drills.sh
```

Attach `output/chaos/spacetime/<timestamp>/SUMMARY.md` and `results.jsonl` to promotion evidence.

## Rollback Triggers

Immediate rollback is required if any are true:

1. Replay divergence with customer-visible data inconsistency.
2. Widespread stale-cursor loop without deterministic recovery.
3. Auth scope bypass or cross-tenant leak.
4. Sustained SLO breach beyond agreed rollback threshold.
5. Unrecoverable error storms in subscribe/apply path.

## Rollback Procedure

1. Freeze promotion and open incident.
2. Roll back affected cohort(s) to last known-good release/configuration.
3. Disable newly introduced sync config changes for impacted clients.
4. Preserve event logs and diagnostics for postmortem.
5. Re-run baseline health checks on fallback lane.
6. Publish incident update with root-cause ETA and corrective action plan.

## Post-Rollback Requirements

Before reattempting promotion:

1. Root cause fixed and peer-reviewed.
2. Regression tests added for failure class.
3. Replay/resume/security/chaos gates re-run and attached.
4. Operator signoff re-confirmed.

## Spacetime Retirement Gate

Spacetime endpoint removal is allowed only when:

1. Two release cycles complete with no required fallback usage.
2. All acceptance gates remain green through full production cohorts.
3. Incident runbooks and docs are updated with Spacetime-only canonical guidance.
4. Explicit approval is recorded by runtime/control/desktop owners.
