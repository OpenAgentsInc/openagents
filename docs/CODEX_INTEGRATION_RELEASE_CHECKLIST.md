# Codex Integration Release Checklist

Use this checklist before cutting a desktop release that includes Codex integration changes.

## Required Gates

1. Run the Codex protocol parity gate:
   - `./scripts/lint/codex-protocol-parity-gate.sh`
2. Run CAD chat-build release gates:
   - `./scripts/cad/release-gate-checklist.sh`
3. Confirm codex protocol parity passed all of:
   - `codex-client` protocol conformance tests.
   - `autopilot-desktop` Codex lane normalization and smoke tests.
   - Codex pane registry smoke coverage.
4. Confirm CAD release gate output includes explicit chat-build pass lines:
   - `CAD release gate pass (E): chat-build e2e harness (success + failure)`
   - `CAD release gate pass (E): week-1 gripper e2e harness`
   - `CAD release gate pass (E): week-1 gripper script + golden present`
   - `CAD release gate pass (E): chat-build runbook present`

## Required Manual Checks

1. Open desktop and validate these panes render and respond:
   - `Autopilot Chat`
   - `Codex Account`
   - `Codex Models`
   - `Codex Config`
   - `Codex MCP`
   - `Codex Apps`
   - `Codex Labs`
   - `Codex Diagnostics`
2. In `Codex Diagnostics`:
   - Verify protocol events are incrementing.
   - Verify method counters update during chat and pane actions.
   - Verify last-failure fields are clear in normal flow.
3. Execute chat-driven CAD smoke path:
   - send a CAD design request in `Autopilot Chat`
   - confirm CAD pane opens/focuses automatically
   - confirm chat progress block shows phase/tool/rebuild updates
   - confirm CAD rebuild receipt is committed and revision advances
   - confirm follow-up CAD edit also commits.
4. Execute week-1 gripper smoke path:
   - use canonical week-1 gripper prompt
   - generate 4 variants and assign 4 distinct materials
   - toggle single/quad layout
   - verify snapshot truth fields (`viewport_layout`, `visible_variant_ids`, `all_variants_visible`, `variant_materials`).

## Optional Wire Log Verification

1. Enable wire logging from `Codex Diagnostics`.
2. Trigger a Codex action (for example, `thread/list` by opening chat thread list).
3. Confirm log file growth at the configured path (default `/tmp/openagents-codex-wire.log`).
4. Disable wire logging and confirm lane restart succeeds.

## CAD Contract / Runbook Docs

- `docs/codex/CODEX_PANE_CAD_TOOLING.md`
- `docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`
- `docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`

## Release Script Integration

`scripts/release/macos-release.sh` executes:

1. ownership boundary gate
2. Codex protocol parity gate
3. CAD demo release gates
4. workspace test/build/release flow
