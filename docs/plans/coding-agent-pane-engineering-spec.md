# Coding Agent Pane Engineering Spec

Status: in progress  
Date: 2026-03-31

Companion docs:

- `docs/plans/coding-agent-pane-prd.md`
- `docs/PANES.md`
- `docs/codex/ROADMAP_CODEX.md`
- `docs/codex/AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`

## Intent

This document answers one concrete implementation question:

> if OpenAgents ships a dedicated `Coding Agent` pane in `autopilot-desktop`,
> what exactly should be built, where should it live, which existing state
> should it reuse, and in what order should it land?

This document does not replace `docs/plans/coding-agent-pane-prd.md`.

That PRD defines the product shape and UX goals.

This companion spec turns that shape into:

- pane contract
- state model
- layout model
- interaction rules
- Rust file ownership
- implementation slices

## Current implementation status

The pane contract and core V1 workbench have already landed in
`apps/autopilot-desktop`.

Implemented slices:

- `PaneKind::CodingAgent` pane contract and registry wiring
- pane-local `CodingAgentPaneState`
- native floating workbench shell
- repo selector and repo-scoped header truth
- session timeline
- composer and interrupt flow
- terminal region bound to the selected thread
- idle-only terminal input submission
- fixed approval bar and approval detail drawer
- changed-files review rail
- inline diff viewer
- real header actions for `Start task` and `Review`

The remaining implementation priority is stabilization:

- repo / thread synchronization edge cases
- approval and review synchronization edge cases
- focus, keyboard, and scroll consistency
- UI polish and truthful empty / disabled states

## Executive Thesis

The right V1 implementation is:

- a new singleton floating pane, `PaneKind::CodingAgent`
- product-owned in `apps/autopilot-desktop`
- visually native to the existing pane system
- backed by app-owned coding-shell truth already present in
  `AutopilotChatState`
- with a new dedicated pane-view state that projects one coding session into a
  purpose-built workbench layout

The key architectural choice is:

> do not create a second coding runtime model beside `AutopilotChatState`.
> Instead, introduce a new pane and view state that reuses existing thread,
> project, terminal, diff, review, and approval truth while presenting it in a
> better coding-workbench layout.

That keeps the product shell app-owned and consistent with
`docs/codex/AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`.

## Current Repo Truth

The implementation must start from what is already real in this repo.

### 1. The app already owns the coding-shell truth

`AutopilotChatState` in
`apps/autopilot-desktop/src/app_state.rs` already owns the main coding-shell
state:

- thread list and thread metadata
- project registry
- selected workspace
- terminal sessions
- transcript cache
- plan artifacts
- diff artifacts
- review artifacts
- approvals and tool requests
- active thread / active turn state
- session configuration such as:
  - model
  - reasoning effort
  - service tier
  - approval mode
  - sandbox mode
  - collaboration mode

Relevant existing types:

- `AutopilotChatState`
- `AutopilotProjectIdentity`
- `AutopilotTerminalSession`
- `AutopilotDiffArtifact`
- `AutopilotReviewArtifact`
- `AutopilotApprovalRequest`
- `AutopilotFileChangeApprovalRequest`

### 2. The pane system already supports the shell shape we need

The desktop app already has:

- pane registration in `pane_registry.rs`
- pane kind routing in `app_state.rs`
- windowed vs docked presentations in `pane_system.rs`
- modern shared pane chrome
- rich pane-local interaction handling in `input.rs` and `input/actions.rs`

That means `Coding Agent` should be implemented as a normal app pane, not a
special out-of-band surface.

### 3. The current chat surface is too broad for the desired workbench UX

`Autopilot Chat` is still the general assistant/chat surface. It already
supports many coding-related controls, but it is not optimized around the full
coding loop of:

- prompt
- run
- watch terminal
- inspect approvals
- review diffs
- interrupt safely

The new pane should not replace the chat model. It should present the same
underlying coding truth through a better focused coding-workbench layout.

## V1 Product Assumptions

The following assumptions are locked for V1 so implementation can move quickly.

### 1. Pane name

The user-facing pane title is `Coding Agent`.

### 2. Pane presentation

V1 ships as a windowed floating pane, not a docked rail.

### 3. Pane multiplicity

