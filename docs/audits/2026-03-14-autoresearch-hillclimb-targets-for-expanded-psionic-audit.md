# 2026-03-14 Autoresearch Hillclimb Targets For Expanded Psionic Audit

## Intent

This audit answers a narrower follow-up question than the older autoresearch
plan:

> now that Psionic is no longer just "a local inference engine idea" and has
> widened into runtime, cluster, datastream, sandbox, collectives, adapters,
> train substrate, and proof-bearing execution identity, what parts of that
> system are actually good hillclimb targets?

The useful answer is not:

- "let the agent mutate the whole repo"
- or "treat every Psionic setting as fair game"

The useful answer is:

- hillclimb the parts of Psionic that are bounded, measurable, replay-safe, and
  promotable through existing evidence and served-path gates
- do not hillclimb the parts that define truth, authority boundaries, receipt
  semantics, proof identity, or product-shell behavior

That is the line this audit tries to make concrete.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`
- `crates/psionic/docs/INFERENCE_ENGINE.md`
- `crates/psionic/docs/LLAMA_VLLM_SGLANG_INFERENCE_SPEC.md`
- `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`
- `docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md`
- `docs/audits/2026-03-13-cocoon-lessons-for-prime-compute-integration-audit.md`
- `docs/audits/2026-03-13-opentensor-ecosystem-adaptation-audit.md`
- `docs/audits/2026-03-13-llama-vllm-sglang-lessons-for-psionic-audit.md`
- `docs/audits/2026-03-13-rust-inference-engine-gap-audit.md`
- `docs/audits/2026-03-13-intellect-lessons-for-psionic-train-audit.md`

## Executive Summary

The original autoresearch note was correct about the loop shape:

- fixed-budget experiments
- measurable score
- keep/discard frontier advancement
- controller owns the search
- Psionic owns execution truth

What has changed is the size of the Psionic surface we can apply that to.

Psionic is now wide enough that the best hillclimb targets are not only:

- model config
- train hyperparameters

They are also:

- local serving scheduler policy
- clustered placement and topology policy
- datastream transfer policy
- sandbox warm-pool and runtime policy
- collective planning policy
- later train-orchestrator, validator, and environment policy

The highest-value immediate hillclimb surface is not full model training.

It is:

> fixed-budget search over serving, cluster, data-plane, and sandbox policy
> spaces that already exist in the expanded Psionic system and already emit
> enough evidence to score honestly.

The second important conclusion is negative:

> we should hillclimb performance envelopes, policy knobs, and bounded lab
> kernels. We should not hillclimb receipt schemas, settlement truth, manifest
> identity rules, session-claims structure, or app-owned product behavior.

So the right OpenAgents reading is:

> the expanded Psionic system gives Autopilot many more honest hillclimb
> surfaces than the 2026-03-09 autoresearch note assumed, but almost all of the
> best near-term surfaces are typed spec mutation over execution policy, not
> open-ended source mutation and not authority mutation.

## What The Expanded Psionic System Changes

The older autoresearch note was written when the useful mental picture was:

- Autopilot controller
- future Rust trainer/eval crates
- bounded sandbox runner
- promotion through served Psionic path

That picture is still right, but it is incomplete now.

The current Psionic docs add several important realities:

- `ARCHITECTURE.md` now defines Psionic as a reusable execution substrate for:
  - runtime
  - cluster
  - datastream
  - sandbox
  - serving
  - collectives
  - proof bundles
  - manifest-bound runtime identity
  - session-bound networked execution identity
- `TRAIN_SYSTEM.md` now defines:
  - training recovery substrate
  - checkpoint lineage
  - data-plane policy propagation
  - validator-aware rollout architecture
  - bounded contributor selection and ranking
  - control-plane versus heavy artifact-plane separation
- the March 13 audits widened the target even further by pointing at:
  - `vLLM`-class scheduling and KV management
  - `SGLang`-class structured serving and routing
  - Cocoon-style manifest and session-claims discipline
  - Bittensor/Covenant-style contributor selection and duplicate detection

That means the hillclimb surface is broader than "train some weights and score
them."

It now includes the policy spaces that determine how Psionic actually executes.

## What Makes A Good Psionic Hillclimb Target

A Psionic surface is a good hillclimb target when all of these are true:

1. The candidate can be expressed as a typed spec or a bounded patch inside a
   narrow lab area.
2. The run can execute under a fixed budget with a declared sandbox and runtime
   profile.
3. The score is measurable and comparable across runs.
4. The run emits artifact digests, receipts, and enough evidence to explain the
   result later.
5. Promotion can pass through an existing served-path or capability-path gate.
6. Failure does not mutate kernel authority or product-shell truth.

If one of those is missing, the surface is usually not ready for autonomous
hillclimbing yet.

## What The Controller Should And Should Not Own

The ownership split from the older autoresearch plan still holds.

Autopilot should own:

- frontier state
- keep/discard policy
- candidate selection
- branching policy
- operator-visible experiment summaries

Psionic should own:

- typed experiment specs for execution-facing work
- runners and receipts
- artifact identity and promotion facts
- the actual execution substrate being scored

The compute unit should remain:

- "execute this declared experiment"

It should not become:

- "decide what to try next"
- "rewrite the architecture until it works"

That remains Labor, not Compute.

## Best Hillclimb Targets Right Now

These are the highest-value surfaces the expanded Psionic system can honestly
hillclimb before full Rust-native training exists.

### 1. Local serving scheduler and decode policy

Why this is a good target:

- the remaining inference gap is still strongly about scheduler shape
- the current docs already describe missing continuous batching, mixed
  prefill/decode, queue admission, and backpressure truth
- these are perfect fixed-budget black-box experiments

Candidate mutation surface:

- queue admission limits
- mixed prefill/decode scheduling rules
- batch-size ceilings
- cancellation and starvation policy
- prefix/KV reuse thresholds
- warm/load/unload policy

Score dimensions:

- tokens per second
- p95 and p99 latency
- memory peak
- cancellation correctness
- conformance pass rate
- evidence completeness

Why this matters:

This is the cleanest immediate hillclimb path from "strong inference substrate"
to "real serving engine."

### 2. Cluster placement and topology policy

Why this is a good target:

- Psionic already has replica, pipeline, layer-sharded, and tensor-sharded
  truth
- cluster performance is dominated by placement, handoff, and cache-affinity
  policy
- those are measurable without changing authority semantics

Candidate mutation surface:

- stage placement
- shard layout
- replica fanout
- cache-affinity rules
- handoff thresholds
- recovery and catch-up policy under bounded failure injection

Score dimensions:

- throughput under topology
- tail latency
- stage idle time
- network bytes transferred
- recovery time
- cache hit or reuse rates

Why this matters:

This is one of the most obvious places where the expanded Psionic system is now
larger than the original autoresearch note.

### 3. Datastream transfer and artifact-staging policy

Why this is a good target:

- `psionic-datastream` already exists
- the train spec and Prime/Intellect audits already treat the heavy artifact
  plane as first-class
- transfer policy is measurable and safe to vary

Candidate mutation surface:

- chunk sizing
- transfer concurrency
- prefetch behavior
- resume and retry policy
- staging order
- artifact locality policy

Score dimensions:

- goodput
- resume overhead
- completion time
- CPU and memory cost
- transfer failure rate
- restore success after interruption

Why this matters:

This is a pure execution hillclimb target and does not require full train
runtime to be useful.

### 4. Sandbox warm-pool and execution policy

Why this is a good target:

- `psionic-sandbox` already owns bounded execution, background jobs, and file
  transfer
- the Intellect audits explicitly say RL and environment-bound execution will
  need throughput-oriented sandboxes
- warm-pool policy is highly tunable and highly measurable

Candidate mutation surface:

- warm-pool size
- pool reuse TTL
- file-staging strategy
- runtime/profile pairing
- restart policy
- local versus remote runner selection

Score dimensions:

- cold-start latency
- warm-start latency
- task throughput
- failure rate
- contamination or reset correctness
- artifact transfer overhead

Why this matters:

This is the most practical bridge from current bounded execution to future
environment-bound train and eval loops.

### 5. Backend runtime and kernel tuning

Why this is a good target:

- Psionic already has real CPU, CUDA, and partial Metal lanes
- backend closure remains a major inference gap
- low-level tuning already fits fixed-budget benchmark loops

Candidate mutation surface:

- quantization dispatch thresholds
- residency and offload policy
- tile or block sizes
- compile-plan heuristics
- backend-specific execution-plan selection

Score dimensions:

- tokens per second
- vector throughput
- peak memory
- parity versus golden fixtures
- compile and warm latency

Why this matters:

This is the closest direct analogue to "classic autoresearch" for the inference
engine itself.

## Good Hillclimb Targets Soon, But Not Fully Today

These are real hillclimb surfaces, but they depend on the next layer of Psionic
build-out first.

### 6. Structured serving and routing policy

Once Psionic has a more generic server and router layer, hillclimb should apply
to:

- cache-aware model routing
- retry and queue policy
- prefill/decode disaggregation policy
- parser or structured-output strategy selection
- response-state retention policy

This is where the `vLLM` and `SGLang` lessons become hillclimbable rather than
just architectural inspiration.

### 7. Collective planning policy

Once collective execution is tied to real trainer work, hillclimb should apply
to:

- ring or mesh choice
- quantization mode
- replan thresholds
- bandwidth-aware sync cadence

This is a good target because `psionic-collectives` already exposes benchmark
and posture facts, which means the search space can stay receipt-bearing.

### 8. Adapter-backed serving policy

Once adapter serving is broader than lineage and packaging, hillclimb should
apply to:

- merge versus unmerged serving strategy
- cache partitioning
- adapter placement
- adapter warm-residency policy

This is likely a later serving optimization surface rather than a first-wave
target.

## Best Hillclimb Targets After The Train Runtime Exists

Once Psionic owns real trainer, eval, and environment runtime crates, the train
hillclimb surface becomes much larger and more valuable.

### 9. Training policy

Candidate mutation surface:

- checkpoint cadence
- optimizer posture
- gradient clipping
- contributor caps
- stage transition thresholds
- off-policy budget
- curriculum and filtering thresholds

Score dimensions:

- eval improvement per unit time
- eval improvement per unit cost
- instability rate
- checkpoint overhead
- rollback frequency

### 10. Orchestrator and validator policy

Candidate mutation surface:

- persistent participant ranking
- contributor reselection cadence
- bounded contributor cap
- validator sampling rates
- duplicate-detection thresholds
- contribution normalization rules

Score dimensions:

- accepted rollout quality
- rollout rejection rate
- duplicate detection precision
- validator cost versus gain
- network and sandbox cost per accepted step

This is where the Bittensor/Covenant and Intellect lessons fit most directly.

### 11. Environment and eval policy

Candidate mutation surface:

- task mix
- difficulty mix
- rubric weighting
- timeout posture
- sandbox pool allocation by environment family

Score dimensions:

- generalization on held-out eval
- reward quality
- false accept and false reject rate
- environment throughput

### 12. Bounded model and kernel architecture mutation

This is the closest thing to classic "autoresearch edits source code."

But it should only happen in:

- typed model specs
- bounded research or lab crates
- narrow kernel experiment areas

It should not mean:

- mutate arbitrary production crates across the whole repo

The right later target is:

- `psionic-lab`-style bounded mutation space

not:

- "the whole Psionic subtree is mutable search state"

## Things We Should Not Hillclimb

The expanded Psionic system also makes the non-targets clearer.

### 1. Do not hillclimb authority truth

Do not hillclimb:

- kernel settlement semantics
- receipt schema meaning
- market authority or payout logic
- final challenge and adjudication authority

Those are truth contracts, not optimization knobs.

### 2. Do not hillclimb identity and proof vocabulary

Do not hillclimb:

- manifest identity tuple structure
- session-claims bundle meaning
- proof posture taxonomy
- runtime identity vocabulary

Thresholds and policy selection around those may be tunable later.
The vocabulary itself should remain stable.

### 3. Do not hillclimb product-shell behavior in Psionic

Do not hillclimb:

- app workflows
- buyer/provider UX
- desktop pane behavior
- wallet or payout presentation

That belongs in app/product loops, not Psionic execution hillclimbs.

### 4. Do not hillclimb open-ended safety policy

Do not hillclimb:

- admissibility of unsigned or unknown artifacts
- environment-package trust model
- allowlist versus denylist truth for proof-bearing supply
- receipt-bearing truth boundaries

Those are governance and safety surfaces first.

### 5. Do not hillclimb the whole repo by default

The original autoresearch plan was right:

- start with typed spec mutation
- later allow bounded source mutation in a narrow lab area

Do not default to:

- "the controller can patch anything under `crates/psionic/*` and call that
  research"

That destroys replayability and makes promotion much harder to trust.

## Recommended Hillclimb Ladder

The best build order is:

### Wave 1: execution-policy hillclimbs on existing substrate

Start with:

- local serving scheduler policy
- cluster placement policy
- datastream transfer policy
- sandbox warm-pool policy
- backend kernel and residency tuning

Why:

- these can run today or near-today
- they already have meaningful fixed-budget scores
- they do not require full train runtime

### Wave 2: structured serving and collective hillclimbs

Add:

- generic serving router policy
- parser and structured-output runtime policy
- collective and sync planning policy

Why:

- these follow naturally from the current inference and collectives issue
  programs

### Wave 3: train-policy hillclimbs

Add:

- training policy
- orchestrator policy
- validator policy
- environment and eval policy

Why:

- these only become honest once `psionic-train`, `psionic-eval`, and
  environment runtime behavior are real

### Wave 4: bounded source hillclimbs

Only after the earlier waves:

- allow bounded source mutation in lab crates or typed kernel surfaces

Why:

- by then the score contracts, receipts, and promotion path are already stable

## GitHub Issues For The First Inference-Policy Hillclimb Loop

The smallest honest first loop is:

- local-first
- bounded
- typed-spec driven
- aimed at one real Psionic serving-policy surface, not full model training

The clearest first target remains:

- local serving scheduler policy on the current real inference path

That first working loop is now covered by this issue set.

### Existing prerequisite substrate issues

- `#3542` `PSI-237: add continuous batching and mixed prefill/decode scheduling`
  - this creates the first high-value serving-policy surface worth optimizing
- `#3593` `Benchmarking: define performance acceptance thresholds for trainer, sandbox, datastream, and validation`
  - this supplies the acceptance discipline needed so hillclimb scores are not
    vague benchmark anecdotes
- `#3594` `Psionic Net and Proof: add runtime manifests and session-claims bundles for policy-meaningful networked execution`
  - this is not required for the smallest purely local loop, but it is part of
    the truth boundary the loop must not bypass as soon as hillclimbs touch
    proof-bearing clustered or sandboxed lanes

### Newly created hillclimb-loop issues

- `#3595` `Psionic Research: define typed experiment specs, result manifests, and promotion records for hillclimb runs`
  - this is the missing reusable experiment vocabulary for one candidate, one
    run, one score, and one promotion decision
- `#3596` `Psionic Research Runner: add a compiled fixed-budget runner for local-first hillclimb experiments`
  - this creates the bounded compiled execution unit for hillclimb candidates
- `#3597` `Autopilot Research: add frontier state, keep-discard policy, and the first local hillclimb controller`
  - this creates the app-owned controller loop and persistent frontier state
- `#3598` `Reference Program: run the first local-first Psionic hillclimb loop against serving scheduler policy`
  - this is the actual end-to-end pilot that proves the loop works on a real
    Psionic surface

### Minimum dependency order

1. `#3595`
   - without typed experiment and result contracts, there is no stable loop
2. `#3596`
   - without a compiled runner, the loop remains a doc rather than execution
3. `#3597`
   - without the app-owned frontier controller, there is no keep/discard logic
4. `#3542`
   - without a real tunable serving scheduler, the first target surface is too
     thin
5. `#3593`
   - without score and acceptance thresholds, the loop is not trustworthy
6. `#3598`
   - this validates the first full local-first loop end to end

### Important non-prerequisites for the first loop

The first working hillclimb loop does not need all later train and distributed
research work first.

It does not require:

- full Rust-native training core
- decentralized rollout workers
- validator-mediated RL adjudication
- full environment runtime
- public market execution

Those are later hillclimb expansions, not day-one requirements for proving the
loop shape.

## GitHub Issues For The First Training-Policy Hillclimb Loop

The first honest training-policy hillclimb loop is different.

It is not:

- "let the controller rewrite the trainer"
- "let the system mutate arbitrary Rust code and benchmark it"
- "count one end-to-end train pilot as hillclimbing"

It is:

- bounded train-policy mutation
- bounded eval-scored comparison
- typed train, eval, and promotion records
- app-owned keep, discard, and branch logic over train outcomes

The clearest first training target is:

- local-first or tightly bounded post-training policy search over one small
  train or eval lane once the Rust-native train and eval surfaces are real

### Existing prerequisite train-substrate issues

- `#3564` `Psionic Train: complete the Rust-native training core beyond recovery substrate`
  - without a real trainer-step substrate there is nothing honest to optimize
- `#3565` `Psionic RL: define rollout artifacts, trainer batches, and policy-lineage contracts`
  - without rollout and policy-lineage truth, candidate comparison becomes
    stringly typed and non-replayable
- `#3568` `Psionic Eval: create the Rust-native eval and rubric runtime`
  - without eval runtime, train-policy scores are not comparable
- `#3573` `Psionic Train: build the orchestrator state machine and trainer-batch assembly contracts`
  - the hillclimb loop needs a real train control plane, not ad hoc batch
    assembly
- `#3576` `Validator Service: add rollout-verification bundles and sampled adjudication protocols`
  - once train-policy search touches untrusted rollout generation, verdicts and
    duplicate detection become part of score honesty
- `#3580` `Psionic Train: add SFT trace ingestion, stage transitions, and agentic pre-RL flows`
  - this creates the first practical policy-search lane richer than a toy loop
- `#3582` `Psionic Train: add instability telemetry, halt policies, and risky-optimization gating`
  - train hillclimbs need safety gates, not only "best metric wins"
- `#3583` `Kernel and Nexus: add training and eval receipt families, policy registries, and read models`
  - accepted train and eval outcomes need canonical receipts before promotion
    is trustworthy

### Shared hillclimb-substrate issues that also apply on the train side

- `#3595` `Psionic Research: define typed experiment specs, result manifests, and promotion records for hillclimb runs`
  - this is now the shared experiment vocabulary for both inference-policy and
    training-policy hillclimbs
- `#3596` `Psionic Research Runner: add a compiled fixed-budget runner for local-first hillclimb experiments`
  - the same runner boundary should support bounded train and eval experiments
    without collapsing them into app-local scripts

### Newly created training-hillclimb issues

- `#3599` `Autopilot Research: add a training-policy hillclimb controller over train and eval receipts`
  - this creates the app-owned frontier and keep or discard logic for train
    candidates rather than only serving-policy candidates
- `#3600` `Reference Program: run the first bounded Psionic training-policy hillclimb loop against eval-scored candidates`
  - this is the end-to-end proof that the train side can do more than one
    static reference run

### Minimum dependency order

1. `#3564`
   - the train core has to exist before train policy search means anything
2. `#3565`
   - policy-lineage and rollout artifacts must be typed before candidates can
     be compared honestly
3. `#3568`
   - without bounded eval scoring there is no stable objective
4. `#3573`
   - without a real orchestrator there is no train-state machine to search over
5. `#3582`
   - without safety and instability gates the loop is not promotable
6. `#3583`
   - accepted train and eval outcomes need canonical receipt and policy truth
7. `#3595`
   - the train hillclimb still needs the same typed experiment contracts as the
     inference loop
8. `#3596`
   - train and eval candidates still need the bounded compiled runner boundary
9. `#3599`
   - the controller loop must be explicit and app-owned
10. `#3600`
   - this is the first end-to-end proof that training-policy hillclimbing is
      real rather than aspirational

### Important non-prerequisites for the first train-policy loop

The first honest train-policy hillclimb loop does not require the full final
post-training system first.

It does not require:

- decentralized or public-market training execution
- every future environment family
- broader human-feedback ingestion pipelines
- arbitrary source-code mutation of the trainer
- the full decentralized-economy settlement path

It does require enough train, eval, receipt, and policy truth that "candidate A
beat candidate B" means something durable.

## Proposed Experiment Families

If OpenAgents wants a practical typed experiment vocabulary, the expanded
Psionic system suggests these families:

| Experiment family | What it mutates | Best current owner |
| --- | --- | --- |
| `ServeSchedulerExperiment` | batching, admission, prefill/decode, cache policy | future Psionic research crate plus `psionic-serve` |
| `ServeTopologyExperiment` | placement, sharding, cache affinity, recovery posture | future Psionic research crate plus `psionic-cluster` |
| `DatastreamTransferExperiment` | chunking, concurrency, resume policy, staging order | future Psionic research crate plus `psionic-datastream` |
| `SandboxPoolExperiment` | warm pools, reuse, runtime pairing, reset policy | future Psionic research crate plus `psionic-sandbox` |
| `BackendTuningExperiment` | quant dispatch, residency, compile heuristics, kernel params | future Psionic research crate plus backend crates |
| `CollectivePolicyExperiment` | mesh layout, quantized sync, replan thresholds | future Psionic research crate plus `psionic-collectives` |
| `TrainingPolicyExperiment` | checkpoint cadence, optimizer posture, stage transitions | future train/eval/research crates |
| `ValidatorPolicyExperiment` | sampling rate, duplicate detection, normalization | future train/eval/research crates |
| `EnvironmentMixExperiment` | task mix, timeout posture, rubric mix | future environment/eval/research crates |
| `LabKernelExperiment` | bounded lab-source mutation | future bounded `psionic-lab` area |

These are the right shape because they mutate:

- typed specs
- execution policy
- measurable runtime behavior

not:

- authority objects
- app workflows
- economic truth

## Promotion Rules

The expanded Psionic system makes the promotion rule even clearer than the old
autoresearch note did.

A hillclimb winner should not be trusted just because the runner reported a
better number.

Promotion should require:

1. typed receipts and artifact digests for the experiment run
2. replay-safe result summaries
3. served-path or runtime-path re-evaluation through real Psionic surfaces
4. capability and evidence recomputation on the promoted artifact or policy
5. refusal when the winning candidate weakens truth, proof, or manifest
   discipline even if it improves raw performance

This is the main guardrail that keeps "hillclimb" from turning into "quietly
degrade the honesty of the system."

## Bottom Line

The best autoresearch reading for the expanded Psionic system is:

> hillclimb the execution policies that Psionic is now rich enough to expose,
> not the authority contracts that make the system truthful.

The immediate high-value hillclimb targets are:

- serving scheduler policy
- clustered placement policy
- datastream policy
- sandbox warm-pool policy
- backend tuning policy

The later high-value hillclimb targets are:

- train policy
- validator policy
- contributor selection and ranking policy
- environment and eval policy
- bounded lab-kernel mutation

What should stay out of bounds is just as important:

- no authority mutation
- no receipt-semantic mutation
- no manifest-identity drift
- no session-claims drift
- no app-shell mutation under the name of Psionic research
- no whole-repo free-form patching as the default search space

That is the honest hillclimb program for the newly expanded Psionic system.
