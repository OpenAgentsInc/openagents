# Codex Integration Release Checklist

Use this checklist before cutting a desktop release that includes Codex integration changes.

## Required Gate

1. Run the Codex protocol parity gate:
   - `./scripts/lint/codex-protocol-parity-gate.sh`
2. Confirm the gate passed all of:
   - `codex-client` protocol conformance tests.
   - `autopilot-desktop` Codex lane normalization and smoke tests.
   - Codex pane registry smoke coverage.

## Required Manual Checks

1. Open desktop and validate these panes render and respond:
   - `Codex`
   - `Codex Account`
   - `Codex Models`
   - `Codex Config`
   - `Codex MCP`
   - `Codex Apps`
   - `Codex Remote Skills`
   - `Codex Labs`
   - `Codex Diagnostics`
2. In `Codex Diagnostics`:
   - Verify protocol events are incrementing.
   - Verify method counters update during chat and pane actions.
   - Verify last-failure fields are clear in normal flow.

## Optional Wire Log Verification

1. Enable wire logging from `Codex Diagnostics`.
2. Trigger a Codex action (for example, `thread/list` by opening chat thread list).
3. Confirm log file growth at the configured path (default `/tmp/openagents-codex-wire.log`).
4. Disable wire logging and confirm lane restart succeeds.

## Release Script Integration

`scripts/release/macos-release.sh` executes:

1. ownership boundary gate
2. Codex protocol parity gate
3. workspace test/build/release flow
