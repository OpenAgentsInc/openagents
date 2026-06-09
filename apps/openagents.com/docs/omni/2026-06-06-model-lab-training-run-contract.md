# Model Lab Training Run Contract

Status: implemented for issue #381 / `OPENAGENTS-LAB-002`.

## Purpose

Model Lab needs to describe fine-tune, adapter, eval-only, optimizer,
distillation, benchmark replay, and data-preparation runs without claiming that
OpenAgents product surface launched training, mutated a provider, installed an adapter, spent money,
promoted a runtime, routed production traffic, settled payouts, or upgraded a
public claim. The `TrainingRun` contract records observed or imported evidence
and candidate outputs only.

Implementation:

- `workers/api/src/omni-model-lab-training-run.ts`
- `workers/api/src/omni-model-lab-training-run.test.ts`

## Training Run Record

The record carries:

- run identity, kind, and state;
- triggering workroom, retained failure, Model Lab loop, candidate, optimizer,
  and source refs;
- data package refs;
- model artifact refs;
- eval rerun and benchmark refs;
- provider and runner refs;
- hyperparameter summaries with evidence refs;
- metric summaries with evidence refs;
- budget/cost caveats and credit refs; and
- operator review receipt refs.

Projection timestamps use friendly labels and do not expose raw ISO strings.

## Authority Boundaries

Training runs cannot:

- launch model training;
- mutate provider state;
- install adapters;
- copy raw datasets;
- spend money;
- promote runtime behavior;
- mutate routing;
- mutate payouts;
- mutate settlement; or
- upgrade public claims.

Payment, training, provider, routing, deployment, and settlement actions require
separate server-authoritative workflows with explicit approvals and receipts.

## Validation Rules

All training runs require source and evidence refs. Fine-tune, adapter,
distillation, and data-preparation runs require data package refs.

Running evidence requires provider and runner refs. Failed runs require failure
refs. Blocked runs require caveat refs. Completed and reviewed runs require
artifact refs, metrics, and eval or benchmark evidence. Reviewed runs require
operator review receipt refs.

Hyperparameters and metrics must each have evidence refs. Budget records cannot
grant payment spend; observed costs require credit or cost evidence refs.

## Projection Audiences

Supported audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public/customer/agent projections redact private artifact, benchmark, budget,
candidate, data package, eval, failure, hyperparameter, metric, provider, run,
runner, source, training, and workroom refs as appropriate. Operator and team
projections can retain the safe ref set, but all projections reject private
prompts, source archives, private datasets, provider payloads, model weights,
secrets, payment or wallet material, private repositories, raw logs, and raw
timestamps.

## Tests

Coverage includes:

- reviewed training-run projection;
- status progression and readiness labels;
- metric, hyperparameter, budget, artifact, eval, benchmark, and review receipt
  validation;
- public redaction; and
- hard false training-launch, provider-mutation, adapter-install,
  raw-dataset-copy, payment-spend, runtime-promotion, routing, payout,
  settlement, and public-claim mutation authority.
