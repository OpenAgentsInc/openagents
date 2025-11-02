## ADR Compliance Audit — 2025-11-02

- Date: 2025-11-02
- Scope: ADRs 0001–0007 vs. current repo (Expo app under `expo/`, Rust bridge under `crates/oa-bridge/`).
- Summary: Overall alignment with ACP-first, Tinyvex-first architecture (ADRs 0002, 0003, 0007). Key improvements landed today for #1380: the app timeline now merges Tinyvex rows across alias + canonical ids and renders live ACP assistant chunks immediately; noisy debug logs removed. Remaining gaps: lingering `any` casts in the app (contrary to ADR‑0002/type-safety policy), a few ad-hoc transforms around generated types, and missing canonical thread-id resolver RPC. Storybook (ADR‑0005) and component organization (ADR‑0006) look solid. Maestro (ADR‑0004) is configured with lanes and anchors; expand coverage.

## Method

- Read ADRs in `docs/adr/` (0001–0007).
- Reviewed bridge code for transports, ACP translation, Tinyvex writer, watchers, and WS controls.
- Reviewed app providers (WS/Tinyvex/ACP), settings, routing, and UI renderers.
- Verified presence of Storybook RN v10, Maestro docs/scripts, and generated TS types from Rust via `ts-rs`.

## ADR‑by‑ADR Compliance

### ADR‑0001 — Adopt ADRs (docs/adr/0001-adopt-adrs.md:1)

- Status: Compliant.
  - ADRs exist with numbering, status, and consistent template under `docs/adr/`.
  - Backfilling is underway; ADR‑0007 (ACP) added and referenced by code and issues.
- Gaps: Process enforcement (referencing ADRs in PRs) is cultural; no tooling noted. Optional: add a PR template that prompts for ADR linkage.

### ADR‑0002 — Rust → TS Types (single source of truth, snake_case) (docs/adr/0002-rust-to-ts-types.md:1)

- Positives
  - Bridge exports transport types via `ts-rs` into the app:
    - `crates/oa-bridge/src/ws.rs:29` exports `ThreadSummaryTs` to `expo/types/bridge/`.
    - `crates/oa-bridge/src/types.rs:1` defines and exports transport types like `MessageRowTs` and `SyncStatusTs`; tests enforce snake_case (see assertions around `message_count`).
    - Generated TS present: `expo/types/bridge/MessageRowTs.ts:1`, `expo/types/bridge/ThreadRowTs.ts:1`, `expo/types/bridge/ThreadSummaryTs.ts:1`.
  - App imports these types and applies bigint→number shims narrowly in the Tinyvex provider (`expo/providers/tinyvex.tsx:1`).
  - WS contract uses discriminated unions with type/name fields (`tinyvex.snapshot`, `tinyvex.query_result`, etc.).
  - CamelCase transport regression is covered by tests and removed in code:
    - `crates/oa-bridge/src/types.rs:190` asserts snake_case `"message_count"`; `:200` asserts absence of `"messageCount"`.

- Gaps (Non‑compliance / Risks)
  - App still uses `any` in several critical paths, violating the “no any” repo policy reinforced by ADR‑0002:
    - `expo/providers/tinyvex.tsx:100`–`220` (multiple `(obj as any)` / `(row as any)` accesses)
    - `expo/lib/timeline.ts:1` (helper currently uses `(r as any)` to accommodate partial type convergence; should switch to generated types)
    - `expo/components/drawer/ThreadListItem.tsx:69`–`75` (historical; improved but still has bypass patterns)
    - `expo/providers/acp.tsx:1` (Session ID extraction via unsafe cast)
  - Generated type location differs from ADR text (exports to `expo/types/bridge/` vs `expo/types/bridge/generated/`). Low risk, but docs/code should align.

- Action Items
  - Replace `(any)` casts in `expo/providers/tinyvex.tsx` and `expo/lib/timeline.ts` by importing the generated `MessageRowTs`/`ThreadRowTs` and adding precise UI adapter functions.
  - Update ADR 0002 or codegen path to reflect the actual export directory and prevent confusion.

