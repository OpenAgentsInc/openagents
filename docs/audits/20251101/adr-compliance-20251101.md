## Summary

- Date: 2025-11-01
- Scope: docs/adr/0001–0006 vs. current code in `expo/` and `crates/oa-bridge/`.
- Outcome: Broad alignment with ADRs 0002–0004. Notable gaps: a camelCase transport field (`messageCount`) emitted by the bridge (violates snake_case). Storybook (ADR‑0005) is implemented with stories under `expo/.rnstorybook/stories`; legacy in‑app Library screens have been removed. ADR‑0006 is updated and enforced: Ignite‑style PascalCase filenames for components, with primitives under `expo/components/ui/`.

## Method

- Read ADRs in `docs/adr/`.
- Grepped and reviewed relevant app/bridge files, focusing on types, WS payloads, controls, and UI anchors.
- Verified Storybook and Maestro wiring and presence of typed Tinyvex transport.

## ADR‑by‑ADR Compliance

### ADR‑0001 — Adopt ADRs

- Status: Implemented. ADRs live under `docs/adr/` with numbering and status fields. Backfilling noted in 0001; additional ADRs present (0002–0006).
- Notes: Process compliance in code/PRs can’t be fully audited here. No blocking issues found.

### ADR‑0002 — Rust → TS Types (single source of truth, snake_case)

- Positives
  - Bridge exports transport types via `ts-rs` into the app:
    - `crates/oa-bridge/src/ws.rs:29` exports `ThreadSummaryTs` to `expo/types/bridge/`.
    - `crates/oa-bridge/src/types.rs:50` exports `ThreadRowTs`, `MessageRowTs`, `ToolCallRowTs`, `SyncStatusTs`.
    - Generated files present, e.g., `expo/types/bridge/ThreadSummaryTs.ts:1`, `expo/types/bridge/MessageRowTs.ts:1`.
  - App imports ACP types from the official SDK via a single re‑export:
    - `expo/types/acp.ts:1`.
  - App uses the bridge types in the Tinyvex provider:
    - `expo/providers/tinyvex.tsx:6`–`8` (imports of `ThreadSummaryTs`, `MessageRowTs`, `ToolCallRowTs`).
  - App WS envelopes use explicit discriminators and typed names (e.g., `tinyvex.snapshot`, `tinyvex.query_result`).

- Issues (non‑compliance)
  - CamelCase field in bridge transport (should be snake_case):
    - `crates/oa-bridge/src/types.rs:61` uses `#[serde(rename = "messageCount")]` for `message_count`.
    - Generated TS reflects `messageCount`:
      - `expo/types/bridge/ThreadRowTs.ts:11`.
    - App code leans into camelCase fallback instead of the canonical snake_case:
      - `expo/components/drawer/ThreadListItem.tsx:70` reads `(row as any)?.messageCount`.
    - Impact: Violates ADR‑0002 “snake_case WS payloads; remove mixed‑case probing”.

  - Pervasive `any` usage in the Expo app:
    - Examples (not exhaustive):
      - `expo/lib/acp/validation.ts:8` (casts input to `any`).
      - `expo/providers/tinyvex.tsx:108`–`161` (multiple `(obj as any)` / `(row as any)` casts).
      - `expo/components/code-block.tsx:27`–`34` (`as any` to satisfy style and highlighter libs).
      - `expo/providers/acp.tsx:28` (Session ID via `(n as SessionNotification as any).sessionId`).
      - `expo/components/drawer/ThreadListItem.tsx:70` (`(row as any)?.messageCount`).
      - `expo/app/scan.tsx:24`–`115` (RN Camera types coerced through `any`).
    - Impact: Violates the ADR‑aligned policy to “never cast to `any`” and to rely on generated types/SDK types.

  - Generated types directory vs. ADR text:
    - ADR mentions `expo/types/bridge/generated/` as the export target; current bridge exports directly to `expo/types/bridge/`.
    - Evidence: `crates/oa-bridge/src/ws.rs:31` and `crates/oa-bridge/src/types.rs:50` use `export_to = "../../expo/types/bridge/"`.
    - Impact: Low. The ADR allows readable shims; however, documentation should be updated or codegen path aligned to reduce confusion.

- Additional observations
  - Bigint fields in generated TS are adapted by app‑side “number shims” in `expo/providers/tinyvex.tsx:16`–`34`. This is acceptable as a UI adapter but could be eliminated by a codegen option if desired.
  - App largely uses `last_message_ts ?? updated_at` per ADR guidance (see `expo/components/drawer/ThreadListItem.tsx:66`–`67`).

### ADR‑0003 — Tinyvex Local Sync over WS; No REST

- Positives
  - Bridge integrates Tinyvex and emits typed WS payloads and controls:
    - Writer broadcasts: `crates/oa-bridge/src/tinyvex_write.rs:7` (`tinyvex.update` with `stream` discriminators).
    - WS query/subscribe controls: `crates/oa-bridge/src/controls.rs:20`–`31` (`tvx.subscribe`, `tvx.query`, `tvx.mutate`, `tvx.backfill`).
    - Sync controls: `sync.status`, `sync.enable`, `sync.two_way` (
      `crates/oa-bridge/src/controls.rs:33`–`41`).
  - App uses only WS for bridge control; no REST calls found:
    - Settings triggers `sync.status`/`sync.enable`/`sync.two_way`:
      - `expo/app/settings/index.tsx:54`, `:66`, `:73`.
    - Tinyvex provider uses `tvx.subscribe`/`tvx.query` verbs:
      - `expo/providers/tinyvex.tsx:292`, `:295`, `:328`, `:339`, `:348`, `:351`, `:360`.
    - App provider constructs `ws://…/ws`; no `fetch()` to bridge:
      - `expo/providers/ws.tsx:138` (URL computation), `rg` found no bridge HTTP fetches.

