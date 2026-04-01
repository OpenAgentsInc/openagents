# PRD: Coding Agent Pane

**Status:** In Progress  
**Date:** 2026-03-31  
**Owner:** `apps/autopilot-desktop`  
**Spec authority:** `docs/PANES.md`, `docs/codex/ROADMAP_CODEX.md`, `docs/codex/AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`

---

## 1. Goal

Add a new `Coding Agent` pane to Autopilot Desktop that gives users a best-in-class local coding-agent workspace inside the existing Rust + `wgpui` application shell.

The pane should feel native to Autopilot rather than a bolted-on terminal clone. It must combine:

- an agent conversation/work log
- a terminal/output surface
- a changed-files and diff review rail
- clear approval controls
- explicit repo, branch, and execution-mode context

The V1 bar is:

**A user can start a coding task, watch the agent work locally, review changes safely, and approve or interrupt execution without leaving the pane.**

### Current implementation status

The `Coding Agent` pane now exists in `autopilot-desktop` with the V1 core
workbench in place:

- floating singleton pane with native chrome
- repo selector and repo-scoped header context
- branch / mode / status / approvals badges
- coding session timeline
- composer and interrupt controls
- terminal region with idle-only input
- fixed approval bar and detail drawer
- changed-files rail and inline diff viewer
- real `Start task` and `Review` actions in the pane header

The next phase of work is no longer feature creation. It is stabilization,
dogfooding, and UX polish so the pane is reliable enough for everyday internal
use.

---

## 2. Problem

Autopilot already has strong coding-related foundations:

- app-owned coding shell direction
- Codex integration surfaces
- thread and workspace context
- review/diff artifacts
- terminal and remote supervision primitives

But there is not yet a dedicated pane designed around the full coding-agent loop.

Today the product experience is still fragmented:

- chat is good for prompting, but not optimized as a coding workbench
- terminal output, approvals, and diffs are not yet presented as one coherent pane
- users who want a CLI-like coding-agent workflow still mentally model the experience as “agent over here, terminal somewhere else, review somewhere else”
- the UX does not yet meet the replacement standard described in `docs/codex/ROADMAP_CODEX.md`

The `Coding Agent` pane should solve that by becoming the dedicated app-owned coding workspace for focused local agent work.

---

## 3. Product principles

### 3.1 Native to Autopilot, not a clone

We should adapt proven patterns from tools like Cursor, Codex, and Claude Code, but the pane must still feel like part of the existing Autopilot product:

- shared pane chrome
- shared HUD typography and controls
- truthful status surfaces
- explicit workspace and repo context
- restrained, operational styling rather than generic IDE chrome

### 3.2 Local-first and explicit

V1 is local-machine-only. The pane must be explicit about:

- which repo is active
- which branch is active
- what mode the agent is in
- whether the agent is running, waiting, blocked, or needs approval
- whether the terminal is agent-controlled or user-editable

### 3.3 Safety without friction

Approvals should be first-class, visible, and quick to act on. Users must be able to:

- inspect what is being requested
- approve or deny with confidence
- understand the current permission mode at all times

### 3.4 One workspace, one job, one clear mental model

V1 should support one active agent session per pane. This keeps the workflow understandable and reduces UI sprawl.

### 3.5 Review is part of the main loop

Diffs and changed files are not secondary diagnostics. They are part of the core coding flow and should stay visible and easy to inspect.

---

## 4. Research synthesis

The strongest patterns across modern coding-agent tools are consistent:

- the active workspace/repo context is always visible
- terminal and agent output are tightly connected
- review/diff surfaces are nearby, not hidden in a different mode
- approval and execution mode are explicit
- the user can interrupt or steer without losing context

What to borrow:

- from Cursor: integrated agent + terminal workflow, visible review surface, clear workspace context
- from Codex: durable thread/session truth, explicit model/session controls, app-owned workflow surfaces
- from Claude Code: explicit local terminal posture, configuration and permission clarity, strong “tooling in place” mental model

What not to copy blindly:

