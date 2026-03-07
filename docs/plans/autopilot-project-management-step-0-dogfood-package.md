# Autopilot Project Management — Step 0 Native Dogfood Package

**Status:** Ready for implementation
**Scope:** Step 0 only — native internal dogfood slice inside `apps/autopilot-desktop`
**Goal:** Let one internal team manage a real cycle in a native PM surface without depending on an external tracker.

---

## 1. Step 0 definition

### What Step 0 is

Step 0 is the smallest native PM subsystem that is still worth dogfooding:

- one feature-gated `Project Ops` pane
- one local authoritative store
- one work-item model
- one active internal pilot team
- one real cycle of use

Its job is not to solve all project management. Its job is to prove that the native surface can answer the daily questions that matter:

- What are we doing now?
- What is blocked?
- Who owns it?
- What should happen next?
- What finished this cycle?

### What Step 0 is not

- not a full collaboration suite
- not a network sync project
- not an agent runner
- not a payout system
- not a permissions/governance project
- not an excuse to postpone MVP-critical work

---

## 2. Required domain shape

### 2.1 Work-item fields

| Field | Required | Notes |
| --- | --- | --- |
| `work_item_id` | yes | Stable internal ID |
| `title` | yes | One-line outcome-oriented title |
| `description` | yes | Short problem statement or expected outcome |
| `status` | yes | One of the canonical workflow states |
| `priority` | yes | `urgent`, `high`, `medium`, `low`, `none` |
| `assignee` | no | Single owner for Step 0 |
| `team_key` | yes | String key only in Step 0 |
| `cycle_id` | no | Optional current-cycle assignment |
| `parent_id` | no | Optional parent work item |
| `area_tags` | no | Zero to two tags |
| `blocked_reason` | no | Empty unless blocked |
| `due_at_unix_ms` | no | Optional external commitment |
| `created_at_unix_ms` | yes | Derived at creation |
| `updated_at_unix_ms` | yes | Derived on change |
| `archived_at_unix_ms` | no | Separate from done/cancelled |

Keep Step 0 fields intentionally narrow. If a field does not change prioritization, execution, or visibility in the pilot, it does not belong in this slice.

### 2.2 Canonical workflow

Use one shared workflow only:

`backlog -> todo -> in_progress -> in_review -> done -> cancelled`

Definitions:

- `backlog`
  - captured, not yet committed
- `todo`
  - accepted into the active cycle and ready to start
- `in_progress`
  - actively being worked by one owner
- `in_review`
  - work complete enough for verification/review
- `done`
  - accepted and complete
- `cancelled`
  - intentionally dropped or superseded

Blocked is not a separate workflow state in Step 0. It is a flag plus a reason on top of the primary status.

### 2.3 Priority scale

- `urgent`
  - directly blocks the MVP earn loop, correctness, or immediate team execution
- `high`
  - should land in the current or next cycle
- `medium`
  - valuable, but not ahead of MVP-critical work
- `low`
  - useful cleanup or enhancement
- `none`
  - captured only; not actively prioritized

### 2.4 Required commands

Step 0 commands:

- `CreateWorkItem`
- `EditWorkItemFields`
- `ChangeWorkItemStatus`
- `AssignWorkItem`
- `ClearAssignee`
- `SetWorkItemCycle`
- `ClearWorkItemCycle`
- `SetBlockedReason`
- `ClearBlockedReason`
- `SetParentWorkItem`
- `ClearParentWorkItem`
- `ArchiveWorkItem`
- `UnarchiveWorkItem`

Every mutating command should include:

- `command_id`
- `issued_at_unix_ms`
- actor identity or actor label

### 2.5 Required events

Step 0 authoritative events:

