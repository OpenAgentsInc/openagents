# ADR 0002 — Agent Client Protocol (ACP) as Canonical Runtime Contract

 - Date: 2025-11-01
 - Status: Accepted

## Context

OpenAgents integrates multiple provider CLIs (e.g., Codex, Claude Code) and a desktop app that consumes live updates and local history via an embedded WebSocket server. Provider event shapes differ significantly (Codex JSONL rollouts, Claude transcripts), and ad‑hoc JSON drifting between components makes testing and evolution brittle.

As of the current Tauri (Rust + React) implementation, the backend is written in Rust and translates provider events into ACP types. A Rust “tinyvex” writer persists typed rows locally (SQLite) and a WebSocket server broadcasts normalized updates. This ADR formalizes ACP as our canonical contract across process boundaries and persistent storage.

## Decision

Adopt Agent Client Protocol (ACP) as the single, canonical runtime contract for agent updates and state. Concretely:

- On‑wire over WebSocket
  - Only ACP‑derived envelopes flow to the UI: tinyvex snapshot/update/finalize notifications that are the result of ACP `SessionUpdate` mirroring.
  - No provider‑native JSON is exposed to the UI; provider formats are translated in the Rust backend into ACP `SessionUpdate`s and persisted/streamed as typed rows and notifications.

- In the backend (Rust)
  - ACP client: `tauri/src-tauri/src/oa_acp/client.rs` spawns provider agents (e.g., `codex-acp`) and parses JSON‑RPC notifications into `agent_client_protocol` types.
  - Session manager: `tauri/src-tauri/src/oa_acp/session_manager.rs` owns session lifecycle (`create_session`, `prompt`), receives ACP updates, and forwards them to the tinyvex writer.
  - Tinyvex (SQLite) + writer: `crates/tinyvex/{lib.rs,writer.rs}` persist normalized rows (threads, messages, tool calls, plan/state) and produce writer notifications.
  - WebSocket server: `tauri/src-tauri/src/tinyvex_ws.rs` broadcasts writer notifications as `tinyvex.update`/`tinyvex.finalize` and serves query/snapshot responses (e.g., `messages.list`).

- In the desktop app (React/assistant‑ui)
  - The UI consumes ACP‑derived state via WebSocket using hooks/adapters:
    - Streaming deltas: `tauri/src/lib/useAcpSessionUpdates.ts` subscribes to tinyvex WS, queries on update, and exposes live assistant text and finalize signals.
    - Runtime composition: `tauri/src/runtime/MyRuntimeProvider.tsx` wires adapters and state into `AssistantRuntimeProvider`.
    - Optionally, an ACP‑native runtime (no SSE) is provided by `tauri/src/runtime/useAcpRuntime.tsx` using assistant‑ui’s ExternalStore runtime to map tinyvex message rows directly into thread messages.

- In the database (Tinyvex, Rust)
  - Tables are the minimal, typed projection required by the app (snake_case fields), plus an append‑only `acp_events` log for traceability.
  - All DB access and writing are implemented in the Rust `tinyvex` crate and used by the Tauri backend.

- Naming & casing
  - General rule: prefer snake_case for public payload fields and align with ACP concepts.
  - ACP canonical exceptions (use as‑is, even if camelCase):
    - `sessionUpdate` — discriminant inside `update` envelope for `session/update` notifications (e.g., `{ "update": { "sessionUpdate": "agent_message_chunk", ... } }`).
    - Content/type keys originating from ACP content model such as `type`, `mimeType`, `lastModified`, `name`, `title`, `uri` where applicable in content blocks and resource descriptors.
  - Rationale: these keys are defined by the ACP spec/examples and widely used across implementations; we adopt them verbatim to ensure interop. All other fields under our control remain snake_case (e.g., `current_mode_id`, `available_commands`).

## Rationale

- Interop: ACP provides a shared vocabulary for agent updates (messages, thoughts, tools, plans) across providers.
- Testability: A single contract enables layered tests (unit translator tests → writer invariants → WebSocket integration) and stable UI flows.
- Evolution: Providers change; centralizing translation in one crate keeps drift contained while preserving the app contract.

## Scope

- Applies to all WS payloads and Tinyvex projections produced by the Rust backend.
- Provider‑native artifacts may be persisted for diagnostics, but must not leak provider formats to the app’s WebSocket contract.
- App renders only ACP‑derived content; demos and tests refer to ACP types.
- Code must use the shared Rust ACP crate (`crates/agent-client-protocol`) for all ACP types and JSON‑RPC method names at the boundary. Do not embed ad‑hoc copies of ACP shapes.

## Implementation Plan

1) Maintain and extend the Rust ACP client and adapters for provider parity (e.g., codex‑acp).
2) Keep the tinyvex writer focused on ACP `SessionUpdate` ingestion and typed row upserts; avoid provider‑specific logic in DB.
3) WS only exposes typed tinyvex snapshots/updates; remove any accidental passthrough of provider JSON.
4) Tests:
   - Unit: writer invariants and ACP update mirroring (`crates/tinyvex` tests).
   - Integration: WebSocket broadcast and queries (`tauri/src-tauri/src/tinyvex_ws.rs`).
   - Persistence: tinyvex history queries round‑trip ACP updates.

## Consequences

- Positive: Stronger guarantees at the app boundary; easier to add providers; simpler tests; fewer ad‑hoc shapes.
- Neutral: More rigorous mapping work when providers evolve; offset by containment in translators.
- Negative: Some provider‑specific richness may be normalized; where needed, we can pass structured metadata through ACP `_meta` fields.

## Acceptance

- All backend→UI payloads are ACP‑derived with snake_case fields (with ACP‑spec exceptions noted above).
- Provider adapters live behind Rust ACP client/session management; tinyvex writer operates only on ACP `SessionUpdate`.
- Tests exist at each layer to enforce the contract (see below).

## References

- ADR‑0003 — Tauri Desktop App (Rust + React)
- ADR‑0004 — Tinyvex WebSocket Server and Persistence
- ACP Introduction: https://agentclientprotocol.com/overview/introduction

### Pointers to code and tests (current Rust/Tauri)

- ACP crate (types): `crates/agent-client-protocol/`
- Provider agent (codex‑acp): `crates/codex-acp/`
- Tauri backend ACP client/session manager: `tauri/src-tauri/src/oa_acp/{client.rs,session_manager.rs}`
- Tinyvex (SQLite) + writer: `crates/tinyvex/{lib.rs,writer.rs}`
- WebSocket server: `tauri/src-tauri/src/tinyvex_ws.rs`
- React assistant‑ui integration:
  - Streaming hook: `tauri/src/lib/useAcpSessionUpdates.ts`
  - Runtime provider: `tauri/src/runtime/MyRuntimeProvider.tsx`
  - ACP ExternalStore runtime (prototype): `tauri/src/runtime/useAcpRuntime.tsx`
 - Tests: see `crates/tinyvex` unit tests and `tauri/src-tauri` integration tests as they are added