- IDE-heavy chrome that clashes with Autopilot’s pane system
- a web-app visual language that feels foreign inside `wgpui`
- hidden permission models
- multi-agent complexity in V1

---

## 5. V1 scope

### In scope

- new `Coding Agent` pane as a primary floating pane
- one active coding-agent session per pane
- local execution only
- fixed approval bar at top of pane with detail drawer
- repo selector in pane header
- visible workspace, branch, mode, status, and approval count in pane chrome
- split workspace layout with:
  - agent thread
  - terminal/output
  - changed-files / diff rail
- terminal editable only when the agent is idle
- changed-files list plus inline patch/diff viewer
- interrupt action inside the pane

### Out of scope for V1

- multiple concurrent agents in one pane
- remote execution or hosted execution
- collaborative multi-user coding sessions
- full embedded code editor replacing external editors
- worktree orchestration beyond simple visibility/state
- background agent queues as a first-class pane feature
- mobile or web parity

---

## 6. Primary users and jobs to be done

### Primary user

An Autopilot Desktop user who wants a focused, local coding-agent workflow inside the app and prefers not to bounce between separate tools just to prompt, watch execution, inspect diffs, and approve actions.

### Core jobs to be done

1. Start a coding task against a selected repo.
2. Watch the agent think, plan, run commands, and update files.
3. Review changed files and diffs without leaving the pane.
4. Approve, deny, or interrupt execution safely.
5. Drop into the terminal when the agent is idle.
6. Maintain confidence about which repo, branch, and mode are active.

---

## 7. Pane contract

`Coding Agent` is a dedicated coding workspace pane, separate from `Autopilot Chat`.

It is not:

- a generic shell pane
- a replacement for every chat mode
- a hidden diagnostics surface

It is:

- the focused pane for local coding-agent work
- app-owned product UI over the current engine/runtime lane
- the main place where prompt, execution, approvals, and review converge

This separation matters. `Autopilot Chat` can remain a more general assistant/chat surface; `Coding Agent` becomes the deliberate workbench for local software tasks.

---

## 8. Information architecture

### 8.1 Pane chrome

The header should always show:

- pane title: `Coding Agent`
- repo selector
- current branch
- current mode
- agent status
- pending approvals count
- close / pane controls

Recommended ordering:

1. title
2. repo selector
3. branch badge
4. mode badge
5. status badge
6. approvals badge

### 8.2 Approval bar

Directly below the header:

- a fixed approval bar
- concise summary of what is blocked or pending
- primary actions:
  - approve
  - deny
  - inspect details

The approval details should open in a drawer or detail panel rather than taking over the whole pane.

### 8.3 Main body layout

V1 should use a split workspace layout:

- main column
  - agent thread / work log
  - terminal/output below it
- right rail
  - changed files list
  - inline diff / patch viewer

Recommended structure:

- left main area: approximately 65-70%
- right review rail: approximately 30-35%
- within the left main area:
  - thread above
  - terminal below
  - resizable divider if practical

### 8.4 Agent thread area

The thread should show:

- user prompts
- agent updates
- tool/action summaries
- approval checkpoints
- errors and interrupts

This is not just a chat transcript. It is the session timeline.

### 8.5 Terminal/output area

The terminal should:

- stream command output live
- clearly show whether it is agent-controlled or user-editable
- allow user input only when the agent is idle
- preserve history for the current pane/session

### 8.6 Review rail

The right rail should contain:

- changed files list
- current file selection
- inline patch/diff viewer
- summary of file status where available

This rail should support quick scanning first and deeper inspection second.

---

## 9. Interaction model

### 9.1 Session lifecycle

The pane should make the session state explicit:

- idle
- preparing
- running
- waiting for approval
- interrupted
- completed
- failed

The user should never have to guess whether the agent is still active.

### 9.2 Prompting and steering

Users should be able to:

- submit a new coding task
- interrupt current execution
- send follow-up guidance after a run completes

Queued follow-up turns can be deferred beyond V1 if needed, but interruption and restart must be clear.

### 9.3 Terminal control

V1 terminal policy:

- agent owns the terminal while running
- user can read terminal output anytime
- user can type only when the agent is idle
- the UI must clearly reflect this state

