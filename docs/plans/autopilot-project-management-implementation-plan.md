# Autopilot Project Management — Implementation Plan

**Status:** Draft execution plan
**Last updated:** 2026-03-07
**Intent:** Define a native, MVP-compatible path to an Autopilot project management subsystem that fits the repo's current Spacetime rollout instead of inventing a separate authority stack.

---

## 1. Guardrails and product position

- The core OpenAgents MVP is still `Go Online -> paid job -> wallet tick up -> withdrawal`. PM work must not derail that loop.
- The first useful version must be a native Autopilot surface in `apps/autopilot-desktop`, not a process wrapper around another tool.
- Keep Step 0 and early PM work behind a default-off `project_ops` feature gate.
- Keep PM product behavior in `apps/autopilot-desktop`; do not move workflow logic into `crates/wgpui`.
- Align PM storage and replay with the repo's current Spacetime Phase 1 semantics:
  - canonical stream ids
  - deterministic `(stream_id, seq)` apply discipline
  - checkpoint hydration and stale-cursor recovery
  - truthful source badges
- Do not introduce a parallel bespoke PM authority model such as `pm_events` plus unrelated metadata tables if the same job can be done with the existing stream/checkpoint architecture.
- Any move from local PM projection caches to live remote Spacetime authority requires an ADR update first. `ADR-0001` does not currently grant PM collaboration domains authority by default.
- Keep wallet and payout truth outside the PM subsystem. PM may reference payment state later, but it must not invent or authoritatively own it.
- Do not make Nostr authoritative for PM storage. If external mirrors exist later, they should sit behind a policy-gated bridge or outbox model.
- Do not begin with importer breadth, workflow automation, or broad external-suite parity.

---

## 2. Spacetime fit analysis

The current PM draft was too local-store-specific. The repo already has a real sync and replay spine that PM should reuse.

### 2.1 What already exists

- `crates/autopilot-spacetime` already defines the canonical sync substrate:
  - `sync_event`
  - `sync_checkpoint`
  - stream-grant auth
  - subscribe/resume planning
  - reducer semantics for idempotency and sequence conflicts
- `apps/autopilot-desktop` already has deterministic replay plumbing:
  - `sync_apply.rs`
  - `sync_bootstrap.rs`
  - `sync_lifecycle.rs`
  - checkpoint hydration from live Spacetime
  - remote checkpoint mirror acknowledgements
- Existing pane/state surfaces already use replay-safe projection streams and persisted local caches:
  - `stream.activity_projection.v1`
  - `stream.earn_job_lifecycle_projection.v1`
  - `stream.managed_chat_projection.v1`

### 2.2 What the current rollout semantics mean

Per `docs/MVP.md` and `docs/SPACETIME_ROLLOUT_INDEX.md`, the repo is currently in:

- `Phase 1`
  - mirror/proxy semantics
  - real token/bootstrap contracts
  - real local replay/checkpoint discipline
  - Spacetime-shaped local presence and projection state
- `Phase 2`
  - target future state
  - live remote Spacetime reducers and subscriptions for ADR-approved domains only

That means PM should not start by pretending the repo has no Spacetime stack, and it also should not skip the ADR gate and declare new authoritative domains casually.

### 2.3 What this implies for PM

- Step 0 should be stream-shaped from day one.
- Local PM state should look like current replay-safe projection documents and checkpoint files, not a parallel homegrown PM database.
- PM collaboration cutover should happen through the same bootstrap, stream-grant, resume, replay, and parity-gate discipline used elsewhere.
- Any PM domain that becomes Spacetime-authoritative must be named explicitly in `ADR-0001`.
- Money-linked workflows still must not become Spacetime-authoritative, even if non-monetary PM projections do.

### 2.4 Recommendation

Build PM in two layers:

1. `Now`
   - native desktop PM pane
   - local projection caches
   - canonical PM stream ids
   - deterministic per-stream apply/checkpoint behavior
2. `Later`
   - live remote Spacetime subscriptions and reducers for ADR-approved PM collaboration domains
   - same stream ids
   - same command/event contracts
   - same replay semantics

