# 2026-02-27 Codex App-Server Full Integration Audit

## Scope

This audit evaluates current Codex integration in `/Users/christopherdavid/code/openagents` against the upstream app-server protocol in `/Users/christopherdavid/code/codex/codex-rs/app-server-protocol` and operational guidance in `/Users/christopherdavid/code/codex/codex-rs/app-server/README.md`.

Goal: define a concrete path to **100% app-server feature integration** in desktop UX, lane wiring, settings, and protocol compatibility.

## Definition Of 100% Integration

For this audit, "100% integration" means:

1. Every app-server v2 request method is supported in `crates/codex-client`.
2. Every server notification and server-initiated request is handled intentionally (typed path, UI behavior, or explicit opt-out).
3. Desktop exposes complete user flows for supported features (chat/threads/review/skills/account/mcp/config/models/apps/approvals/diagnostics).
4. Type-level parity with upstream protocol for required fields and enums.
5. Conformance tests catch drift when app-server protocol changes.

## Sources Reviewed

- OpenAgents:
  - `crates/codex-client/src/client.rs`
  - `crates/codex-client/src/types.rs`
  - `apps/autopilot-desktop/src/codex_lane.rs`
  - `apps/autopilot-desktop/src/input.rs`
  - `apps/autopilot-desktop/src/input/reducers/codex.rs`
  - `apps/autopilot-desktop/src/input/reducers/skl.rs`
  - `apps/autopilot-desktop/src/panes/chat.rs`
  - `apps/autopilot-desktop/src/panes/skill.rs`
  - `apps/autopilot-desktop/src/pane_registry.rs`
  - `apps/autopilot-desktop/src/app_state.rs`
- Upstream Codex:
  - `codex-rs/app-server/README.md`
  - `codex-rs/app-server-protocol/src/protocol/common.rs`
  - `codex-rs/app-server-protocol/src/protocol/v1.rs`
  - `codex-rs/app-server-protocol/src/protocol/v2.rs`
  - `codex-rs/app-server-test-client/src/lib.rs`

## Current State Summary

- `crates/codex-client` currently implements **23 methods** (including `initialize`).
- Upstream v2+initialize surface is **51 methods** (`50` v2 wire methods + `initialize`).
- `apps/autopilot-desktop` Codex lane exposes **8 command kinds** to desktop reducers:
  - `thread/start`, `thread/resume`, `thread/read`, `thread/list`, `turn/start`, `turn/interrupt`, `skills/list`, `skills/config/write`.
- Desktop has one Codex-focused pane (`PaneKind::AutopilotChat`, title "Codex") plus skill registry interactions via Codex skills endpoints.

### Critical Protocol Drift Detected

1. Delta notification method mismatch in lane parser:
   - lane expects `agent_message/delta`
   - protocol emits `item/agentMessage/delta`
2. Error notification mismatch:
   - lane expects `turn/error`
   - protocol uses top-level `error` plus `turn/completed` with failed status.
3. Server requests are not handled functionally:
   - lane currently auto-acks all server requests with `{ "status": "unsupported" }`.
   - this blocks approval/dynamic-tool/request-user-input integrations.
4. `initialize` capabilities are not modeled/sent:
   - no `experimentalApi` opt-in.
   - no `optOutNotificationMethods` support.

## Coverage Audit

## 1) Client Method Coverage (`crates/codex-client`)

Implemented now:

- Thread: `thread/start`, `thread/resume`, `thread/read`, `thread/list`, `thread/archive`
- Turn/Review: `turn/start`, `turn/interrupt`, `review/start`
- Models: `model/list`
- Skills: `skills/list`, `skills/config/write`
- Account/Auth: `account/read`, `account/login/start`, `account/login/cancel`, `account/logout`, `account/rateLimits/read`
- MCP: `mcpServerStatus/list`, `mcpServer/oauth/login`
- Config: `config/read`, `config/value/write`, `config/batchWrite`
- Utility: `command/exec`

Missing v2 request methods:

- `thread/fork`
- `thread/unsubscribe`
- `thread/name/set`
- `thread/unarchive`
- `thread/compact/start`
- `thread/backgroundTerminals/clean`
- `thread/rollback`
- `thread/loaded/list`
- `turn/steer`
- `thread/realtime/start`
- `thread/realtime/appendAudio`
- `thread/realtime/appendText`
- `thread/realtime/stop`
- `skills/remote/list`
- `skills/remote/export`
- `app/list`
- `experimentalFeature/list`
- `collaborationMode/list`
- `config/mcpServer/reload`
- `configRequirements/read`
- `externalAgentConfig/detect`
- `externalAgentConfig/import`
- `feedback/upload`
- `windowsSandbox/setupStart`
- plus fuzzy-file-search session methods and experimental mock endpoint if full parity is strict.

