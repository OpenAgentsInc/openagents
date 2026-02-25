# Spacetime Production Phased Rollout

Date: 2026-02-25
Status: Active
Owner lanes: Runtime, Control, Desktop, Infra, Ops

## Purpose

Run production promotion of Spacetime sync by deterministic cohorts with hard SLO/error-budget gates and a controlled rollback drill.

## Cohort Progression Policy

Default production progression:

1. `1%`
2. `5%`
3. `10%`
4. `25%`
5. `50%`
6. `100%`

Per cohort:

1. Set both user and autopilot canary percentages to the cohort value.
2. Keep shadow publishing enabled during rollout windows.
3. Respect soak metadata between stages (`OA_PROD_ROLLOUT_COHORT_SOAK_SECONDS`, default `1800` seconds).

## Command

```bash
./scripts/spacetime/run-production-phased-rollout.sh
```

Recommended invocation:

```bash
./scripts/spacetime/run-production-phased-rollout.sh \
  --cohorts 1,5,10,25,50,100 \
  --cohort-soak-seconds 1800 \
  --control-base-url "$OA_PROD_CONTROL_BASE_URL" \
  --auth-token "$OA_PROD_CONTROL_AUTH_TOKEN" \
  --runtime-metrics-url "$OA_RUNTIME_METRICS_URL" \
  --control-status-url "$OA_CONTROL_STATUS_URL" \
  --slo-snapshot-url "$OA_SPACETIME_SLO_SNAPSHOT_URL" \
  --max-p95-latency-ms 600 \
  --max-error-budget-ratio 0.020 \
  --rollback-drill-command "./scripts/local-ci.sh canary-drill"
```

Local smoke (not for real promotion):

```bash
./scripts/spacetime/run-production-phased-rollout.sh --skip-gates --skip-rollback-drill
```

## Gate Set

Baseline rollout gates:

1. `scripts/spacetime/provision-check.sh prod`
2. `scripts/local-ci.sh spacetime-replay-resume`
3. `scripts/local-ci.sh spacetime-chaos`
4. `scripts/local-ci.sh sync-security`

## Real-Time SLO/Error Budget Monitoring

When `--slo-snapshot-url` is configured, the harness evaluates each stage against:

1. `p95_latency_ms` (or `sync.p95_latency_ms`) <= `max-p95-latency-ms`
2. `error_budget_ratio` (or `sync.error_budget_ratio`) <= `max-error-budget-ratio`

Any threshold breach blocks rollout.

## Cohort Probe Contract

When control URL and auth token are present, each cohort probes:

1. `POST /api/v1/control/runtime-routing/evaluate`
2. Payload:
   - `thread_id`: `production-rollout-thread-<cohort>`
   - `cohort_key`: `user:production-rollout-<cohort>`

Any non-`200` response blocks rollout.

## Rollback Drill

Rollback drill is required by default and runs:

`--rollback-drill-command` (default `./scripts/local-ci.sh canary-drill`)

If rollback drill fails, rollout is blocked.

## Output Artifacts

The harness writes:

`output/canary/spacetime/production-<timestamp>/`

Files:

1. `gate-results.jsonl`
2. `cohort-results.jsonl`
3. `slo-results.jsonl`
4. `cohort-env-sequence.txt`
5. `SUMMARY.md`
6. `logs/`
7. optional runtime/control snapshot JSON files

Use `SUMMARY.md` as production promotion evidence.

## Local CI Lane

Direct lane:

```bash
./scripts/local-ci.sh spacetime-production-rollout
```