This preserves MVP scope while making Spacetime integration the spine instead of an afterthought.

---

## 3. Recommended architecture

### 3.1 Ownership boundaries

- `apps/autopilot-desktop/src/project_ops/*`
  - Owns pane wiring, view state, commands, UX flows, and feature gating.
- Optional later extraction into dedicated crates
  - Only after the entity model and stream contracts stabilize through dogfooding.
  - Good extraction candidates later: reducer/domain types, workspace manager, workflow policy loader, agent-task runner.
- `crates/wgpui`
  - May expose reusable controls used by the PM pane.
  - Must not own PM workflow rules, reducers, or app behavior.
- `crates/autopilot-spacetime`
  - Remains the canonical source for sync schema, stream auth, resume rules, reducer semantics, and live client contracts.

### 3.2 Authority model

#### Step 0 through early Phase 2

Use the repo's current mirror/proxy semantics for PM:

- PM commands are issued locally in the desktop app.
- PM visible state is stored as replay-safe local projection documents keyed by canonical PM stream ids.
- PM apply order and duplicate handling follow the existing `(stream_id, seq)` discipline.
- Checkpoints persist through the same `SyncApplyEngine` pattern already used for other streams.
- Source labels must stay truthful:
  - local projection stream state may use `source: stream.pm.*`
  - sync lifecycle status may use `source: spacetime.sync.lifecycle`
  - no local-only field should be mislabeled as live Spacetime authority

#### Later collaboration cutover

After ADR approval, PM collaboration domains may move to live remote Spacetime authority for:

- non-monetary work-item projections
- comments and notifications
- team/project coordination views
- derived counters and reporting projections

They may not move authority for:

- wallet balances
- settlement truth
- payout finality
- trust, policy, or security verdicts

### 3.3 PM stream catalog

Start with a small stream catalog and expand only when earned.

Recommended Step 0 and Phase 1 stream ids:

| Stream id | Purpose |
| --- | --- |
| `stream.pm.work_items.v1` | Current work-item state and list projection |
| `stream.pm.activity_projection.v1` | Human-readable item history and state-change feed |
| `stream.pm.cycles.v1` | Active cycle definitions and summaries |
| `stream.pm.saved_views.v1` | Built-in and user-defined saved views |

Likely later stream ids:

| Stream id | Purpose |
| --- | --- |
| `stream.pm.projects.v1` | Project records and scoped project views |
| `stream.pm.teams.v1` | Team records and defaults |
| `stream.pm.comments.v1` | Comment and mention projection state |
| `stream.pm.notifications.v1` | Assignment, mention, due-date, and blocked-change notifications |
| `stream.pm.agent_tasks.v1` | Agent-task queue and lifecycle state |
| `stream.pm.reporting.v1` | Derived reporting projections |

Use `coordination_event` or a dedicated PM coordination stream only for transient signals such as claims, leases, or operator coordination. Do not use transient coordination rows as durable work-item truth.

### 3.4 Local persistence shape

Do not start with a bespoke relational PM schema. Follow the projection-document pattern already used elsewhere in the desktop app.

Recommended local artifacts:

- `~/.openagents/autopilot-pm-work-items-projection-v1.json`
- `~/.openagents/autopilot-pm-activity-projection-v1.json`
- `~/.openagents/autopilot-pm-cycles-v1.json`
- `~/.openagents/autopilot-pm-saved-views-v1.json`
- existing shared checkpoint file:
  - `~/.openagents/autopilot-sync-checkpoints-v1.json`

Each PM projection document should include:

- `schema_version`
- `stream_id`
- projection rows or snapshot payload

Sequence progress should not be tracked in a separate PM-only metadata table. It should follow the shared checkpoint contract already used for other streams.

### 3.5 Runtime shape

- Feature gate: `project_ops`
- One explicit PM command/service loop in `apps/autopilot-desktop`
- Local Step 0 write path:
  1. UI issues command
  2. PM service validates command
  3. PM service emits explicit PM events and projection updates on canonical stream ids
  4. local projection documents persist deterministically
  5. `SyncApplyEngine` advances per-stream sequence checkpoints
  6. UI re-renders from projections
