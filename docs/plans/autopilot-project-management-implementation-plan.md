# Autopilot Project Management — Implementation Plan

**Status:** Draft execution plan
**Last updated:** 2026-03-07
**Intent:** Define a native, MVP-compatible path to an Autopilot project management subsystem without depending on an external tracker as the starting point.

---

## 1. Guardrails and product position

- The core OpenAgents MVP is still `Go Online -> paid job -> wallet tick up -> withdrawal`. PM work must not derail that loop.
- The first useful version must be a native Autopilot surface, not a process wrapper around another tool.
- Keep Step 0 and Phase 1 behind a default-off feature flag.
- Keep PM product behavior in `apps/autopilot-desktop`; do not move workflow logic into `crates/wgpui`.
- Prefer deterministic command handling, append-only history, and replay-safe projections over ad hoc mutable state.
- Keep wallet and payout truth outside the PM subsystem. PM may reference payment state later, but it must not invent or authoritatively own it.
- Do not make Nostr authoritative in the first shipping slice. Mirror first, then evaluate authority boundaries later.
- Do not begin with importer breadth, workflow automation, or external-suite parity.

---

## 2. What success looks like

This effort is successful when:

1. One internal team can run one real cycle entirely inside a native Autopilot PM surface.
2. Status, ownership, blockers, and next steps are visible without side-channel clarification.
3. Restart/replay preserves PM state correctly and deterministically.
4. Agent-task execution, Nostr sharing, and Bitcoin-linked workflows can extend the same model later instead of replacing it.

---

## 3. Recommended architecture

### 3.1 Ownership boundaries

- `apps/autopilot-desktop/src/project_ops/*`
  - Owns pane wiring, view state, commands, UX flows, and feature gating.
- Optional later extraction into dedicated crates
  - Only after the entity model and reducer contracts stabilize through dogfooding.
  - Good extraction candidates later: reducer/domain types, workflow policy loader, workspace manager, agent-task runner.
- `crates/wgpui`
  - May expose reusable controls used by the PM pane.
  - Must not own PM workflow rules, reducers, or app behavior.

### 3.2 Authority model

For Phases 0-3, use a local desktop authority model:

- `pm_events`
  - Append-only authoritative log of PM commands after validation.
- `pm_*` read models
  - Derived projections for list view, board view, detail view, saved views, and activity summaries.
- Activity and notifications
  - Projections only, never the source of truth.
- Nostr
  - Read-only mirror first.
  - Never the first authoritative home of PM data.
- Wallet/payout state
  - Remains authoritative in wallet/payment systems; PM only references it later.

### 3.3 Runtime shape

- Feature gate: `project_ops`
- One explicit PM service loop that accepts commands and emits deterministic state updates.
- Startup behavior:
  - Load the local store.
  - Replay unapplied events if needed.
  - Rebuild projections before the pane becomes interactive.
- Command handling:
  - Every mutating command gets an idempotency key.
  - Validation happens before event append.
  - UI updates come from projection refresh, not from optimistic mutation alone.

### 3.4 Storage recommendation

Use a local durable store with migrations and indexed query support. The recommended shape is:

- `pm_events`
  - event_id
  - aggregate_type
  - aggregate_id
  - seq
  - command_id
  - event_type
  - payload_json
  - created_at_unix_ms
- `pm_work_items`
  - current projected state for list/detail queries
- `pm_cycles`
  - current cycle definitions and status
- `pm_saved_views`
  - user-scoped filter presets
- `pm_projection_meta`
  - last applied event sequence per projection

The store should support:

- projection rebuild from zero
- projection catch-up from last applied sequence
- stable sorting/filtering
- future sync checkpoints without schema churn

### 3.5 First-slice entities

The native thin slice does not need the full long-term model on day one. Start with:

| Entity | Purpose | Notes |
| --- | --- | --- |
| Work Item | Primary planning/execution record | Core Step 0 entity |
| Cycle | Timeboxed commitment bucket | One active cycle needed for the first pilot |
| Saved View | Reusable filters | Needed for daily usability |
| Activity Event | Human-readable history derived from `pm_events` | Projection, not authority |

Keep these deferred until Phase 3 or later unless Step 0 proves they are immediately necessary:

- full Team entity
- full Project entity
- comments/mentions
- notifications
- agent-task execution metadata
- bounties/payouts

---

## 4. Delivery sequence at a glance

