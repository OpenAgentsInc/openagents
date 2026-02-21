# openagents-app-state (OA-RUST-026)

Shared route graph, reducer, command-intent state core, and command planning layer for web/desktop/iOS.

## Modules

1. `route`: typed route graph + path mapping (chat/workers/account/settings/billing/admin/debug).
2. `intent`: typed command-intent queue model.
3. `state`: deterministic app/auth/stream state model (including per-topic sync watermarks).
4. `reducer`: reducer actions + transition application.
5. `command_bus`: typed intent-to-HTTP adapter mapping, deterministic error taxonomy, retry baseline, and latency metric contract.

## Determinism

- Reducer transitions are pure state mutations driven by explicit `AppAction`.
- Intent IDs are monotonically assigned from state (`next_intent_id`), not wall clock.
- Fixture replay tests validate deterministic transitions and queue ordering.

## Current consumer

- `apps/openagents.com/web-shell` (bootstrap route + command-intent queue wiring)
