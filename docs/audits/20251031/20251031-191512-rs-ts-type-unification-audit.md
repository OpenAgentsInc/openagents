# Rust → TypeScript Types — Unification Audit (2025-10-31)

Author: Audit agent
Scope: Identify app/bridge surfaces that still rely on ad‑hoc shapes or `any`, and specify where to adopt a single, definitive Rust type exported to TypeScript via `ts-rs`. Includes a primer on how `ts-rs` exporting works and concrete next steps to wire it in.

---

## Executive Summary

We have multiple JSON shapes flowing over the bridge (`threads.list`, `threadsAndTails.list`, `messages.list`, `toolCalls.list`, `bridge.sync_status`, Tinyvex snapshots/updates). On the app side, several places still probe mixed‑case fields (`updated_at` vs `updatedAt`, etc.) and use `any`. This creates fragile timestamp logic (“just now”), duplicate fallbacks, and inconsistent keys.

Adopt a single‑source‑of‑truth contract: define Rust structs for every WS payload and derive TypeScript definitions using `ts-rs`, exporting to `docs/types/bridge.d.ts`. The app should import those types and drop all mixed‑case probing and `any` in favor of the generated shapes.

High‑value targets:
- Thread rows: canonical `ThreadSummaryTs` with `last_message_ts`.
- Message rows: canonical `MessageRowTs` (last N in ascending `ts`).
- Tool‑call rows: canonical `ToolCallRowTs` (Tinyvex subset, snake_case fields).
- Envelopes: `TinyvexSnapshot<T>`, `TinyvexQueryResult<T>`.
- Sync: `SyncStatusTs` (`enabled`, `two_way`, `watched[]`).

---

## Gaps In The App (needs Rust→TS types)

- Drawer thread list
  - File: expo/components/drawer/ThreadListItem.tsx
  - Issues: `row: any`, mixed‑case timestamp fallbacks, recomputes from message tails, “just now” artifacts.
  - Fix: use generated `ThreadSummaryTs` and prefer `last_message_ts` (fallback to `updated_at` only if absent).

- Tinyvex provider
  - File: expo/providers/tinyvex.tsx
  - Issues: context value typed locally; alias helpers probe `id | thread_id | threadId` with `as any`; messages/tool‑calls arrays typed ad‑hoc.
  - Fix: import generated `ThreadSummaryTs`, `MessageRowTs`, `ToolCallRowTs`. Expose a typed `TinyvexContextValue` without `any` or mixed‑case property sniffing. Remove camelCase fallbacks.

- Thread screen timeline
  - File: expo/app/thread/[id].tsx and expo/hooks/use-thread-timeline.ts
  - Status: tool‑call mapping is being hardened. Ensure the hook consumes `MessageRowTs`/`ToolCallRowTs` and does not probe alternate casings or use `Date.now()` fallback.

- WS provider event shapes
  - File: expo/providers/ws.tsx
  - Issues: event envelopes (`tinyvex.snapshot`, `tinyvex.query_result`, `bridge.sync_status`) parsed as `any`.
  - Fix: import `TinyvexSnapshot<T>`, `TinyvexQueryResult<T>`, `SyncStatusTs` and narrow by `name/stream`.

