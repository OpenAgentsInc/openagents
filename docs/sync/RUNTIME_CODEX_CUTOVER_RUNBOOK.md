# Khala Runtime/Codex WS-Only Rollout Runbook

Owner: Khala lane (`apps/runtime`)  
Scope: WS-only rollout gate for runtime/Codex sync across:
- `apps/openagents.com/web-shell`
- `apps/autopilot-desktop`
- `apps/autopilot-ios`
- `apps/onyx` (limited scope)

This runbook is the canonical rollout procedure for OA-RUST-048 / OA-RUST-106 sync-surface alignment.

## 1. Preconditions

1. WS auth/topic ACL/replay tests pass in `openagents-runtime-service`.
2. `POST /api/sync/token` is healthy in control service.
3. Target surface builds are deployed with WS-only subscriptions enabled.
4. Rollback revision IDs are captured for control service and runtime service.
5. Surface matrix in `docs/sync/SURFACES.md` matches deployed topic scopes.

## 2. Contract Rules (Must Hold)

1. Commands/mutations are HTTP-only.
2. Khala WebSocket is subscription/replay-only.
3. Clients persist per-topic watermarks and apply idempotently by `(topic, seq)`.
4. `stale_cursor` forces HTTP snapshot/bootstrap before live tail resume.

## 3. Rollout Stages

1. Stage 0: Preflight gate
   - Validate contract/routing tests and sync-table health.
2. Stage 1: Internal canary
   - Internal cohort only for at least 60 minutes.
3. Stage 2: External canary (5%)
   - Hold one business day when SLOs are green.
4. Stage 3: Broad rollout (25% -> 50%)
   - Two-step expansion with hold between steps.
5. Stage 4: Full rollout (100%)
   - Keep rollback window active for 24 hours.

## 4. KPI and SLO Gates

Do not advance stages unless all gates are green:

1. Error budget gate
   - WS auth/topic errors and `stale_cursor` rate below thresholds.
2. Replay gate
   - Replay bootstrap latency within budget per surface.
3. Reconnect gate
   - Reconnect storm indicators remain bounded.
4. Slow-consumer gate
   - `slow_consumer_evicted` remains below threshold.
5. UX gate
   - No duplicated/jumbled messages; near-real-time updates on active surfaces.

## 5. Verification Commands

Runtime WS correctness:

```bash
cargo test -p openagents-runtime-service server::tests::khala_topic_messages -- --nocapture
```

Fanout metrics snapshot:

```bash
curl -sS "$RUNTIME_BASE_URL/internal/v1/khala/fanout/metrics?topic_limit=20" \
  -H "Authorization: Bearer $RUNTIME_ADMIN_TOKEN" | jq
```

Docs consistency gate:

```bash
./scripts/local-ci.sh docs
```

## 6. Rollback Procedure

Use the smallest blast-radius rollback first:

1. Traffic rollback
   - Route back to last known-good service revisions.
2. Runtime guard rollback
   - Tighten `RUNTIME_KHALA_*` limits prior to full rollback where possible.
3. Surface rollback
   - Roll back only impacted client build if server lane is healthy.
4. Validation
   - Confirm error recovery and stable replay lag after rollback.

## 7. Required Artifacts Per Gate Execution

1. Stage report in `docs/sync/status/` with pass/fail decision.
2. KPI/SLO snapshots with command evidence.
3. Explicit go/no-go statement with owner and timestamp.
4. If no-go: blocker issues and remediation plan.
