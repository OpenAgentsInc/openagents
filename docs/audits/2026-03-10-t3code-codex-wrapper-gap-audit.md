# T3 Code Audit And OpenAgents Codex Wrapper Gap Analysis

Date: 2026-03-10
OpenAgents branch audited: `main`
OpenAgents commit audited: `34803a723`
T3 Code branch audited: `main`
T3 Code commit audited: `1e9bac7f`

## Scope

This audit reviews `~/code/t3code` as it exists today and compares:

- T3 Code current shipped features
- T3 Code clearly signaled near-term / envisioned features
- OpenAgents current Codex-wrapper surface

OpenAgents authority documents reviewed first, per repo contract:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Primary T3 Code surfaces reviewed:

- `README.md`
- `TODO.md`
- `AGENTS.md`
- `REMOTE.md`
- `KEYBINDINGS.md`
- `apps/server/src/codexAppServerManager.ts`
- `apps/server/src/wsServer.ts`
- `apps/server/src/orchestration/*`
- `apps/server/src/provider/*`
- `apps/server/src/git/*`
- `apps/server/src/terminal/*`
- `apps/server/src/checkpointing/*`
- `apps/web/src/components/ChatView.tsx`
- `apps/web/src/components/Sidebar.tsx`
- `apps/web/src/components/ThreadTerminalDrawer.tsx`
- `apps/web/src/session-logic.ts`
- `apps/web/src/proposedPlan.ts`
- `packages/contracts/src/*`

Primary OpenAgents comparison surfaces reviewed:

