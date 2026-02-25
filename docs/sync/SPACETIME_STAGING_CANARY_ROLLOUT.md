# Spacetime Staging Canary Rollout

Date: 2026-02-25
Status: Active
Owner lanes: Runtime, Control, Desktop, Infra

## Purpose

Run staged Spacetime canary promotion in staging with deterministic gates, cohort progression evidence, and explicit go/no-go output.

## Cohort Strategy

Default cohort progression:

1. `5%`
2. `10%`
3. `25%`
4. `50%`
5. `100%`

Per-cohort policy:

1. Keep runtime shadow enabled (`OA_RUNTIME_SHADOW_ENABLED=true`, `OA_RUNTIME_SHADOW_SAMPLE_RATE=1.0`).
2. Apply both user and autopilot canary percent to the same cohort value.
3. Use soak scheduling metadata (`OA_STAGING_CANARY_COHORT_SOAK_SECONDS`, default `900` seconds).

## Command

```bash
./scripts/spacetime/run-staging-canary-rollout.sh
```

Useful flags:

```bash
./scripts/spacetime/run-staging-canary-rollout.sh \
  --cohorts 5,10,25,50,100 \
  --cohort-soak-seconds 900 \
  --control-base-url "$OA_STAGING_CONTROL_BASE_URL" \
  --auth-token "$OA_STAGING_CONTROL_AUTH_TOKEN" \
  --runtime-metrics-url "$OA_RUNTIME_METRICS_URL" \
  --control-status-url "$OA_CONTROL_STATUS_URL"
```

Dry-run style execution without local gates:

```bash
./scripts/spacetime/run-staging-canary-rollout.sh --skip-gates
```

## Gate Set

The script blocks promotion when any required gate fails:

1. `scripts/spacetime/provision-check.sh staging`
2. `scripts/local-ci.sh spacetime-replay-resume`
3. `scripts/local-ci.sh spacetime-chaos`
4. `scripts/local-ci.sh sync-security`

## Cohort Probe Contract

When `--control-base-url` and `--auth-token` are provided, each cohort executes:

1. `POST /api/v1/control/runtime-routing/evaluate`
2. Payload shape:
   - `thread_id`: `staging-canary-thread-<cohort>`
   - `cohort_key`: `user:staging-canary-<cohort>`

Any non-`200` probe response blocks rollout.

## Output Artifacts

The script writes artifacts to:

`output/canary/spacetime/staging-<timestamp>/`

Files:

1. `gate-results.jsonl`
2. `cohort-results.jsonl`
3. `canary-env-sequence.txt`
4. `SUMMARY.md`
5. `logs/`
6. optional `runtime_metrics_*.json` / `control_status_*.json` snapshots

Use `SUMMARY.md` as staging promotion evidence.

## Local CI Lane

This rollout lane is callable directly:

```bash
./scripts/local-ci.sh spacetime-staging-canary
```

## Go/No-Go Rule

Promotion is allowed only when:

1. all gates are `passed`,
2. all cohort probes are `passed` or explicitly marked as skipped due to missing live probe config,
3. final decision line in `SUMMARY.md` is `allow`.