## 2) Notification Coverage (`codex_lane::normalize_notification`)

Parsed into typed lane notifications now:

- `thread/started`
- `turn/started`
- `turn/completed`
- non-standard legacy-looking: `agent_message/delta`, `turn/error`

Everything else is forwarded as `Raw { method }` without typed behavior.

Notably missing typed handling includes:

- `item/agentMessage/delta`
- `item/started`, `item/completed`
- command/file/reasoning/mcp deltas
- `thread/status/changed`, `thread/tokenUsage/updated`, archive/unarchive/closed/name-updated
- `turn/diff/updated`, `turn/plan/updated`
- `model/rerouted`
- `account/*` notifications
- `mcpServer/oauthLogin/completed`
- `app/list/updated`
- realtime notifications

## 3) Server-Initiated Request Coverage

Protocol server requests:

- `item/commandExecution/requestApproval`
- `item/fileChange/requestApproval`
- `item/tool/call`
- `item/tool/requestUserInput`
- `account/chatgptAuthTokens/refresh`

Current lane behavior:

- all requests receive static unsupported ack.
- no UI prompt, no decision capture, no tool callback routing.

## 4) Type Parity Gaps

Key mismatches between local `types.rs` and upstream protocol:

1. `InitializeParams` missing `capabilities` (`experimentalApi`, `optOutNotificationMethods`).
2. `AskForApproval` missing `Reject { ... }` variant.
3. `ThreadListParams` missing `sortKey`, `sourceKinds`, `archived`, `searchTerm`.
4. `ModelListParams` missing `includeHidden`.
5. `ConfigReadParams` missing optional `cwd`.
6. `TurnStartParams` missing `personality`, `outputSchema`, `collaborationMode`.
7. `UserInput::Text` missing `textElements` support.
8. `GetAccountRateLimitsResponse` missing `rateLimitsByLimitId`.
9. Multiple response/notification structs are simplified (`Value` fallbacks), reducing typed guarantees.

## 5) Desktop UI/Pane Coverage

Existing Codex-facing surfaces:

- `Codex` pane (`PaneKind::AutopilotChat`) for threads, model cycle, send prompt.
- `Skill Registry` pane uses `skills/list` + `skills/config/write`.

Missing dedicated Codex product surfaces for full integration:

- account/auth pane
- approval queue/dialog component
- thread lifecycle controls (fork/archive/unarchive/name/rollback/compact/unsubscribe)
- turn diagnostics (diff, plan, item timeline, token usage)
- MCP pane (status/oauth/reload)
- model catalog pane (hidden models, reroute indicators, capability flags)
- config/editor pane for Codex config APIs and requirements
- app connector pane (`app/list`, update notifications)
- remote skill catalog pane (`skills/remote/*`)
- diagnostics pane (raw protocol stream, wire logs, server request/notification counters)

## Root-Cause Assessment

The current integration was built as a minimal local chat+skills lane and is materially below full app-server parity. Main blockers:

1. protocol surface under-modeled in `crates/codex-client`.
2. lane command model too narrow for full API.
3. notification/request router is not typed beyond a small subset.
4. no UI architecture yet for approvals/account/mcp/config/apps/review diagnostics.
5. no automated protocol drift guardrails against upstream.

## Phased Plan To Reach Full Integration

## Phase 0: Protocol Contract Foundation (Required First)

1. Expand `crates/codex-client` to full v2 request parity.
2. Add missing type fields/variants listed above.
3. Add `InitializeCapabilities` support and wire through lane config.
4. Add protocol-conformance tests that compare supported methods/notifications/requests against upstream lists (from `common.rs` or generated fixtures).
5. Add feature-gating flags in lane config for experimental endpoints.

Deliverable:

- client can encode/decode full request/notification/request surface.
- build-time or CI test fails on protocol drift.

## Phase 1: Core Turn/Notification Correctness

1. Replace legacy notification method parsing with v2 method names.
2. Handle `error` notification and failed `turn/completed` status correctly.
3. Parse and persist item lifecycle (`item/started`, deltas, `item/completed`) into chat timeline state.
4. Add interrupt action in Codex pane and wire `turn/interrupt`.
5. Add token-usage and plan/diff model in state.

UI/components:

