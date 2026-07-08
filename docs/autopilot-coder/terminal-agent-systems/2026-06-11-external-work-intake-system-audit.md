# External Work Intake System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #43 from the Bun/Effect terminal-agent systems list. It defines
how work can enter the terminal-agent runtime from APIs, Forum posts, backlog
issues, web workrooms, Pylon requests, schedules, and other agents.

## Target

Build an external intake layer that normalizes every request into a typed work
order with identity, payment, scope, data boundary, acceptance criteria,
adapter requirements, and review policy.

## User-Visible Capability

Users and agents should be able to:

- Submit a coding task through a UI or API.
- Attach repository, workspace, issue, or artifact refs.
- Declare constraints, budget, deadline, and verification command refs.
- Track admission, rejection, routing, execution, review, and delivery.
- Receive clear refusal reasons before any work runs.
- Pay or reserve budget where required.
- Reuse the same status shape regardless of intake surface.

An intake channel is not execution authority. Admission must produce a work
order before the runtime acts.

## Intake Record Model

Each intake record should include:

- Request ref.
- Requester identity and account state.
- Intake channel.
- Work kind.
- Objective.
- Scope and data classification.
- Required capability refs.
- Adapter preferences.
- Payment or budget refs.
- Verification refs.
- Acceptance policy.
- Review policy.
- Expiration.
- Status and blocker refs.

The normalized record should be stable enough that a browser order, API call,
Forum request, and autonomous administrator proposal can all land in the same
mission/work-order system.

## Bun/Effect Boundary

Use Effect services for:

- `WorkIntakeService`: validates and normalizes external requests.
- `AdmissionPolicyService`: checks identity, payment, data scope, capability,
  and risk.
- `WorkOrderService`: creates the durable work order.
- `RoutingProposalService`: proposes placement and adapter options.
- `IntakeProjectionService`: shows request status back to the intake channel.
- `IntakeReceiptService`: records admission, rejection, and delivery refs.

Use Schema for external payloads and normalized work orders. Use Queue for
admitted work. Use idempotency keys for every public write.

## Safety Rules

- Reject requests with missing identity, missing budget, unsupported data
  scope, or unknown verification refs.
- Never accept raw private repo material through public channels.
- Do not infer adapter authority from free-form text.
- Do not start work before admission and routing receipts exist.
- Do not let payment evidence replace acceptance or provider settlement.
- Preserve channel-specific caveats in the normalized record.
- Require API parity for every browser-only action.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has multiple work-entry concepts: live Autopilot
workrooms, Pylon assignment APIs, Forum plans, agent API plans, labor-market
planning, and product-promise gates. The terminal-agent README does not yet
include an external intake audit.

Related open issue anchors:

- #4773 API parity contract.
- #4774 agent payment in both currencies.
- #4775 Forum to coding request.
- #4776 autonomous administrator proposals.
- #4777 first negotiated labor job.
- #4778 mission/work-order unification.
- #4781 backlog faucet.

No claim should say all intake channels are live until each channel has
admission, routing, idempotency, status projection, and delivery receipts.

## Tests

Minimum coverage:

- Normalize requests from UI, API, Forum, issue, and schedule fixtures.
- Reject unsupported scopes and capabilities.
- Enforce idempotency on repeated submissions.
- Preserve payment and budget refs without treating them as completion.
- Route adapter-specific and adapter-agnostic requests.
- Project intake state back to each channel.
- Scan public intake payloads for private material.
- Verify every browser action has an API peer.

## Decision

External work intake should be one admission pipeline with many doors. Once a
request is admitted, downstream runtime systems should not care which door it
came through except for projection and policy caveats.

