# 2026-03-15 Psionic Apple Adapter Training Prerequisites Audit

## Intent

This audit answers the practical follow-up question:

> if OpenAgents wants to train an Apple Foundation Models adapter that is
> genuinely useful for the Psionic system in this repo, what is still missing,
> and do we need to generate datasets first?

The short answer is:

- yes, after defining the target behavior, dataset work is the first
  unavoidable step
- but dataset generation alone is not enough
- the current repo has a real narrow operator lane, yet it still falls short
  of a realistic Psionic-specialized adapter program

## Relationship To Current Repo Scope

Per `docs/MVP.md`, the current product MVP is still the inference-led provider
earn loop, not Apple adapter training as a primary product loop.

Per `docs/OWNERSHIP.md`, reusable training substrate belongs in
`crates/psionic/*`, while operator flow and UX belong in
`apps/autopilot-desktop`.

This audit is therefore a Psionic and operator-readiness document, not a claim
that Apple adapter training should replace the current MVP focus.

## Sources Reviewed

Repo sources:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
- `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
- `crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`
- `crates/psionic/docs/ROADMAP_FM.md`
- `docs/headless-compute.md`
- `docs/kernel/compute-training-authority.md`
- `docs/audits/2026-03-14-afmtrainer-apple-adapter-toolkit-integration-audit.md`
- `docs/audits/2026-03-15-repo-native-apple-training-gap-audit.md`
- `docs/audits/2026-03-15-decentralized-training-target-sequencing-audit.md`
- `crates/psionic/psionic-train/src/apple_adapter.rs`
- `crates/psionic/psionic-eval/src/apple_adapter.rs`
- `crates/psionic/psionic-data/src/apple_adapter.rs`
- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`
- `crates/psionic/fixtures/apple_adapter/datasets/*`

External reference sources:

- Apple Foundation Models adapter training overview:
  <https://developer.apple.com/apple-intelligence/foundation-models-adapter-training/>
- Apple article:
  `Creating and training custom adapters with the Adapter Training Toolkit`
- Apple article:
  `Preparing your data for training adapters`
- Apple article:
  `Loading and using a custom adapter with Foundation Models`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/docs/schema.md`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/train_adapter.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/data.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/messages.py`

## Executive Summary

The repo already has something important:

- a repo-owned Apple adapter dataset contract
- a repo-owned `.fmadapter` package and lineage contract
- a repo-owned operator flow for `launch`, `export`, and `accept`
- held-out eval plus Apple runtime-smoke validation
- accepted training outcomes in kernel authority

That is real progress, but it is not yet the same thing as a realistic
Psionic-specialized training program.

The current lane is still missing four major ingredients:

1. a real domain dataset program
2. a benchmark program that measures Psionic-specific usefulness
3. faithful Apple-compatible preprocessing and compatibility capture
4. a training backend that is closer to the real Apple math than the current
   reference-feature backend

So the correct near-term answer is:

> do not start by trying to "train the whole repo into the model."
>
> first define a narrow Psionic adapter target, then build train, held-out,
> and benchmark datasets for that target, then tighten preprocessing and
> training fidelity until the eval results are believable.

## Current Repo Reality

## 1. The repo does have a narrow Apple operator lane now

`TRAIN_SYSTEM.md`, `docs/headless-compute.md`, and
`apps/autopilot-desktop/src/apple_adapter_training_control.rs` are consistent
about the current operator story:

- import train and held-out datasets
- run repo-native Apple adapter SFT export
- stage a `.fmadapter`
- run held-out eval and runtime smoke
- optionally export
- accept the result into kernel authority

That means OpenAgents is no longer at the "paper plan only" stage.

## 2. But the current data corpus is only fixture-grade

`crates/psionic/fixtures/apple_adapter/datasets/` contains only tiny one-line
fixtures. They are enough to freeze schema and test parsing, not enough to
teach Psionic-specific behavior.

So if the question is:

> can we train a Psionic-focused adapter from data that already exists in the
> repo?

the honest answer is:

- not yet in any realistic sense
- the repo has format fixtures, not a production-quality corpus

## 3. The current desktop operator flow still injects synthetic lineage

`apps/autopilot-desktop/src/apple_adapter_training_control.rs` currently:

