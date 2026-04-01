# Coding Agent Pane — Engineering Tickets

Generated from:

- `docs/plans/coding-agent-pane-prd.md`
- `docs/plans/coding-agent-pane-engineering-spec.md`

Date: 2026-03-31

## Progress

| Ticket | Status | Notes |
|--------|--------|-------|
| CA-1 | ✅ Completed | `PaneKind::CodingAgent` registered and command-palette accessible |
| CA-2 | ✅ Completed | `CodingAgentPaneState` added without duplicating runtime truth |
| CA-3 | ✅ Completed | Native pane shell and truthful workbench scaffolding landed |
| CA-4 | ✅ Completed | Repo selector bound to project registry |
| CA-5 | ✅ Completed | Branch / mode / status / approvals header badges landed |
| CA-6 | ✅ Completed | Session timeline bound to selected coding thread |
| CA-7 | ✅ Completed | Composer and interrupt controls are live |
| CA-8 | ✅ Completed | Terminal region is bound to the selected thread |
| CA-9 | ✅ Completed | Fixed approval bar added |
| CA-10 | ✅ Completed | Approval detail drawer added |
| CA-11 | ✅ Completed | Changed-files rail landed |
| CA-12 | ✅ Completed | Inline diff viewer landed |
| CA-13 | ✅ Completed | Header `Start task` and `Review` actions made real |
| CA-14 | ✅ Completed | PRD, engineering spec, and ticket plan realigned to shipped work |
| CA-15 | ✅ Completed | Repo/thread binding now stays correct across prompt submit, repo switch, and delayed bootstrap |
| CA-16 | ✅ Completed | Review rail and approval drawer now reconcile against live repo/thread truth |
| CA-17 | ✅ Completed | Keyboard, click, and focus rules now match visible enabled/disabled state |
| CA-18 | ✅ Completed | Header/buttons and empty/disabled states now explain unavailable actions honestly |

---

## Epic

**Title:** `Coding Agent Pane`

**Goal:**
Ship a dedicated `Coding Agent` pane in `apps/autopilot-desktop` that gives
users a local coding-agent workspace with:

- repo/workspace context
- agent timeline
- terminal output
- approvals
- changed-files and inline diff review

all inside one native Autopilot pane.

**V1 success sentence:**

> A user can start a coding task, watch the agent work locally, review changes
> safely, and approve or interrupt execution without leaving the pane.

**Source docs:**

- `docs/plans/coding-agent-pane-prd.md`
- `docs/plans/coding-agent-pane-engineering-spec.md`
- `docs/codex/ROADMAP_CODEX.md`
- `docs/codex/AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`

**V1 scope summary:**

- new singleton floating pane: `Coding Agent`
- local execution only
- one active coding session per pane
- repo selector in header
- visible branch / mode / status / approval count
- fixed approval bar + detail drawer
- agent timeline
- terminal editable only when idle
- changed-files list + inline diff viewer

**Current delivery status:**

The V1 feature surface is now implemented. The remaining work is stabilization,
dogfooding, and polish so the pane behaves reliably in real repo-scoped
workflows.

**Out of scope for this epic:**

- multi-agent pane support
- hosted or remote execution
- full embedded code editor
- background agent queue UX

---

## Phase 1 — Pane Contract And Empty Shell

### CA-1: Add `PaneKind::CodingAgent` and pane registry entry

**Type:** Task  
**Phase:** 1  
**Priority:** P0

**Summary:**  
Register the new pane in the desktop pane system so it can open from the command
palette and behave like a normal Autopilot pane.

**Requirements:**

- Add `PaneKind::CodingAgent`
- Add pane spec in `pane_registry.rs`
- Use title `Coding Agent`
- Register command id `pane.coding_agent`
- Set singleton `true`
- Set startup `false`
- Set default windowed presentation

**Acceptance criteria:**

- Pane opens from command palette
- Pane title renders correctly
- `cargo check -p autopilot-desktop` passes

**Files:**

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_system.rs`

---

### CA-2: Add `CodingAgentPaneState` and default initialization

**Type:** Task  
**Phase:** 1  
**Priority:** P0

**Summary:**  
Introduce pane-local view state without duplicating engine/runtime truth.

**Requirements:**

- Add `CodingAgentPaneState`
- Add minimal companion enums if needed
- Add default initialization in render-state creation
- Keep existing coding-shell truth in `AutopilotChatState`

**Acceptance criteria:**

- Render state initializes cleanly
- No duplicated coding runtime model is introduced
- `cargo check -p autopilot-desktop` passes

**Files:**

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/render.rs`

---

### CA-3: Render an empty `Coding Agent` shell with native pane styling

**Type:** Task  
**Phase:** 1  
**Priority:** P0

**Summary:**  
Add a dedicated pane module and render a truthful empty-state shell.

**Requirements:**

- Add `apps/autopilot-desktop/src/panes/coding_agent.rs`
- Route `PaneKind::CodingAgent` through `pane_renderer.rs`
- Render:
  - native pane shell
  - placeholder header context row
  - empty state explaining repo selection / future pane purpose
