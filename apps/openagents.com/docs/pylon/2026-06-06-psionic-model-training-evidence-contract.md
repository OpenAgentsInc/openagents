# Psionic Model And Training Evidence Contract

Date: 2026-06-06

Status: implemented contract note for issue #338 / `OPENAGENTS-RUST-005`.

## Purpose

OpenAgents product surface now has a schema-first Psionic evidence contract for eval, training,
optimizer, candidate-module, scorecard, promotion proposal, and rollback
evidence.

The implementation lives in `workers/api/src/psionic-evidence-contract.ts`.

This is a contract and projection layer only. It does not run Psionic, train a
model, promote a module, route work, mutate public claims, pay anyone, or
settle accepted outcomes.

## Evidence Model

`OpenAgentsPsionicEvidenceRecord` records:

- evidence kind;
- status;
- model refs;
- provider refs;
- dataset refs;
- source refs;
- fixture refs;
- metric refs;
- failure refs;
- review refs;
- candidate module refs;
- optimizer refs;
- scorecard refs;
- promotion proposal refs;
- rollback refs;
- training run refs;
- retained failure refs; and
- evidence receipt refs.

## Authority Boundary

The default authority block is
`OPENAGENTS_PSIONIC_EVIDENCE_ONLY_AUTHORITY`.

It explicitly denies:

- direct module promotion;
- routing mutation;
- payout mutation;
- public claim upgrade; and
- accepted-outcome settlement.

`openAgentsPsionicEvidenceCanMutateRuntime` returns false for records using
that authority block. Psionic output is review evidence, not product
acceptance authority.

## Review Semantics

`openAgentsPsionicEvidenceNeedsReview` returns true when evidence is already
marked `needs_review` or includes candidate module refs, promotion proposals,
or rollback refs.

Completed scorecard evidence without candidates, proposals, or rollback refs
does not automatically require review.

## Projection And Redaction

Public/customer/agent projections hide operator-only review, metric, failure,
provider, source, and dataset refs. Operator/private projections can show safe
internal refs but still reject raw payload material.

The contract rejects:

- raw datasets;
- private customer data;
- provider payloads;
- secrets, bearer tokens, callback tokens, cookies, OAuth material, and API
  keys;
- wallet/payment material;
- payout targets;
- raw source archives;
- raw prompts, raw logs, raw payloads, and raw emails; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/psionic-evidence-contract.test.ts` covers:

- schema/projection decoding;
- evidence-only authority checks;
- no promotion/routing/settlement authority;
- review requirement detection;
- public redaction of operator-only refs; and
- unsafe dataset, private customer, provider payload, secret, wallet/payment,
  and timestamp rejection.