- Live sync path later:
  1. bootstrap with `POST /api/sync/token`
  2. require PM stream grants
  3. hydrate remote checkpoints
  4. subscribe to PM streams
  5. apply in order through the same `apply_stream_event_seq(...)` discipline
  6. mirror checkpoint acknowledgements back to Spacetime
  7. rebootstrap deterministically on stale cursor or out-of-order delivery

### 3.6 Phase 1 badge, grant, and checkpoint truth rules

Freeze these semantics now so Phase 2 wiring does not improvise them later:

- Primary `Project Ops` pane badge
  - `source: stream.pm.work_items.v1`
  - use for visible work-item list/detail/activity state sourced from local PM projection documents
- Sync/bootstrap diagnostics badge
  - `source: spacetime.sync.lifecycle`
  - use only for sync lifecycle, grant failures, checkpoint hydration state, rebootstrap state, or stale-cursor recovery diagnostics
- Required PM stream grants for later live bootstrap wiring
  - `stream.pm.work_items.v1`
  - `stream.pm.activity_projection.v1`
  - `stream.pm.cycles.v1`
  - `stream.pm.saved_views.v1`
- Per-stream checkpoint rules
  - duplicate `seq <= checkpoint` is dropped
  - out-of-order `seq` requires explicit rebootstrap or rewind before apply continues
  - stale cursor resumes from `max(local_checkpoint, remote_head - stale_clamp_window)`
  - remote checkpoint adoption only happens when it advances the local checkpoint
- Live-vs-local truth rule
  - In Phase 1, PM truth is local replay-safe projection state plus shared checkpoint discipline.
  - Live remote PM reducers/subscriptions remain Phase 2 target behavior only after ADR approval.

### 3.7 First-slice entities

The native thin slice does not need the full long-term model on day one. Start with:

| Entity | Purpose | Notes |
| --- | --- | --- |
| Work Item | Primary planning/execution record | Core Step 0 entity |
| Cycle | Timeboxed commitment bucket | One active cycle needed for the first pilot |
| Saved View | Reusable filters | Needed for daily usability |
| Activity Event | Human-readable history derived from PM stream events | Projection, not authority |

Keep these deferred until later phases unless Step 0 proves they are immediately necessary:

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
| 0 | Native Internal Dogfood on PM Streams | One team can run a cycle inside a native PM pane backed by local PM projection streams |
| 1 | Contract, ADR, and Stream Freeze | Stable PM contracts, stream ids, checkpoint rules, and ADR path |
| 2 | Core PM MVP on Mirror/Proxy Semantics | Daily-use PM flows built on the current Spacetime rollout discipline |
| 3 | Collaboration and Live Spacetime Cut-In | Multi-user PM domains move to live subscriptions and reducers after ADR approval |
| 4 | Agent Execution and Coordination | Agent-task execution and transient coordination fit into the same stream model |
| 5 | Bitcoin Workflows | Bounties and payout-linked visibility remain honest about money authority |
| 6 | Hardening, Reporting, and Cutover Evidence | Operational trust, reporting, and parity evidence for broader rollout |

Detailed Step 0 package: `docs/plans/autopilot-project-management-step-0-dogfood-package.md`

---

## 5. Phase 0 — Native internal dogfood on PM streams

### Objective

Ship the smallest native PM slice that one internal team can use immediately inside Autopilot Desktop, while shaping the subsystem around canonical PM stream ids and deterministic replay.

### Thin-slice definition

Step 0 should be:

- feature-gated
- single-team friendly
- list-first
- restart-safe
- stream-shaped
- truthful about data source and authority

Step 0 should not include:

- live multi-user Spacetime authority cutover
- comments/mentions
- push notifications
- bounty/payment logic
- autonomous agent execution
- broad importer/exporter work
- external-suite parity goals

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
- activity timeline sourced from PM projection streams

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
  - initial projection load or rebuild
