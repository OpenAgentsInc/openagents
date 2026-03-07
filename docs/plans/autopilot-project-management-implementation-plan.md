# Autopilot Project Management — Granular Implementation & Deployment Plan

**Status:** Draft implementation plan  
**Last updated:** 2026-03-06  
**Source:** Derived from the Autopilot Project Management Specification  
**Intent:** Convert the broad product spec into a step-by-step plan the engineering team can execute with low context switching.

---

## 1. Planning assumptions and guardrails

- This plan is sequenced to respect the current OpenAgents MVP: do not let PM tooling derail the core Autopilot money-printing loop.
- The first deployment step must be useful internally without requiring new protocol, wallet, or relay work.
- Start with the smallest viable internal operating system, then progressively replace manual pieces with native product surfaces.
- Keep product-specific PM behavior in `apps/autopilot-desktop` if/when implemented there.
- Keep `crates/wgpui` limited to reusable UI primitives only; do not move Autopilot PM workflows into shared crates.
- Keep Nostr-first as the long-term architecture direction, but do not force Nostr-authoritative storage on day one if it slows adoption.
- Treat Bitcoin bounties, agent queues, and advanced analytics as follow-on layers, not prerequisites for internal adoption.

---

## 2. What success looks like

This effort is successful when:

1. The team can manage real work in one shared system with minimal confusion.
2. Work items have a consistent schema, clear workflow states, and reliable ownership.
3. The system becomes useful internally before it becomes ambitious externally.
4. Later Nostr, agent, and Bitcoin-native features extend the same workflow instead of replacing it.

---

## 3. Recommended delivery sequence at a glance

| Phase | Name | Primary outcome |
| --- | --- | --- |
| 0 | Minimal Deliverable / Internal Dogfood | A usable internal PM workflow the team can adopt immediately |
| 1 | Domain Model and Contracts | Freeze the core entities, workflows, and authority model |
| 2 | Core PM MVP | Native issue CRUD, list/board views, and filters |
| 3 | Project Structure and Collaboration | Projects, cycles, comments, activity, and notifications |
| 4 | Nostr and Agent-Native Layer | Nostr-backed state, agent tasks, and NIP-90 alignment |
| 5 | Bitcoin Workflows | Bounties, escrow, payouts, and payment-linked completion |
| 6 | Analytics, Automation, and Hardening | Metrics, automations, governance, and rollout polish |

---

## 4. Deployment Step 0 — Minimal deliverable we can use ASAP

### Objective

Stand up a team-usable Autopilot PM workflow immediately, without waiting for native product implementation.

### Minimal deliverable definition

Use **GitHub Issues + labels + milestones/cycles + a lightweight operating runbook** as the v0 source of truth, while keeping the future Autopilot PM schema aligned with the long-term product spec.

This gives the team something usable now while reducing rework later.

Detailed internal package: `docs/plans/autopilot-project-management-step-0-dogfood-package.md`

### Step-by-step tasks

1. **Freeze the v0 operating model.**
   - Adopt one canonical workflow: `Backlog -> Todo -> In Progress -> In Review -> Done -> Cancelled`.
   - Adopt one priority scale: `Urgent / High / Medium / Low / None`.
   - Adopt one issue type set: `Bug / Feature / Improvement / Task / Epic / Research / Agent Task`.
   - Decide which fields are required in v0: title, description, status, priority, assignee, labels, parent link, due date.

2. **Create the internal issue taxonomy.**
   - Define label prefixes such as `type:*`, `prio:*`, `team:*`, `area:*`, `state:*`.
   - Define parent/child conventions for epics and sub-tasks.
   - Define blocking conventions using linked issues.

3. **Create templates the team can use on day one.**
   - Bug template.
   - Feature template.
   - Research template.
   - Agent Task template.
   - Epic template with child-issue checklist.

4. **Stand up the default team views.**
   - My work.
   - Current cycle.
   - Bugs only.
   - Blocked work.
   - Recently updated.
   - Agent tasks.

5. **Create a simple cycle ritual.**
   - Weekly backlog grooming.
   - Weekly cycle commit.
   - Daily async status updates in issue comments.
   - End-of-cycle review and carry-over.

