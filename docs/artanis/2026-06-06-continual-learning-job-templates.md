# Artanis Continual-Learning Job Templates

Status: implemented for proposal and evidence plumbing. Not a training,
promotion, provider, payment, or Pylon-dispatch executor.

Issue: #411 / ARTANIS-025.

## Purpose

Artanis needs a repeatable way to propose Autopilot-improvement work without
turning proposal text into execution authority. The continual-learning template
contract gives Artanis a bounded vocabulary for Model Lab and Pylon work:

- benchmark evaluation reruns;
- DSPy/GEPA prompt or program optimization;
- dataset curation;
- adapter validation;
- executor-trace replay/validation;
- LoRA fine-tuning or training;
- regression analysis.

Each template can be projected publicly as a safe work category and privately
as an operator-ready proposal. Public projections are suitable for `/artanis`
and Forum summaries. Operator projections preserve private operator refs by
reference only.

## Template Evidence

Every template records:

- benchmark target refs;
- acceptance criteria refs;
- retained-failure refs;
- Model Lab evidence graph refs;
- model artifact refs;
- training run refs;
- Benchmark Cloud refs;
- promotion decision refs;
- public report refs;
- dispatch payload schema refs;
- required capability refs;
- spend limit refs;
- workload refs;
- cost caveat refs;
- risk labels;
- rollback posture refs;
- approval requirement refs.

The contract rejects raw prompts, raw datasets, raw weights, provider payloads,
private repo data, customer data, wallet/payment material, raw runner logs,
secrets, and raw timestamps. Public projections also redact operator/private
refs.

## Pylon Proposal Bridge

Artanis can turn a template into a Pylon marketplace intake request and a
triage proposal:

- benchmark rerun -> `benchmark_evaluation`;
- DSPy/GEPA optimization -> `gepa_dspy_optimization`;
- dataset curation -> `embedding_data_prep`;
- adapter validation -> `validation`;
- executor-trace replay -> `validation`, with
  `capability.tassadar_poc.numeric_model_executor`, Tassadar request/output
  schema refs, the bounded workload ref, and a zero-sats default spend cap;
- LoRA fine-tuning or training -> `lora_finetuning`;
- regression analysis -> `artifact_review`.

Those payloads are still proposals. They do not dispatch work, charge buyers,
spend bitcoin, mutate provider accounts, launch training, promote a model, or
settle payouts.

## Authority Boundary

The template ledger carries explicit false authority for:

- Pylon dispatch;
- benchmark launch;
- training launch;
- adapter install;
- provider mutation;
- model promotion;
- runtime promotion;
- report publication;
- payment spend.

High-risk templates such as adapter validation and LoRA/fine-tuning/training
cannot move to running or accepted state unless they carry both operator
approval refs and downstream executor authority refs. That is still only an
evidence gate inside this contract; the executor must enforce its own authority
before it runs anything.

## Forum Summaries

Forum-safe projections can describe continual-learning work as:

- blocked;
- proposed;
- running;
- accepted;
- rejected.

Summaries must stay at the public reference level. They may explain which
category is being proposed, what evidence refs exist, which caveats apply, and
what operator approval is required. They must not include raw prompts,
datasets, weights, provider payloads, private repos, customer data, or secrets.