| Phase | Name | Primary outcome |
| --- | --- | --- |
| 0 | Native Internal Dogfood | One team can run a cycle inside a local, native PM pane |
| 1 | Contract and Reducer Freeze | Stable entities, commands, events, and replay rules |
| 2 | Core PM MVP | Work-item CRUD, search, saved views, and board support |
| 3 | Collaboration Layer | Projects, teams, comments, notifications, and multi-team use |
| 4 | Agent Execution and Nostr Mirror | Agent-task runner and read-only Nostr sync |
| 5 | Bitcoin Workflows | Bounties, verification, and payment-linked visibility |
| 6 | Hardening and Reporting | Automation, metrics, permissions, audit, and scale polish |

Detailed Step 0 package: `docs/plans/autopilot-project-management-step-0-dogfood-package.md`

---

## 5. Phase 0 — Native internal dogfood

### Objective

Ship the smallest native PM slice that one internal team can use immediately inside Autopilot Desktop.

### Thin-slice definition

Step 0 should be:

- local-only
- feature-gated
- single-team friendly
- list-first
- restart-safe
- simple enough to build quickly

Step 0 should not include:

- external tracker dependency
- Nostr sync
- comments/mentions
- push notifications
- bounty/payment logic
- autonomous agent execution
- broad importer/exporter work

### Minimum deliverable

Build a `Project Ops` pane in `apps/autopilot-desktop` with:

- work-item create/edit flows
- status changes
- priority changes
- assignee field
- cycle assignment
- blocked state and blocked reason
- list view with saved default filters
- detail panel/editor
- activity timeline sourced from the event log

### Step 0 pane spec

The first shipping surface should be one composed pane with four functional regions:

1. **Project Ops shell**
   - owns the feature gate, pane title, active saved view, and initial ready/error states
   - should make it obvious whether PM data is loading, ready, empty, or needs recovery

2. **Toolbar/context bar**
   - owns saved-view switching, search, quick-create, and active filters
   - should be the only place needed for most view-switching actions

3. **Work-item list pane**
   - owns scanning and selection
   - each row should show at least title, status, priority, assignee, blocked flag, cycle, and updated time
   - should optimize for keyboard navigation and fast triage

4. **Detail/editor pane**
   - owns authoritative editing for the selected work item
   - should surface editable fields, validation failures, and saved state clearly

5. **Activity timeline region**
   - owns human-readable event history for the selected work item
   - should explain state changes without relying on side-channel context

Pane-state requirements for Step 0:

- loading
  - initial store open or projection rebuild
- empty
  - no items match the active view/filter
- error
  - local store or projection failure with a recoverable next step
- stale selection
  - selected item filtered out, archived, or deleted from the active view

The Step 0 pane should stay list-first. Do not start with multiple tabs, nested workspaces, or a broad pane hierarchy.

### Required Step 0 tasks

1. **Freeze the thin-slice data model.**
   - Define the exact Step 0 fields.
   - Decide which fields are editable vs derived.
   - Keep Team and Project as plain identifiers first, not rich entities.

2. **Define commands and events.**
   - Commands: create, edit fields, change status, assign, set cycle, set blocked, clear blocked, archive/cancel.
   - Events: one append-only event per accepted state change.
   - Record idempotency and timestamps for every command.

3. **Implement the local durable store.**
   - Add migrations.
   - Add event append/replay support.
   - Add projection rebuild support.

4. **Implement a PM reducer/service layer.**
   - Validate transitions.
   - Emit deterministic events.
   - Rebuild projections in a stable order.

5. **Implement the first pane.**
   - Saved-view selector.
   - Search/filter bar.
   - Work-item list.
   - Detail/editor panel.
   - Quick-create flow.

6. **Implement default saved views.**
   - My Work
   - Current Cycle
   - Blocked
   - Backlog
   - Recently Updated

7. **Add pilot instrumentation.**
   - Count commands by type.
   - Count state transitions.
   - Record restart/replay time.
   - Record search/filter usage.

8. **Run one real internal cycle.**
   - Use one pilot team only.
   - Record friction immediately.
   - Promote only proven needs into later phases.

### Deliverables

- A feature-gated `Project Ops` pane
- Local store and replay-safe projections
- One pilot cycle completed natively
- A short friction log with concrete follow-up work

### Exit criteria

- One team can create, triage, update, and complete work without leaving the native surface.
- Restarting the app preserves state and activity history correctly.
- The team can answer: what is active, what is blocked, who owns it, and what should happen next.
- The top missing native features are known from real use, not speculation.

---

## 6. Phase 1 — Contract and reducer freeze

### Objective

Turn Step 0 learnings into a stable contract before broadening scope.

### Required outputs

