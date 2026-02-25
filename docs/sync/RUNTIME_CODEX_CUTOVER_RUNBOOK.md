# Spacetime Runtime/Codex Rollout Runbook

Owner: sync lane (`apps/runtime`, `apps/openagents.com/service`, `apps/autopilot-desktop`)
Scope: retained-client sync rollout, gate enforcement, and rollback posture

## 1. Preconditions

1. Runtime sync observability tests pass.
2. Retired Spacetime runtime route guard test passes.
3. Control sync claim issuance is healthy (`POST /api/spacetime/token`).
4. Target surface builds are deployed with retained sync compatibility settings.
5. Rollback revision IDs are captured for control and runtime services.

## 2. Contract Rules (Must Hold)

1. Commands/mutations remain HTTP authority operations.
2. Sync transport remains delivery/projection only.
3. Clients persist per-stream checkpoints and apply idempotently by `(stream_id, seq)`.
4. Stale cursor requires deterministic bootstrap/replay recovery.

## 3. Rollout Stages

1. Stage 0: preflight gate
   - run verification commands and capture baseline metrics.
2. Stage 1: internal canary
   - internal cohort only for at least 60 minutes.
3. Stage 2: external canary (5%)
   - hold one business day when SLOs are green.
4. Stage 3: broad rollout (25% -> 50%)
   - two-step expansion with hold between steps.
5. Stage 4: full rollout (100%)
   - maintain rollback window for 24 hours.

## 4. KPI and SLO Gates

Do not advance stages unless all gates are green:

1. Error budget gate
   - sync auth/topic errors and stale-cursor rates below thresholds.
2. Replay gate
   - replay bootstrap latency within budget.
3. Reconnect gate
   - reconnect storm indicators remain bounded.
4. Delivery gate
   - no sustained growth in failed publish/dropped delivery metrics.
5. UX gate
   - no duplicated/jumbled messages; near-real-time updates on retained surface.

## 5. Verification Commands

Runtime sync correctness:

```bash
cargo test -p openagents-runtime-service spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture
cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture
```

Cross-surface replay/resume parity:

```bash
./scripts/spacetime/replay-resume-parity-harness.sh
```

Chaos drill gate:

```bash
./scripts/spacetime/run-chaos-drills.sh
```

Runtime sync metrics snapshot:

```bash
curl -sS "$RUNTIME_BASE_URL/internal/v1/spacetime/sync/metrics" \
  -H "Authorization: Bearer $RUNTIME_ADMIN_TOKEN" | jq
```

Docs consistency gate:

```bash
./scripts/local-ci.sh docs
```

## 6. Rollback Procedure

Use smallest blast-radius rollback first:

1. traffic rollback to last known-good revisions.
2. tighten sync throttle/guard configs before full rollback when possible.
3. rollback impacted client cohort only if server lane remains healthy.
4. validate recovery and replay stability after rollback.

## 7. Required Artifacts Per Gate Execution

1. stage report with pass/fail and go/no-go decision.
2. KPI/SLO snapshots with command evidence.
3. owner + timestamp signoff statement.
4. blocker issues and remediation plan for no-go outcomes.