### ADR‑0003 — Tinyvex Local Sync over WS; No REST (docs/adr/0003-tinyvex-local-sync-engine.md:1)

- Positives
  - Tinyvex is integrated; writer returns typed notifications and the WS adapter re-broadcasts them:
    - `crates/oa-bridge/src/tinyvex_write.rs:1` emits `tinyvex.update` for `threads/messages/toolCalls/plan/state`.
  - WS controls are in place and used by the app; no REST paths are introduced:
    - Controls in `crates/oa-bridge/src/ws.rs:840` handle `tvx.query`, `tvx.subscribe`, `sync.status`, `sync.enable`, `sync.two_way`, `sync.full_rescan`.
  - Watchers tail provider stores and mirror updates as ACP → Tinyvex; alias mirroring works:
    - `crates/oa-bridge/src/watchers/sessions_watch.rs:1` (Codex JSONL watcher) and `claude_watch.rs` handle inbound sync.
  - App Settings screen wires sync toggles and status, querying `bridge.sync_status`:
    - `expo/app/settings/index.tsx:1` sends WS controls and renders status (`SyncStatusTs`).

- Gaps / Notes
  - Identity: multi‑id (session id + client doc id) duplication is still present at the data level; the app now merges/dedupes, but a canonical id resolver RPC would simplify clients (see ADR‑0007 section).
  - Two‑way sync writer for non‑Codex providers is guarded; advisable to document exact write paths and retention policy (docs exist, but operator guide could be clearer).

### ADR‑0004 — Maestro E2E Testing (docs/adr/0004-maestro-e2e-testing.md:1)

- Positives
  - Maestro RN setup and scripts exist with env-driven workflows and device warmups:
    - Scripts referenced in ADR are present (see `docs/audits/20251101/maestro-e2e-audit.md:1`).
    - Stable selectors present in UI: `settings-root`, `header-connection-indicator`, composer anchors.
  - E2E strategy prefers verifying persisted Tinyvex history after send (stable), aligned with ADR‑0003.

