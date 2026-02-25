# Spacetime Client Crate (`autopilot-spacetime::client`)

Status: active
Date: 2026-02-25

## Scope

`crates/autopilot-spacetime/src/client.rs` provides typed Rust client primitives for:

1. protocol negotiation (`v2.bsatn.spacetimedb` preferred, `v1` fallback)
2. subscribe lifecycle with resume/stale-cursor handling
3. reducer calls (`append_sync_event`, `ack_checkpoint`) with scope/stream-grant enforcement
4. reconnect planning helpers (resume/rebootstrap decision + bounded backoff)

## Core Types

1. `ProtocolVersion`
2. `SpacetimeClientError`
3. `SpacetimeClientConfig`
4. `SubscribeRequest`
5. `SubscribeResult`
6. `ResumePlan` / `ResumeAction`
7. `SpacetimeClient`

## Guarantees

1. Negotiation fails explicitly if no protocol overlap exists.
2. Subscription authorization is scoped and stream-grant enforced.
3. Cursor continuity uses stale-cursor policy from mapping module.
4. Reducer calls surface sequence conflicts with typed errors.
5. Reconnect/backoff is deterministic and capped.