- `crates/codex-client/src/*`
- `crates/codex-client/tests/*`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/codex_lane/types.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/codex.rs`
- `apps/autopilot-desktop/src/input/reducers/skl.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/panes/codex.rs`

## Executive Summary

T3 Code is ahead of OpenAgents as a coding-workbench product around Codex. OpenAgents is ahead of T3 Code as a broad Codex protocol wrapper and OpenAgents-specific extension layer.

The practical split is:

- T3 Code has built more of the `coding shell around Codex`
- OpenAgents has built more of the `full Codex control plane inside our desktop`

T3 Code already has first-class:

- project/workspace orchestration
- git and worktree workflows
- thread-scoped terminals
- plan capture and plan handoff UX
- structured turn diffs and checkpoint revert
- image attachments
- remote web plus Electron desktop packaging

OpenAgents already has first-class:

- much broader Codex app-server method coverage
- explicit protocol drift guardrails
- dedicated panes for account, models, config, MCP, apps, labs, and diagnostics
- richer server-request handling for approvals, tool calls, tool user input, and auth refresh
- OpenAgents-native dynamic tool bridging into panes, CAD, wallet, provider, and labor flows

Bottom line:

- If the goal is parity with T3 Code as a coding IDE shell, our largest gaps are not protocol gaps. They are product-layer gaps: workspace/project orchestration, git/worktree/PR flows, terminal UX, plan artifact UX, checkpoint/diff UX, and richer turn input.
- If the goal stays aligned to current MVP, most of T3 Code should be treated as reference material, not as a parity mandate. The parts with the highest leverage for us are the plan workflow, checkpoint/diff workflow, and coding-environment controls.

## T3 Code Current Feature Inventory

### 1. App-owned orchestration layer, not just a thin Codex wrapper

T3 Code is not only a WebSocket façade over `codex app-server`.

It has an app-owned orchestration domain with:

- explicit project and thread models in `packages/contracts/src/orchestration.ts`
- command dispatch and event replay in `apps/server/src/orchestration/Services/OrchestrationEngine.ts`
- persisted projections in `apps/server/src/orchestration/Services/ProjectionPipeline.ts`
- snapshot queries in `apps/server/src/orchestration/Services/ProjectionSnapshotQuery.ts`
- provider runtime ingestion into orchestration events in `apps/server/src/orchestration/Layers/ProviderRuntimeIngestion.ts`

That means T3 Code already has its own durable app model for coding sessions instead of treating the Codex thread as the only product object.

### 2. Project, thread, and environment management

T3 Code has first-class support for:

- projects with workspace roots and default models
- threads tied to projects
- per-thread runtime mode: `approval-required` vs `full-access`
- per-thread interaction mode: `default` vs `plan`
- per-thread branch and worktree state
- thread-scoped activity logs, proposed plans, checkpoints, and session state

This is materially ahead of our current chat-thread-only wrapper model.

### 3. Coding workflow surfaces: git, branches, worktrees, PR prep

T3 Code ships a real coding workflow layer:

- Git status, pull, branch listing, branch creation, checkout, repo init
- worktree create/remove
- PR resolution and PR-thread preparation
- stacked actions for commit / push / PR
- branch toolbar logic that preserves local vs worktree context

This is spread across:

- `packages/contracts/src/git.ts`
- `apps/server/src/git/*`
- `apps/web/src/components/BranchToolbar.tsx`
- `apps/web/src/components/GitActionsControl.tsx`
- `apps/web/src/components/PullRequestThreadDialog.tsx`

### 4. Terminal integration is first-class

T3 Code has a thread-scoped PTY terminal system, not just shell-command helpers.

Current shipped surface includes:

- terminal open, write, resize, clear, restart, close
- terminal event push channel
- multi-terminal drawer UI
- per-thread terminal state
- keybindings for terminal workflows
- persisted terminal history in the server layer

Primary files:

- `packages/contracts/src/terminal.ts`
- `apps/server/src/terminal/*`
- `apps/web/src/components/ThreadTerminalDrawer.tsx`
- `apps/web/src/keybindings.ts`

### 5. Proposed plans are durable product objects

T3 Code treats plans as more than transient `turn/plan/updated` data.

It has:

- first-class `proposedPlans` in the thread model
- a dedicated plan sidebar
- collapsed plan previews
- export-to-markdown
- “implement this plan” handoff logic
- plan-mode follow-up behavior

Primary files:

- `packages/contracts/src/orchestration.ts`
- `apps/web/src/proposedPlan.ts`
- `apps/web/src/components/PlanSidebar.tsx`
- `apps/web/src/components/ChatView.tsx`

### 6. Turn diffs and checkpoints are stronger than raw diff text

T3 Code has a full checkpoint and diff subsystem:

- checkpoint capture to hidden git refs
- checkpoint diff computation
- full-thread diff and turn diff query APIs
- checkpoint revert command
- projected checkpoint summaries attached to threads
- diff panel UI

Primary files:

- `apps/server/src/checkpointing/Services/CheckpointStore.ts`
- `apps/server/src/checkpointing/Services/CheckpointDiffQuery.ts`
- `packages/contracts/src/orchestration.ts`
- `apps/web/src/components/DiffPanel.tsx`
- `apps/web/src/components/ChatView.tsx`

This is materially more advanced than our current raw `turn/diff/updated` handling.

### 7. Richer turn input than we currently expose

T3 Code currently supports:

- text input
- image attachments
- composer mention handling
- reasoning-effort selection
- plan/default slash commands

Primary files:

- `packages/contracts/src/orchestration.ts`
- `apps/web/src/components/ChatView.tsx`
- `apps/web/src/composer-logic.ts`
- `apps/web/src/composer-editor-mentions.ts`

### 8. Approval and user-input flows are persisted into app activity

T3 Code projects provider requests into app activity rather than leaving them only as ephemeral protocol events.

It has:

- pending approval derivation
- pending user-input derivation
- projected approval activity entries
- request open / resolve events

Primary files:

- `apps/web/src/session-logic.ts`
- `apps/server/src/orchestration/Layers/ProviderRuntimeIngestion.ts`
- `apps/server/src/persistence/Services/ProjectionPendingApprovals.ts`

### 9. Delivery surfaces: web, Electron desktop, remote access

T3 Code is already shaped as a multi-surface product:

- browser-hosted web app
- Electron desktop app
- remote access instructions for LAN/Tailscale use
- desktop auto-update logic

Primary files:

- `apps/web/*`
- `apps/desktop/*`
- `REMOTE.md`
- `docs/release.md`

### 10. Provider health and Codex readiness checks

T3 Code checks Codex availability and auth state at server startup and surfaces provider health in server config.

Primary files:

- `apps/server/src/provider/Layers/ProviderHealth.ts`
- `packages/contracts/src/server.ts`

This is a stronger preflight story than “try to spawn Codex and fail later”.

## T3 Code Envisioned / Clearly Signaled Features

### Explicitly signaled

- `Claude Code support coming soon`
  - explicit in `README.md`
  - explicit in `AGENTS.md`
- message queueing
  - explicit in `TODO.md`
- thread archiving
  - explicit in `TODO.md`
- more thread/project ordering and browsing polish
  - explicit in `TODO.md`

### Clearly signaled by current architecture or UI placeholders

These are inference-backed, not explicit shipping commitments:

- multi-provider support beyond Codex
  - `ProviderAdapterRegistry` is written as a provider abstraction
  - `ProviderPickerKind` includes `claudeCode` and `cursor`
  - `PROVIDER_OPTIONS` renders `Codex`, `Claude Code`, and `Cursor`, with the latter two disabled today
- broader provider event normalization than current UI uses
  - the server adapter already maps MCP, realtime, request, user-input, plan, diff, and account-style events into a canonical runtime stream

### Important caveat

T3 Code is `Codex-only in practice today`.

Its multi-provider story is architectural readiness plus UI placeholders, not a shipped second provider.

## T3 Code Audit Notes And Risks

### 1. The README materially undersells the product surface

The top-level pitch is “minimal web GUI,” but the codebase is already a real app platform with its own orchestration, persistence, git workflows, checkpointing, terminals, and desktop shell.

That is not necessarily bad, but it means the public docs lag the actual scope.

### 2. T3 Code is much stronger at coding-product UX than at generic Codex control-plane coverage

T3 Code clearly prioritizes coding workflows over complete Codex feature exposure.

It does not appear to have productized equivalents for our dedicated:

- account pane
- model catalog pane
- config pane
- MCP pane
- apps pane
- realtime/fuzzy/collaboration/experimental diagnostics panes

It normalizes many of those events server-side, but does not expose them as broad operator-facing surfaces the way OpenAgents does.

### 3. Multi-provider is not real yet

The provider abstraction is future-ready, but current `ProviderKind` is still effectively `codex` only.

### 4. Some visible backlog items are still basic UX infrastructure

The explicit TODO list still includes:

- message queueing
- thread archiving
- thread sorting and limiting polish

So while the product is richer than advertised, some day-to-day usability flows are still unfinished.

## OpenAgents Current Codex Wrapper Inventory

### 1. Protocol surface is broad and guarded

OpenAgents currently has:

- `48` supported client request methods in `crates/codex-client`
- `43` supported server notification methods
- `5` supported server request methods
- upstream protocol conformance tests in `crates/codex-client/tests/protocol_conformance.rs`

This is a real strength. Our wrapper is much deeper at the Codex protocol edge than T3 Code appears to be.

### 2. Desktop lane surface is broad

`apps/autopilot-desktop/src/codex_lane.rs` currently exposes:

- `50` command kinds
- `64` typed notifications

The lane covers:

- thread lifecycle
- turn lifecycle
- approvals and server requests
- account and auth
- models
- config and external config import
- MCP
- apps
- review
- command execution
- collaboration modes
- experimental features
- realtime
- Windows sandbox setup
- fuzzy file search
- local and remote skills

### 3. OpenAgents already has dedicated Codex product panes

Unlike T3 Code, OpenAgents has explicit pane surfaces for:

- account
- models
- config
- MCP
- apps
- labs
- diagnostics

Primary files:

- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/panes/codex.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

### 4. Skills support is stronger

OpenAgents currently supports:

- `skills/list`
- `skills/config/write`
- `skills/remote/list`
- `skills/remote/export`
- selected-skill attachment into turns
- a dedicated skill registry pane

This is more complete than T3 Code’s current user-facing skill story.

### 5. Server-request handling is stronger

OpenAgents already has explicit handling for:

- command approvals
- file change approvals
- dynamic tool calls
- tool user input
- auth token refresh

Primary files:

- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/codex.rs`

### 6. OpenAgents-native dynamic tools are much richer

OpenAgents currently publishes `29` dynamic tool specs into Codex, spanning:

- pane control
- CAD intent/action
- swap/treasury actions
- goal scheduling
- wallet checks
- provider control
- labor evidence / verifier / claim workflows

Primary file:

- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`

This is a very different product emphasis from T3 Code’s coding-workbench focus.

## Gap Analysis: T3 Code vs OpenAgents Codex Wrapper

| Category | T3 Code Current | OpenAgents Current | Gap / Implication |
| --- | --- | --- | --- |
| Codex protocol breadth | Solid but product-focused; broad canonical runtime mapping, no obvious equivalent to our parity gate | Very broad: `48` client methods, conformance tests, `50` lane commands, `64` typed notifications | We lead here |
| Project/workspace model | First-class projects, threads, sessions, branch/worktree state | No comparable app-owned project/workspace registry for Codex-driven coding work | T3 Code leads decisively |
| Git / branch / worktree / PR | First-class | No comparable generic coding workflow surface in current wrapper | T3 Code leads decisively |
| Terminal UX | First-class PTY terminals with keybindings | No comparable thread-scoped terminal product surface | T3 Code leads decisively |
| Proposed plan artifact UX | First-class proposed plans, sidebar, export, implement-plan handoff | We show live turn-plan updates, but not a durable plan artifact workflow | T3 Code leads |
| Structured turn diff / checkpoints | Full checkpoint capture, query, diff, and revert flow | We currently store raw diff text and turn-plan data, not checkpointed coding-state artifacts | T3 Code leads |
| Image attachments | Shipped in chat turn model and UI | `codex-client` supports image inputs, but `assemble_chat_turn_input` only emits text plus skill attachments today | T3 Code leads |
| Mention / workspace assist | Composer mention handling plus workspace search and file write/open-editor helpers | `codex-client` has a mention input type, but our current chat assembly does not use it; no comparable workspace search/write UX in the wrapper | T3 Code leads |
| Approvals / tool user input / auth refresh | Persisted and projected into thread activity | Supported and more directly exposed in wrapper control flow | Roughly even; our lane is more direct, theirs is more productized in activity state |
| Skills | Limited visible product emphasis | Strong local + remote skill support, registry pane, toggling, turn attachment | We lead |
| Account / models / config / MCP / apps / realtime / labs | Largely not productized as dedicated surfaces | Dedicated panes and actions already exist | We lead decisively |
| Provider health / Codex readiness | Strong startup health checks and surfaced provider status | Less explicit startup health productization in current wrapper | T3 Code leads |
| Desktop/web/remote delivery | Web + Electron + remote access docs + auto-updates | Desktop-first only | T3 Code leads |
| Multi-provider readiness | Placeholder/architecture ready, but not shipped | Codex-only lane | T3 Code is slightly more future-ready, but neither has a real multi-provider ship today |
| OpenAgents-specific tool/economy integration | Not present | Strong and product-specific | We lead decisively |

## Highest-Value Gaps For OpenAgents To Consider

If the goal is to close the most important product gap versus T3 Code without blowing up MVP scope, the best targets are:

### 1. First-class coding environment controls

The biggest missing layer in our current Codex wrapper is not more protocol surface. It is a coding environment layer:

- workspace/project identity
- branch/worktree selection
- terminal access
- git/PR actions

Without that, our wrapper remains more of a powerful Codex console than a coding workbench.

### 2. Durable plan and diff artifacts

T3 Code’s plan sidebar and checkpointed diff flow are high-leverage product ideas.

For us, the most valuable subset would be:

- persist latest proposed plan as a first-class app object
- allow “implement this plan” as an explicit follow-up action
- persist structured diff/checkpoint summaries instead of only raw diff strings

### 3. Richer turn input

We already have protocol types for:

- image inputs
- local image inputs
- mention inputs

But our current chat input assembly only attaches:

- text
- skills

That is an easy-to-state product gap and a likely fast win.

### 4. Coding-shell preflight truth

T3 Code does a better job of answering basic questions before the user hits a wall:

- is Codex installed
- is it authenticated
- is the version supported

We should probably have an equivalent readiness surface if Codex remains a primary user lane.

## What OpenAgents Should Not Copy Blindly

### 1. Do not port T3 Code’s architecture literally

Their implementation is TypeScript server + React web + Electron. Ours is desktop-first Rust + WGPUI.

The correct lesson is to borrow product concepts, not architecture wholesale.

### 2. Do not move product-specific orchestration into `crates/wgpui`

Per `docs/OWNERSHIP.md`, coding-workflow product state belongs in `apps/autopilot-desktop`, not in `crates/wgpui`.

If we build any T3 Code-style parity layer, it should start app-owned.

### 3. Do not let “coding IDE parity” derail current MVP unless explicitly desired

Current MVP authority is still:

- reliable personal agent wrapper around Codex
- provider mode
- wallet truth
- earn loop

T3 Code is a good reference for coding-workbench UX, but much of it is outside the narrow MVP loop.

## Recommended OpenAgents Response

### Option A: Stay MVP-scoped

Keep our current Codex wrapper direction and borrow only the highest-ROI pieces:

- image and mention turn input
- durable proposed-plan artifact UX
- structured diff/checkpoint summaries
- Codex readiness preflight

This keeps us aligned with `docs/MVP.md` and avoids turning OpenAgents into a general-purpose coding IDE.

### Option B: Intentionally compete on coding-workbench UX

If the goal is genuine parity with T3 Code’s product shape, then we need an app-owned layer in `apps/autopilot-desktop` for:

- project/workspace registry
- per-thread branch/worktree state
- thread-scoped terminal runtime
- git and PR workflow actions
- durable plan and checkpoint artifacts

If we choose this path, we should treat it as a new product initiative, not as a few Codex wrapper tweaks.

### My recommendation

Take Option A for MVP, but explicitly steal three T3 Code ideas soon:

1. first-class proposed-plan handoff
2. first-class diff/checkpoint summaries
3. richer coding input context: image + mention + file/workspace affordances

Those close real usability gaps without forcing a full product-shape pivot.
