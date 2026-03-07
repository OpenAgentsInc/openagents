# Autopilot Project Management — Step 0 Native Dogfood Package

**Status:** Ready for implementation
**Scope:** Step 0 only — native internal dogfood slice inside `apps/autopilot-desktop`
**Goal:** Let one internal team manage a real cycle in a native PM surface using canonical PM stream ids, local projection caches, and the repo's existing replay/checkpoint discipline.

---

## 1. Step 0 definition

### What Step 0 is

Step 0 is the smallest native PM subsystem that is still worth dogfooding:

- one feature-gated `Project Ops` pane
- one local set of PM projection documents
- one shared checkpoint path using the existing sync apply engine
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
- not a live multi-user Spacetime cutover
- not a separate bespoke PM database
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

Step 0 explicit accepted events:

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

### 2.6 Step 0 PM stream catalog

Step 0 should use these canonical stream ids:

| Stream id | Purpose |
| --- | --- |
| `stream.pm.work_items.v1` | Current work-item state and list projection |
| `stream.pm.activity_projection.v1` | Human-readable item history and state-change feed |
| `stream.pm.cycles.v1` | Active cycle definitions and summaries |
| `stream.pm.saved_views.v1` | Built-in and user-defined saved views |

These stream ids should appear in:

- projection documents
- source badges where appropriate
- checkpoint rows
- later stream grants when live sync is introduced

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
- truthful source badge for the active PM surface

Must support these states:

- loading initial snapshot
- ready with selected view
- empty with no work items
- recoverable projection or apply error

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
  - waiting on initial projection load or rebuild
- empty
  - no items match the current view/filter
- error
  - projection load failure, apply failure, or rejected save
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

### 4.1 Local storage shape

Step 0 should persist local PM state as projection documents, not as a bespoke PM relational store:

- `autopilot-pm-work-items-projection-v1.json`
- `autopilot-pm-activity-projection-v1.json`
- `autopilot-pm-cycles-v1.json`
- `autopilot-pm-saved-views-v1.json`

Each projection document should include:

- `schema_version`
- `stream_id`
- normalized projection rows or snapshot payload

Checkpoint progress should use the existing shared sync checkpoint file managed by `SyncApplyEngine`.

### 4.2 Step 0 write path

The write path is:

1. UI issues PM command.
2. PM service validates command.
3. PM service emits one or more explicit accepted PM events.
4. PM projection documents update in deterministic stream order.
5. Shared checkpoint state advances per PM stream.
6. UI re-renders from projections.

The Step 0 implementation should mirror current Spacetime contracts as closely as practical:

- explicit stream ids
- ordered per-stream sequence numbers
- duplicate command handling via idempotency
- deterministic replay behavior

### 4.3 Replay behavior

On app launch:

1. Load PM projection documents.
2. Load shared checkpoints.
3. Validate `schema_version` and `stream_id`.
4. Rebuild stale or invalid PM projections if needed.
5. Publish a ready snapshot to the pane.

Rebuild rules:

- the same accepted event stream must produce the same projected state
- duplicate command IDs must not duplicate state changes
- partial projection failure must be recoverable by replay or rebuild

### 4.4 Projection requirements

Step 0 requires these projections:

- work-item list projection
- work-item detail projection
- activity timeline projection
- saved-view projection
- cycle summary projection

### 4.5 Migration rules

- Never rewrite old PM events in place.
- Projection schema may change across migrations.
- Event payloads may version forward, but replay of older versions must remain explicit and test-covered.
- Stream ids must stay stable unless a deliberate versioned cutover is performed.

### 4.6 Live Spacetime alignment

Step 0 is not the live collaboration cutover, but it must stay aligned with it:

- PM stream ids should be reusable later as stream grants.
- PM sequence and checkpoint behavior should match the repo's existing apply engine expectations.
- Source badges must not imply live Spacetime authority before it exists.
- If remote checkpoints are later hydrated for PM streams, local Step 0 behavior must remain deterministic and restart-safe.

### 4.7 Phase 1 badge, grant, and checkpoint rules

Use these exact truth rules in Step 0 and Phase 1:

- Primary pane badge
  - `source: stream.pm.work_items.v1`
  - use for visible PM list/detail state coming from local PM projection documents
- Sync/bootstrap diagnostics badge
  - `source: spacetime.sync.lifecycle`
  - use only for sync lifecycle, grant failures, checkpoint hydration, and rebootstrap diagnostics
- Reserved PM stream grants for later bootstrap wiring
  - `stream.pm.work_items.v1`
  - `stream.pm.activity_projection.v1`
  - `stream.pm.cycles.v1`
  - `stream.pm.saved_views.v1`
- Checkpoint rules
  - duplicate `seq <= checkpoint` is dropped
  - out-of-order delivery requires explicit rebootstrap or checkpoint rewind
  - stale cursor resumes from `max(local_checkpoint, remote_head - stale_clamp_window)`
  - remote checkpoint adoption only happens when it moves the local checkpoint forward
- Live-vs-local truth
  - PM work-item values stay local/replay-safe in Phase 1.
  - Live remote PM reducers/subscriptions are later work only after ADR approval.

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
5. Did replay, rebuild, or checkpoint behavior ever undermine trust?

Only proven answers should shape later phases.

### 5.5 Pilot-driven simplifications and rejection rules