- Issues (minor)
  - App’s `ws.tsx` carries an `httpBase` field for logging/diagnostics:
    - `expo/providers/ws.tsx:139`, `:339`. Not used for REST — acceptable.

### ADR‑0004 — Maestro E2E (iOS & Android)

- Positives
  - Maestro flows present with stable anchors and warm‑up patterns:
    - `.maestro/flows/bridge_connect_and_stream.yaml:1` and peers.
  - UI exposes durable `testID` anchors used by flows:
    - Header/menu/connection dot: `expo/components/app-header.tsx:54`, `:64`.
    - Composer input/send: `expo/components/composer.tsx:97`, `:114`.
    - Settings root/inputs/buttons: `expo/app/settings/index.tsx:90`, `:111`, `:121`, `:131`.
    - Drawer containers/links: `expo/app/_layout.tsx:139`, `:203`, `:223`.
  - CI workflow for Maestro exists:
    - `.github/workflows/e2e-maestro.yml:1` (build + Maestro Cloud test run).

- Issues
  - None blocking. Keep selectors consistent as components evolve.

### ADR‑0005 — Storybook React Native (v10)

- Positives
  - Storybook wiring present and toggleable via env:
    - Metro config uses the official RN v10 integration and points to `.rnstorybook`: `expo/metro.config.js:3`, `:9`.
    - App toggle: `expo/app/_layout.tsx:258`–`264` requires `../.rnstorybook` when enabled.
    - RN v10 config files exist in `expo/.rnstorybook/`:
      - `expo/.rnstorybook/main.ts:5` (stories glob, `initialSelection` → `App/Home`).
      - `expo/.rnstorybook/index.ts:8` (creates `StorybookUIRoot`).
      - `expo/.rnstorybook/preview.tsx:1` (global decorators: fonts/theme).
    - Stories are present under `expo/.rnstorybook/stories/**`: e.g.,
      - `expo/.rnstorybook/stories/app/home.stories.tsx:17` (`title: 'App/Home'`).
      - Multiple ACP stories `expo/.rnstorybook/stories/acp/*.stories.tsx` (AgentMessage, AgentThought, ToolCall, Plan, AvailableCommands, CurrentMode, ExampleConversation, content variants).
    - Scripts: `expo/package.json:7`, `:9`, `:11`.

- Follow‑ups completed
  - Legacy in‑app library (`expo/app/library/*`) was removed after parity.
  - Default landing story is `App/Home`.

### ADR‑0006 — Component Organization (Accepted)

- Status: Accepted; harmonization in progress.
- Current state:
  - UI primitives live under `expo/components/ui/` with PascalCase filenames (e.g., `Text.tsx`, `Button.tsx`, `TextField.tsx`, `Collapsible.tsx`, etc.).
  - App shell components renamed to PascalCase (`AppHeader.tsx`, `Composer.tsx`, `InlineToast.tsx`, `ToastOverlay.tsx`, `ErrorBoundary.tsx`, `HapticTab.tsx`). Imports updated across the app.
  - Domain renderers remain under `expo/components/acp/*` (already PascalCase) and will be normalized opportunistically when touched.
- Notes: New components and stories must follow PascalCase file naming and live under the correct layer as per ADR‑0006.

## High‑Priority Fixes

- Enforce snake_case in bridge transports
  - Remove camelCase serialization for `message_count`:
    - Change `crates/oa-bridge/src/types.rs:61` to stop emitting `messageCount`.
    - Update any app references that look for `messageCount` (e.g., `expo/components/drawer/ThreadListItem.tsx:70`) to use `message_count`.

- Eliminate `any` casts in Expo app
  - Enforced in AGENTS.md; new stories and UI use proper types from ADR‑0002.
  - Remaining casts (if any) must be removed when touching affected files; block PRs otherwise.

- Storybook adoption (initial parity)
  - Add stories for ACP domain components and key primitives (as per ADR‑0005 acceptance):
    - Agent Message, Agent Thought, Tool Call, Plan, Available Commands, Current Mode, Example Conversation.
  - Gate Storybook from production builds; keep `EXPO_PUBLIC_USE_STORYBOOK` dev‑only.

## Additional Recommendations

- Clarify/export path
  - Either switch codegen to `expo/types/bridge/generated/` as in ADR text or update ADR docs to match current `expo/types/bridge/` target.

- Tests
  - Add a Rust test to assert that serialized WS rows are snake_case (no `messageCount`), covering future regressions.

## Appendix — Notable File References

- Bridge types export
  - crates/oa-bridge/src/ws.rs:29
  - crates/oa-bridge/src/types.rs:50
  - expo/types/bridge/ThreadSummaryTs.ts:1
  - expo/types/bridge/MessageRowTs.ts:1

- CamelCase field (non‑compliance)
  - crates/oa-bridge/src/types.rs:61
  - expo/types/bridge/ThreadRowTs.ts:11
  - expo/components/drawer/ThreadListItem.tsx:70

- Tinyvex WS usage in app
  - expo/providers/tinyvex.tsx:292
  - expo/providers/tinyvex.tsx:351
  - expo/app/settings/index.tsx:54

- Maestro anchors and CI
  - expo/components/app-header.tsx:54
  - expo/components/composer.tsx:97
  - expo/app/settings/index.tsx:90
  - .github/workflows/e2e-maestro.yml:1
