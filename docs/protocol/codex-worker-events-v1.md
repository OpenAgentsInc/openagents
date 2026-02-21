# Codex Worker/Event Contract v1

This document anchors the cross-surface Codex worker/event contract under `openagents.codex.v1`.

Canonical proto files:
- `proto/openagents/codex/v1/events.proto`
- `proto/openagents/codex/v1/workers.proto`
- `proto/openagents/codex/v1/auth.proto`
- `proto/openagents/codex/v1/sandbox.proto`

Fixture corpus:
- `docs/protocol/fixtures/codex-worker-events-v1.json`

## Scope

The v1 contract covers:
1. Worker summary/snapshot shapes consumed by web/desktop/iOS.
2. Stream envelope semantics for replay/resume (`seq`, replay metadata, `stale_cursor`).
3. Turn/item/message/reasoning/tool and handshake notification payloads.
4. Auth hydration state envelopes needed for desktop-first Codex runtime flows.

## Compatibility Rules

1. `openagents.codex.v1` is additive-only.
2. Existing field numbers and enum values are never repurposed.
3. Breaking changes require a new package version.
4. Client reducers must apply by `(topic, seq)` idempotently and drop duplicates where `seq <= last_applied`.

## Codex Source Alignment

The notification method set tracks the Codex event/notification families used by:
- `~/code/codex/sdk/typescript/src/events.ts`
- `~/code/codex/codex-rs/app-server-protocol/schema/typescript/EventMsg.ts`

OpenAgents intentionally ships a constrained subset first (thread/turn/item/message/reasoning/tool + handshake/auth errors) and can add additional method enums without breaking v1.