- hard-codes a base-model signature
- hard-codes tokenizer and prompt-shaping digests
- loads dataset metadata with those fixed values
- estimates token counts from character counts in `derive_captures(...)`

That is good enough for a bounded reference lane.
It is not yet good enough for a trustworthy Psionic training program where the
lineage must match the actual Apple-compatible preprocessing path.

## 4. The current backend is explicitly a reference backend

`crates/psionic/psionic-train/src/apple_adapter.rs` repeatedly describes the
current path as a reference backend.

The code path today:

- builds deterministic prompt and target feature vectors
- seeds synthetic matrices from stable strings
- trains LoRA groups against those feature vectors
- exports a valid `.fmadapter` package

That is valuable because it proves:

- OpenAgents can own the packaging, receipts, lineage, eval hooks, and control
  plane

But it does not yet prove:

- OpenAgents is fine-tuning against the real Apple Foundation Model internals
- the learned adapter weights are the result of a faithful Apple-compatible
  forward and backward path

For a realistic Psionic adapter, that distinction matters a lot.

## Direct Answer: Do We Need To Generate Datasets First?

Yes, with one clarification:

- step `0` is choosing the narrow behavior target
- step `1` is generating and curating the dataset program

Without that target, "generate a dataset" is too vague and will produce
low-signal junk.

Without the dataset, there is nothing meaningful to train.

More concretely, you need at least three splits:

- `train`
  what the adapter learns from
- `held_out`
  what catches overfitting during iteration
- `benchmark`
  what decides whether the adapter is actually useful for Psionic work

Apple's own toolkit references also assume this shape:

- prepared JSONL training data
- optional evaluation data
- checkpointed training loops with repeated eval

So yes, dataset work is first-class, not optional cleanup.

## What The Psionic Adapter Should Actually Learn

The wrong target is:

- "memorize the whole repo"

That will go stale quickly and compete with retrieval.

The better target is:

- stable Psionic architecture vocabulary
- crate-boundary discipline
- truthful statements about what is implemented versus planned
- common maintainer and operator workflows
- tool-calling or structured-output patterns for Psionic tasks

In other words:

> train the adapter on stable behavior and domain style, and use retrieval for
> volatile repo state.

That is a much better fit for an adapter than trying to bake every current file
detail into weights.

## Required Workstreams For A Real Psionic Adapter

## 1. Define one narrow adapter product first

Before data work, choose one bounded role such as:

- `Psionic architecture explainer`
- `Psionic training-operator assistant`
- `Psionic code-review and ownership-boundary assistant`
- `Psionic Apple adapter operator assistant`

Do not start with "general OpenAgents super-brain."

A narrow role gives you:

- clearer examples
- clearer eval
- less stale data
- less chance of teaching contradictory behavior

## 2. Build a real dataset program

You will need a curated corpus, not just raw repo dumps.

Likely sources:

- canonical docs:
  `ROADMAP.md`, `TRAIN_SYSTEM.md`, `ARCHITECTURE.md`, `OWNERSHIP.md`,
  `MVP.md`, `compute-training-authority.md`, `headless-compute.md`
- Apple-adapter operator docs and runbooks
- CLI examples and expected command explanations
- code-grounded architecture examples from stable crate APIs
- positive and negative examples from prior audits

For a Psionic-focused adapter, the most useful record families are:

- doc-grounded Q/A
- "what owns this?" boundary questions
- "what is implemented vs planned?" truthfulness examples
- tool-calling examples for local docs or artifact lookup
- structured summaries of runs, eval results, or receipts
- refusal and correction examples when the evidence is missing or stale

The important rule is:

- synthetic generation is fine as a bootstrapping tool
- synthetic generation without maintainer review is not enough

## 3. Add negative and refusal data, not only happy paths

For this repo, realism requires examples teaching the adapter not to:

- claim distributed Apple training when the lane is single-host
- confuse the FM bridge with the training engine
- pull archived Backroom code by default
- violate `docs/OWNERSHIP.md`
- market training features as current MVP truth

A Psionic adapter that answers fluently but overclaims will be worse than the
base model plus retrieval.

## 4. Build a real eval program before scaling training

Current held-out eval and runtime smoke are necessary, but not sufficient.

