# openagents-app-state (OA-RUST-026)

Shared route graph, reducer, and command-intent state core for web/desktop/iOS.

## Modules

1. `route`: typed route graph + path mapping.
2. `intent`: typed command-intent queue model.
3. `state`: deterministic app/auth/stream state model.
4. `reducer`: reducer actions + transition application.

## Determinism

- Reducer transitions are pure state mutations driven by explicit `AppAction`.
- Intent IDs are monotonically assigned from state (`next_intent_id`), not wall clock.
- Fixture replay tests validate deterministic transitions and queue ordering.

## Current consumer

- `apps/openagents.com/web-shell` (bootstrap route + command-intent queue wiring)