- empty
  - no items match the active view/filter
- error
  - local projection or apply failure with a recoverable next step
- stale selection
  - selected item filtered out, archived, or deleted from the active view

The Step 0 pane should stay list-first. Do not start with multiple tabs, nested workspaces, or a broad pane hierarchy.

### Required Step 0 tasks

1. **Freeze the thin-slice data model and PM stream ids.**
   - Define the exact Step 0 fields.
   - Decide which fields are editable vs derived.
   - Keep Team and Project as plain identifiers first, not rich entities.

2. **Define commands, events, and apply rules.**
   - Commands: create, edit fields, change status, assign, set cycle, set blocked, clear blocked, archive/cancel.
   - Events: one explicit event per accepted state change.
   - Record idempotency, timestamps, and actor metadata for every command.

3. **Implement local PM projection documents and shared checkpoint use.**
   - Add PM projection files with schema and stream ids.
   - Reuse the existing checkpoint file and apply discipline.
   - Do not create a separate PM-only authority stack.

4. **Implement a PM reducer/service layer.**
   - Validate transitions.
   - Emit deterministic PM events and projection updates.
   - Rebuild projections in a stable order.

5. **Implement the first pane.**
   - Saved-view selector
   - Search/filter bar
   - Work-item list
   - Detail/editor panel
   - Quick-create flow

6. **Implement default saved views.**
   - My Work
   - Current Cycle
   - Blocked
   - Backlog
   - Recently Updated

7. **Add pilot instrumentation and replay diagnostics.**
   - Count commands by type.
   - Count state transitions.
   - Record rebuild time and checkpoint advance behavior.
   - Record search/filter usage.

8. **Run one real internal cycle.**
   - Use one pilot team only.
   - Record friction immediately.
   - Promote only proven needs into later phases.

### Deliverables

- A feature-gated `Project Ops` pane
- Local PM projection streams and persisted projection documents
- Shared checkpoint discipline through the existing sync apply engine
- One pilot cycle completed natively
- A short friction log with concrete follow-up work

### Exit criteria

- One team can create, triage, update, and complete work without leaving the native surface.
- Restarting the app preserves PM visible state and activity history correctly.
- The pane uses canonical PM stream ids and replay-safe checkpoint discipline rather than a bespoke PM store.
- The team can answer: what is active, what is blocked, who owns it, and what should happen next.
- The top missing native features are known from real use, not speculation.

---

## 6. Phase 1 — Contract, ADR, and stream freeze

### Objective

Turn Step 0 learnings into a stable PM contract and define the exact Spacetime cutover rules before broadening scope.

### Required outputs

1. **Canonical entity definitions.**
   - Work Item
   - Cycle
   - Saved View
   - Activity Event
   - Deferred placeholders for Comment, Team, Project, Agent Task, Bounty

2. **Canonical state machines.**
   - Work item: `backlog -> todo -> in_progress -> in_review -> done -> cancelled`
   - Blocked is a flag/reason, not a separate terminal state
   - Archive behavior is explicit and separate from completion

3. **Command and event contract.**
   - input validation rules
   - idempotency behavior
   - error taxonomy
   - transition rejection rules
   - exact event names and required payload fields

4. **PM stream catalog and checkpoint contract.**
   - exact Step 0 and Phase 2 stream ids
   - required stream grants
   - per-stream checkpoint rules
   - source-badge truth rules

5. **ADR path for live PM authority.**
   - define which PM domains are candidates for Spacetime authority
   - keep wallet/trust domains out of scope
   - update `ADR-0001` before implementation of live authority

6. **Test fixtures and release evidence hooks.**
   - golden PM stream fixtures
   - projection rebuild fixtures
   - invalid transition fixtures
   - parity evidence requirements for later live cutover

### Exit criteria

- The reducer/event model can be implemented and tested without reopening product-shape debates.
- PM stream ids, grants, checkpoint semantics, and badge rules are stable.
- A concrete ADR update path exists for live PM authority.
- Projection rebuild from a clean state produces semantically stable results.

---