6. **Write the operating runbook.**
   - How to create work.
   - When to open an epic vs a task.
   - How to mark blockers.
   - How to move status.
   - How to close work.
   - How to attach PRs and commits.

7. **Pilot with one engineering slice for one cycle.**
   - Use only the v0 workflow for one sprint/cycle.
   - Record friction points.
   - Capture missing fields and redundant steps.

8. **Run a retrospective before building product surfaces.**
   - Document what the team actually used.
   - Remove fields nobody touched.
   - Promote only proven needs into the native PM MVP.

### Deliverables

- A documented workflow the team can begin using immediately.
- Templates, labels, and default views.
- One completed pilot cycle with feedback.

### Exit criteria

- At least one team is actively using the workflow.
- New work is being created consistently through templates.
- Status changes are understandable without side-channel clarification.
- The team can identify the top 5 native features worth building next.

---

## 5. Phase 1 — Domain model and contracts

### Objective

Lock the product model before building UI or sync behavior.

### Step-by-step tasks

1. **Define the canonical entity model.**
   - Work Item.
   - Comment.
   - Team.
   - Project.
   - Cycle.
   - Module.
   - Notification.
   - Agent Task.
   - Bounty.

2. **Define required fields and optional fields for each entity.**
   - Mark fields required for MVP.
   - Mark fields deferred to later phases.
   - Remove speculative fields not needed in the first two phases.

3. **Define the lifecycle rules.**
   - Allowed status transitions.
   - Parent/sub-issue rules.
   - Blocking/dependency rules.
   - Assignment rules.
   - Comment edit/delete rules.

4. **Define the authority model.**
   - Decide what is authoritative in v1.
   - Decide what may be mirrored locally.
   - Decide what may be mirrored to Nostr later.
   - Explicitly separate authoritative state from notifications/activity projections.

5. **Define identifiers and reference strategy.**
   - Human-readable issue IDs.
   - Stable internal IDs.
   - Git integration references.
   - Future Nostr tag/index strategy.

6. **Define the API/contract surface.**
   - CRUD operations for each MVP entity.
   - Filter/query contract.
   - Bulk update contract.
   - Activity event shape.
   - Notification event shape.

7. **Write the v1 state diagrams and sequence diagrams.**
   - Create issue.
   - Update issue.
   - Move through workflow.
   - Link PR to issue.
   - Convert issue to Agent Task.

8. **Hold one review and freeze the v1 contract.**
   - Product review.
   - Engineering review.
   - Architecture review.
   - Record explicit non-goals.

### Exit criteria

- The team has one agreed schema and one agreed workflow model.
- The first implementation team can build without reopening product-shape debates.
- Deferred features are clearly marked so they do not leak into MVP.

---

## 6. Phase 2 — Core PM MVP

### Objective

Ship the smallest native Autopilot PM surface that replaces the most painful manual v0 steps.

### Scope for this phase

- Work item CRUD.
- List view.
- Board view.
- Basic filters.
- Search.
- Keyboard-first create/edit flows.

### Step-by-step tasks

1. **Implement the storage layer.**
   - Add local schema/tables for MVP entities.
   - Add migrations.
   - Add repository/query layer.
   - Add test fixtures and seed data.

2. **Implement the command and service layer.**
   - Create work item.
   - Update fields.
   - Change status.
   - Link related items.
   - Assign/unassign.
   - Bulk update selected items.

3. **Implement the first create/edit flows.**
   - Quick-create modal.
   - Full issue editor.
   - Keyboard save/close behavior.
   - Validation and error states.

4. **Implement list view first.**
   - Sort by updated date, priority, due date.
   - Inline status/assignee updates.
   - Empty state and loading state.
   - Saved default filters.

5. **Implement board view second.**
   - One lane per workflow state.
   - Drag/drop status changes.
   - Card summary fields.
   - WIP visibility.

6. **Implement search and filter syntax.**
   - Text search across title and description.
   - Filter by state, assignee, priority, label, project, cycle.
   - Save personal views.