- `WorkItemCreated`
- `WorkItemFieldsEdited`
- `WorkItemStatusChanged`
- `WorkItemAssigned`
- `WorkItemAssigneeCleared`
- `WorkItemCycleSet`
- `WorkItemCycleCleared`
- `WorkItemBlocked`
- `WorkItemUnblocked`
- `WorkItemParentSet`
- `WorkItemParentCleared`
- `WorkItemArchived`
- `WorkItemUnarchived`

If one accepted user action causes more than one logical state change, emit multiple explicit events rather than hiding changes inside a generic blob.

---

## 3. UI package definition

### 3.1 Pane layout

The Step 0 pane should be list-first and fast to scan:

- top toolbar
  - view selector
  - search input
  - filter chips
  - quick-create button
- left column
  - work-item list for the selected view
- right column
  - detail/editor panel for the selected item
- optional lower section in the detail panel
  - activity timeline

### 3.2 Pane descriptions

#### `Project Ops` shell

Purpose:

- contain the full Step 0 workflow in one place
- make the currently selected view and item obvious

Must show:

- pane title
- active saved view
- active cycle indicator if one exists
- quick-create entry point

Must support these states:

- loading initial snapshot
- ready with selected view
- empty with no work items
- recoverable store/projection error

#### Top toolbar

Purpose:

- let the user change context without leaving the pane

Must show:

- saved-view selector
- text search input
- active filter chips
- quick-create button

Must support:

- clearing search
- switching views without losing unsaved detail edits silently
- showing active filter count when filters are applied

#### Work-item list pane

Purpose:

- provide the fastest scanning surface for active work

Must show per row:

- title
- status
- priority
- assignee if present
- blocked indicator
- cycle if present
- updated time

Must support:

- row selection
- stable sort order
- keyboard navigation
- empty state per selected view

List empty-state copy should be specific to the active view:

- `My Work`
  - no assigned work right now
- `Blocked`
  - no blocked work
- `Backlog`
  - no captured backlog items

#### Work-item detail/editor pane

Purpose:

- provide the authoritative editing surface for one selected work item

Must show:

- title
- description
- status
- priority
- assignee
- cycle
- parent link if present
- blocked reason if present
- created and updated timestamps

Must support:

- inline field editing
- explicit save/apply behavior or autosave with visible success/failure
- rejecting invalid state transitions clearly
- preserving unsaved text during transient projection refresh

If no row is selected, this pane should show a useful placeholder instead of blank chrome.

#### Activity timeline region

Purpose:

- explain what changed and when without relying on memory

Must show:

- create event
- field edits
- status changes
- assignee changes
- cycle changes
- blocked/unblocked events
- archive/unarchive events

Must support:

- newest-first order
- human-readable labels
- actor label if available
- timestamp per event

This region may collapse by default on smaller layouts, but it must remain reachable from the selected item.

### 3.3 Required default views

Ship these built-in views:

- `My Work`
  - assigned to me, not done/cancelled
- `Current Cycle`
  - items in the active cycle
- `Blocked`
  - items with a blocked reason
- `Backlog`
  - uncommitted items
- `Recently Updated`
  - most recently changed active items

Step 0 should also support one custom saved view per pilot user.

### 3.4 Required create/edit flows

- Quick create
  - title
  - description
  - priority
  - status defaults to `backlog`
- Detail editor
  - edit title/description
  - change status
  - change priority
  - assign/clear assignee
  - set/clear cycle
  - set/clear blocked reason
  - set/clear parent

### 3.5 Required pane states

Every Step 0 pane region should have defined behavior for:

- loading
  - waiting on initial store open or projection rebuild
- empty
  - no items match the current view/filter
- error
  - store open failure, projection failure, or rejected save
- stale selection
  - selected item was archived or filtered out

Expected behavior:

- loading should block destructive actions until the current snapshot is ready
- empty should present the next useful action
- error should explain whether retry or rebuild is possible
- stale selection should move focus predictably to the next valid row or to the empty placeholder

### 3.6 UX rules

