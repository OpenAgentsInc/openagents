# OA-RUST-048 WS-Only Rollout Gate Report

Date: 2026-02-21  
Issue: OA-RUST-048 (`#1863`)  
Owner lane: `owner:khala`

## Scope

Executed the WS-only rollout gate process for migrated surfaces (`openagents.com`, `autopilot-desktop`, `autopilot-ios`) and recorded a production readiness decision artifact.

## Stage Results

1. Stage 0 (gate preflight): PASS
   - OA-RUST dependency chain is closed:
     - `#1858 OA-RUST-043 -> CLOSED`
     - `#1859 OA-RUST-044 -> CLOSED`
     - `#1860 OA-RUST-045 -> CLOSED`
     - `#1861 OA-RUST-046 -> CLOSED`
     - `#1862 OA-RUST-047 -> CLOSED`
   - Runtime WS/backpressure/auth test suite is passing:
     - `cargo test -p openagents-runtime-service` => `49 passed`.

2. Stage 1 (internal canary readiness check): PASS (code and test gate)
   - WS-only delivery controls and telemetry endpoints are live in runtime source:
     - `GET /internal/v1/khala/fanout/hooks`
     - `GET /internal/v1/khala/fanout/metrics`
   - Deterministic failure-mode tests pass:
     - poll interval guard (`429 rate_limited`)
     - slow consumer eviction (`409 slow_consumer_evicted`)
     - stale cursor handling (`410 stale_cursor`).

3. Stage 2+ (external canary and production progression): BLOCKED
   - Non-prod Khala service health probe failed in current environment:
     - `apps/runtime/deploy/khala/check-nonprod-health.sh`
     - error: `Cannot find service [oa-khala-backend-nonprod]`.
   - Without deployed non-prod/prod cohort endpoints, rollout telemetry windows cannot be observed.

## KPI/SLO Snapshot

| Gate | Status | Evidence |
| --- | --- | --- |
| Dependency completion (`043-047`) | Green | GitHub issue states all closed |
| WS correctness tests | Green | `cargo test -p openagents-runtime-service` |
| Runtime delivery controls | Green | Backpressure/reconnect metrics and API fields in runtime server responses |
| Canary telemetry window | Red (blocked) | Missing non-prod Khala backend service |
| Production readiness | Red (blocked) | No staged cohort metrics available yet |

## Decision

No-Go for production WS-only rollout expansion at this timestamp.

Reason:
1. Stage 0 and Stage 1 engineering gates are complete.
2. Stage 2 telemetry and staged cohort observation cannot run until non-prod/prod Khala rollout targets are present and monitored.

## Required Remediation Before Re-Run

1. Re-establish non-prod rollout targets and validate health checks (`check-nonprod-health.sh`).
2. Run staged cohort windows (internal -> 5% -> 25% -> 50% -> 100%) with SLO capture.
3. Attach telemetry evidence and final go/no-go signoff to a follow-up gate report.
