# Artanis Operator Steering Contract

Date: 2026-06-06

Issue: #388 / `ARTANIS-003`

## Purpose

Artanis is steered privately through the Autopilot operator surface. The public
`/artanis` page and Forum posts are downstream projections, not control planes.

This contract records the first Artanis-specific operator steering model in
OpenAgents product surface. It complements the existing `/api/operator/autopilot/goals` goal routes
by adding Artanis-only lifecycle support, priority commands, private evidence,
risky-action approval decisions, and public-safe projections.

## Live Operator Goal Surface

The existing operator goal route family is the steering substrate:

- `POST /api/operator/autopilot/goals` with `agentId: "agent_artanis"`
- `GET /api/operator/autopilot/goals/current`
- `GET /api/operator/autopilot/goals/{goalId}`
- `PATCH /api/operator/autopilot/goals/{goalId}`
- `POST /api/operator/autopilot/goals/{goalId}/pause`
- `POST /api/operator/autopilot/goals/{goalId}/resume`
- `POST /api/operator/autopilot/goals/{goalId}/clear`

The Artanis steering contract adds the narrower command vocabulary:

- create goal
- pause goal
- resume goal
- cancel goal
- reprioritize goal

Cancel currently maps to the existing `clear` action because the generic goal
API archives the current goal. Reprioritize maps to the patchable operator goal
record plus Artanis-specific command priority in the steering ledger.

## Private Operator View

The operator projection can include:

- private evidence pack refs
- raw workroom state refs by reference only
- operator receipt refs
- approval decision records
- authority receipt refs
- command priority and state
- operator-only endpoint refs

Those refs are for authorized operator inspection inside `/autopilot`. They are
not public proof and must not be rendered on `/artanis` or posted to the Forum.

## Public Projection

The public Artanis and Forum audiences receive only:

- public-safe command refs
- public-safe goal refs
- public-safe action proposal refs
- public-safe caveat/blocker refs
- public-safe approval status refs
- public-safe projection refs from accepted or completed commands

Public projections do not include operator endpoint refs, private evidence,
raw workroom state, operator receipts, private steering refs, or raw
timestamps.

Blocked or superseded commands do not project public update refs. This keeps
early operator exploration from looking like public Artanis progress until the
state is accepted or completed.

## Approval Decisions

The steering ledger supports both approval and rejection of risky action
proposals. Every decision requires:

- an action ref
- an operator receipt ref
- a separate authority receipt ref
- a public-safe status ref
- a caveat ref when useful

This is still not execution authority by itself. Later approval-gate work must
bind these decisions to the specific wallet, provider, Forum publish, training,
eval, runtime promotion, or deployment executor that would perform the risky
action.

That approval-gate layer now exists as #393 /
`docs/artanis/2026-06-06-operator-approval-gates.md`. Operator steering can
record approval or rejection decisions, while the approval-gate contract
decides whether a specific risky action has explicit authority, expiry, caveat,
rollback posture, and public-safe projection. Neither layer executes the risky
action by itself.

## Implementation

Code:

- `workers/api/src/artanis-operator-steering.ts`
- `workers/api/src/artanis-operator-steering.test.ts`

Core exports:

- `ArtanisOperatorSteeringWorkspaceRecord`
- `projectArtanisOperatorSteeringWorkspace`
- `artanisOperatorGoalLifecycleReady`
- `artanisOperatorProjectionHasPrivateMaterial`
- `ARTANIS_AUTOPILOT_OPERATOR_ENDPOINTS`

The tests prove:

- operator projections expose the lifecycle commands and approval decisions;
- public Artanis and Forum projections omit private evidence, raw workroom
  refs, operator endpoints, and operator receipts;
- public update refs are emitted only from accepted or completed commands;
- non-Artanis targets, incomplete lifecycle support, and unsafe refs are
  rejected.