- Chat transcript item timeline rows
- Turn status strip (inProgress/completed/interrupted/failed)
- Diff + plan expandable sections

Deliverable:

- Codex pane reflects complete turn lifecycle and item stream accurately.

## Phase 2: Thread Lifecycle Completion

1. Add lane commands + reducers for:
   - `thread/fork`, `thread/unsubscribe`, `thread/name/set`, `thread/unarchive`, `thread/compact/start`, `thread/rollback`, `thread/loaded/list`.
2. Extend thread list filters:
   - archived toggle, search term, source/model-provider filters.
3. Handle thread notifications:
   - status changes, archived/unarchived, closed, name-updated.

UI/components:

- Thread rail actions menu (fork/archive/unarchive/rename/rollback/compact/unsubscribe)
- Loaded-thread indicator + status badges

Deliverable:

- full thread lifecycle operations from desktop.

## Phase 3: Approval + Server Request Orchestration

1. Implement typed server request router in lane.
2. Implement `item/commandExecution/requestApproval` flow with supported decisions.
3. Implement `item/fileChange/requestApproval` flow.
4. Implement `item/tool/call` callbacks and `item/tool/requestUserInput` response flow.
5. Implement `account/chatgptAuthTokens/refresh` routing (if auth mode requires it).

UI/components:

- Approval queue modal/pane with command/file context and decision actions
- Request-user-input form widget (1-3 questions)

Deliverable:

- server-initiated requests are interactive, not auto-rejected.

## Phase 4: Account, Model, And Config Product Surfaces

1. Add Codex Account pane:
   - `account/read`, `account/login/start`, `account/login/cancel`, `account/logout`, `account/rateLimits/read`.
   - subscribe to `account/updated`, `account/login/completed`, `account/rateLimits/updated`.
2. Add Model Catalog pane:
   - `model/list` with `includeHidden` option.
   - display reasoning capabilities, default model, hidden flag.
   - show `model/rerouted` notifications in active turn.
3. Add Codex Config pane:
   - `config/read`, `config/value/write`, `config/batchWrite`, `configRequirements/read`.
   - `externalAgentConfig/detect|import` integration.

Settings/components:

- Extend `SettingsState` with Codex config domain (currently relay/wallet/provider only).

Deliverable:

- full account/model/config administration from desktop.

## Phase 5: MCP, Apps, And Skills Remote

1. Add MCP pane:
   - `mcpServerStatus/list`, `mcpServer/oauth/login`, `config/mcpServer/reload`.
   - handle `mcpServer/oauthLogin/completed` notification.
2. Add Apps pane:
   - `app/list` and `app/list/updated` flow.
3. Add Remote Skills pane:
   - `skills/remote/list`, `skills/remote/export`.
4. Keep existing repo `skills/` flow as first-class local source.

Deliverable:

- full connector/app/remote-skill coverage.

## Phase 6: Review + Utility + Experimental APIs

1. Wire `review/start` end-to-end in UI (inline + detached).
2. Expose `command/exec` utility panel for sandboxed one-off commands.
3. Add optional experimental panes/commands:
   - collaboration mode presets
   - experimental feature list
   - realtime thread APIs
   - windows sandbox setup (platform-gated)
   - fuzzy-file-search sessions

Deliverable:

- parity with utility and experimental app-server domains.

## Phase 7: Diagnostics, Hardening, And Conformance Gates

1. Add Codex diagnostics pane:
   - raw protocol events, counts by method, last failures.
   - optional wire-log path configuration through `AppServerWireLog`.
2. Add integration harness tests using app-server test-client patterns.
3. Add end-to-end desktop smoke scenarios for each domain.
4. Add release checklist requiring method/notification/request parity report.

Deliverable:

- observable, testable, maintainable full integration.

## Recommended Implementation Sequence (Strict)

1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Phase 4
6. Phase 5
7. Phase 6
8. Phase 7

This order keeps protocol correctness and chat safety ahead of UI expansion, and ensures approvals/server-requests are solved before adding more tooling surfaces.

## Acceptance Criteria For Full Integration Program

1. No unsupported app-server server requests in normal workflows.
2. All v2 methods represented in `codex-client` and lane command router.
3. Desktop has intentional UX for every major feature family:
   - chat/threads/turns/review
   - approvals
   - skills local+remote
   - account/auth/rate limits
   - models/config/requirements
   - mcp/apps
   - diagnostics
4. Conformance tests fail fast when upstream protocol changes.
5. Codex pane and related panes function against current `~/code/codex` app-server without method-name drift.
