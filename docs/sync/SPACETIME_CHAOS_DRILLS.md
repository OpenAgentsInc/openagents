# Spacetime Chaos Drills

Status: active  
Updated: 2026-02-25

## Objective

Run deterministic chaos scenarios for Spacetime sync lanes, capture recovery evidence, and enforce rollout safety gates before canary/production promotion.

## Automation Entry Point

```bash
./scripts/spacetime/run-chaos-drills.sh
```

Optional staging snapshots (before/after drill):

```bash
OA_RUNTIME_METRICS_URL="https://<runtime-host>/api/sync/metrics" \
OA_CONTROL_STATUS_URL="https://<control-host>/api/status" \
./scripts/spacetime/run-chaos-drills.sh --output-dir output/chaos/spacetime/staging-$(date -u +%Y%m%dT%H%M%SZ)
```

Artifacts:

- `results.jsonl` (per-scenario status and duration)
- `SUMMARY.md` (drill table and aggregate duration/failure count)
- `logs/*.log` (command output for each scenario)
- optional HTTP snapshots (`runtime_metrics_before/after.json`, `control_status_before/after.json`)

## Scenario Matrix and Expected Behavior

1. `replay_resume_harness`
   - Command: `./scripts/spacetime/replay-resume-parity-harness.sh`
   - Expectation: replay/resume parity tests pass across runtime/shared-client/desktop.
2. `runtime_stale_cursor_floor`
   - Command: `cargo test -p openagents-runtime-service khala_topic_messages_returns_stale_cursor_when_replay_floor_is_missed -- --nocapture`
   - Expectation: stale cursor detected deterministically with rebootstrap contract.
3. `runtime_stale_cursor_budget`
   - Command: `cargo test -p openagents-runtime-service khala_topic_messages_returns_stale_cursor_when_replay_budget_is_exceeded -- --nocapture`
   - Expectation: replay budget breach handled deterministically (no silent drift).
4. `runtime_token_scope_matrix`
   - Command: `cargo test -p openagents-runtime-service khala_topic_messages_enforce_scope_matrix -- --nocapture`
   - Expectation: auth scope boundaries remain enforced under churn.
5. `runtime_slow_consumer_eviction`
   - Command: `cargo test -p openagents-runtime-service khala_topic_messages_evict_slow_consumers_deterministically -- --nocapture`
   - Expectation: lagging consumers are evicted predictably; lane recovers without global collapse.
6. `shared_client_reconnect_storm`
   - Command: `cargo test -p autopilot-spacetime reconnect_storm_resubscribe_keeps_duplicate_delivery_deterministic -- --nocapture`
   - Expectation: repeated reconnect/resubscribe preserves deterministic duplicate batches.
7. `desktop_reconnect_backoff`
   - Command: `cargo test -p autopilot-desktop reconnect_backoff_grows_and_caps_across_disconnects -- --nocapture`
   - Expectation: reconnect backoff is bounded and monotonic.

## Promotion Gate

Chaos drill must pass with `Failures: 0` in `SUMMARY.md` before:

1. staging canary promotion (`OA-SPACETIME-031`)
2. production phased rollout (`OA-SPACETIME-032`)

If any scenario fails:

1. block promotion,
2. file incident + attach artifact directory,
3. fix regression,
4. rerun drill to green before retry.