### 9.4 Approval model

V1 must support three modes:

1. ask / read-only
2. edit with approval
3. full auto in sandbox

Mode must be visible in header chrome and influence approval behavior honestly.

### 9.5 Review behavior

Users should be able to:

- see which files changed
- select a changed file
- inspect inline diff/patch content
- understand the impact before approving or continuing

Full embedded editor functionality is not required for V1.

---

## 10. UX requirements

### 10.1 Header and context clarity

The repo selector must be easy to discover and efficient to change. Users should not need to leave the pane to understand which repo they are operating on.

### 10.2 Thread readability

The session timeline should favor readability over decorative chat styling:

- clear grouping
- obvious distinction between user turns, agent output, and system/tool states
- compact but scannable status rows

### 10.3 Approval UX

Approvals should feel like a lightweight checkpoint, not a jarring modal interruption.

Desired posture:

- visible when needed
- quiet when not needed
- always accessible
- actionable in one glance

### 10.4 Terminal honesty

The terminal should feel real, not simulated. If the user cannot type because the agent is active, the pane should say so clearly.

### 10.5 Review confidence

The diff rail should help users answer:

- what changed?
- where did it change?
- do I want to allow this?

without losing the thread or terminal context.

### 10.6 Native visual language

The pane should inherit the updated Autopilot language:

- shared floating pane chrome
- dark surfaces
- subtle borders
- mono typography
- Mission Control-class internal section structure where appropriate
- restrained accent usage for states, not decoration

---

## 11. Functional requirements

### FR-1 Repo selection

- The pane must provide a repo selector in the header.
- Switching repos must update visible context before execution begins.
- The repo selector must make the current choice obvious at all times.

### FR-2 Session status

- The pane must show current agent status in the header.
- Status updates must propagate live while a task is executing.

### FR-3 Mode visibility

- The current approval/sandbox mode must be visible in the header.
- Changing mode must update the approval behavior of the pane honestly.

### FR-4 Approval bar

- Pending approvals must appear in a fixed bar near the top of the pane.
- The user must be able to inspect approval details in a drawer.
- Approvals must be actionable without losing session context.

### FR-5 Terminal gating

- Terminal input must be disabled while the agent is running.
- Terminal input must be enabled when the agent is idle.
- The UI must visually distinguish those states.

### FR-6 Review rail

- The pane must show a changed-files list.
- Selecting a file must open an inline diff/patch view.
- Long diffs must be scrollable inside the review surface.

### FR-7 Interrupt behavior

- The user must be able to interrupt active execution from inside the pane.
- Interrupt state must be reflected in the session timeline and status.

### FR-8 One agent per pane

- V1 supports one active coding-agent session per pane.
- Nonessential multi-agent controls are deferred.

---

## 12. Non-functional requirements

- Layout must remain usable at moderate pane widths.
- Scroll behavior must be explicit and stable for:
  - thread area
  - terminal area
  - diff rail
- Pane state should preserve:
  - selected repo
  - current mode
  - scroll position where practical
  - current changed-file selection where practical
- The pane should degrade honestly when prerequisites are missing:
  - no repo selected
  - no agent available
  - local execution unavailable
  - no pending changes

---

## 13. Success metrics

### Qualitative

- Internal users can complete a normal local coding-agent workflow from one pane.
- The pane feels like part of Autopilot rather than a foreign mini-IDE.
- Users report high confidence in repo context, permissions, and review state.

### Quantitative

- user can start a task without leaving the pane: yes
- user can see current repo, branch, mode, and status at all times: yes
- user can approve or interrupt without leaving the pane: yes
- user can inspect changed files and diffs in-pane: yes
- user can tell when terminal input is available: yes

---

## 14. V1 non-goals and deferrals

These are intentionally deferred:

- background agents as first-class pane objects
- multiple parallel sessions in one pane
- repo-wide embedded file explorer
- full in-pane code editing
- PR creation and worktree management as primary-pane controls
- hosted or remote execution
- mobile companion parity

These can become V2 or later expansions once the focused local loop is solid.

---