V1 ships as a singleton pane.

Reason:

- it reuses app-owned coding-shell truth cleanly
- it avoids multi-pane coding-session ownership problems
- it still supports one active agent session clearly

Future versions can revisit multi-pane support once the workbench contract is
stable.

### 4. Execution posture

V1 is local-machine-only.

### 5. Session multiplicity

V1 supports one active coding-agent session inside the pane.

The pane may show prior session context, but there is only one active working
session in focus at a time.

## Pane Contract

`Coding Agent` is the focused coding workbench for local software tasks.

It owns:

- repo/workspace selection for the active coding session
- status and permission visibility for the active coding session
- session timeline / thread view for coding work
- live terminal output for the active session
- in-pane changed-files and diff review
- approval handling for coding actions
- interrupt and continue controls for the active session

It does not own:

- wallet truth
- provider truth
- global Codex account/config panes
- remote companion behavior
- generic social/system chat modes

## Proposed Pane Registration

### New pane kind

Add:

- `PaneKind::CodingAgent`

### Pane registry contract

Add a new pane spec in `apps/autopilot-desktop/src/pane_registry.rs`:

- title: `Coding Agent`
- command id: `pane.coding_agent`
- label: `Coding Agent`
- description: `Open the local coding-agent workspace with terminal and diff review`
- startup: `false`
- singleton: `true`
- default width: `1180.0`
- default height: `720.0`

### Minimum size

Add a dedicated minimum size in `pane_system.rs`.

Recommended V1 minimum:

- content width: `980.0`
- content height: `620.0`

This is wide enough for:

- thread area
- terminal area
- right review rail

without instantly collapsing into an unreadable layout.

## State Model

## 1. Reuse existing app-owned coding state

Do not duplicate the following state into a new engine model:

- project registry
- active coding thread identity
- terminal transcript lines
- diff artifacts
- review artifacts
- pending approvals
- session configuration

Those already live in `AutopilotChatState` and should remain the product truth.

## 2. Add dedicated pane-view state

Introduce a new pane-local view state in `apps/autopilot-desktop/src/app_state.rs`,
for example:

```rust
pub struct CodingAgentPaneState {
    pub selected_project_id: Option<String>,
    pub active_thread_id: Option<String>,
    pub header_repo_menu_open: bool,
    pub status_drawer_open: bool,
    pub approval_drawer_open: bool,
    pub selected_diff_file_path: Option<String>,
    pub right_rail_tab: CodingAgentRailTab,
    pub thread_scroll_offset: f32,
    pub terminal_scroll_offset: f32,
    pub diff_scroll_offset: f32,
    pub terminal_input_draft: String,
    pub split_main_ratio: f32,
    pub split_terminal_ratio: f32,
}
```

Recommended companion enums:

```rust
pub enum CodingAgentRailTab {
    ChangedFiles,
    Diff,
}

pub enum CodingAgentSessionVisualState {
    Idle,
    Preparing,
    Running,
    AwaitingApproval,
    Interrupted,
    Completed,
    Failed,
}
```

These pane-local states are view concerns, not new engine truth.

## 3. Mapping to existing session truth

The pane should compute a view model from `AutopilotChatState`.

Primary derived values:

- active project identity:
  - `project_registry`
- active thread:
  - `active_thread_id` or pane-selected thread id
- active terminal session:
  - `terminal_sessions[thread_id]`
- latest diff artifact:
  - `thread_diff_artifacts[thread_id]`
- latest review artifact:
  - `thread_review_artifacts[thread_id]`
- approvals:
  - `pending_command_approvals`
  - `pending_file_change_approvals`
- mode:
  - `approval_mode`
  - `sandbox_mode`
- visible status:
  - `active_turn_id`
  - `last_turn_status`
  - terminal session status
  - pending approval counts

## 4. Mode mapping

The product modes chosen in the PRD should map to existing `codex_client`
enums.

Recommended V1 mapping:

### Ask / read-only

- `AskForApproval::OnRequest`
- `SandboxMode::ReadOnly`

### Edit with approval

- `AskForApproval::OnRequest`
- `SandboxMode::WorkspaceWrite`

### Full auto in sandbox

- `AskForApproval::Never`
- `SandboxMode::WorkspaceWrite`

