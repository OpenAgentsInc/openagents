# Pylon Marketplace Job Intake API

Issue: `ARTANIS-024` / GitHub #410

OpenAgents product surface now has an operator API for Artanis-administered Pylon marketplace job
intake and assignment proposal.

This is still not a live dispatch, buyer-charge, payout, or settlement path.
It creates durable marketplace records that Artanis and operators can triage
before any Nexus/Pylon executor receives authority.

## Operator Endpoints

- `GET /api/operator/artanis/pylon-marketplace/jobs`
- `POST /api/operator/artanis/pylon-marketplace/jobs`
- `POST /api/operator/artanis/pylon-marketplace/jobs/:intakeRef/triage`

The routes accept an OpenAgents admin browser session or the existing admin
bearer token. Every write requires an `Idempotency-Key` header.

## Intake

`POST /api/operator/artanis/pylon-marketplace/jobs` creates a job intake record
for OpenAgents-seeded, external-human, or external-agent work.

The request records:

- requester ref;
- source;
- job kind;
- privacy class;
- Pylon resource-mode preference;
- benchmark, model, data, budget, spend caveat, source, policy gate, resource,
  result, eligibility, and evidence expectation refs.

The supported job kinds now include:

- artifact review;
- benchmark evaluation;
- embedding/data preparation;
- GEPA/DSPy optimization;
- inference;
- LoRA fine-tuning;
- training;
- validation.

OpenAgents-seeded jobs enter `intake_ready`. External jobs enter
`policy_gated` and must carry policy gate refs.

## Triage

`POST /api/operator/artanis/pylon-marketplace/jobs/:intakeRef/triage` moves an
intake into one of these states:

- `accepted_for_review`;
- `needs_input`;
- `rejected`;
- `assignment_proposed`.

`needs_input` and `rejected` require blocker refs. `assignment_proposed`
requires an assignment proposal with acceptance criteria refs, assignment
authority refs, provider eligibility refs, and the proposed resource mode.

Assignment proposals are stored as `proposed` work with `planned` payout state.
They can name an eligible provider pool or provider refs, but they do not create
a live Pylon dispatch receipt.

## Persistence

Migration `0121_pylon_marketplace_jobs.sql` adds:

- `pylon_marketplace_job_intakes`;
- `pylon_marketplace_assignments`;
- `pylon_marketplace_triage_actions`.

The triage action table stores idempotent action receipts so retries cannot
create duplicate assignment proposals.

## Authority Boundary

The API response always returns:

- `liveDispatchAllowed: false`;
- `buyerChargeMutationAllowed: false`;
- `paidAssignmentDispatchAllowed: false`;
- `payoutMutationAllowed: false`;
- `settlementMutationAllowed: false`.

Dispatch still requires the later Artanis production launch gate plus a
target-specific Nexus/Pylon executor approval.

## Verification

Coverage lives in:

- `workers/api/src/pylon-marketplace-jobs.test.ts`;
- `workers/api/src/operator-pylon-marketplace-routes.test.ts`.

The tests cover artifact-review job support, admin gating, idempotent intake
creation, idempotency conflicts, public requester redaction, proposed
assignments, blocker requirements, and unsafe raw ref rejection.