The pilot did not justify reopening these yet:

- rich `Team` or `Project` entities
- comments, mentions, notifications
- estimates, story points, custom fields, or custom workflow states
- multi-assignee ownership
- money-authoritative bounty or payout logic inside PM

Stable Step 0 and Phase 1 rejection codes:

- `project_ops.invalid_command`
- `project_ops.work_item_exists`
- `project_ops.work_item_missing`
- `project_ops.invalid_transition`
- `project_ops.dependency_missing`
- `project_ops.archived_mutation`
- `project_ops.noop_mutation`
- `project_ops.checkpoint_conflict`

Required rejection cases to preserve:

- same-status change rejects as no-op
- blocked is not a workflow state
- archived items reject mutation until unarchived
- missing cycle or parent references reject clearly
- self-parenting rejects clearly
- out-of-order projection apply surfaces a checkpoint conflict instead of silently rewriting state

---

## 6. Test matrix

| Layer | What must be tested |
| --- | --- |
| reducer | valid transitions, invalid transitions, idempotency, blocked semantics |
| persistence | projection persist/load, rebuild from zero, migration behavior |
| checkpointing | duplicate handling, out-of-order detection, checkpoint advance, restart reload |
| pane state | create/edit flows, selection behavior, dirty-state handling |
| search/filter | built-in views, text search, blocked filter, cycle filter |
| recovery | restart with stale projections, duplicate command replay, corrupted projection rebuild |

Minimum regression set:

- creating a work item yields a visible list row and detail view
- status changes appear in both current state and activity timeline
- blocked items surface in the blocked view
- replay from a clean state reproduces the same visible state
- archive/unarchive does not lose event history
- duplicate commands do not duplicate projection state
- checkpoint reload after restart preserves per-stream progress

---

## 7. Implementation checklist

1. Add `project_ops` feature gate and pane entry point.
2. Create `project_ops` module structure in `apps/autopilot-desktop`.
3. Define Step 0 entity, command, event, and stream-id types.
4. Add PM projection documents and persistence helpers.
5. Integrate PM sequence/checkpoint behavior with the shared apply engine.
6. Implement reducer/service layer.
7. Implement quick-create flow.
8. Implement list and detail projections.
9. Implement built-in saved views.
10. Implement activity timeline projection.
11. Add replay/rebuild regression tests.
12. Enable for pilot users only.
13. Run one real cycle and capture friction.

---

## 8. Exit criteria and go/no-go

Step 0 is successful if:

- one team completes one real cycle in the native pane
- the team can identify active, blocked, and completed work without side-channel tracking
- visible state survives restart and replay without ambiguity
- the PM pane uses canonical stream ids and shared checkpoint discipline
- the missing features list is short and concrete

Do not move to broad Phase 2 build-out if:

- replay or rebuild behavior is still untrustworthy
- the pane is materially slower than the team's current habit loop
- the stream/checkpoint contract is still unstable
- the pilot reveals that the schema is still too speculative

The correct output of Step 0 is not "more features". The correct output is a trusted thin slice plus a short list of earned follow-ups.

---

## 9. Suggested GitHub issues for this plan

This Step 0 package maps to the following GitHub issues. The broader full-implementation backlog lives in `docs/plans/autopilot-project-management-implementation-plan.md`.

| Suggested issue title | Summary |
| --- | --- |
| `[PM P0] Add project_ops feature gate and pane shell` | Register the `Project Ops` pane behind a default-off feature gate so it can be developed without changing the default MVP experience. |
| `[PM P0] Define Step 0 work-item schema and workflow enums` | Add the first work-item domain model, workflow enum set, blocked semantics, priority model, and field constraints described in this plan. |
| `[PM P0] Define Step 0 PM commands, events, and stream ids` | Implement the authoritative command/event types and canonical PM stream ids, including `command_id`, timestamps, actor metadata, and explicit event names. |
| `[PM P0] Add local PM projection documents and stream persistence` | Create the persisted PM projection documents for work items, activity, cycles, and saved views with stable `schema_version` and `stream_id` contracts. |
| `[PM P0] Integrate PM with sync_apply_engine checkpoints` | Reuse the shared checkpoint engine for PM streams so restart recovery, duplicate handling, and out-of-order detection match the rest of the app. |
| `[PM P0] Implement PM reducer/service loop` | Build the mutation path that validates commands, emits explicit events, updates projections, and rejects invalid transitions clearly. |
| `[PM P0] Build built-in saved views and search/filter toolbar` | Add My Work, Current Cycle, Blocked, Backlog, and Recently Updated plus the toolbar used to switch views and search. |
| `[PM P0] Implement work-item list projection and selection behavior` | Build the left-column list with stable sorting, row selection, view-specific empty states, and keyboard-friendly navigation. |
| `[PM P0] Implement work-item detail editor and quick-create flow` | Add the primary editing surfaces for title, description, status, priority, assignee, cycle, parent, and blocked reason. |
| `[PM P0] Implement activity timeline and pane-state handling` | Show create/edit/status/assignment/block/archive history as human-readable events for the selected work item and make load/error states predictable. |
| `[PM P0] Add pilot instrumentation and run one internal PM cycle` | Record command counts, view usage, rebuild/checkpoint timing, and qualitative friction so later phases only promote features the pilot earned. |