- Gaps / Next Steps
  - Add a smoke flow asserting “one user + one assistant” in the thread timeline (now that #1380 is fixed) using durable anchors.
  - Optional Android lane enablement after iOS stabilization.

### ADR‑0005 — Storybook RN v10 (docs/adr/0005-storybook-react-native.md:1)

- Positives
  - RN v10 Storybook config is present and wired behind `EXPO_PUBLIC_USE_STORYBOOK`:
    - `.rnstorybook/` config: `expo/.rnstorybook/main.ts:1`, `preview.tsx:1`, `index.ts:1`.
    - Metro integration: `expo/metro.config.js:1` uses `withStorybook`.
    - Entrypoint switch: `expo/app/_layout.tsx:260` branches to Storybook when enabled.
    - Dependencies/scripts: `expo/package.json:7` `start:sb`, `ios:sb`, `android:sb`.

- Gaps / Notes
  - Ensure new component stories cover key ACP renderers and UI primitives; keep legacy in‑app Library removed (as it is now).

### ADR‑0006 — Component Organization (docs/adr/0006-component-organization.md:1)

- Positives
  - UI primitives live under `expo/components/ui/` with PascalCase filenames:
    - e.g., `Text.tsx`, `Button.tsx`, `TextField.tsx`, `Card.tsx`, `ListItem.tsx`, `AutoImage.tsx`, `Switch.tsx`, `Screen.tsx`, `Collapsible.tsx` (see `expo/components/ui/index.ts:1`).
  - Domain renderers remain under `expo/components/acp/*` and `expo/components/jsonl/*`.
  - App shell components remain in place (`app-header`, composer, drawer items). Stable testIDs appear in shell where required by Maestro.

- Gaps / Notes
  - Minor doc inconsistency: ADR mentions PascalCase naming but one acceptance bullet says “kebab‑case filenames” for `ui/`. Current code uses PascalCase consistently; suggest fixing ADR text to avoid confusion.

### ADR‑0007 — ACP as Canonical Runtime Contract (docs/adr/0007-agent-client-protocol.md:1)

- Positives
  - Provider adapters translate provider events → ACP updates; Tinyvex writer ingests ACP `SessionUpdate`:
    - Codex mapping: used in `crates/oa-bridge/src/ws.rs:840` and watchers.
  - WS only exposes typed snapshots/updates and controls; no provider JSON is leaked to the app contract.
  - App renders ACP-derived content; timeline now uses ACP agent chunks live (merged across ids) and Tinyvex rows for persistence.

- Gaps / Next Steps
  - Add a small `threads.resolve` control so clients can resolve `{ alias or id }` → `{ canonical thread_id, aliases }`, then subscribe/query by canonical id only.
  - Consider surfacing a stable `message_id` in the transport for robust id-based dedupe in the UI.

## Cross‑Cutting Gaps and Risks

- Type Safety (Repo policy + ADR‑0002): Several `any` casts remain in app code paths that should be typed by generated TS. Risk: drift and subtle UI bugs (like #1380) when shapes change.
- Identity: Dual writes (session id + client doc id) complicate clients. The current UI merge/dedupe is a good hotfix, but moving to canonical id + alias resolution will harden the system.
- Test Coverage: Maestro suite should include the critical “assistant renders once” flow. Add unit tests for transport mapping (ACP → Tinyvex rows) where not present.

## Action Items (Concrete)

1) Replace `any` casts in app providers and timeline
   - Files: `expo/providers/tinyvex.tsx:1`, `expo/providers/acp.tsx:1`, `expo/lib/timeline.ts:1`, `expo/components/drawer/ThreadListItem.tsx:55`.
   - Import and use generated `MessageRowTs` instead of structural probing; add precise adapters for bigint fields.

2) Add `threads.resolve` WS control (bridge + app)
   - Bridge: `crates/oa-bridge/src/ws.rs:840` (controls dispatch) — add handler to map `{ id_or_alias }` → `{ thread_id, aliases }`.
   - App: resolve on thread open; prefer canonical id for `tvx.subscribe`/`tvx.query`.

3) Expose stable `message_id` in transport rows (optional)
   - Bridge transport type (`crates/oa-bridge/src/types.rs:1`) — add field if available, or compute deterministic ids for dedupe.

4) Maestro smoke for #1380 regression guard
   - Add flow: send “Hi”, assert one user + one assistant in timeline; rely on Tinyvex history anchor.

5) Docs alignment for codegen output directory
   - Update ADR 0002 or adjust `export_to` paths to match `expo/types/bridge/generated/` or clarify current `expo/types/bridge/` location.

## Evidence (File References)

- Bridge
  - crates/oa-bridge/src/ws.rs:840 — WS controls, queries, and sync.
  - crates/oa-bridge/src/tinyvex_write.rs:1 — Writer notifications → WS updates.
  - crates/oa-bridge/src/watchers/sessions_watch.rs:1 — Codex watcher and alias mirroring.
  - crates/oa-bridge/src/types.rs:1 — Transport types, snake_case tests (`message_count`).

- App
  - expo/providers/tinyvex.tsx:1 — Generated transport types, WS handling, alias projections.
  - expo/hooks/use-thread-timeline.tsx:1 — Merged rows across alias + canonical ids; live ACP agent chunks rendering and dedupe.
  - expo/lib/timeline.ts:1 — Message render rules (dedupe, XML filtering, classification).
  - expo/app/settings/index.tsx:1 — Sync WS controls and status rendering.
  - expo/.rnstorybook/main.ts:1 — Storybook RN v10 config; expo/metro.config.js:1 — withStorybook.

## Conclusion

The codebase materially aligns with ADR‑0002, ADR‑0003, and ADR‑0007. Today’s changes harden the ACP→Tinyvex→UI path and close #1380 at the UI layer. Biggest gaps are type hygiene in the app (remove `any`) and identity simplification (canonical thread id resolver). Addressing those will reduce future friction and regressions.