Do not map V1 default workflows to `DangerFullAccess`.

If a future advanced mode exposes it, that should remain explicit and gated.

## Layout Model

V1 uses a three-region workbench layout inside one floating pane.

### Region A: header chrome

Always visible:

- `Coding Agent`
- repo selector
- branch badge
- mode badge
- status badge
- approvals badge
- pane close controls

### Region B: approval bar

Fixed directly under header.

Behavior:

- visible only when approvals exist, or collapsed to a minimal idle strip
- primary actions:
  - approve
  - deny
  - inspect
- opening `inspect` reveals the approval detail drawer

### Region C: main workspace

Two-column split:

- left: thread + terminal stack
- right: changed-files / diff rail

Recommended initial proportions:

- left workspace: `68%`
- right rail: `32%`

### Region D: left stack

Vertical split:

- thread area on top
- terminal below

Recommended initial proportions:

- thread: `62%`
- terminal: `38%`

If split resizing is not already available as a stable primitive, V1 may ship
with fixed ratios and local constants.

## Interaction Rules

## 1. Repo selection

The header repo selector drives the pane’s active project context.

Behavior:

- selecting a repo updates `selected_project_id`
- pane derives branch and workspace context from the selected project
- if no repo is selected:
  - composer disabled
  - terminal input disabled
  - review rail shows empty state

Repo choices should come from existing `project_registry` truth where possible.

## 2. Thread/session selection

V1 should initially bind to one coding thread for the selected project.

Simplest V1 rule:

- if the selected project already has a suitable Autopilot coding thread, use
  its latest active thread id
- otherwise allow `New task` to bootstrap a new coding session for that project

Do not expose a full thread rail in this pane during V1 unless it becomes
necessary for usability.

## 3. Terminal editability

The terminal is:

- read-only while the agent is running
- editable only when the agent is idle

UI requirements:

- visible status text near terminal header
- input field disabled while agent owns execution
- no ambiguous half-enabled state

Terminal editability should be derived from:

- `active_turn_id`
- terminal session status

## 4. Approval behavior

Approval flow:

1. request appears in existing pending approval queues
2. approval bar becomes active
3. user can approve or deny from bar
4. user can open detail drawer for context
5. session resumes or fails accordingly

The drawer should display:

- request type
- command or file-change summary
- cwd or grant root if available
- reason string if available

## 5. Review rail behavior

The review rail defaults to:

- changed-files list on the upper portion
- selected file diff below or full-height diff when a file is selected

If there is no current diff:

- show explicit empty state
- do not show stale diff content

## 6. Interrupt behavior

Interrupt is a first-class action in the pane header or session controls strip.

Behavior:

- visible whenever a session is active
- sends the same underlying interrupt command already used by chat
- updates visible state immediately to `interrupting` / `interrupted` once the
  lane acknowledges it

## Rendering and View Model Strategy

### 1. Add a dedicated pane module

Add a pane-specific renderer/layout module:

- `apps/autopilot-desktop/src/panes/coding_agent.rs`

It should own:

- internal bounds calculation
- pane-local view model assembly
- paint helpers for:
  - repo selector
  - approval bar
  - session status strip
  - thread area
  - terminal area
  - right review rail

### 2. Keep generic pane chrome shared

Use existing pane shell rendering in `pane_renderer.rs`.

Do not clone pane frame logic into the new module.

### 3. Keep layout separate from paint

Recommended module shape:

- `compute_coding_agent_layout(...)`
- `paint_coding_agent(...)`
- smaller helpers for subregions

Avoid paint-position nudging as the primary layout mechanism.

### 4. Reuse existing shared design language

Use the updated app design system:

- dark surfaces
- subtle borders
- mono typography
- existing button/trigger styles
- Mission Control-class section rhythm

The `Coding Agent` pane should feel like a mature native pane, not a foreign
editor widget inside the app.

## File Ownership and Expected Changes

### `apps/autopilot-desktop/src/app_state.rs`

Add:

- `PaneKind::CodingAgent`
- `CodingAgentPaneState`
- any small companion enums
- default initialization
- helper methods to derive current project/thread/session bindings

Do not move existing coding-shell ownership out of `AutopilotChatState`.

