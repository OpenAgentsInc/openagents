# Model Lab Promotion Decision Ledger

Date: 2026-06-06

Issue: #384 / `OPENAGENTS-LAB-005`

Status: implemented as a read-only schema/projection contract in
`workers/api/src/omni-model-lab-promotion-decision.ts`.

## Purpose

The Model Lab promotion decision ledger records reviewed decisions for model
artifacts, training runs, candidates, adapters, and routes. It is the evidence
surface that Blueprint release gates can consume when deciding whether a
candidate is ready for a separate runtime promotion action.

The ledger is not the runtime promotion action. It cannot deploy a model,
install an adapter, mutate routing, execute rollback, mutate provider state,
spend money, change marketplace rank, pay out, settle, or upgrade public
claims.

## Records

- `OmniPromotionDecisionRecord`: one reviewed pass, fail, block, or supersede
  decision with candidate/artifact/run/adapter/route refs, release gate refs,
  reviewer receipts, eval and Benchmark Cloud evidence refs, risk labels,
  rollback posture, rollback refs, marketplace memory refs, attribution refs,
  blockers, caveats, and supersession refs.
- `OmniPromotionDecisionLedgerRecord`: the aggregate ledger linking decisions
  to known release gates, benchmark evidence, eval evidence, artifacts,
  training runs, candidates, adapters, routes, marketplace memory, and outcome
  attribution.

## Validation Rules

- Ledgers require at least one decision and cannot contain duplicate decision
  refs.
- Every decision must have target refs matching its target kind.
- Every decision must reference release gates in the ledger.
- Every decision must carry eval evidence or Benchmark Cloud evidence.
- Every decision requires risk labels.
- Passed and failed decisions require reviewer receipts.
- Passed decisions require ready or verified rollback posture, rollback refs,
  marketplace memory refs, and outcome attribution refs.
- Critical-risk decisions cannot pass in this contract.
- Failed and blocked decisions require blocker refs.
- Blocked decisions require caveat refs.
- Superseded decisions require superseded-by refs.
- Decision refs for release gates, benchmark evidence, eval evidence,
  marketplace memory, and attribution must link to ledger-level refs.
- Raw prompts, source archives, raw datasets, provider payloads, model weights,
  secrets, payment or wallet material, private repos, raw logs, raw traces, and
  raw timestamps are rejected.

## Projection

`projectOmniPromotionDecisionLedger(ledger, audience, nowIso)` returns an
`OmniPromotionDecisionProjection` with:

- friendly time labels for ledger and decision timestamps,
- counts for passed, failed, blocked, and superseded decisions,
- a claim-state label: `passed_not_deployed`, `failed_reviewed`, `blocked`, or
  `superseded`,
- public, agent, customer, team, and operator redaction,
- hard false authority booleans for runtime promotion, model deployment,
  adapter install, route mutation, rollback execution, provider mutation,
  marketplace rank mutation, payment spend, payout, settlement, and public
  claim upgrade.

## Blueprint Consumption

Blueprint release gates can read this ledger as reviewed evidence. A passed
decision only means "reviewed and ready for a separate promotion authority."
Runtime deployment, adapter installation, routing change, rollback, or public
claim promotion still requires a separate authorized action and receipt.

## Tests

Coverage lives in `workers/api/src/omni-model-lab-promotion-decision.test.ts`.
The tests cover:

- read-only projection of a passed promotion decision,
- release gate, reviewer receipt, rollback posture, marketplace memory, and
  attribution requirements,
- failed, blocked, and superseded decision labels,
- redaction of private promotion refs,
- rejection of unsafe material and false runtime-promotion authority.