## 7. Phase 2 — Core PM MVP on mirror/proxy semantics

### Objective

Expand Step 0 into a broadly usable PM MVP while staying aligned with the current Spacetime rollout semantics.

### Scope

- full work-item CRUD
- richer search/filter support
- board view
- bulk actions
- saved personal views
- stronger editor UX
- PM stream grant and checkpoint bootstrap wiring

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

5. **Persist user-level PM preferences.**
   - personal saved views
   - sort preferences
   - low-risk local PM preferences

6. **Wire PM into sync bootstrap and checkpoint hydration.**
   - request PM stream grants
   - hydrate remote checkpoints when available
   - keep stale-cursor recovery explicit

7. **Expand validation coverage.**
   - replay after abrupt shutdown
   - projection rebuild after migration
   - board rebuild and filter correctness
   - stream resume behavior and duplicate handling

### Exit criteria

- The pilot team prefers the native surface to side-channel tracking.
- List and board views cover most daily triage/execution behavior.
- Search and saved views materially reduce time spent hunting for work.
- PM state remains stream-shaped and restart-safe under the current mirror/proxy rollout.

---

## 8. Phase 3 — Collaboration and live Spacetime cut-in

### Objective

Support multi-user PM coordination and move approved PM domains onto live Spacetime subscriptions and reducers after ADR approval and parity evidence.

### Scope

- Project entity
- Team entity
- comments
- mentions
- notifications
- cycle planning across teams
- live PM subscriptions and reducers for approved collaboration domains

### Required tasks

1. **Promote Team and Project to real entities and streams.**
   - definitions
   - ownership rules
   - defaults
   - scoped stream ids

2. **Add comments and mentions.**
   - append-only comment events
   - mention parsing
   - correction/edit policy
   - comment projection stream

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

5. **Cut approved PM domains to live Spacetime.**
   - update ADR first
   - use canonical PM stream grants
   - wire live subscriptions/reducers
   - retain deterministic replay and checkpoint rules

6. **Run a multi-team pilot.**
   - validate cross-team dependency handling
   - validate notification signal/noise
   - validate live subscription health and recovery behavior

### Exit criteria

- Multiple teams can coordinate in one system without inventing separate process layers.
- Comments and notifications reduce status-chasing instead of amplifying noise.
- Live PM subscription health is observable and truthful.
- PM collaboration domains are live only where the ADR explicitly allows it.

### Pane additions in this phase

- `Project Overview`
  - summary pane for project backlog, cycle status, and blocked work
- `Project Detail`
  - scoped list/board view for a single project
- `Inbox/Notifications`
  - assignment, mention, due-date, and blocked-change feed

---

## 9. Phase 4 — Agent execution and coordination

### Objective

Add OpenAgents-native execution behavior after the human PM system is stable and the collaboration spine is in place.

### Scope

- agent-task specialization
- workspace management
- workflow policy loading
- Codex runner integration
- transient coordination semantics for claims, leases, or operator actions
- optional bridge/outbox plumbing for external mirrors later

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

7. **Add coordination semantics.**
   - define whether agent claims or leases live in dedicated PM streams or `coordination_event`
   - keep transient coordination distinct from durable work-item truth
   - bound duplicate or stale coordination effects

8. **Add operator surfaces.**
   - coordination health
   - checkpoint lag
   - bridge/outbox status if external mirrors are enabled later

### Exit criteria

- Agent tasks can run through a controlled, replay-safe lifecycle.
- Coordination semantics are explicit and debuggable.
- External mirrors, if later enabled, remain downstream of PM truth instead of replacing it.

### Pane additions in this phase

- `Agent Queue`
  - shows draft, ready, claimed, running, failed, and awaiting-verification agent tasks
- `Agent Run Detail`
  - shows workspace, attempt history, logs, artifacts, and operator controls
- `Coordination Status`
  - shows claim/lease health, checkpoint lag, and bridge-outbox status if configured

---

## 10. Phase 5 — Bitcoin workflows

### Objective

