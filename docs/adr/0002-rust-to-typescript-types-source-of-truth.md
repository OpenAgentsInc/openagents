# ADR 0002 — Rust → TypeScript Types as Single Source of Truth

- Date: 2025-10-31
 - Status: Accepted — Implemented (typed endpoints live; app imports `expo/types/bridge/*`)
- Deciders: OpenAgents maintainers
- Consulted: Mobile, Bridge, Tinyvex contributors

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
- Derive `TS` on these structs and export per‑type `.ts` files into `expo/types/bridge/generated/`.
- Import from `expo/types/bridge/*` in the Expo app and remove all app‑side `any`/mixed‑case fallbacks for these payloads.
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
- Tooling: Ensure `expo/types/bridge/generated/` exists; export per‑type `.ts` files there. We maintain readable shims under `expo/types/bridge/*` if needed.
- Conventions: snake_case fields; use `#[ts(optional)]` for truly optional fields (avoid `T | null` where absence is intended).

## Implementation Status (2025-10-31)

- Bridge emits `ThreadSummaryTs[]`/`MessageRowTs[]`/`ToolCallRowTs[]` and `SyncStatusTs` over WS.
- App consumes only generated types in `TinyvexProvider`/drawer/thread timeline/WS envelopes; mixed‑case probing removed.
- `bridge.sync_status` uses snake_case (`two_way`, `last_read`), matching `SyncStatusTs`.

## Implementation Plan

1) Bridge (Rust)
   - Keep Tinyvex transport‑neutral (no `ts-rs` derives in `tinyvex`).
   - Add transport TS types in the bridge (`ThreadRowTs`, `MessageRowTs`, `ToolCallRowTs`, `SyncStatusTs`) and map from Tinyvex rows.
   - Keep `ThreadSummaryTs` in `ws.rs` (list rows). Export all to `expo/types/bridge/`.
   - Switch `threads.list` / `threadsAndTails.list` / `messages.list` / `toolCalls.list` / snapshots / `sync.status` to emit only canonical types; compute `last_message_ts` from data/file mtime.

2) App (Expo)
   - Import from `expo/types/bridge/*`.
   - Replace local provider/drawer/thread timeline types with imports from the generated files.
   - Remove `any` and camelCase/snake_case fallbacks in these paths.

3) Tests/Docs
   - Add unit tests mapping sample ACP events → canonical bridge types.
   - Document the contract in `docs/types/README.md` (what we export, how to add fields).

## References

- PR 1345 (introduces ADR process; pending)
- ACP generator: `agent-client-protocol/rust/bin/generate.rs` (schemars → JSON Schema + MDX docs)
- Audit: `docs/audits/20251031/20251031-191512-rs-ts-type-unification-audit.md`

---

## Canonical Type Locations (Rust)

- Data rows (Tinyvex — source of truth for persisted rows)
  - `crates/tinyvex/src/lib.rs`
    - `ThreadRow` — threads table shape (+ `last_message_ts`)
    - `MessageRow` — last‑N messages, ascending `ts`
    - `ToolCallRow` — tool calls (content/locations as JSON strings)

- Transport/status (Bridge — WS contract specifics)
  - `crates/oa-bridge/src/ws.rs`
    - `ThreadSummaryTs` — list rows with `last_message_ts`
  - `crates/oa-bridge/src/types.rs`
    - `SyncWatchedDirTs`, `SyncStatusTs` — sync.status payload
    - Envelopes: `TinyvexSnapshot<T>`, `TinyvexQueryResult<T>` (Rust only for now; not exported to TS due to generics)

## Export Details

- We use `#[derive(TS)]` with `#[ts(export, export_to = "../../expo/types/bridge/generated/")]` (from the bridge crate) on non‑generic structs to emit per‑type `.ts` files during `cargo build`.
- Generic envelopes are not exported initially; if we need TS coverage, we will introduce concrete variants (e.g., `TinyvexMessagesSnapshot`) or leave envelopes untyped in TS and type the `rows` contents.
- The export directory is `expo/types/bridge/` (within the app). Ensure the directory exists before building.

## Ownership Boundaries

- Tinyvex owns data row types (`ThreadRow`, `MessageRow`, `ToolCallRow`).
- The bridge owns WS‑specific types (`ThreadSummaryTs`, `SyncStatusTs` and envelopes).
- A dedicated shared types crate is not required now; we may introduce one later if multiple crates beyond the bridge need to depend on the same transport types.
