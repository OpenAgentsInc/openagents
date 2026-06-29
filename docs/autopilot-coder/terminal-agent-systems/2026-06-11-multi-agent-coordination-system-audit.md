# Multi-Agent Coordination System Audit

Date: 2026-06-11

This is system #42 from the Bun/Effect terminal-agent systems list. It defines
how OpenAgents should coordinate multiple local agents, hosted agents,
contributor Pylons, and external work providers while keeping assignment,
authority, and settlement boundaries explicit.

## Target

Build a coordination layer that can decompose work, assign roles, supervise
parallel lanes, collect evidence, resolve conflicts, and close out results
without letting agent prose claim completion.

## User-Visible Capability

Users should be able to:

- Start one task that fans out into bounded subruns.
- See which lane owns which subtask.
- Inspect status, blockers, artifacts, and receipts per lane.
- Stop or pause the whole coordination plan.
- Accept, reject, or request follow-up from each lane.
- Run local, hosted, and market-provider agents through one progress shape.
- Understand which actions cost money or rely on external providers.

Coordination should show real run state. It should not bury partial failures
behind a final summary.

## Coordination Model

Core records:

- Coordination plan ref.
- Parent run ref.
- Lane refs.
- Assignment refs.
- Dependency edges.
- Budget caps.
- Provider or adapter refs.
- Required capability refs.
- Artifact and receipt refs.
- Conflict and merge state.
- Closeout verdict.

Every lane should produce typed events and a closeout. A parent run may
complete only when its declared acceptance policy is satisfied.

### Orchestrator → lane messaging (capability gap, nice-to-have)

Today's lane lifecycle is effectively fire-and-forget: the orchestrator spawns
a lane, then can only `pause`, `cancel`, or `resume` it and await its closeout
— there is no way to **send a steering message into a running lane**. In
practice this bites: an orchestrator that has already spawned a subagent and
then learns new, relevant context (e.g. "the funded MDK wallets are at these
paths, use them" or "repoint the objective at a smaller bounded issue") cannot
relay it. The only options are to wait for the closeout and re-spawn with the
new context (wasting the in-flight work) or to over-specify everything up front
(impossible when the context arrives mid-run). Observed live on
2026-06-14 driving the first negotiated labor job: the orchestrator had to say
"I can't message a running agent" and fall back to doing the side-task itself.

Proposed: a `LaneInboxService` / `LaneSteerService` that lets the orchestrator
deliver a typed, append-only message to a running lane's inbox (additional
context, a scope amendment, a soft-redirect, or a "prefer this approach" hint),
which the lane drains between steps. Keep it bounded and typed (not arbitrary
control): messages are advisory context + scoped amendments, the lane still
owns its own decisions and closeout, and every injected message is a recorded
coordination event for the audit trail. This makes long-running lanes
correctable without the kill-and-respawn tax.

## Bun/Effect Boundary

Use Effect services for:

- `CoordinationPlannerService`: builds a typed plan and dependency graph.
- `LaneSchedulerService`: starts, pauses, cancels, and resumes lanes.
- `LaneInboxService` (proposed, see gap above): delivers typed, advisory
  steering messages into a running lane's inbox so the orchestrator can relay
  context that arrived after spawn without a kill-and-respawn.
- `AgentAdapterSelectionService`: picks local, hosted, Pylon, or market
  adapters under policy.
- `ConflictResolutionService`: manages overlapping edits and incompatible
  closeouts.
- `CoordinationProjectionService`: renders parent and lane status.
- `CoordinationReceiptService`: records assignment, completion, merge, and
  rejection receipts.

Use Queue for lane work. Use Stream for child-run events. Use Scope for
cleanup. Use Schedule for backoff and continuation.

## Authority Rules

- Subagents inherit only the capabilities explicitly granted to their lane.
- One lane cannot read another lane's private artifacts unless policy allows.
- Shared workspace writes require a merge strategy or isolated workspaces.
- Provider-market execution requires opt-in, bid, acceptance, and settlement
  refs.
- Parent closeout cannot hide failed mandatory lanes.
- Registry, issue, PR, payment, deployment, and payout writes remain separate
  authority actions.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents already has Pylon assignments, provider loops,
agent runtime adapter projections, labor-market planning, and work-order
surfaces. The terminal-agent README does not yet include a multi-agent
coordination audit.

Related open issue anchors:

- #4776 autonomics spawning coding threads.
- #4777 first live negotiated labor job.
- #4778 one mission/work-order record layer.
- #4781 backlog faucet for open market work requests.
- #4782 spare-capacity provider mode.
- #4783 Lane C fanout to the labor market.
- #4786 Autopilot MVP ladder.

No broad multi-agent orchestration claim should be green until a fanout run
has per-lane events, isolated workspace evidence, conflict handling, and
parent closeout receipts.

## Tests

Minimum coverage:

- Plan a multi-lane run with dependencies.
- Start, pause, resume, cancel, and fail child lanes.
- Enforce lane-specific capabilities.
- Keep isolated workspaces separate.
- Merge non-conflicting artifacts and reject conflicts.
- Prevent failed mandatory lanes from being summarized as success.
- Project parent and child status with freshness.
- Verify market-provider lanes require policy and receipts.

## Decision

Multi-agent coordination should be a typed supervision graph over normal
runtimes and assignments. It should not create a privileged agent mode outside
the existing event, artifact, permission, and receipt systems.

