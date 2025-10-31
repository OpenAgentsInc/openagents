# ADR 0002 — Rust → TypeScript Types as Single Source of Truth

Date: 2025-10-31
Status: Proposed (PR 1345 introduces ADR process; this is ADR #2)
Deciders: OpenAgents maintainers
Consulted: Mobile, Bridge, Tinyvex contributors

## Context

The mobile app renders data provided by the Rust bridge over WebSocket. Today, several app surfaces rely on ad‑hoc shapes and `any`, probing mixed‑case fields (`updated_at` vs `updatedAt`) and guessing identifiers. This leads to brittle UI logic (e.g., “just now” timestamps) and drift between Rust and TypeScript.

We want a single, definitive Rust source of truth for bridge payload types that the app consumes directly, eliminating mixed‑case probing and `any` while staying aligned with the upstream Agent Client Protocol (ACP) crate we use in the bridge.

## Decision

Adopt `ts-rs` to export TypeScript definitions from Rust structs that represent our bridge WebSocket payloads. These exported types live in‑repo and are imported by the Expo app.

- Define canonical, snake_case Rust structs for all WS payloads we own:
  - ThreadSummaryTs (with `last_message_ts`)
  - MessageRowTs (last N messages, ascending `ts`)
  - ToolCallRowTs
  - Envelopes: TinyvexSnapshot<T>, TinyvexQueryResult<T>
  - SyncStatusTs
- Derive `TS` on these structs and export to `docs/types/bridge.d.ts`.
- Include that `.d.ts` in the Expo TypeScript config and remove all app‑side `any`/mixed‑case fallbacks for these payloads.
- Map ACP events into these canonical bridge types at the Rust boundary (thin wrappers). When ACP changes, adjust the mapping, keeping the app contract stable.

## Rationale

- Single source of truth: The canonical payload shapes are authored and versioned alongside the bridge code that emits them.
- Type safety in the app: The Expo app consumes the generated `.d.ts` directly; no schema‑to‑TS build step or manual duplication.
- Operationally simple: `derive(TS)` integrates with Cargo builds; no extra CLI/codegen pipeline.
- Alignment with ACP: We wrap ACP updates into our canonical shapes; we can later switch to upstream‑generated TS if ACP provides it without breaking the app.

## Alternatives Considered

1) Schemars (JSON Schema) + TS codegen
   - Pros: language‑agnostic schema; good docs.
   - Cons: requires an additional codegen step to feed the app; higher drift risk without strict integration.

2) typeshare/specta
   - Similar goal to `ts-rs`. `ts-rs` is simpler for our use and already integrated.

3) Import ACP types directly
   - Requires ACP to publish TS types or derive TS; not available today. Wrapping preserves stability and lets us ship now.

## Consequences

- App: Remove `any` and mixed‑case probing for threads/messages/tool calls/sync/envelopes. Prefer `row.last_message_ts ?? row.updated_at`.
- Bridge: All WS endpoints must serialize the canonical TS‑exported structs; synthesized history rows must set real timestamps (no `now()` fallbacks).
- Tooling: Ensure `docs/types/` exists; export to `docs/types/bridge.d.ts`; include in `expo/tsconfig.json`.
- Conventions: snake_case fields; use `#[ts(optional)]` for truly optional fields (avoid `T | null` where absence is intended).

## Implementation Plan

1) Bridge (Rust)
   - Derive `TS` for `tinyvex::MessageRow`, `tinyvex::ToolCallRow`; keep `ThreadSummaryTs` in `ws.rs`.
   - Add `SyncStatusTs` and typed envelopes (TinyvexSnapshot<T>, TinyvexQueryResult<T>); export all to `docs/types/bridge.d.ts`.
   - Switch `threads.list` / `threadsAndTails.list` / `messages.list` / `toolCalls.list` / snapshots / `sync.status` to emit only canonical types; compute `last_message_ts` from data/file mtime.

2) App (Expo)
   - Add `../docs/types/*.d.ts` to `expo/tsconfig.json` include (or `typeRoots`).
   - Replace local provider/drawer/thread timeline types with imports from `bridge.d.ts`.
   - Remove `any` and camelCase/snake_case fallbacks in these paths.

3) Tests/Docs
   - Add unit tests mapping sample ACP events → canonical bridge types.
   - Document the contract in `docs/types/README.md` (what we export, how to add fields).

## References

- PR 1345 (introduces ADR process; pending)
- ACP generator: `agent-client-protocol/rust/bin/generate.rs` (schemars → JSON Schema + MDX docs)
- Audit: `docs/audits/20251031/20251031-191512-rs-ts-type-unification-audit.md`

