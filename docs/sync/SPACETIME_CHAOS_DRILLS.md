# Spacetime Chaos Drills

Status: active
Updated: 2026-02-25

## Objective

Run deterministic chaos scenarios for retained Spacetime sync lanes, capture recovery evidence, and enforce rollout safety gates before canary/production promotion.

## Automation Entry Point

```bash
./scripts/spacetime/run-chaos-drills.sh
```

Optional staging snapshots (before/after drill):

```bash
OA_RUNTIME_METRICS_URL="https://<runtime-host>/internal/v1/spacetime/sync/metrics" \
OA_CONTROL_STATUS_URL="https://<control-host>/api/status" \
./scripts/spacetime/run-chaos-drills.sh --output-dir output/chaos/spacetime/staging-$(date -u +%Y%m%dT%H%M%SZ)
```

## Scenario Matrix and Expected Behavior

1. `replay_resume_harness`
   - Command: `./scripts/spacetime/replay-resume-parity-harness.sh`
   - Expectation: replay/resume parity checks pass across runtime/shared-client/desktop.
2. `runtime_desktop_e2e_suite`
   - Command: `./scripts/spacetime/runtime-desktop-e2e.sh`
   - Expectation: runtime publish + desktop parse/apply gates remain green for Spacetime-only retained paths.
3. `legacy_symbol_guard`
   - Command: `./scripts/spacetime/verify-spacetime-only-symbols.sh`
   - Expectation: no retired sync endpoints/protocol symbols are reintroduced in retained surfaces/docs.
4. `runtime_publish_observability`
   - Command: `cargo test -p openagents-runtime-service spacetime_publisher::tests::http_publish_failure_queues_outbox_for_retry -- --nocapture`
   - Expectation: runtime publish failure classes and outbox-depth observability remain deterministic.
5. `runtime_retired_spacetime_routes`
   - Command: `cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture`
   - Expectation: retired Spacetime internal routes remain removed with deterministic 404 behavior.
6. `shared_client_stale_cursor`
   - Command: `cargo test -p autopilot-spacetime subscribe_rejects_stale_cursor -- --nocapture`
   - Expectation: stale-cursor handling remains deterministic.
7. `shared_client_reconnect_helpers`
   - Command: `cargo test -p autopilot-spacetime reconnect_resume_helpers_plan_rebootstrap_and_backoff -- --nocapture`
   - Expectation: reconnect helpers choose deterministic bootstrap/backoff plans.
8. `shared_client_reconnect_storm`
   - Command: `cargo test -p autopilot-spacetime reconnect_storm_resubscribe_keeps_duplicate_delivery_deterministic -- --nocapture`
   - Expectation: repeated reconnect/resubscribe preserves deterministic duplicate handling.
9. `desktop_reconnect_backoff`
   - Command: `cargo test -p autopilot-desktop reconnect_backoff_grows_and_caps_across_disconnects -- --nocapture`
   - Expectation: desktop reconnect backoff is bounded and monotonic.

## Promotion Gate

Chaos drill must pass with `Failures: 0` in `SUMMARY.md` before staging canary or production promotion.

If any scenario fails:

1. block promotion,
2. file incident + attach artifact directory,
3. fix regression,
4. rerun drill to green before retry.
