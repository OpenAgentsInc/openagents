# 2026-03-15 GEPA Psionic Economy Kernel Integration Audit

## Intent

This audit answers a specific integration question:

> after reading the current `crates/psionic/*` specs and `~/code/gepa`, how
> should OpenAgents use GEPA to improve Psionic and the economy kernel without
> breaking the repo's Rust-native, receipt-first, authority-split architecture?

The useful answer is not:

- "embed the GEPA Python runtime into kernel authority"
- "let GEPA mutate the whole repo"
- "treat optimization runs as canonical market truth"

The useful answer is:

- use GEPA first as an optional controller-side search engine over bounded,
  typed Psionic experiment surfaces
- keep Psionic as the execution-truth layer
- keep kernel and Nexus as the accepted-outcome and settlement-truth layer
- let only promoted artifacts, accepted evals, and accepted training outcomes
  cross into kernel truth

That is the line this audit makes concrete.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`
- `crates/psionic/docs/RESEARCH_EXPERIMENT_REFERENCE.md`
- `crates/psionic/docs/TRAIN_CURRICULUM_REFERENCE.md`
- `crates/psionic/docs/TRAIN_OFF_POLICY_BUDGET_REFERENCE.md`
- `crates/psionic/psionic-research/src/lib.rs`
- `crates/psionic/psionic-research/src/runner.rs`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/markets/compute-market.md`
- `docs/kernel/markets/labor-market.md`
- `docs/kernel/markets/data-market.md`
- `docs/kernel/markets/liquidity-market.md`
- `docs/kernel/markets/risk-market.md`
- `docs/kernel/compute-evaluation-runs.md`
- `docs/kernel/compute-synthetic-data.md`
- `docs/kernel/compute-training-authority.md`

GEPA sources reviewed:

- `~/code/gepa/README.md`
- `~/code/gepa/src/gepa/optimize_anything.py`
- `~/code/gepa/src/gepa/core/adapter.py`
- `~/code/gepa/src/gepa/core/engine.py`
- `~/code/gepa/src/gepa/core/state.py`
- `~/code/gepa/src/gepa/core/result.py`
- `~/code/gepa/src/gepa/core/callbacks.py`
- `~/code/gepa/src/gepa/proposer/reflective_mutation/reflective_mutation.py`
- `~/code/gepa/src/gepa/proposer/merge.py`
- `~/code/gepa/src/gepa/adapters/optimize_anything_adapter/optimize_anything_adapter.py`
- `~/code/gepa/src/gepa/gskill/README.md`
- `~/code/gepa/examples/adrs/cloudcast/main.py`
- `~/code/gepa/examples/adrs/can_be_late/main.py`
- `~/code/gepa/docs/docs/blog/posts/2026-02-18-introducing-optimize-anything/index.md`

## Executive Summary

GEPA is a strong fit for Psionic, but only if it is integrated at the right
layer.

The most important observation from this review is structural:

> Psionic already has a Rust-native substrate that looks like the thing GEPA
> wants underneath it.

That substrate already exists in:

- `psionic-research`
  - typed `ExperimentSpec`, `ExperimentResult`, `ExperimentScoreContract`, and
    `PromotionRecord`
- `psionic-sandbox`
  - bounded execution and receipts
- `psionic-eval`
  - environment-bound evaluation
- `psionic-train`
  - training policy, rollout, validator, curriculum, and off-policy truth
- `psionic-runtime` / `psionic-serve` / `psionic-datastream`
  - machine-legible execution evidence, scheduler posture, and artifact
    receipts

So the main opportunity is not "find a place to stuff GEPA."

The main opportunity is:

> let GEPA drive search over bounded Psionic experiment families that are
> already typed, measurable, replay-safe, and promotable.

The highest-value immediate integration targets are already named in
`psionic-research`:

- `ServingScheduler`
- `BackendTuning`
- `DatastreamTransfer`
- `SandboxWarmPool`
- `TrainingPolicy`
- `ValidatorPolicy`
- `EnvironmentMix`

That is a much better starting point than full model-weight search, arbitrary
repo mutation, or direct market-policy mutation.

The economy kernel also benefits, but indirectly rather than architecturally.
GEPA helps the kernel most when it raises:

- verifiable share
- verification throughput
- environment and benchmark quality
- labor-market task success
- calibration of validator and risk policies

It should not become an authority participant itself.

The recommended plan is:

1. integrate external GEPA first as an optional app/operator-side optimizer
   over `psionic-research` contracts
2. use Psionic runners, receipts, and eval/train substrates as the evaluator
   backend
3. publish only promoted artifacts, canonical eval runs, and accepted training
   outcomes into kernel truth
4. only after that, consider a Rust-native GEPA-style search engine inside
   `psionic-research` or a sibling crate

## What GEPA Actually Contributes

Reading `~/code/gepa` closely, the important value is not "prompt tuning."

The durable ideas are:

- `Actionable Side Information`
  - the evaluator returns rich diagnostics, not only a scalar
- Pareto-efficient candidate selection
  - candidates survive because they are best on some task or metric, not only
    by average score
- generalization mode
  - train on one set of scenarios, validate on held-out scenarios
- bounded merge and lineage
  - specialized candidates can be recombined rather than discarded
- evaluator caching and resumable run state
  - expensive rollouts are reused instead of rerun blindly
- callback/instrumentation surfaces
  - full optimization lifecycle can be observed externally

GEPA also proves it is useful outside prompt work:

- `gskill` learns repository-specific coding-agent skills
- `cloudcast` evolves routing algorithms
- `can_be_late` evolves cloud scheduling policies
- `optimize_anything` treats code, configs, policies, and agent architectures
  as search surfaces

That matters for OpenAgents because Psionic and the kernel are exactly about
machine-execution policy, environment policy, validation policy, and agent
behavior under receipts.

## Why GEPA Fits Psionic Unusually Well

The fit is better than it looks at first glance because the OpenAgents side is
already more typed than GEPA's general interface expects.

| GEPA concept | Psionic-native counterpart | Integration reading |
| --- | --- | --- |
| candidate text artifact | `ExperimentSpec.family` plus `CandidateMutation` | candidate text should serialize a typed Psionic policy or bounded lab artifact |
| evaluator + dataset/valset | `ResearchRunner`, `psionic-eval`, benchmark packages, environment packages | GEPA should call real Psionic runs rather than invent a second execution stack |
| ASI / side info | runtime observability, proof bundles, sandbox receipts, validator receipts, eval summaries | Psionic already emits the diagnostics GEPA wants |
| optimization budget | `ExperimentBudget` | fixed wall-time, step, and sample budgets are already first-class |
| Pareto frontier | app-owned frontier controller plus `PromotionRecord` | keep frontier state outside kernel truth, but record promotion decisions durably |
| accepted winner | `PromotionRecord`, `ComputeEvaluationRun`, `ComputeTrainingRun`, `ComputeAcceptedOutcome` | market and operator truth should only see promoted/accepted candidates |

This is the most important architectural conclusion of the audit:

> OpenAgents does not need GEPA to define a new object model first. It already
> has one in `psionic-research`.

What it lacks is the search controller and evaluator adapters.

## Best Near-Term GEPA Targets Inside Psionic

These are the best targets because they are already typed, bounded, and
measurable.

### 1. Serving scheduler policy

Why this is a strong fit:

- `ARCHITECTURE.md` and `ROADMAP.md` both call out queue admission,
  prefill/decode balance, cache behavior, and routing posture as unfinished
  execution-truth work
- `psionic-research` already has a `ServingScheduler` family
- GEPA already has real examples of scheduling-policy discovery in
  `cloudcast` and `can_be_late`

Likely candidate surfaces:

- batch-token ceilings
- active-sequence limits
- prefill/decode share
- queue slack
- warm/cold placement thresholds
- cache admission and reuse thresholds

Likely score dimensions:

- throughput
- p95/p99 latency
- TTFT/ITL
- memory peak
- correctness/conformance pass rate
- receipt completeness

### 2. Backend tuning policy

Why this is a strong fit:

- Psionic still has backend-truth and performance closure work across CPU,
  Metal, CUDA, and later AMD
- `psionic-research` already has a `BackendTuning` family
- GEPA can optimize code/config search spaces with compiler and benchmark
  diagnostics as ASI

Likely candidate surfaces:

- compile cache mode
- chunk/token sizing
- fusion on/off
- compile parallelism
- backend-specific kernel knobs in bounded lab areas

Likely score dimensions:

- compile latency
- run latency
- throughput
- determinism/refusal correctness
- crash rate

### 3. Datastream transfer policy

Why this is a strong fit:

- `TRAIN_SYSTEM.md` treats weight broadcast, checkpoint transport, and artifact
  freshness as core train-system work
- `psionic-research` already has a `DatastreamTransfer` family
- `psionic-datastream` already has manifests, resumable cursors, and delivery
  receipts

Likely candidate surfaces:

- chunk sizing
- mirror selection
- resume thresholds
- freshness cutoffs
- relay/direct path preferences

Likely score dimensions:

- bytes/sec
- completion rate
- recovery time after interruption
- stale-artifact rejection correctness
- mirror cost

### 4. Sandbox warm-pool policy

Why this is a strong fit:

- `psionic-sandbox` already owns warm reusable pools and repeated iteration
  receipts
- `psionic-research` already has a `SandboxWarmPool` family
- `TRAIN_SYSTEM.md` explicitly says RL/post-training throughput still needs
  work

Likely candidate surfaces:

- pool size
- reuse TTL
- runtime preference
- prewarm thresholds
- retry/quarantine policy

Likely score dimensions:

- cold-start latency
- job success rate
- pool utilization
- tail latency
- isolation failures

### 5. Training policy

Why this is a strong fit:

- `TRAIN_SYSTEM.md` makes training policy a named surface
- `psionic-research` already has a `TrainingPolicy` family
- recent train references landed curriculum, off-policy budgeting,
  scheduling/accounting, rollout verification, and policy-weight broadcast
  substrate

Likely candidate surfaces:

- checkpoint cadence
- max rollout age
- contributor cap
- curriculum weighting
- gradient clipping
- halt thresholds

Likely score dimensions:

- trainer throughput
- held-out eval quality
- stability signals
- replay correctness
- validator cost per accepted unit

### 6. Validator policy

Why this is a strong fit:

- `TRAIN_SYSTEM.md` treats validator posture as one of the core train-system
  control surfaces
- the economy kernel makes verification a production resource and correlation
  risk a first-class concern
- `psionic-research` already has a `ValidatorPolicy` family

Likely candidate surfaces:

- cheap-check vs expensive-check sampling rates
- duplicate detection thresholds
- quarantine posture
- challenge escalation thresholds
- environment- or benchmark-specific validator mix

Likely score dimensions:

- false accept / false reject rate
- validator runtime cost
- adjudication latency
- correlation exposure
- accepted-outcome quality on held-out audits

### 7. Environment mix

Why this is a strong fit:

- `TRAIN_SYSTEM.md` and kernel docs both treat environment packages and
  benchmark packages as first-class
- `psionic-research` already has an `EnvironmentMix` family
- GEPA's generalization mode is naturally aligned with scenario mixture search

Likely candidate surfaces:

- package mix
- difficulty mix
- rubric mix
- synthetic-vs-real sample ratios
- benchmark family mix

Likely score dimensions:

- generalization quality
- failure diversity
- verification throughput
- scenario coverage
- synthetic-practice usefulness

## Where GEPA Helps The Economy Kernel Most

The kernel benefit is real, but it is downstream.

The kernel docs repeatedly emphasize that the scarce resource is not only model
quality. It is:

- verifiable share
- verification independence
- provenance quality
- synthetic practice capacity
- trustworthy compute delivery

GEPA helps all of those if it is used upstream of authority.

### 1. Higher verifiable share, not just better raw models

The biggest kernel-level benefit is to `sv`.

GEPA can search for:

- better validator policies
- better environment mixtures
- better benchmark package composition
- better labor-agent skills
- better runtime/scheduler policies that reduce failure and timeout rates

That means more work can reach an accepted and verified outcome at a lower
verification cost.

This is more important to the kernel than any single accuracy number.

### 2. Stronger synthetic-practice and ground-truth loops

`economy-kernel.md` treats GroundTruthCase -> SimulationScenario as core
infrastructure, and the kernel now already has:

- `ComputeSyntheticDataJob`
- `ComputeEvaluationRun`
- environment-bound evaluation and verification lifecycles

