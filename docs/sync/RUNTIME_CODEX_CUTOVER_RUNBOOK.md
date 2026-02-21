# Khala Runtime/Codex WS-Only Rollout Runbook

Owner: Khala lane (`apps/runtime`)  
Scope: WS-only rollout gate for runtime/Codex sync across `openagents.com`, `autopilot-desktop`, and `autopilot-ios`.

This runbook is the canonical rollout procedure for OA-RUST-048.

## 1. Preconditions

1. OA-RUST-043 through OA-RUST-047 are closed.
2. Runtime WS auth and topic ACL tests pass in `openagents-runtime-service`.
3. WS-only clients are deployed for target surfaces (no SSE/poll fallback lanes).
4. Rollback revision IDs are captured for control, runtime, and Khala services before progression.

## 2. Rollout Stages

1. Stage 0: Gate preflight
   - Validate dependency issues are closed.
   - Run runtime WS contract tests.
   - Verify migration/backfill jobs and sync tables are healthy.
2. Stage 1: Internal canary (employees/admins)
   - Route only internal users to WS-only lane.
   - Keep scope limited to explicit internal cohort for at least 60 minutes.
3. Stage 2: External canary (5%)
   - Expand to external users in bounded cohort.
   - Hold for at least one business day if SLOs remain green.
4. Stage 3: Broad rollout (25% -> 50%)
   - Expand in two steps with a required hold period between each step.
5. Stage 4: Full rollout (100%)
   - Promote WS-only lane to full traffic.
   - Keep rollback window active for 24 hours.

## 3. KPI and SLO Gates

Do not advance stages unless all gates are green in the active observation window.

1. Error budget gate
   - WS auth/topic errors (`401/403`) and `410 stale_cursor` rates remain under agreed incident thresholds.
2. Replay gate
   - Replay bootstrap latency remains within budget for each surface.
3. Reconnect gate
   - Reconnect storm metrics remain bounded (no sustained surge trend).
4. Slow-consumer gate
   - `slow_consumer_evicted` remains below threshold and does not show monotonic growth.
5. Surface UX gate
   - Web, desktop, and iOS message flow remains duplicate-free and near-real-time.

## 4. Monitoring Commands

Runtime WS health:

```bash
cargo test -p openagents-runtime-service server::tests::khala_topic_messages -- --nocapture
```

Fanout delivery metrics (environment URL and auth required):

```bash
curl -sS "$RUNTIME_BASE_URL/internal/v1/khala/fanout/metrics?topic_limit=20" \
  -H "Authorization: Bearer $RUNTIME_ADMIN_TOKEN" | jq
```

Dependency gate check:

```bash
for i in 1858 1859 1860 1861 1862; do
  gh issue view "$i" --json number,title,state --jq '"'"'"#" + (.number|tostring) + " " + .title + " -> " + .state'"'"'
done
```

## 5. Rollback Procedure

Use the smallest blast-radius rollback first:

1. Traffic rollback
   - Shift control/runtime/Khala traffic back to prior known-good revision.
2. Policy rollback
   - Tighten poll/reconnect controls (`RUNTIME_KHALA_*` envs) before full rollback when possible.
3. Surface rollback
   - Roll back affected client release only for the impacted surface if server lane is healthy.
4. Validation
   - Confirm error-rate recovery and stable replay/lag metrics.

## 6. Required Artifacts Per Gate Execution

1. Gate report in `docs/sync/status/` with explicit stage-by-stage pass/fail.
2. KPI/SLO snapshot table with command evidence.
3. Go/no-go decision statement with owner and timestamp.
4. If no-go: blocker list and required remediation issues.