## 15. Phase plan

### Phase 0: Product and architecture alignment

Define the pane contract and wire it to existing app-owned coding shell direction.

Deliverables:

- pane contract documented in `docs/PANES.md`
- final V1 IA approved
- ownership boundaries aligned with `docs/codex/ROADMAP_CODEX.md`
- engine/runtime seam clarified for local-only execution

### Phase 1: Pane shell and empty-state experience

Ship the visible `Coding Agent` pane shell with no deep execution yet.

Deliverables:

- pane registration and opening path
- native pane chrome
- repo selector in header
- branch, mode, status, and approvals badges
- empty states for:
  - no repo selected
  - ready to start task
  - unavailable engine/runtime

### Phase 2: Session timeline and prompt flow

Turn the pane into a usable session surface.

Deliverables:

- agent thread / session timeline
- composer input
- status transitions in-pane
- interrupt action
- basic session persistence/restoration

### Phase 3: Terminal integration

Add the local terminal as a first-class pane region.

Deliverables:

- streamed terminal/output region
- clear running vs idle control state
- user terminal input only when idle
- scrollback and stable layout behavior

### Phase 4: Approval system

Add the fixed approval bar and approval detail drawer.

Deliverables:

- approval bar
- detail drawer
- ask / read-only mode behavior
- edit with approval mode behavior
- full auto in sandbox mode behavior

### Phase 5: Review rail

Add changed-files list and inline diff/patch review.

Deliverables:

- changed-files rail
- file selection state
- inline diff/patch viewer
- scroll and clipping behavior

### Phase 6: UX polish and replacement-bar testing

Refine the end-to-end workflow until it feels trustworthy and fast.

Deliverables:

- layout tuning
- keyboard and focus polish
- session recovery polish
- clearer blocked/approval states
- dogfood validation against normal local coding tasks

---

## 16. Implementation strategy for Rust + `wgpui`

### Product-layer strategy

Keep the new pane product-owned in `apps/autopilot-desktop`.

Do not move coding-agent orchestration into `crates/wgpui`.

### Likely implementation surfaces

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- new pane-specific module, likely:
  - `apps/autopilot-desktop/src/panes/coding_agent.rs`

### Layout strategy

- compute pane regions in layout first
- keep paint logic separate
- reuse existing pane chrome and shared section styles
- use explicit sub-viewports for:
  - thread
  - terminal
  - diff rail
- avoid paint-only hacks for split layouts

### State strategy

Add app-owned pane state for:

- selected repo
- branch snapshot
- mode
- session status
- approval queue summary
- terminal idle/running state
- changed-file selection
- diff viewer state

---

## 17. Risks and mitigations

### Risk: too much complexity in V1

Mitigation:

- one agent per pane
- local-only execution
- no embedded editor
- focused diff rail instead of full IDE file system

### Risk: pane duplicates Chat too closely

Mitigation:

- keep `Coding Agent` as a separate workbench contract
- optimize for execution + review, not generic conversation

### Risk: review and terminal compete for space

Mitigation:

- explicit three-region IA
- resizable splitters if necessary
- prioritize thread readability and diff usefulness over ornamental layout

### Risk: approval UX becomes modal and disruptive

Mitigation:

- fixed approval bar
- detail drawer instead of modal takeover

---

## 18. Open questions for later phases

- Should V2 allow the pane to dock like Mission Control, or remain floating-only?
- Should V2 support background agents supervised from the same pane family?
- How much git/worktree/PR control belongs in-pane versus separate supporting panes?
- When Probe replaces or supplements Codex, which state transitions remain identical?

These questions do not block V1.

---

## 19. Research references

Official references used to shape this PRD:

- Cursor Agent Terminal: <https://docs.cursor.com/agent/terminal>
- OpenAI Codex app: <https://openai.com/index/introducing-the-codex-app/>
- OpenAI Codex CLI help: <https://help.openai.com/en/articles/11096431>
- Claude Code overview: <https://docs.anthropic.com/es/docs/claude-code/overview>
- Claude Code settings: <https://docs.anthropic.com/en/docs/claude-code/settings>