GEPA is a natural fit for optimizing:

- teacher prompts and generation policies
- scenario-generation recipes
- benchmark-adapter behavior
- rubric-specific environment mixtures
- verifier practice curricula

That directly helps the kernel scale verification capacity without loosening
authority rules.

### 3. Better compute-market products and comparability

The compute market is increasingly about execution under artifact lineage, not
just "GPU time."

GEPA helps by improving the policies that determine:

- scheduler posture
- transfer behavior
- sandbox readiness
- validator posture
- environment compatibility

If those policies are optimized through bounded Psionic experiments and then
promoted through accepted eval/train outcomes, the compute market gets:

- better deliverability
- better comparability
- better evidence quality
- lower failure rates

That improves both market truth and operator truth.

### 4. Better labor-market outcomes through repo-specific skills

`gskill` is directly relevant to OpenAgents.

The labor market already has starter authority objects:

- `WorkUnit`
- `Contract`
- `Submission`
- `Verdict`

GEPA-style skill learning can improve the labor lane by optimizing:

- repository-specific coding skills
- domain-specific task playbooks
- critique/checklist prompts
- multi-step work protocols

The kernel win is not "GEPA inside labor authority."

The kernel win is:

- higher submission pass rate
- lower remediation cost
- better verdict confidence
- stronger provenance on why a worker or skill package is trustworthy

### 5. Better risk signals and validator calibration

The risk market should not let GEPA set claims or pay claims.

But GEPA can help calibrate:

- challenge policies
- validator mixtures
- threshold choices
- false-positive/false-negative tradeoffs

That helps the risk layer produce better bounded signals without violating the
kernel's rule that verification and claims remain authority-owned.

### 6. Limited but real value for liquidity and routing simulation

Liquidity is the weakest immediate fit.

GEPA could still be useful for simulated routing and quote-policy research,
but only as a bounded advisory or benchmarking loop.

It should not sit in live settlement paths or envelope issuance logic.

## The Main Architectural Guardrails

This integration is only good if these guardrails stay hard:

### 1. GEPA must not become authority

It must not:

- mutate kernel state directly
- bypass authenticated HTTP authority flows
- produce settlement truth
- decide accepted outcomes by itself

Only kernel and Nexus do that.

### 2. Frontier state should stay outside kernel truth

Raw optimization state is not canonical economic state.

Keep/discard/branch experiments belong in:

- app-owned controller state
- `psionic-research` experiment/result/promotion records
- optional operator read models

Kernel should only receive:

- promoted artifacts
- canonical eval runs
- accepted training runs
- accepted outcomes

### 3. Candidate surfaces should stay typed and bounded

Do not start with:

- arbitrary whole-repo mutation
- receipt schema mutation
- proto mutation
- settlement or policy-bundle semantics mutation

Start with:

- typed policy structs
- config-like experiment families
- bounded lab-code areas only when necessary

### 4. Every optimization run must bind to the same truth surfaces as normal Psionic work

That means:

- runtime profile
- sandbox profile
- artifact refs and digests
- environment version
- score-contract digest
- receipt refs
- stdout/stderr digests

If the run cannot be replayed and audited as a Psionic experiment, it should
not be promotable.

### 5. Held-out eval remains mandatory

GEPA is powerful enough to overfit.

Promotion should require:

- held-out evaluation
- explicit hard gates
- promotion reasons
- accepted outcome only after recheck where economically important

`psionic-research::PromotionRecord` already points in exactly this direction.

### 6. Do not pull Python into the core execution path

GEPA today is Python.

That is acceptable for:

- controller-side prototyping
- optional offline optimization campaigns
- external research harnesses

It is not acceptable as the hidden source of truth for:

- Psionic execution
- training runtime
- kernel authority
- market settlement

## Suggested Integration Roadmap

The right sequence is incremental.

### Phase 0: Freeze the owner split and the initial target set

Goals:

- make a repo-level decision that GEPA is controller-side only
- adopt the existing `psionic-research` experiment families as the first-class
  search surfaces
- avoid new kernel object proliferation until there is a proven need

Concretely:

- use `ExperimentFamilyKind` as the integration boundary
- use `ExperimentSpec`, `ExperimentResult`, and `PromotionRecord` as the
  canonical local experiment vocabulary
