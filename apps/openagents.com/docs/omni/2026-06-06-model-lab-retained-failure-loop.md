# Model Lab Retained-Failure Loop

Status: implemented for issue #376 / `OPENAGENTS-LATE-016`.

## Purpose

Retained failures should feed Model Lab improvement work without becoming
automatic runtime promotion. This contract records how failures produce
signature/model candidates, eval reruns, adapter-validation evidence,
promotion gates, rollback posture, and attribution while denying model
training, deployment, promotion, routing, payout, settlement, and public-claim
side effects.

Implementation:

- `workers/api/src/omni-model-lab-retained-failure-loop.ts`
- `workers/api/src/omni-model-lab-retained-failure-loop.test.ts`

## Loop Records

The loop record carries:

- retained failure records;
- candidate records;
- eval rerun records;
- adapter validation records;
- promotion gate records;
- attribution records;
- source refs;
- blocker refs;
- caveat refs; and
- authority.

Projection timestamps use friendly labels and never expose raw ISO strings.

## Lifecycle States

Supported states:

- `retained`;
- `candidate_created`;
- `eval_rerun`;
- `adapter_validated`;
- `gate_passed`;
- `attributed`;
- `blocked`; and
- `archived`.

Each later state requires the evidence that justifies it. Candidate-created
loops require candidates, eval-rerun loops require passed evals,
adapter-validated loops require passed adapter validations, gate-passed loops
require passed gates, and attributed loops require recorded attribution.
Blocked loops require blocker refs.

## Retained Failures

Retained failures record failure kind, failure ref, workroom refs, trace refs,
source refs, evidence refs, and redaction policy refs. They require evidence,
source, and redaction policy refs. The contract stores failure and trace
material as refs only and rejects raw prompts, raw traces, source archives, and
customer data.

## Candidates And Eval Reruns

Candidates support program signatures, module versions, model adapters, eval
fixtures, and prompt policies. Proposed/reviewed candidates require source
failure and evidence refs, and every source failure ref must point to a
retained failure in the same loop.

Eval reruns must link to same-loop candidates and retained failures. Passed
evals require receipt, evidence, fixture, and scorecard refs.

## Adapter Validation

Adapter validations link candidate refs to adapter, dataset, provider,
evidence, and receipt refs. Passed adapter validations require adapter,
provider, evidence, and receipt refs.

This is validation evidence only; it does not install adapters.

## Promotion Gates And Rollback

Promotion gates link candidates, eval reruns, adapter validations, policy refs,
review receipt refs, rollback refs, and a rollback posture. Passed gates
require:

- passed eval refs;
- passed adapter validation refs when adapter validation is cited;
- policy refs;
- evidence refs;
- review receipt refs;
- rollback refs;
- `ready` or `verified` rollback posture; and
- no self-promotion attempt.

The projection still reports `runtimePromotionAllowed: false`.

## Attribution

Attribution records link candidates to accepted outcome refs, contributor refs,
receipt refs, and caveats. Recorded attribution requires a passed gate,
accepted outcome refs, contributor refs, and receipt refs. It is attribution
evidence, not payout authority.

## Authority Boundaries

Model Lab retained-failure loops cannot:

- execute evals;
- mutate model training;
- install adapters;
- promote runtime behavior;
- mutate routing;
- mutate payouts;
- mutate settlement; or
- upgrade public claims.

Any training, deployment, promotion, or payout workflow must be a separate
server-authoritative path with explicit approval and receipts.

## Projection Audiences

Supported audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public and agent projections redact private adapter, candidate, dataset, eval,
failure, gate, provider, receipt, review, rollback, source, target, trace, and
workroom refs as appropriate. Operator and team projections can retain the
full safe ref set, but all projections reject private prompts, source archives,
provider payloads, customer data, secrets, payment/wallet material, private
repos, raw logs, raw traces, and raw timestamps.

## Tests

Coverage includes:

- full retained-failure loop projection;
- lifecycle separation;
- retained failure validation;
- candidate and eval linkage;
- passed eval and adapter validation requirements;
- promotion gate requirements;
- rollback posture and no-self-promotion checks;
- attribution receipt requirements;
- public redaction; and
- hard false eval, training, adapter install, runtime promotion, routing,
  payout, settlement, and public-claim mutation authority.