- Keep design native to updated pane chrome

**Acceptance criteria:**

- Pane renders without placeholder overflow
- It clearly reads as a workbench shell, not a blank black pane
- `cargo check -p autopilot-desktop` passes

**Files:**

- `apps/autopilot-desktop/src/panes/mod.rs`
- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`

---

## Phase 2 — Repo Context And Header Truth

### CA-4: Add repo selector and bind pane to project registry

**Type:** Task  
**Phase:** 2  
**Priority:** P0

**Summary:**  
Use existing project/workspace identity to let the pane select an active repo.

**Requirements:**

- Expose repo selector in header
- Bind choices to `AutopilotChatState.project_registry`
- Store selected project id in `CodingAgentPaneState`
- Show explicit empty state when no repo exists

**Acceptance criteria:**

- User can select a repo in the pane
- Selected repo remains visible in header
- No-repo state is honest and non-broken

**Files:**

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

### CA-5: Add branch, mode, status, and approvals badges to header

**Type:** Task  
**Phase:** 2  
**Priority:** P0

**Summary:**  
Make the pane chrome honest about the active coding session context.

**Requirements:**

- Show branch badge
- Show mode badge
- Show agent status badge
- Show pending approvals count
- Derive values from `AutopilotChatState`

**Acceptance criteria:**

- Header always surfaces repo-context truth when available
- Badges degrade gracefully when data is missing
- `cargo check -p autopilot-desktop` passes

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`

---

## Phase 3 — Session Timeline And Composer

### CA-6: Add coding session timeline area

**Type:** Task  
**Phase:** 3  
**Priority:** P0

**Summary:**  
Render a coding-session timeline instead of a generic chat transcript.

**Requirements:**

- Project current thread/session into timeline rows
- Distinguish:
  - user prompts
  - agent progress
  - tool/action updates
  - interrupts
  - failures
- Add pane-local scroll state

**Acceptance criteria:**

- A user can understand the current coding session from the timeline alone
- Long timeline content scrolls cleanly

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`

---

### CA-7: Add composer and interrupt controls to the pane

**Type:** Task  
**Phase:** 3  
**Priority:** P0

**Summary:**  
Let users start a coding task and interrupt it from inside the pane.

**Requirements:**

- Add composer input
- Add send action
- Add interrupt action
- Reuse existing underlying chat/coding lane behavior where possible

**Acceptance criteria:**

- User can submit a coding task from the pane
- User can interrupt active execution from the pane
- Visible status updates when actions are triggered

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

## Phase 4 — Terminal Integration

### CA-8: Add terminal region with running vs idle gating

**Type:** Task  
**Phase:** 4  
**Priority:** P0

**Summary:**  
Expose the current terminal session inside the pane and gate user input based on
agent activity.

**Requirements:**

- Render terminal region below timeline
- Use existing `AutopilotTerminalSession`
- Show running/idle state
- Allow user terminal input only when agent is idle
- Add pane-local terminal scroll state

**Acceptance criteria:**

- Terminal output streams into the pane
- User cannot type while agent is running
- User can type when agent is idle

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

## Phase 5 — Approval UX

### CA-9: Add fixed approval bar

**Type:** Task  
**Phase:** 5  
**Priority:** P0

**Summary:**  
Add a fixed approval bar beneath the pane header.

**Requirements:**

- Show when command or file approvals are pending
- Provide primary actions:
  - approve
  - deny
  - inspect
- Keep layout stable when no approvals are pending

**Acceptance criteria:**

- Approval bar becomes visible when pending approvals exist
- User can act without leaving the pane

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

### CA-10: Add approval detail drawer

**Type:** Task  
**Phase:** 5  
**Priority:** P1

**Summary:**  
Allow the user to inspect approval context in a drawer rather than a blocking
modal.

**Requirements:**

- Show request details:
  - type
  - command or file-change target
  - cwd or grant root
  - reason if present
- Support drawer open/close state in `CodingAgentPaneState`

**Acceptance criteria:**

- User can inspect approval details without losing timeline context
- Drawer stays visually native to the pane system

**Files:**

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input.rs`

---

## Phase 6 — Review Rail

### CA-11: Add changed-files rail and file selection

**Type:** Task  
**Phase:** 6  
**Priority:** P0

**Summary:**  
Expose changed files as a first-class review rail on the right side of the pane.

**Requirements:**

- Derive changed-file list from `AutopilotDiffArtifact`
- Render rail with file selection state
- Add rail tab or default view state if needed

**Acceptance criteria:**

- User can scan which files changed
- File selection is stable and preserved while diff data remains current

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input.rs`

---

### CA-12: Add inline patch/diff viewer

**Type:** Task  
**Phase:** 6  
**Priority:** P0

**Summary:**  
Render inline diff/patch content for the selected changed file.

**Requirements:**

- Show diff for selected file
- Support long diffs with local scroll behavior
- Show honest empty state when no diff exists

**Acceptance criteria:**

- User can inspect file-level diff content inside the pane
- Diff viewer does not overlap timeline or terminal surfaces

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`

