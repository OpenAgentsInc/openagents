# Cross-Surface Contract Harness

This harness validates Codex control-lane contracts across iOS, runtime API, and desktop parser boundaries.

## Scenario Catalog

The source of truth for scenario IDs/descriptions is:

- `docs/autopilot/testing/cross-surface-contract-scenarios.json`

## Deterministic Scenarios

1. `codex-control-turn-start-interrupt`
2. `codex-control-restart-resume-terminal-receipts`

## Exact Reproduction Steps

1. From repo root, run:

   ```bash
   ./scripts/run-cross-surface-contract-harness.sh
   ```

2. Confirm summary artifacts were generated (timestamped UTC folder):

   ```bash
   ls -1 docs/autopilot/testing/reports/cross-surface/
   ```

3. Open the latest report:

   ```bash
   cat docs/autopilot/testing/reports/cross-surface/<TIMESTAMP>/SUMMARY.md
   ```

4. Verify all surfaces are `passed`:

   ```bash
   jq -r '.overall_status, .totals' docs/autopilot/testing/reports/cross-surface/<TIMESTAMP>/summary.json
   ```

## Adapter Commands Used by Harness

- Web shell contract tests:

  ```bash
  cargo test -p openagents-web-shell codex_thread::tests
  ```

- Desktop runtime codex proto tests:

  ```bash
  cargo test -p autopilot-desktop runtime_codex_proto::tests
  ```

- Runtime API contract tests:

  ```bash
  (cd apps/openagents.com && ./vendor/bin/pest tests/Feature/Api/RuntimeCodexWorkersApiTest.php)
  ```

- iOS contract tests:

  ```bash
  xcodebuild test \
    -project apps/autopilot-ios/Autopilot/Autopilot.xcodeproj \
    -scheme Autopilot \
    -destination 'platform=iOS Simulator,name=iPhone 17 Pro' \
    -only-testing:AutopilotTests
  ```

## Restart/Resume Continuity Checks

The `codex-control-restart-resume-terminal-receipts` scenario is considered satisfied when:

1. iOS coordinator replay tests pass.
2. Runtime API idempotent replay passthrough tests pass.
3. Desktop terminal receipt dedupe key stability test passes.
4. Harness summary reports no failed surface runs.