- keep raw optimization runs out of kernel truth

### Phase 1: Build a thin GEPA <-> Psionic bridge

Goals:

- let external GEPA drive bounded Psionic experiments without inventing a new
  runtime

Concretely:

- serialize each typed `ExperimentFamily` into a deterministic text candidate
  for GEPA
- create evaluator adapters that:
  - materialize an `ExperimentSpec`
  - invoke `psionic-research::ResearchRunner` or a thin CLI wrapper
  - collect `ExperimentResult`
  - translate receipts, metrics, and failure details into GEPA side info
- persist keep/discard/branch/promote decisions as `PromotionRecord`

This is the first real proof point.

### Phase 2: Productize the first three experiment packs

Start with:

1. `ServingScheduler`
2. `SandboxWarmPool`
3. `DatastreamTransfer`

Why these first:

- they are high-value for the compute-provider MVP
- they are already bounded and measurable
- they require no new market family
- they exercise runtime, sandbox, and data-plane receipts immediately

Deliverables:

- canonical score contracts
- held-out scenario suites
- operator-visible comparison reports
- promotion gates for accepted winners

### Phase 3: Extend into train/eval policy search

Next add:

1. `TrainingPolicy`
2. `ValidatorPolicy`
3. `EnvironmentMix`

This phase should reuse the work already landed in:

- curriculum
- off-policy budgeting
- rollout admission
- benchmark packages
- training/eval authority registries

Concretely:

- score candidates on held-out eval quality, stability, validator cost, and
  freshness correctness
- require explicit hard gates before promotion
- bind promotions to canonical benchmark/environment versions

### Phase 4: Wire promoted winners into existing kernel authority paths

The default rule should be:

> no new kernel GEPA object unless existing eval/train/registry surfaces prove
> insufficient.

Use the current kernel objects first:

- `ComputeEvaluationRun`
- `ComputeTrainingRun`
- `ComputeAcceptedOutcome`
- training-policy / validator-policy / benchmark-package registries
- `source_ref`, `run_artifacts`, and metadata extensions where appropriate

This keeps the kernel clean and respects the authority split.

### Phase 5: Add labor-market skill optimization as a sibling lane

Once Psionic policy search is working, add a separate controller-side lane for:

- coding-agent skill optimization
- domain work-playbook optimization
- repo-specific task protocols

This should target:

- labor success metrics
- verdict pass rates
- runtime cost and latency

The result should be promotable skill artifacts with evaluation provenance, not
raw GEPA run state.

### Phase 6: Only then consider a Rust-native GEPA-style optimizer

If Phases 1-5 show clear value, the next step is not "depend on Python
forever."

The next step is:

- implement a Rust-native reflective search engine inspired by GEPA
- place it in `psionic-research` or a sibling crate
- keep the same typed contracts:
  - `ExperimentSpec`
  - `ExperimentResult`
  - `ExperimentScoreContract`
  - `PromotionRecord`

This would preserve the good parts of GEPA:

- ASI
- Pareto search
- merge/branch
- held-out generalization

without pulling Python into the core execution stack.

## Recommended Priority Order

If the goal is practical value rather than conceptual completeness, the order
should be:

1. serving scheduler policy
2. sandbox warm-pool policy
3. datastream transfer policy
4. validator policy
5. training policy
6. environment mix
7. labor skill optimization
8. Rust-native optimizer port

Do not start with:

- arbitrary source mutation
- full weight-search systems
- kernel authority mutation
- liquidity-market optimization

## Bottom Line

GEPA is not a replacement for Psionic or the economy kernel.

It is a strong search layer for them.

The key architectural reading is:

> GEPA should optimize bounded Psionic experiment families, using Psionic
> receipts and eval/train truth as the evaluator substrate, while kernel and
> Nexus remain the only accepted-outcome and settlement authorities.

That gives OpenAgents the best of both systems:

- GEPA's reflective Pareto search and skill/policy discovery
- Psionic's typed, Rust-native, replay-safe execution truth
- the kernel's deterministic, authority-owned market truth

If integrated that way, GEPA can materially improve:

- compute-market deliverability
- verification throughput
- synthetic-practice generation
- labor-market task success
- validator calibration

without violating the repo's core architecture.