Attach money-adjacent workflows only after PM state and agent execution are trustworthy.

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
   - PM references payment truth
   - PM does not own wallet balances or payment finality
   - failures must be explicit and user-visible

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
- Any PM projections of bounty state remain subordinate to real settlement authority elsewhere.

### Pane additions in this phase

- `Bounty Detail`
  - shows amount, claimant, verifier, funding state, and payout history
- `Settlement History`
  - shows payment-linked lifecycle records for work items with bounties

---

## 11. Phase 6 — Hardening, reporting, and cutover evidence

### Objective

Make the subsystem operationally trustworthy at larger internal scope and collect the evidence needed for wider live rollout.

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
   - restart and live-subscription churn behavior

5. **Rollout evidence.**
   - PM parity and chaos evidence where live Spacetime authority is in use
   - checkpoint and stale-cursor recovery evidence
   - operator runbooks and cutover signoff

### Exit criteria

- Operators can trust both execution and reporting.
- The system remains fast and legible as usage grows.
- Wider rollout decisions are backed by parity evidence rather than optimism.

### Pane additions in this phase

- `Reporting`
  - cycle summaries, throughput, lead time, and blocker trends
- `Audit`
  - append-only change visibility and export surfaces
- `Operations`
  - projection rebuild health, migration status, checkpoint state, and sync diagnostics

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
- a bespoke PM database that bypasses the repo's existing Spacetime stream/checkpoint architecture

These do not belong in active implementation until the earlier phases are proven.

---

## 13. Recommended first 10 implementation tickets

1. Add `project_ops` feature gate and pane entry in `apps/autopilot-desktop`.
2. Define Step 0 work-item schema, command types, event types, and canonical PM stream ids.
3. Add local PM projection documents for work items, activity, cycles, and saved views.
4. Integrate PM apply/checkpoint behavior with the existing `SyncApplyEngine`.
5. Implement reducer/service layer with deterministic transition validation.
6. Implement quick-create work-item flow.
7. Implement list view with default saved views.
8. Implement detail/editor panel with status and priority changes plus activity timeline.
9. Add restart/rebuild/duplicate-command regression tests for PM streams.
10. Draft the ADR update and stream-grant plan needed for later live PM authority cutover.

---

## 14. Suggested GitHub issue backlog

Use the following issue titles and summaries as the starting backlog for full implementation. These are intentionally scoped as single-deliverable issues rather than giant epics.

### Phase 0 — Native internal dogfood on PM streams

| Suggested issue title | Summary |
| --- | --- |
| `[PM P0] Add project_ops feature gate and pane shell` | Wire a default-off feature gate, pane registration, and a minimal `Project Ops` shell so PM can ship without changing the default MVP path. |
| `[PM P0] Define Step 0 work-item schema and workflow enums` | Add the first PM domain types for work items, priorities, statuses, blocked state, and cycle references used by the Step 0 pane. |
| `[PM P0] Define Step 0 PM commands, events, and stream ids` | Freeze the initial command set, explicit accepted-event set, and canonical PM stream ids needed for replay-safe mutation handling. |
| `[PM P0] Add local PM projection documents and stream persistence` | Create the Step 0 projection documents for work items, activity, cycles, and saved views with `schema_version` and `stream_id` contracts. |
| `[PM P0] Integrate PM with sync_apply_engine checkpoints` | Reuse the shared checkpoint engine for PM streams so duplicate handling, out-of-order detection, and restart recovery match the rest of the app. |
| `[PM P0] Implement PM reducer/service loop` | Build the validation and apply path that accepts PM commands, enforces transition rules, and updates PM projections deterministically. |
| `[PM P0] Build built-in saved views and search/filter toolbar` | Implement the toolbar and default views for My Work, Current Cycle, Blocked, Backlog, and Recently Updated. |
| `[PM P0] Implement work-item list projection and selection behavior` | Render the list-first scanning surface with stable sorting, row selection, keyboard navigation, and view-specific empty states. |
| `[PM P0] Implement work-item detail editor and quick-create flow` | Add the primary create/edit UX for title, description, status, priority, assignee, cycle, parent, and blocked reason. |
| `[PM P0] Implement activity timeline and pane-state handling` | Show human-readable PM event history for the selected item and cover loading, empty, error, and stale-selection states. |
| `[PM P0] Add pilot instrumentation and run one internal PM cycle` | Prove restart safety, capture command/view usage, and complete one real internal pilot cycle so follow-on scope is earned by use. |

