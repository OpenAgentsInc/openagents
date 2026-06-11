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

## Bun/Effect Boundary

Use Effect services for:

- `CoordinationPlannerService`: builds a typed plan and dependency graph.
- `LaneSchedulerService`: starts, pauses, cancels, and resumes lanes.
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

