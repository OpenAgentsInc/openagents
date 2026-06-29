# Pylon GEPA Metric-Call Assignment Lifecycle

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#506`.

OpenAgents product surface now has a typed lifecycle contract for Probe GEPA metric-call assignments
that can be accepted by a Pylon worker, updated with progress refs, submitted
with artifact/proof refs, closed by an operator or evaluator, and imported by
the Psionic GEPA coordinator.

The implementation lives in
`workers/api/src/pylon-gepa-metric-call-assignments.ts`.

The first unpaid demo-worker lease proof lives in
`workers/api/src/probe-gepa-unpaid-pylon-lease-proof.ts` and is documented in
`2026-06-08-probe-gepa-unpaid-pylon-worker-lease-proof.md`.

## Scope

This is the OpenAgents product surface/Pylon lease lifecycle shape for benchmark rollouts. It does
not replace Benchmark Cloud manifests, Probe closeouts, or Psionic candidate
frontier state. It records the assignment work slice and keeps the state machine
compatible with the existing Pylon marketplace direction:

1. assignment created;
2. worker accepts and receives a lease ref;
3. worker reports progress refs;
4. worker submits artifact, proof, verifier, closeout, and resource refs;
5. evaluator/operator closes as accepted or rejected work;
6. GEPA coordinator imports the normalized public-safe result.

## Required Assignment Fields

Each assignment records campaign id, benchmark suite ref, split ref, task ref,
Probe commit ref, candidate hash, backend profile, runtime ref, expected
artifact refs, expected proof bundle refs, verifier/scorer refs,
timeout/budget ref, closeout requirement refs, and payment mode.

Payment mode is explicit on every record:

- `unpaid_smoke`
- `operator_credit`
- `payable_pending_settlement`
- `settled_bitcoin`
- `rejected_no_pay`

These modes prevent an accepted benchmark work slice from being described as
payable or settled by default.

## Claim Boundary

Accepted work and settled payout are separate claims.

Accepted work is allowed only after a submitted result has artifact refs, proof
bundle refs, closeout result refs, verifier result refs, and resource usage
refs.

Public projection may say accepted unpaid smoke work when `unpaid_smoke` work
has accepted artifacts and no-spend evidence refs. It may say payable pending
settlement only when `payable_pending_settlement` has payment receipt refs. It
may say settled bitcoin payout only when `settled_bitcoin` has payment and
settlement receipt refs. Rejected work closes as `rejected_no_pay`.

## GEPA Coordinator Import

`pylonGepaMetricCallCoordinatorImport` emits
`openagents.pylon_gepa_metric_call_coordinator_import.v1` with campaign, split, task,
candidate hash, worker ref, assignment state, closeout decision, artifact refs,
proof refs, verifier result refs, resource refs, no-spend evidence refs,
payment receipt refs, settlement receipt refs, and derived accepted-work,
payable-work, and settled-bitcoin booleans.

That import is the bridge to Psionic's rollout coordinator. It is evidence
input, not runtime promotion authority.

## Verification

`workers/api/src/pylon-gepa-metric-call-assignments.test.ts` covers:

- explicit no-spend assignment creation and worker acceptance;
- progress refs and result submission;
- accepted and rejected operator closeout;
- coordinator import shape;
- refusal to close accepted work before refs are submitted;
- refusal to attach settlement receipt refs without `settled_bitcoin`;
- public-safe ref validation for private, provider, raw log, wallet, and payment
  material.

`workers/api/src/probe-gepa-unpaid-pylon-lease-proof.test.ts` covers the
three-demo-Pylon unpaid lease proof, including worker accept, progress refs,
artifact/proof/resource/verifier refs, accepted and rejected closeouts,
coordinator imports, and refusal to project payable or settled claims.