---

## Recommended implementation order

1. CA-1
2. CA-2
3. CA-3
4. CA-4
5. CA-5
6. CA-6
7. CA-7
8. CA-8
9. CA-9
10. CA-10
11. CA-11
12. CA-12
13. CA-13
14. CA-14
15. CA-15
16. CA-16
17. CA-17
18. CA-18

This ordering keeps each slice narrow and builds visible product value early.

---

## Phase 7 — Stabilization And Dogfood Readiness

### CA-13: Make header `Start task` and `Review` actions real

**Type:** Task  
**Phase:** 7  
**Priority:** P1

**Summary:**  
Replace placeholder header affordances with repo-aware actions that operate on
the selected coding context.

**Status notes:**  
Completed. `Start task` now focuses the pane composer in the selected repo
context, and `Review` issues a repo-scoped review start action.

---

### CA-14: Align planning docs and ticket status with shipped work

**Type:** Task  
**Phase:** 7  
**Priority:** P1

**Summary:**  
Bring the PRD, engineering spec, and ticket plan back into sync with the code
that has already landed.

**Requirements:**

- Update document status from proposed/draft to in-progress
- Mark completed implementation tickets
- Record the additional post-plan slices already delivered
- Make stabilization the explicit next phase

**Acceptance criteria:**

- Planning docs match the actual `Coding Agent` implementation surface
- Ticket file makes current progress and next work obvious

**Files:**

- `docs/plans/coding-agent-pane-prd.md`
- `docs/plans/coding-agent-pane-engineering-spec.md`
- `docs/plans/coding-agent-pane-tickets.md`

---

### CA-15: Repo / thread stabilization pass

**Type:** Task  
**Phase:** 7  
**Priority:** P0

**Summary:**  
Tighten repo-scoped state so pane interactions remain truthful while switching
projects, threads, or artifacts.

**Status notes:**  
Completed. Prompt submission no longer writes a workspace path into
`selected_project_id`, repo switching now clears stale focus, and delayed
thread-bootstrap submission now rebinds the pane to the actual project owning
the new thread instead of the repo that happened to be selected when the thread
finished starting.

**Requirements:**

- Verify repo switching resets only the state that should reset
- Prevent stale thread- or diff-specific UI from surviving a repo change
- Keep repo header truth synchronized with the selected thread

**Acceptance criteria:**

- Repo switches never leave stale review, approval, or terminal state visible
- Selected thread context remains stable and explainable

**Files:**

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

### CA-16: Approval / review synchronization stabilization

**Type:** Task  
**Phase:** 7  
**Priority:** P0

**Summary:**  
Make approval and review surfaces react cleanly as queue state and diff artifacts
change over time.

**Status notes:**  
Completed. The pane now reconciles repo/thread state before paint, closes the
approval drawer when repo-scoped approvals disappear, falls back from `Diff`
when no valid diff artifact remains, and surfaces live review status in the
review summary/header actions.

**Requirements:**

- Close or degrade approval UI honestly when approvals resolve
- Keep right-rail review selection valid as diff artifacts update
- Avoid stale detail content after approval or review state transitions

**Acceptance criteria:**

- Approval drawer and review rail always reflect live repo-scoped truth
- No stale approval or diff details remain visible after state changes

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

---

### CA-17: Focus, keyboard, and terminal/composer stabilization

**Type:** Task  
**Phase:** 7  
**Priority:** P0

**Summary:**  
Harden focus and submit behavior so the pane feels consistent during normal
keyboard-heavy coding workflows.

**Status notes:**  
Completed. The pane now supports `Tab` focus cycling between composer and
terminal when terminal input is available, `Escape` blurs both inputs, and both
mouse hits and `Enter` submission now respect the same enabled-state rules used
by the rendered buttons.

**Requirements:**

- Ensure composer and terminal input never fight for focus
- Preserve honest idle-only terminal behavior
- Make keyboard submission behavior deterministic across pane states

**Acceptance criteria:**

- Focus transitions are predictable
- Terminal input stays locked while the agent is active
- Keyboard submission triggers the intended surface only

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/shortcuts.rs`

---

### CA-18: UI polish and truthful empty / disabled states

**Type:** Task  
**Phase:** 7  
**Priority:** P1

**Summary:**  
Improve the pane’s surface quality now that the core features are present.

**Status notes:**  
Completed. Header actions now use truthful labels such as `Select repo`,
`Reviewing`, and `Refresh review`, the composer disables cleanly when no repo is
available, and terminal empty/locked messaging now explains whether the user
needs to bind a repo thread or wait for the active run to finish.

**Requirements:**

- Review spacing and hierarchy across timeline, terminal, approvals, and review rail
- Ensure empty states are explicit and non-broken
- Ensure disabled actions explain why they are unavailable when appropriate

**Acceptance criteria:**

- Pane feels cohesive during real use
- Empty and disabled states are honest and easy to understand

**Files:**

- `apps/autopilot-desktop/src/panes/coding_agent.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