### `apps/autopilot-desktop/src/pane_registry.rs`

Add:

- pane spec for `Coding Agent`
- command palette registration

### `apps/autopilot-desktop/src/pane_system.rs`

Add:

- minimum size
- content bounds logic if needed
- hit regions for:
  - repo selector
  - approval bar buttons
  - interrupt
  - diff selection
  - splitters if implemented in V1

### `apps/autopilot-desktop/src/pane_renderer.rs`

Add:

- routing into `panes/coding_agent.rs`
- any shared helper hooks needed for the pane

### `apps/autopilot-desktop/src/panes/coding_agent.rs`

Add the main pane implementation:

- layout
- view model projection
- paint
- subregion rendering

### `apps/autopilot-desktop/src/input.rs`

Add:

- keyboard focus routing for the pane
- approval shortcut handling if any
- terminal input behavior gating

### `apps/autopilot-desktop/src/input/actions.rs`

Add:

- pane actions for:
  - select repo
  - approve / deny
  - open approval drawer
  - interrupt
  - select changed file
  - switch rail tab
  - send terminal input when idle

## Delivery Slices

These slices are intentionally issue-ready and narrow.

### Slice 1: Pane registration and empty shell

Ship:

- `PaneKind::CodingAgent`
- registry entry
- minimum size
- empty shell with header context placeholders

Done when:

- pane opens from command palette
- pane renders with native shell
- no runtime wiring is required yet

### Slice 2: Pane state and repo selection

Ship:

- `CodingAgentPaneState`
- repo selector
- branch/status/mode/approvals badges
- no-repo empty state

Done when:

- pane can bind to a selected project
- header reflects active project context truthfully

### Slice 3: Session timeline surface

Ship:

- thread/timeline region
- composer
- session status strip
- interrupt control

Done when:

- a user can submit a coding task and see timeline progress

### Slice 4: Terminal region

Ship:

- terminal output region
- idle/running edit gating
- terminal scroll

Done when:

- terminal output is visible in-pane
- user input is blocked while agent is active

### Slice 5: Approval bar and drawer

Ship:

- fixed approval bar
- approval detail drawer
- approve / deny actions

Done when:

- pending approvals can be resolved without leaving the pane

### Slice 6: Review rail

Ship:

- changed-files list
- file selection
- inline diff viewer

Done when:

- user can inspect changed files and diffs inside the pane

### Slice 7: polish and replacement-bar pass

Ship:

- spacing and density tuning
- keyboard polish
- empty-state clarity
- resilient scroll behavior
- failure-state messaging

Done when:

- internal dogfood can complete the V1 success sentence inside this pane

## Acceptance Criteria

### AC-1

Opening `Coding Agent` produces a native Autopilot pane with no broken layout
or placeholder overflow.

### AC-2

The header always shows:

- repo
- branch
- mode
- agent status
- approvals count

### AC-3

The terminal is visibly read-only while the agent is running and visibly
editable when the agent is idle.

### AC-4

Pending approvals are actionable from the pane without leaving the pane.

### AC-5

Changed files and inline diffs are inspectable in the right rail.

### AC-6

The pane remains visually native to the existing pane system and does not adopt
foreign IDE chrome.

## Risks and Mitigations

### Risk: state duplication between Chat and Coding Agent

Mitigation:

- keep `AutopilotChatState` as coding-shell truth
- make `CodingAgentPaneState` view-only and pane-specific

### Risk: V1 becomes a second general chat surface

Mitigation:

- no generic thread rail in V1
- optimize the pane around coding execution, terminal, approvals, and review

### Risk: layout becomes too ambitious

Mitigation:

- fixed ratios first
- resizable splits only if already low-risk in current `wgpui` stack

### Risk: approval UX becomes too modal

Mitigation:

- fixed bar + detail drawer
- no modal takeover

## Definition of Done

The engineering slice is done only when all of the following are true:

- `Coding Agent` exists as a real pane in `autopilot-desktop`
- the pane binds to a selected repo/project
- a user can run a local coding task from the pane
- the user can see thread progress, terminal output, pending approvals, and
  changed-file diffs in the same pane
- interrupt and approval actions work without requiring a jump back to chat
- the pane remains honest about local execution, repo context, and permission
  state