7. **Implement activity basics.**
   - Record create/update/status-change events.
   - Surface a simple activity timeline per issue.

8. **Dogfood with the same team from Step 0.**
   - Move one full cycle into the native surface.
   - Keep fallback to the v0 workflow only where needed.

### Exit criteria

- Team can create, edit, triage, and complete work in the native PM MVP.
- List and board views cover most daily usage.
- Search and filters reduce manual issue hunting.

---

## 7. Phase 3 — Project structure and collaboration

### Objective

Add the organizational features needed for multi-team use.

### Scope for this phase

- Projects.
- Cycles.
- Teams.
- Comments.
- Notifications.
- Basic pages/runbooks.

### Step-by-step tasks

1. **Implement team and project entities.**
   - Team creation/edit flows.
   - Project creation/edit flows.
   - Team-specific defaults.

2. **Implement cycles.**
   - Create cycle.
   - Assign issues to cycle.
   - Current cycle view.
   - Carry-over flow for incomplete work.

3. **Implement comments and mentions.**
   - Markdown comments.
   - @mention parsing.
   - Edit/delete permissions.
   - Comment activity log.

4. **Implement notifications.**
   - Assignment notifications.
   - Mention notifications.
   - State-change notifications.
   - Due-date reminders.

5. **Implement project-level views.**
   - Project backlog.
   - Cycle board.
   - Team workload list.
   - Recently blocked items.

6. **Implement lightweight documentation pages.**
   - Link docs/pages to projects.
   - Link issues inside docs.
   - Version basic content changes.

7. **Run a second internal pilot with more than one team.**
   - Validate cross-team dependencies.
   - Validate notification usefulness.
   - Validate cycle rituals.

### Exit criteria

- Multiple teams can use the system without inventing their own process.
- The collaboration layer is good enough to replace ad hoc status chasing.

---

## 8. Phase 4 — Nostr and agent-native layer

### Objective

Start making the system OpenAgents-native instead of just internally useful.

### Scope for this phase

- Nostr event model.
- Relay sync strategy.
- Agent Task flows.
- NIP-90-compatible assignment/request patterns.

### Step-by-step tasks

1. **Define the Nostr mapping.**
   - Map each core entity to an event kind.
   - Define replaceable vs append-only behavior.
   - Define tags for projects, assignees, labels, and links.

2. **Build read/write adapters.**
   - Local-to-Nostr publish.
   - Relay-to-local sync.
   - Conflict and replay handling.
   - Retry and duplicate suppression.

3. **Implement visibility and privacy rules.**
   - Public work items.
   - Private team/project visibility.
   - Encrypted payload plan where needed.

4. **Implement Agent Task specialization.**
   - Mark issue as Agent Task.
   - Add capability requirements.
   - Add input/output artifact fields.
   - Add verification result fields.

5. **Integrate NIP-90 job request semantics where appropriate.**
   - Request format.
   - Result linkage.
   - Agent attribution.
   - Status handoff between PM workflow and execution workflow.

6. **Ship read-only Nostr mirror before making Nostr authoritative.**
   - Validate real relay behavior.
   - Validate replay semantics.
   - Validate operational visibility.

7. **Only then evaluate Nostr-authoritative domains.**
   - Keep comments/activity as earlier candidates.
   - Keep payout-related truth out of Nostr authority.

### Exit criteria

- PM state can be mirrored and shared through Nostr without breaking local usability.
- Agent tasks are first-class enough to support OpenAgents-native workflows.

---

## 9. Phase 5 — Bitcoin workflows

### Objective

Add Bitcoin-native value flows only after core work management is stable.

### Scope for this phase

- Bounties.
- Escrow.
- Payout triggers.
- Payment history on work items.

### Step-by-step tasks

1. **Define the bounty state machine.**
   - Unfunded.
   - Funding pending.
   - Funded.
   - Claimed.
   - Verification pending.
   - Released.
   - Refunded/failed.

2. **Define authority boundaries for money state.**
   - Wallet/payment truth must remain explicit and authoritative.
   - Activity feeds may mirror payment status but not invent it.