### Phase 1 — Contract, ADR, and stream freeze

| Suggested issue title | Summary |
| --- | --- |
| `[PM P1] Update ADR-0001 for PM collaboration authority candidates` | Extend the domain authority matrix so any future live PM domains are explicitly approved before reducers or subscriptions make them authoritative. |
| `[PM P1] Freeze v1 PM entity, command, event, and stream contracts` | Convert Step 0 learnings into a stable v1 contract for entities, commands, events, projection responsibilities, and stream ids. |
| `[PM P1] Define PM source-badge truth, stream grants, and checkpoint rules` | Document the exact badge semantics, required stream grants, checkpoint behavior, and live-vs-local truth rules for PM panes. |
| `[PM P1] Build golden fixtures for PM streams and projections` | Add deterministic fixtures that prove a known PM stream produces the same projected list, detail, and activity state after rebuild. |
| `[PM P1] Document invalid transitions, error taxonomy, and pilot-driven simplifications` | Capture rejection cases, operator-visible errors, and any fields or transitions the Step 0 pilot failed to justify. |

### Phase 2 — Core PM MVP on mirror/proxy semantics

| Suggested issue title | Summary |
| --- | --- |
| `[PM P2] Wire PM sync bootstrap, stream grants, and checkpoint hydration` | Request PM stream grants during sync bootstrap, hydrate remote checkpoints when available, and keep stale-cursor recovery explicit. |
| `[PM P2] Expand work-item editor to full MVP fields` | Add the richer metadata needed for daily use, including due date, tags/areas, richer parent relationships, and better field validation. |
| `[PM P2] Implement board projection and drag-drop workflow lanes` | Add a board view over the same PM stream-backed state with one lane per status and safe drag/drop transition handling. |
| `[PM P2] Add advanced search and filter syntax` | Support text search plus structured filters for state, assignee, priority, cycle, blocked, and tag/area queries. |
| `[PM P2] Add bulk triage and planning actions` | Enable bulk status changes, bulk assignment, bulk cycle changes, and bulk archive/cancel flows for faster planning work. |
| `[PM P2] Persist personal saved views and PM user preferences` | Store user-defined saved views, sort preferences, and other low-risk PM preferences in a durable and replay-safe way. |
| `[PM P2] Add regression coverage for PM stream resume, board rebuilds, and migrations` | Extend test coverage to stream resume behavior, board projection correctness, filter semantics, and rebuild behavior across schema changes. |

### Phase 3 — Collaboration and live Spacetime cut-in

| Suggested issue title | Summary |
| --- | --- |
| `[PM P3] Promote Team to a first-class PM entity and stream` | Replace plain Step 0 team identifiers with a real Team model that can own defaults, views, and planning surfaces. |
| `[PM P3] Promote Project to a first-class PM entity and stream` | Introduce Project records with scoped backlogs, project-level defaults, and project-specific overview/detail panes. |
| `[PM P3] Add comments and mentions as PM collaboration streams` | Implement append-only comment activity, mention parsing, and clear correction rules using explicit PM collaboration streams. |
| `[PM P3] Build notifications inbox and delivery rules` | Add assignment, mention, blocked-change, and due-date notifications with enough structure to avoid noisy spam. |
| `[PM P3] Implement Project Overview and Project Detail panes` | Add project-scoped list/board surfaces and high-level status panes for multi-team coordination. |
| `[PM P3] Cut PM collaboration domains over to live Spacetime subscriptions and reducers` | Move approved PM domains from mirror/proxy semantics to live Spacetime authority after ADR updates and parity evidence are complete. |

### Phase 4 — Agent execution and coordination

