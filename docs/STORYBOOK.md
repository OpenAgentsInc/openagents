# Storybook Status

Status: archived legacy workflow.

OpenAgents no longer runs the legacy `apps/web` Storybook stack.

Current UI verification lanes are Rust-native:

1. `apps/openagents.com/web-shell/scripts/perf-budget-gate.sh`
2. `apps/openagents.com/web-shell/check-host-shim.sh`
3. `scripts/run-cross-surface-contract-harness.sh`

Historical Storybook material was archived to backroom in OA-RUST-113.