1. **Canonical entity definitions.**
   - Work Item
   - Cycle
   - Saved View
   - Activity Event
   - Deferred placeholders for Comment, Team, Project, Agent Task, Bounty

2. **Canonical state machines.**
   - Work item: `backlog -> todo -> in_progress -> in_review -> done -> cancelled`
   - Blocked is a flag/reason, not a separate terminal state.
   - Archive behavior is explicit and separate from completion.

3. **Command contract.**
   - Input validation rules
   - Idempotency behavior
   - Error taxonomy
   - Transition rejection rules

4. **Event contract.**
   - Exact event names
   - Required payload fields
   - Versioning approach
   - Replay ordering guarantees

5. **Projection contract.**
   - List projection
   - Board projection
   - Detail projection
   - Activity projection
   - Saved-view projection

6. **Test fixtures.**
   - Golden event streams
   - Projection rebuild fixtures
   - Invalid transition fixtures

### Exit criteria

- The reducer/event model can be implemented and tested without reopening product-shape debates.
- Projection rebuild from a clean store produces byte-stable or semantically stable results.
- The app team knows what stays local, what is derived, and what is deferred.

---

## 7. Phase 2 — Core PM MVP

### Objective

Expand Step 0 into a broadly usable PM MVP for daily engineering work.

### Scope

- full work-item CRUD
- richer search/filter support
- board view
- bulk actions
- saved personal views
- stronger editor UX

### Required tasks

1. **Complete work-item editing.**
   - title, description, priority, assignee, cycle, parent, due date, tags/areas

2. **Add board support.**
   - one lane per workflow state
   - drag/drop status change
   - lane counts and WIP visibility

3. **Add search and filter syntax.**
   - text search
   - `state:`
   - `assignee:`
   - `priority:`
   - `cycle:`
   - `blocked:`
   - `tag:`

4. **Add bulk actions.**
   - bulk status change
   - bulk assign
   - bulk cycle assignment
   - bulk archive/cancel

5. **Strengthen UX polish.**
   - keyboard-first create/edit flows
   - empty/loading/error states
   - obvious dirty-state handling
   - consistent success/failure feedback

6. **Expand validation coverage.**
   - replay after abrupt shutdown
   - projection rebuild after migration
   - filter correctness
   - drag/drop transition correctness

### Exit criteria

- The pilot team prefers the native surface to side-channel tracking.
- List and board views cover most daily triage/execution behavior.
- Search and saved views materially reduce time spent hunting for work.

---

## 8. Phase 3 — Collaboration layer

### Objective

Support multi-team coordination without losing the simplicity of the thin slice.

### Scope

- Project entity
- Team entity
- comments
- mentions
- notifications
- cycle planning across teams

### Required tasks

1. **Promote Team and Project to real entities.**
   - definitions
   - ownership rules
   - defaults

2. **Add comments and mentions.**
   - append-only comment events
   - edit/delete policy
   - mention parsing
   - comment activity projection

3. **Add notifications.**
   - assignment changes
   - mentions
   - due-date reminders
   - blocked/unblocked changes

4. **Add project-level views.**
   - backlog
   - current cycle
   - blocked items
   - recently changed

5. **Run a multi-team pilot.**
   - validate cross-team dependency handling
   - validate notification signal/noise
   - validate cycle planning behavior

### Exit criteria

- Multiple teams can coordinate in one system without inventing separate process layers.
- Comments and notifications reduce status-chasing instead of amplifying noise.

### Pane additions in this phase

- `Project Overview`
  - summary pane for project backlog, cycle status, and blocked work
- `Project Detail`
  - scoped list/board view for a single project
- `Inbox/Notifications`
  - assignment, mention, due-date, and blocked-change feed

---

## 9. Phase 4 — Agent execution and Nostr mirror

### Objective

Add OpenAgents-native execution behavior after the human PM system is stable.

### Scope

- agent-task specialization
- workspace management
- workflow policy loading
- Codex runner integration
- read-only Nostr mirroring

### Required tasks

1. **Define the Agent Task state machine.**
   - `draft`
   - `ready`
   - `claimed`
   - `running`
   - `awaiting_verification`
   - `completed`
   - `failed`
   - `cancelled`

2. **Build the execution contracts.**
   - required inputs
   - expected outputs
   - verification fields
   - retry policy
   - timeout policy

3. **Build a workspace manager.**
   - root containment checks
   - stable workspace identifiers
   - cleanup policy
   - retention policy for logs/artifacts

4. **Add workflow policy loading.**
   - repo-owned workflow contract
   - typed parsing
   - strict validation
   - reload semantics