You also need benchmark tasks such as:

- architecture Q/A scored against canonical docs
- boundary classification:
  "which crate owns this?"
- operator workflow correctness:
  "what command comes next?"
- truthfulness checks:
  "implemented, partial, or planned?"
- structured output conformance for run summaries or manifests
- tool-calling correctness when a lookup tool is involved

The key acceptance question should be:

> is the adapted model measurably better than the base model on Psionic tasks?

If you cannot answer that, you do not yet have a real adapter program.

## 5. Replace synthetic compatibility placeholders with real lineage capture

Before calling the result realistic, OpenAgents needs to derive, not invent:

- base-model compatibility anchors
- tokenizer digest
- prompt-template or default-instruction digest
- locale-sensitive prompt-shaping behavior
- packing and token-count truth

Today the app still uses hard-coded signatures and estimated token counts.

That means the next honest step is a real compatibility-capture path based on
the Apple toolkit references and the actual runtime assumptions, then carrying
that truth through `psionic-data`, `psionic-train`, `psionic-adapters`, and
kernel authority.

## 6. Close the training-fidelity gap

This is the biggest technical gap.

If the goal is a believable Psionic adapter, OpenAgents needs more than package
export parity. It needs a trainer that is much closer to the real Apple lane.

Right now the backend is useful as:

- a control-plane proof
- a lineage proof
- a package-generation proof
- an eval and runtime-smoke proof

It is not yet enough as:

- a high-confidence replacement for Apple's own adapter-training math

So the realistic options are:

- implement a much more faithful Rust-native Apple-compatible trainer
- or use the Apple toolkit only as an offline development oracle while the
  repo-native path is being validated

The second option can help development, but it should not become a hidden
runtime dependency if the repo keeps the zero-Python rule.

## 7. Add training iteration discipline

A real Psionic adapter program also needs:

- repeatable hyperparameter configs
- checkpoint comparison and model selection
- benchmark trend tracking
- data versioning and split freeze discipline
- regression detection after doc or codebase drift

Right now the operator lane proves one bounded path.
A real adapter program needs repeated experiment management.

## 8. Keep runtime validation in scope from day one

Apple's own adapter docs make the runtime side explicit too:

- training uses the Adapter Training Toolkit
- loading and local use flow through Foundation Models
- custom adapters depend on the Apple-side runtime and Background Assets story

So a realistic OpenAgents program cannot stop at "the package exported."

It also needs:

- repeatable bridge-backed load and attach checks
- structured-output and tool-calling validation against the live runtime
- explicit compatibility tracking for the targeted Apple base-model family
- a plan for what happens when Apple updates the underlying model or assets

Otherwise the training loop can produce a package that is internally consistent
for OpenAgents but operationally brittle on the actual Apple runtime.

## Recommended Sequence

If the goal is one realistic Psionic-focused Apple adapter, the dependency
order should be:

1. choose one narrow target role
2. assemble source corpus from stable docs and operator workflows
3. generate and curate `train`, `held_out`, and `benchmark` splits
4. build benchmark scoring against the base model first
5. tighten tokenizer, prompt-shaping, and compatibility capture
6. improve training fidelity beyond the current reference-feature backend
7. iterate until the adapted model beats the base model on the benchmark
8. only then worry about decentralization, marketplace surfaces, or broader
   training families

## Practical Recommendation For The First Psionic Dataset

If starting immediately, the best first dataset is probably not raw code.

It should be a mixed corpus built from:

- canonical architecture docs
- operator docs and CLI workflows
- ownership-boundary examples
- train-system truthfulness examples
- Apple-adapter operator examples

And it should produce at least these sample families:

- short factual Q/A
- longer architectural explanations
- "what should I do next?" operator answers
- structured summary outputs
- refusal or correction outputs for unsupported claims

That gives the adapter a job that is:

- stable enough to train
- narrow enough to evaluate
- useful enough to matter

## Bottom Line

Yes, you need dataset generation first, after defining the target.

But the bigger realistic answer is:

- dataset
- held-out plus benchmark eval
- real lineage capture
- and a more faithful trainer

all have to arrive before a "Psionic Apple adapter" is something we should
trust instead of just something we can export.