- All state changes should be visible in the activity timeline.
- The pane should never silently drop edits.
- Invalid transitions should explain why they were rejected.
- Empty states should make the next action obvious.
- Step 0 should optimize for keyboard flow before animation or visual flourish.

---

## 4. Persistence and replay contract

### 4.1 Authoritative storage

Use a local durable store with:

- append-only PM event history
- projected current-state tables
- projection metadata

The write path is:

1. UI issues command.
2. Service validates command.
3. Service appends one or more events.
4. Projections update in deterministic sequence order.
5. UI re-renders from projections.

### 4.2 Replay behavior

On app launch:

1. Open the PM store.
2. Detect projection version.
3. Rebuild stale projections from `pm_events` if needed.
4. Publish a ready snapshot to the pane.

Rebuild rules:

- same event stream must produce the same projected state
- duplicate command IDs must not duplicate state changes
- partial projection failure must be recoverable by replay

### 4.3 Projection requirements

Step 0 requires these projections:

- work-item list projection
- work-item detail projection
- activity timeline projection
- saved-view projection
- cycle summary projection

### 4.4 Migration rules

- Never rewrite old PM events in place.
- Projection schema may change across migrations.
- Event payloads may version forward, but replay of older versions must remain explicit and test-covered.

---

## 5. Pilot operating rules

### 5.1 Pilot scope

- one internal team only
- one active cycle
- no broad rollout until exit criteria are met

### 5.2 Daily operating expectations

The pilot team should use the native pane as the primary PM surface for the selected work slice.

Required hygiene:

- every active work item has a clear owner or is explicitly unassigned
- every blocked item has a blocked reason
- every `in_progress` item has a visible next step in its description
- carry-over should be rare and justified

### 5.3 Work triage rules

Prioritize in this order:

1. urgent bugs
2. blocked items that unblock other work
3. committed cycle items
4. near-term MVP work
5. backlog and research

### 5.4 End-of-cycle review questions

At the end of the pilot cycle, answer:

1. Which fields were used every day?
2. Which fields were ignored?
3. Which status transitions were confusing?
4. Which missing features actually slowed the team down?
5. Did restart/replay behavior ever undermine trust?

Only proven answers should shape Phase 1 and Phase 2.

---

## 6. Test matrix

| Layer | What must be tested |
| --- | --- |
| reducer | valid transitions, invalid transitions, idempotency, blocked semantics |
| persistence | event append, projection catch-up, rebuild from zero, migration behavior |
| pane state | create/edit flows, selection behavior, dirty-state handling |
| search/filter | built-in views, text search, blocked filter, cycle filter |
| recovery | restart with unapplied events, duplicate command replay, corrupted projection rebuild |

Minimum regression set:

- creating a work item yields a visible list row and detail view
- status changes appear in both current state and activity timeline
- blocked items surface in the blocked view
- replay from a clean store reproduces the same visible state
- archive/unarchive does not lose event history

---

## 7. Implementation checklist

1. Add `project_ops` feature gate and pane entry point.
2. Create `project_ops` module structure in `apps/autopilot-desktop`.
3. Define Step 0 entity, command, and event types.
4. Add local store migrations and projection metadata.
5. Implement reducer/service layer.
6. Implement quick-create flow.
7. Implement list and detail projections.
8. Implement built-in saved views.
9. Implement activity timeline projection.
10. Add replay/rebuild regression tests.
11. Enable for pilot users only.
12. Run one real cycle and capture friction.

---

## 8. Exit criteria and go/no-go

Step 0 is successful if:

- one team completes one real cycle in the native pane
- the team can identify active, blocked, and completed work without side-channel tracking
- state survives restart and replay without ambiguity
- the missing features list is short and concrete

Do not move to broad Phase 2 build-out if:

- replay behavior is still untrustworthy
- the pane is materially slower than the team's current habit loop
- the pilot reveals that the schema is still too speculative

The correct output of Step 0 is not "more features". The correct output is a trusted thin slice plus a short list of earned follow-ups.