- JSONL renderers (legacy)
  - File: expo/components/jsonl/* (renderers used by library samples and ACP content helpers)
  - Note: The app uses ACP/Tinyvex typed data and components for live session rendering.

- Settings sync view
  - File: expo/app/settings/index.tsx
  - Issues: `bridge.sync_status` parsed loosely.
  - Fix: use `SyncStatusTs` for state and UI.

---

## Gaps In The Bridge (types to export and use)

- Thread summaries in list endpoints
  - File: crates/oa-bridge/src/ws.rs
  - Status: `ThreadSummaryTs` already defined. JSON responses still serialize `tinyvex::ThreadRow` and synthesized rows with ad‑hoc shapes.
  - Fix: map Tinyvex rows and fallback history rows into `ThreadSummaryTs` and serialize that type for both `threads.list` and `threadsAndTails.list`.

- Message rows
  - File: crates/tinyvex/src/lib.rs (struct `MessageRow`), crates/oa-bridge/src/ws.rs (messages.list handler)
  - Fix: derive `TS` for `MessageRow` (exported as `MessageRowTs`) and return exactly that from `messages.list` and `tinyvex.snapshot(stream:"messages")`.

- Tool‑call rows
  - File: crates/tinyvex/src/lib.rs (struct `ToolCallRow`), crates/oa-bridge/src/ws.rs (toolCalls.list)
  - Fix: derive `TS` for `ToolCallRow` (exported as `ToolCallRowTs`), ensure fields are snake_case; no camelCase fallbacks in the app.

- Sync status
  - File: crates/oa-bridge/src/ws.rs (control `SyncStatus`)
  - Fix: introduce `SyncStatusTs` with `enabled: bool`, `two_way: bool`, `watched: { provider: string; base: string; files: i64; last_read: i64 }[]`; derive `TS` and send that.

- Envelopes for push/pull
  - File: crates/oa-bridge/src/ws.rs (snapshots and query results)
  - Fix: add generic envelopes and derive `TS`:
    - `TinyvexSnapshot<T> { type: "tinyvex.snapshot"; stream: "threads" | "messages"; thread_id?: String; rows: T[]; rev: i64 }`
    - `TinyvexQueryResult<T> { type: "tinyvex.query_result"; name: string; thread_id?: String; rows: T[] }`

---

## ts-rs Exporting — Quick Primer

- What it does
  - `ts-rs` derives TypeScript type declarations from Rust structs/enums at compile time and writes them to a `.d.ts` file.

- How to use
  - Add dependency in the bridge crate (already present):
    - `ts-rs = "*"` in `crates/oa-bridge/Cargo.toml`.
  - Derive on your Rust type and point to the export file:
    - Example:
      ```rust
      use ts_rs::TS;
      #[derive(serde::Serialize, TS)]
      #[ts(export, export_to = "docs/types/bridge.d.ts")]
      pub struct ThreadSummaryTs {
          pub id: String,
          pub thread_id: Option<String>,
          pub title: String,
          pub project_id: Option<String>,
          pub resume_id: Option<String>,
          pub rollout_path: Option<String>,
          pub source: Option<String>,
          pub created_at: i64,
          pub updated_at: i64,
          pub message_count: Option<i64>,
          pub last_message_ts: Option<i64>,
      }
      ```
  - Build the bridge; `ts-rs` writes or updates `docs/types/bridge.d.ts` with `export interface ThreadSummaryTs { ... }`.
  - Notes:
    - `Option<T>` becomes `T | null` by default; to make a property optional (no `null`), use `#[ts(optional)]` on that field.
    - `#[serde(rename = ...)]` and field casing are reflected in the TS names. Decide on snake_case or camelCase and stick to it. Recommendation: snake_case for all bridge WS payloads.
    - Ensure the export directory exists (create `docs/types/`), or adjust `export_to` to an existing path.

- App consumption
  - Include the generated d.ts in the TypeScript program so types are visible:
    - Add to `expo/tsconfig.json` → `include`: `"../docs/types/*.d.ts"` (path from `expo/`)
    - Or add `"typeRoots": ["./node_modules/@types", "../docs/types"]`.
  - Import in app code using `import type { ThreadSummaryTs } from '../../docs/types/bridge'` (adjust relative path) and remove `any`/fallbacks.

---

## Proposed Canonical Types To Export (initial set)

- ThreadSummaryTs (already present)
  - Fields: `id`, `thread_id?`, `title`, `project_id?`, `resume_id?`, `rollout_path?`, `source?`, `created_at`, `updated_at`, `message_count?`, `last_message_ts?`.

- MessageRowTs (derive TS for `crates/tinyvex::MessageRow`)
  - Fields: `id`, `thread_id`, `role?`, `kind`, `text?`, `item_id?`, `partial?`, `seq?`, `ts`, `created_at`, `updated_at?`.

- ToolCallRowTs (derive TS for `crates/tinyvex::ToolCallRow`)
  - Fields: `thread_id`, `tool_call_id`, `title?`, `kind?`, `status?`, `content_json?`, `locations_json?`, `created_at`, `updated_at`.

- TinyvexSnapshot<T>
  - `type: "tinyvex.snapshot"`, `stream: "threads" | "messages"`, `thread_id?`, `rows: T[]`, `rev: number`.

- TinyvexQueryResult<T>
  - `type: "tinyvex.query_result"`, `name: string`, `thread_id?`, `rows: T[]`.

- SyncStatusTs
  - `enabled: boolean`, `two_way: boolean`, `watched: { provider: string; base: string; files: number; last_read: number }[]`.

Optional next:
- PlanRowTs and StateRowTs (if we add queries for plan/state).
- BridgeErrorsTs for structured error messages if we standardize them.

---

## Implementation Plan (thin, iterative)

P0 — Make timestamps correct and types consumable
- Bridge
  - Map `threads.list` and `threadsAndTails.list` rows to `ThreadSummaryTs`; set `last_message_ts` from real data (history parse or file mtime), never `now()`.
  - Derive `TS` for `MessageRow` and `ToolCallRow`; export to `docs/types/bridge.d.ts`.
  - Add `SyncStatusTs` and switch `sync.status` to emit it.
  - Add `TinyvexSnapshot<T>` and `TinyvexQueryResult<T>` and use them for snapshots and queries.
- App
  - Update `expo/tsconfig.json` to include `../docs/types/*.d.ts` (or `typeRoots`).
  - Replace local types in `expo/providers/tinyvex.tsx` with generated `ThreadSummaryTs`, `MessageRowTs`, `ToolCallRowTs`.
  - Drawer item (`expo/components/drawer/ThreadListItem.tsx`): use `row.last_message_ts ?? row.updated_at` for the timestamp; remove mixed‑case fallbacks.

P1 — Remove `any` and case probing
- Thread screen mapping (`expo/app/thread/[id].tsx`, `expo/hooks/use-thread-timeline.ts`): ensure tool‑call mapping uses `ToolCallRowTs` only; remove camelCase/snake_case fallback logic and `Date.now()` fallbacks.
- WS provider (`expo/providers/ws.tsx`): narrow incoming envelopes to `TinyvexSnapshot<T>`/`TinyvexQueryResult<T>`/`SyncStatusTs`.

P2 — Document and stabilize
- Create `docs/types/README.md` with the contract and usage guidance.
- Note migration policy: new fields must be added in Rust structs and exported; app code must not probe mixed‑case alternates.

---

## Acceptance Checklist

- Bridge returns only the canonical, snake_case shapes for the above endpoints.
- `docs/types/bridge.d.ts` contains exported interfaces for all listed types and is committed.
- App builds with zero `any` in Tinyvex provider, drawer item, and thread screen tool‑call mapping.
- Drawer timestamps reflect `last_message_ts` (no “just now” unless truly recent).

---

## File References

- crates/oa-bridge/src/ws.rs
- crates/tinyvex/src/lib.rs
- expo/providers/tinyvex.tsx
- expo/components/drawer/ThreadListItem.tsx
- expo/app/thread/[id].tsx
- expo/tsconfig.json

---

## Addendum — ACP Generator vs Our ts-rs Approach

Reference: agent-client-protocol/rust/bin/generate.rs

- ACP’s approach
  - Uses `schemars` (`derive(JsonSchema)`) on core protocol types to emit a single JSON Schema (`schema/schema.json`) plus `meta.json` and rendered docs (`docs/protocol/schema.mdx`).
  - Leans on schema extensions (`x-docs-ignore`, `x-side`, `x-method`) to structure documentation by method and side (agent/client). The generator post-processes the schema to produce human-readable MDX.
  - Outcome: authoritative schema + docs for wire-level protocol; not directly producing TypeScript types for app consumption.

- Our audit approach
  - Uses `ts-rs` (`derive(TS)`) on bridge payload structs to emit a `.d.ts` (`docs/types/bridge.d.ts`) consumed directly by the Expo app.
  - Optimized for immediate type‑safety in the UI (no mixed-case probing, no `any`) with snake_case fields and precise optional/required semantics.
  - Outcome: compile‑time synchronized Rust↔TS contracts for the bridge WS payloads specifically (threads/messages/tool calls/sync/envelopes).

- Pros/cons
  - JSON Schema (ACP)
    - Pros: language‑agnostic spec and docs; rich validation metadata; good for external implementers and long‑form documentation.
    - Cons: requires a second step to get TS types (codegen or manual), which we haven’t wired into the app; drift risk if app types are hand‑maintained.
  - ts-rs (bridge/app)
    - Pros: first‑class TS types with zero extra tooling; trivially importable in the app; eliminates `any` and casing fallbacks.
    - Cons: TS only; no automatic prose docs; needs the export directory to exist; Option<T> defaults to `T | null` unless annotated.

- Practical reconciliation
  - Keep ACP’s schemars generator as the protocol source‑of‑truth and docs pipeline.
  - For our bridge payloads (Tinyvex + WS envelopes), use `ts-rs` to export TS directly for the app.
  - Where we surface ACP structures inline (e.g., tool call content), prefer thin Rust wrapper types that mirror the ACP subset we need and `derive(TS)` on those. If/when ACP exposes ts‑rs or a TS bundle, we can switch to importing that directly.

- Conventions to align
  - Field casing: standardize on snake_case in bridge payloads; reflect the same in `ts-rs` outputs and app code. Avoid mixed‑case probing.
  - Optional vs nullable: favor `#[ts(optional)]` for fields that may be absent, instead of `T | null`, to simplify TS usage in the app.
  - Export placement: continue exporting to `docs/types/bridge.d.ts` and include it in `expo/tsconfig.json` so type updates flow with `cargo build`.