5. **Integrate a runner using `crates/codex-client`.**
   - use a dedicated worker runtime
   - keep execution snapshots/events separate from pane-local state
   - surface run summaries back into PM projections

6. **Add reconciliation and recovery.**
   - stalled run detection
   - retry/backoff
   - restart recovery
   - explicit operator controls

7. **Add read-only Nostr mirror.**
   - publish mirrored PM events or derived records
   - replay and duplicate handling
   - local state remains authoritative

### Exit criteria

- Agent tasks can run through a controlled, replay-safe lifecycle.
- The Nostr mirror shares PM state without becoming the first source of truth.

### Pane additions in this phase

- `Agent Queue`
  - shows draft, ready, claimed, running, failed, and awaiting-verification agent tasks
- `Agent Run Detail`
  - shows workspace, attempt history, logs, artifacts, and operator controls
- `Mirror Status`
  - shows Nostr mirror health, replay lag, publish failures, and duplicate handling summaries

---

## 10. Phase 5 — Bitcoin workflows

### Objective

Attach money flows only after PM state and agent execution are trustworthy.

### Scope

- bounty attachment
- funding status
- verification-gated release
- payout visibility

### Required tasks

1. **Define the bounty state machine.**
   - `unfunded`
   - `funding_pending`
   - `funded`
   - `claimed`
   - `verification_pending`
   - `released`
   - `refunded`
   - `failed`

2. **Define authority boundaries.**
   - PM references payment truth.
   - PM does not own wallet balances or payment finality.
   - Failures must be explicit and user-visible.

3. **Attach bounties to work items.**
   - amount
   - funding source reference
   - claimant
   - verifier

4. **Add payout visibility.**
   - badges
   - history
   - failure reasons
   - audit trail

5. **Pilot under tight control only.**
   - internal or design-partner use
   - manual approval first
   - clear rollback posture

### Exit criteria

- Payment-linked completion is understandable, honest, and auditable.
- PM never obscures whether money actually moved.

### Pane additions in this phase

- `Bounty Detail`
  - shows amount, claimant, verifier, funding state, and payout history
- `Settlement History`
  - shows payment-linked lifecycle records for work items with bounties

---

## 11. Phase 6 — Hardening and reporting

### Objective

Make the subsystem operationally trustworthy at larger internal scope.

### Required tasks

1. **Metrics and reporting.**
   - throughput
   - lead time
   - cycle time
   - blocker count
   - agent-task success/failure rate

2. **Automation.**
   - optional status suggestions from repo activity
   - reminder policies
   - stale-item detection

3. **Permissions and audit.**
   - role model
   - audit export
   - change history views

4. **Performance hardening.**
   - large-board behavior
   - projection rebuild time
   - search responsiveness
   - offline/restart performance

### Exit criteria

- Operators can trust both execution and reporting.
- The system remains fast and legible as usage grows.

### Pane additions in this phase

- `Reporting`
  - cycle summaries, throughput, lead time, and blocker trends
- `Audit`
  - append-only change visibility and export surfaces
- `Operations`
  - projection rebuild health, migration status, and performance diagnostics

---

## 12. Features to defer explicitly

- importer breadth
- advanced workflow automation
- spreadsheet/grid view
- Gantt/dependency charting
- mobile-first PM surface
- public marketplace reputation layers
- Nostr-authoritative PM storage
- autonomous payout release without strong verification

These do not belong in active implementation until the earlier phases are proven.

---

## 13. Recommended first 10 implementation tickets

1. Add `project_ops` feature gate and pane entry in `apps/autopilot-desktop`.
2. Define Step 0 work-item schema, command types, and event types.
3. Add local PM store migrations and projection metadata tables.
4. Implement reducer/service layer with deterministic transition validation.
5. Implement quick-create work-item flow.
6. Implement list view with default saved views.
7. Implement detail/editor panel with status and priority changes.
8. Implement activity projection and timeline rendering.
9. Add restart/replay regression tests for PM projections.
10. Run one pilot cycle and capture a friction log for Phase 1 contract freeze.

---

## 14. Final recommendation

Start with a native, local, feature-gated PM slice inside `apps/autopilot-desktop`, not a process workaround and not a broad external-suite replacement effort.

The key discipline is:

1. ship the smallest useful pane,
2. make its state replay-safe,
3. dogfood it for one real cycle,
4. only then expand into comments, agent execution, Nostr sharing, and Bitcoin-linked workflows.

That path stays aligned with the repo's MVP constraints, respects ownership boundaries, and gives the PM subsystem a real architectural spine instead of a temporary detour.