| Suggested issue title | Summary |
| --- | --- |
| `[PM P4] Define Agent Task state machine and execution contracts` | Add the authoritative states, required fields, verification contract, and retry semantics for agent-runnable work items. |
| `[PM P4] Build agent-task workspace manager with root containment` | Create the workspace lifecycle layer for agent runs, including root safety, naming rules, retention, and cleanup. |
| `[PM P4] Add workflow policy loader and validator` | Load repo-owned workflow contracts, parse them strictly, and make policy reload behavior explicit and testable. |
| `[PM P4] Integrate Codex runner for agent-task execution` | Run agent tasks through `crates/codex-client` in a dedicated worker runtime and surface run summaries back into PM state. |
| `[PM P4] Implement Agent Queue and Agent Run Detail panes` | Add panes for queue state, run attempts, logs, artifacts, retry posture, and operator controls. |
| `[PM P4] Add run reconciliation and restart recovery` | Detect stale or interrupted runs, reconcile their state after restart, and make retry/recovery behavior deterministic. |
| `[PM P4] Model agent-task claims and coordination events in Spacetime` | Define how transient claims, leases, or operator coordination signals use PM streams or coordination events without becoming durable work-item truth. |
| `[PM P4] Build coordination health and bridge-outbox operator surfaces` | Show coordination lag, checkpoint state, and any enabled external bridge or outbox backlog in a dedicated operator-facing surface. |

### Phase 5 — Bitcoin workflows

| Suggested issue title | Summary |
| --- | --- |
| `[PM P5] Define bounty state machine and external money authority boundaries` | Freeze the states and trust boundaries for bounty funding, claiming, verification, release, refund, and failure handling. |
| `[PM P5] Attach bounty records and funding references to work items` | Extend work items with structured bounty attachments, funding source references, and claimant/verifier relationships. |
| `[PM P5] Implement bounty funding, claimant, and verifier UX` | Build the detail surfaces needed to understand who funded work, who claimed it, and who must verify it before release. |
| `[PM P5] Add verification-gated release workflow` | Implement the manual-first release path that prevents PM from marking money as moved before settlement truth exists elsewhere. |
| `[PM P5] Build settlement history and payout failure visibility` | Add work-item level history for funding, release, refund, and failure states with explicit audit-friendly messaging. |
| `[PM P5] Run controlled bounty pilot and document dispute handling` | Test bounty flows with internal or design-partner users and document how disputes, failed payouts, and manual overrides behave. |

### Phase 6 — Hardening, reporting, and cutover evidence

| Suggested issue title | Summary |
| --- | --- |
| `[PM P6] Add reporting projections for throughput, lead time, and blockers` | Build the derived metrics and reporting surfaces needed for cycle and project-level operational visibility. |
| `[PM P6] Implement stale-item detection and reminder automation` | Add low-risk automation for stale work, due-date reminders, and operator nudges without over-automating status movement. |
| `[PM P6] Define permissions, roles, and audit export model` | Introduce the role and audit model required for broader rollout and explain how changes can be exported or reviewed. |
| `[PM P6] Build Operations pane for PM rebuild, checkpoint, and sync health` | Add operator-facing visibility into projection rebuilds, migration state, checkpoint position, and PM sync diagnostics. |
| `[PM P6] Performance-test large boards, replay, and live subscription churn` | Validate that the PM subsystem remains responsive under larger datasets, repeated rebuilds, and live subscription churn. |
| `[PM P6] Prepare rollout checklist, parity evidence, and cutover review` | Document the go/no-go checks, parity evidence, and release review needed before broader internal or partner rollout. |

---

## 15. Final recommendation

Do not build PM as a bespoke database product living next to the repo's actual sync model.

The correct path is:

1. ship a native, default-off PM pane,
2. back it with local PM projection streams and shared checkpoint discipline,
3. dogfood it for one real cycle,
4. freeze the PM stream and ADR contracts,
5. only then cut approved collaboration domains to live Spacetime reducers and subscriptions.

That path stays aligned with the repo's MVP constraints, respects ownership boundaries, and gives the PM subsystem a real architectural spine instead of a future migration problem.