3. **Implement bounty attachment to work items.**
   - Amount.
   - Funding method.
   - Claimant.
   - Verification requirement.

4. **Implement escrow and release integration.**
   - Manual approval path first.
   - Automated release only after verification is trustworthy.

5. **Implement payout visibility.**
   - Funding status badge.
   - Payment history.
   - Failure states.
   - Audit trail.

6. **Pilot with internal or tightly controlled design-partner use only.**
   - Validate payout correctness.
   - Validate failure messaging.
   - Validate dispute handling.

### Exit criteria

- Users can attach and track bounties without ambiguity.
- Payment-linked completion is honest, legible, and auditable.

---

## 10. Phase 6 — Analytics, automation, and hardening

### Objective

Improve leverage, reporting, and operational confidence after the core workflow is proven.

### Step-by-step tasks

1. **Implement baseline metrics.**
   - Throughput.
   - Lead time.
   - Cycle time.
   - Completion rate.
   - Blocker count.

2. **Implement health views.**
   - At-risk projects.
   - Overdue issues.
   - Workload imbalance.
   - Stalled agent tasks.

3. **Implement simple automations.**
   - PR opened -> move to `In Review`.
   - PR merged -> move to `Done`.
   - First commit -> suggest `In Progress`.
   - Due date reached -> notify assignee.

4. **Implement report export and status summaries.**
   - Cycle summary.
   - Project summary.
   - Team summary.

5. **Implement governance and audit surfaces.**
   - Permission model.
   - Audit log.
   - Change history export.

6. **Run performance and UX hardening.**
   - Keyboard speed checks.
   - Large-board performance.
   - Search responsiveness.
   - Offline/reconnect behavior.

### Exit criteria

- Operators can trust the system for both execution and reporting.
- Routine status work is meaningfully automated.

---

## 11. Recommended deployment waves

1. **Wave A — Core team internal use**
   - Use Step 0 only.
   - Goal: prove workflow usefulness before product build-out.

2. **Wave B — Engineering alpha**
   - Use Phases 1-2 outputs.
   - Goal: replace the most painful manual workflows.

3. **Wave C — Multi-team internal beta**
   - Use Phase 3 outputs.
   - Goal: validate cross-team coordination and notifications.

4. **Wave D — OpenAgents-native beta**
   - Use Phase 4 outputs.
   - Goal: validate Nostr and agent-native workflows.

5. **Wave E — Incentivized/market beta**
   - Use Phase 5 outputs.
   - Goal: validate bounty funding, completion, and payout.

6. **Wave F — Public or partner-facing rollout**
   - Use Phase 6 outputs.
   - Goal: ship with governance, reporting, and operational confidence.

---

## 12. Features to explicitly defer until after core adoption

- Gantt charts.
- Spreadsheet view.
- Predictive analytics.
- PDF exports.
- Advanced role matrices.
- Marketplace-style agent reputation systems.
- Full Jira/Linear importer breadth.
- Mobile-first PM surface.

These should not enter active implementation until the team is successfully using the earlier phases.

---

## 13. Team operating guidance to reduce context switching

### Recommended workstreams

- **Product/Process track:** templates, workflow, cycles, UX rules, acceptance criteria.
- **Core app track:** native issue flows, list/board views, search, notifications.
- **Protocol/Sync track:** Nostr mapping, relay sync, activity/event transport.
- **Payments/Trust track:** bounty state machine, escrow, verification, auditability.

### Recommended execution order inside each phase

1. Freeze the entity/contract shape.
2. Implement storage and commands.
3. Implement one view at a time.
4. Dogfood immediately.
5. Remove unnecessary fields and steps.
6. Only then add automation or expansion features.

---

## 14. Final recommendation

Do **not** start by building the full Linear/Plane replacement.

Start with the smallest internal operating system in **Deployment Step 0**, run one real cycle on it, and let that pilot determine which native surfaces earn the right to be built next.

That approach gets the team using Autopilot PM quickly, preserves alignment with the repo's MVP discipline, and gives later Nostr/agent/Bitcoin-native features a real workflow to attach to.