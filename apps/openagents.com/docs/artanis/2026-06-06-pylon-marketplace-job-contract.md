# Pylon Marketplace Job Contract

Issue: `ARTANIS-015` / GitHub #400

OpenAgents product surface now has a schema-first contract for Artanis-administered Pylon
marketplace work in `workers/api/src/pylon-marketplace-jobs.ts`.

Issue `ARTANIS-024` / GitHub #410 added the first operator API and D1-backed
current-record persistence for the same contract. The API surface is documented
in `docs/artanis/2026-06-06-pylon-marketplace-job-intake-api.md`.

This is not a dispatch or payment implementation. It is the evidence boundary
that lets Artanis intake, triage, and propose Pylon jobs without implying that
paid work has been dispatched or that bitcoin payouts can be made.

## Job Sources

The contract distinguishes three sources:

- `openagents_seeded`: initial jobs created by OpenAgents for Pylon v0.2 launch
  and Autopilot continual-learning work.
- `external_human`: a human-requested marketplace job, admitted only behind
  policy gates.
- `external_agent`: an agent-requested marketplace job, admitted only behind
  policy gates.

External jobs must carry policy gate refs before Artanis can triage them.
OpenAgents-seeded jobs can be used first while the external marketplace policy,
pricing, and payment rails continue maturing.

## Supported Work

The initial marketplace job kinds are:

- artifact review;
- inference;
- GEPA/DSPy optimization;
- LoRA fine-tuning;
- training;
- benchmark evaluation;
- embedding/data preparation;
- validation.

Each intake record carries requester refs, work kind, benchmark/model/data refs,
budget refs, spend caveats, resource requirements, privacy class, eligibility
requirements, result expectations, evidence expectations, source refs, and
policy gate refs when required.

The operator API can triage records into `accepted_for_review`, `needs_input`,
`rejected`, or `assignment_proposed`. `needs_input` and `rejected` states require
blocker refs.

## Assignment Records

Assignment records separate proposal from paid work:

- proposed and held-for-authority records can exist without provider assignment;
- assigned/running/result-submitted/accepted records require provider
  eligibility, assignment authority refs, and Pylon assignment receipt refs;
- result-submitted records require artifact and result evidence refs;
- accepted records require acceptance criteria and accepted-work refs.

The resource mode is linked to the Pylon resource-mode setup contract:
`background_20`, `balanced`, `overnight_full`, or `dedicated_full_blast`.

## Payment Boundary

Accepted-work payout states are not based on Forum rewards or generic job
creation. Once payout state reaches accepted work, the record must carry:

- accepted-work refs;
- Nexus receipt refs;
- Pylon receipt refs;
- Treasury receipt refs;
- payout caveat refs.

The contract grants no buyer-charge mutation, no paid-assignment dispatch, no
payout mutation, and no settlement mutation. It only records evidence and safe
projections.

## Public Projection

Public and agent projections redact:

- private requester refs;
- private provider refs;
- private evidence;
- operator authority details;
- buyer payment evidence;
- raw model artifacts;
- raw datasets;
- runner logs;
- wallet/payment material;
- raw timestamps.

Operator and private projections can show safe private refs, but the same raw
secret and payment-material rejection still applies.

## Verification

Coverage lives in `workers/api/src/pylon-marketplace-jobs.test.ts`.
Route coverage lives in `workers/api/src/operator-pylon-marketplace-routes.test.ts`.

The tests prove:

- OpenAgents-seeded and external policy-gated jobs project safely.
- The initial job-kind coverage includes inference, GEPA/DSPy optimization,
  LoRA fine-tuning, training, benchmark evaluation, embedding/data preparation,
  validation, and artifact review.
- External jobs require policy gate refs.
- Assigned work requires provider eligibility and Nexus/Pylon authority
  receipts.
- Accepted-work payout states require Nexus, Pylon, and Treasury receipts.
- Forum rewards and generic job-creation refs cannot be used as payout basis.
- Raw private data, model artifacts, provider tokens, runner logs, wallet
  material, payment material, and raw timestamps are rejected.
- Operator API writes require admin authority and `Idempotency-Key`; retries
  replay the original result and conflicting bodies are rejected.
