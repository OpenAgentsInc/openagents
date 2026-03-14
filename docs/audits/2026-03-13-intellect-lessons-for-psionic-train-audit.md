# 2026-03-13 Intellect Lessons For Psionic Train Audit

## Intent

This audit answers a narrower question than the broader Prime ecosystem docs:

> after reading the three INTELLECT papers, what should OpenAgents adapt for
> `Psionic Train`, environment-bound training/eval, and future decentralized
> post-training products?

The useful answer is not "copy Prime's stack."

The useful answer is:

- `INTELLECT-1` is the strongest local reference for elastic pretraining under
  unreliable membership and bad inter-node bandwidth.
- `INTELLECT-2` is the strongest local reference for permissionless async RL
  with untrusted rollout workers, streamed weight distribution, and sampled
  validator checks.
- `INTELLECT-3` is the strongest local reference for the mature shape of an
  open post-training stack: disaggregated trainer/inference, reusable
  environments, online/offline eval parity, and high-throughput sandboxes for
  agentic RL.

Taken together, the papers say something very concrete about OpenAgents:

> the repo already has the right nouns for future training-class compute
> (`TrainingElastic`, environment bindings, checkpoint bindings, validator
> requirements, `psionic-datastream`, validator service substrate), but it still
> needs the right control loops, data-plane contracts, and failure semantics.

That is what we should adapt.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/plans/prime-ecosystem-compute-integration-spec.md`
- `docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/openagents-validator-service/src/lib.rs`

Papers reviewed:

- `/Users/christopherdavid/Desktop/2412.01152v1.pdf`
  - `INTELLECT-1 Technical Report`
- `/Users/christopherdavid/Desktop/2505.07291v1.pdf`
  - `INTELLECT-2: A Reasoning Model Trained Through Globally Decentralized Reinforcement Learning`
- `/Users/christopherdavid/Desktop/2512.16144v1.pdf`
  - `INTELLECT-3: Technical Report`

## Executive Summary

The most important thing these papers add to the existing Prime audit is proof
that the Prime training ideas are not just repo architecture. They were used in
real runs with real failure modes:

- elastic join/leave
- checkpoint-mediated recovery
- bandwidth-aware sync and broadcast
- disaggregated trainer/orchestrator/inference roles
- untrusted rollout workers
- probabilistic and sampled validation
- reusable environment packages
- high-throughput sandbox execution
- agentic SFT before RL

OpenAgents should adapt five big patterns.

1. `Psionic Train` should treat elastic membership, recovery posture, and
   topology replanning as first-class execution truth, not runtime accidents.
2. `Psionic Datastream` should become the explicit data plane for checkpoints,
   policy weights, datasets, and eval artifacts, with integrity digests and
   stale-artifact rules.
3. Future training and RL lanes should have a hard split between trainer,
   orchestrator, inference workers, and validators.
4. Environment packages should be independently versioned products that power
   training and eval through the same rollout-and-rubric contract.
5. `Psionic Sandbox` should be widened for RL-style throughput, not only
   single-job remote execution.

What OpenAgents should not copy is equally important:

- do not copy Prime's exact Python, Kubernetes, nginx, or relay stack
- do not make a centralized ledger or protocol the new core dependency
- do not treat one specific GRPO recipe as canonical market truth
- do not hard-code one optimizer, one model family, or one HPC topology into
  the product architecture

The right OpenAgents reading is:

> Intellect validates the shape of `Wave 6` in the Prime compute plan, but it
> also says that `Psionic Train` cannot just mean "add collectives later." It
> needs explicit training control-plane contracts, validator-aware rollout
> flows, and environment-native sandboxes.

## Current OpenAgents Gap, Precisely

The repo already widened the schema in the right direction:

- `ComputeTopologyKind::TrainingElastic`
- `ComputeEnvironmentBinding`
- `ComputeCheckpointBinding`
- `ComputeValidatorRequirements`
- `ComputeProofPosture`

Psionic also already owns the right reusable substrate buckets:

- `psionic-datastream`
- `psionic-net`
- `psionic-cluster`
- `psionic-sandbox`
- proof bundle and validator seams

So the gap is not missing nouns.

The gap is that training-class execution still needs explicit contracts for:

- join/leave policy
- checkpoint catch-up policy
- stale weight rejection policy
- off-policy budget
- rollout validation posture
- environment packaging and versioning
- sandbox pooling and readiness
- training/eval run receipts that can survive retries and restarts

That is exactly what the Intellect papers make concrete.

## Rust-Only Clarification: What Psionic Still Lacks As Prerequisites

If the requirement is:

> do all of this in Rust, with no Python trainer, no Python environment
> plugins, and no Python orchestration core

then yes, there are still prerequisite gaps beyond the architectural issue set
above.

The most important one is simple:

- there is no `psionic-train` crate in the workspace today

The current workspace members include runtime, serve, sandbox, datastream,
cluster, provider, and backend crates, but not a Rust-native training crate.

The repo also already states this directly in
`crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`:

- Psionic is still inference-first today
- the backward and optimizer substrate are not in Psionic yet
- the Rust-native loop needs new crates such as `psionic-train`,
  `psionic-eval`, `psionic-research`, and `psionic-data` or
  `psionic-datasets`

So the prerequisite answer is not "just add orchestration."

It is:

### 1. We still need the actual Rust-native training core

At minimum Psionic still needs:

- backward/autodiff or an equivalent explicit training graph
- optimizer updates and optimizer-state ownership
- training checkpoint serialization and restore
- trainer-step scheduling and fixed-budget trainer loops

Without that, there is no honest Rust-only RL stack. There is only a plan for
one.

### 2. We still need a Rust-native RL rollout artifact model

The papers assume a real rollout substrate:

- prompts or tasks
- completions or trajectories
- token ids
- logprobs
- rewards
- advantages
- validator-facing proof or commitment refs

In the current Psionic tree, I can find conformance-side `top_logprobs`
request fields, but not a real reusable rollout artifact crate or training
batch contract.

So a Rust-only RL path still needs:

- rollout record types
- trainer-batch assembly types
- policy-revision lineage on rollouts
- reward and advantage attachment contracts
- serialization formats for replay and validator inspection

### 3. We still need a Rust-native environment ABI, not just environment ideas

If we are not using Python at all, then the environment layer itself cannot be
a Python-style plugin system.

That means we still need:

- a Rust-native environment package ABI
- Rust entrypoints for rollout execution
- Rust rubric or reward-function contracts
- Rust-native tool-calling and multi-turn environment interfaces
- package loading, version pinning, and composition without a Python runtime

This is a real prerequisite, not a polish item.

### 4. We still need a Rust-native data and corpus layer

The training loop cannot stay honest if datasets, tokenizer state, split
definitions, and curriculum metadata are all implicit side files.

So Psionic still needs:

- dataset manifests
- tokenizer and corpus digests
- split declarations
- streamed training-corpus iteration
- sequence-packing or batch-packing contracts for long-context work

This is why the older Psionic autoresearch plan called out
`psionic-data` or `psionic-datasets`.

### 5. We still need a Rust-native eval and rubric runtime

For a Rust-only stack, evaluation cannot remain an afterthought.

We still need:

- a `psionic-eval`-style crate for fixed scoring contracts
- reusable benchmark and held-out evaluation runners
- online and offline eval over the same environment contract
- durable eval summaries and artifact refs

Without that, "train in Rust" still leaves the truth layer for reward and
quality underspecified.

### 6. We still need a compiled runner and crash boundary

The existing Psionic docs are also right that long-running training should not
just run inside the serving process.

So a prerequisite for a real Rust-only RL system is:

- a compiled training runner binary
- sandboxed execution outside the in-process serving boundary
- explicit crash, restart, and resume contracts

That is a prerequisite for serious runs, not a deployment nicety.

### 7. We probably need adapter or promotion policy later, but not as day-zero prerequisite

Adapter packaging and promotion matter, but they are not the first blocker.

The first blockers are:

- training graph
- optimizer state
- rollout artifacts
- environment ABI
- eval runtime
- dataset layer
- runner isolation

Adapter-hosting or LoRA productization can come after the base Rust-native loop
is real.

## What INTELLECT-1 Says Psionic Train Should Learn

### 1. Elastic topology should be explicit and two-layered

`INTELLECT-1`'s key architectural move is not merely "use DiLoCo."

It explicitly separates:

- local high-bandwidth intra-node coordination
- global low-bandwidth fault-tolerant synchronization

That maps directly onto the existing OpenAgents topology split:

- local node facts and runtime truth in reusable Psionic crates
- wider-network cluster planning in `psionic-net` and `psionic-cluster`
- future training topology under `TrainingElastic`

OpenAgents should adapt this as:

- explicit local-vs-global rank or role identity in training topology objects
- collective plans that distinguish intra-host and inter-host communication
- topology digests that capture membership, role assignment, and sync posture

This is more important than any one `FSDP`-specific implementation.

### 2. Join and recovery posture should be policy, not ad hoc behavior

`INTELLECT-1` is unusually useful here because it describes both a
non-blocking join path and a conservative blocking join path, then explains why
the blocking path won in practice: it reduced loss spikes for new joiners.

That is an important product lesson for OpenAgents.

`Psionic Train` should support explicit recovery modes such as:

- blocking checkpoint catch-up
- overlapped checkpoint catch-up
- next-outer-step admission
- resume-from-last-stable-checkpoint after major topology shock

Those modes should land in execution contracts and receipts, not stay hidden in
runtime logs.

### 3. Membership needs heartbeat and explicit departure semantics

`INTELLECT-1` uses both heartbeat timeouts and explicit graceful-exit signaling.

OpenAgents should adapt that as:

- regular liveness assertions for training participants
- explicit voluntary-leave receipts
- separate reason codes for graceful departure, crash, timeout, and policy
  eviction
- topology revisions that are replay-safe and queryable later

This fits `psionic-cluster` much more naturally than trying to bury training
membership in app logic.

### 4. Bandwidth-aware planning should be a background service

One of the most valuable practical lessons in `INTELLECT-1` is not the specific
ring algorithm. It is the discipline of continuously re-measuring inter-node
link quality and replanning around network drift.

OpenAgents should adapt this as:

- transport observations feeding collective planning
- background topology scoring
- replan triggers on degraded links, node exits, or new admissions
- explicit evidence of which topology and transport observations were used for
  a given training step range

That is much stronger than pretending internet links are stable.

### 5. Quantized sync is useful only when the contract is honest about it

`INTELLECT-1`'s communication win comes from reduced-frequency sync plus int8
communication, not from pretending arbitrary lossy sync is free.

OpenAgents should adapt:

- quantized collective transport as an explicit policy surface
- stated sync cadence and precision posture in training contracts
- receipts that record which collective policy and quantization policy were
  actually used

Do not hide this behind one "fast mode" flag.

### What not to copy from INTELLECT-1

OpenAgents should not copy:

- the exact `Gloo`/`Tailscale`/parallel-TCP-store stack
- the assumption that data-parallel world resizing is the only important mode
- the specific hybrid `FSDP2 + DiLoCo` implementation as product architecture

The reusable lesson is elastic topology plus explicit recovery semantics, not
their exact Python/PyTorch transport choices.

## What INTELLECT-2 Says Psionic Train Should Learn

### 1. Trainer, orchestrator, inference, and validator are separate roles

`INTELLECT-2` is the clearest proof that permissionless RL needs a harder owner
split than ordinary pretraining.

Its architecture separates:

- trusted trainer nodes
- rollout-generating inference workers
- validators verifying rollout integrity
- orchestration and weight-distribution services

OpenAgents should adapt this directly:

- `psionic-train` owns trainer and rollout-control contracts
- `psionic-datastream` owns checkpoint and weight distribution contracts
- validator services own rollout challenge and verification work
- kernel authority later accepts summarized outcomes into market truth

This is a much stronger architecture than "a training job is just another
cluster run."

### 2. Weight distribution is its own data plane

`INTELLECT-2`'s `SHARDCAST` pattern matters because it treats policy-weight
distribution as a separate operational problem:

- sharded delivery
- pipelined download
- relay selection
- checksum verification
- stale checkpoint cutoff

OpenAgents should adapt this in Psionic terms:

- streamed checkpoint and weight manifests in `psionic-datastream`
- final assembled-artifact digests
- explicit freshness windows and stale-artifact rejection
- mirrored or relay-backed artifact distribution where direct peer exchange is
  unsafe or too leaky

This is directly relevant to future checkpoint-bearing delivery proofs.

### 3. Untrusted rollout workers need fast, layered validation

`INTELLECT-2`'s biggest contribution to OpenAgents is probably not RL itself.
It is the validation posture for untrusted inference workers.

The paper combines:

- activation-commitment style computation checks
- sampling-shape checks
- termination checks
- deterministic sample-selection checks
- schema and sanity validation
- partial random sampling so validators stay much cheaper than generators

That maps extremely well onto OpenAgents' existing validator direction:

- delivery proof alone for some products
- augmented proof posture for stronger products
- challenge-eligible products where economics justify it

OpenAgents should adapt:

- a rollout-verification bundle shape, not only execution-proof bundles
- cheap universal checks plus sampled expensive checks
- explicit penalties or rejection reasons for stale weights, malformed rollout
  bundles, schema failures, and suspicious sampling
- training and eval lanes that declare their validator posture up front

This is the strongest argument for widening validator work beyond GPU matrix
challenges alone.

### 4. Off-policy budget should be an explicit contract

`INTELLECT-2` is very clear that asynchronous RL works because off-policy drift
is bounded, measured, and pruned.

OpenAgents should adapt:

- step-based or revision-based off-policy budgets
- discard rules for overly stale rollouts
- receipts that record the policy revision range contributing to each trainer
  batch
- telemetry for "dropped as too stale" rather than silent filtering

This belongs in `psionic-train`, not in an experiment notebook.

### 5. Curriculum and filtering are first-class infrastructure

The paper's offline and online difficulty filtering is not a side trick. It is
a core throughput and learning-signal control surface.

OpenAgents should adapt:

- environment-side difficulty metadata
- online sampling and filtering policies
- ability to express "only train on non-zero-advantage samples" or analogous
  gates
- run receipts that say what filtering policy was active

If future OpenAgents training lanes ignore this, they will likely waste most of
their inference budget on bad samples.

### 6. Stability guards deserve productized telemetry

`INTELLECT-2` repeatedly surfaces operational instability signals:

- gradient norm growth
- clipping-ratio escalation
- entropy trend reversal
- model-specific fragility
- runtime-kernel regressions such as `torch.compile`

OpenAgents should adapt the principle, not their exact thresholds:

- training runs need explicit instability telemetry
- instability thresholds and halt policies should be machine-legible
- dangerous runtime optimizations should be policy-gated, not silently on

This should end up in control-plane and receipt surfaces, not just charts in a
paper.

### What not to copy from INTELLECT-2

OpenAgents should not copy:

- the specific GRPO variant as canonical algorithm truth
- the centralized relay and firewall model as the only allowed architecture
- a Prime-specific ledger/domain/pool protocol as our economic base layer

The reusable lesson is explicit async RL control planes plus validator-aware
rollout economics.

## What INTELLECT-3 Says Psionic Train Should Learn

### 1. Environments should be packages, not folders inside the trainer

`INTELLECT-3` gives the clearest operational argument for the environment work
already sketched in `Wave 6`.

Its strongest point is simple:

> environments should be independently versioned, testable, and installable
> units, not subdirectories inside the training framework.

OpenAgents should adapt this as:

- environment packages with stable ids and versions
- dataset refs, tool schemas, rubric refs, and sandbox requirements bundled
  into the environment contract
- independent lifecycle for training environments vs trainer runtime
- ability to use the same environment in offline eval, online eval, SFT data
  generation, and RL

This fits `ComputeEnvironmentBinding` directly.

### 2. Training and eval should share the same environment contract

One of `INTELLECT-3`'s best ideas is that the same rollout/rubric contract can
be used both online and offline.

OpenAgents should adapt:

- one canonical environment package contract
- one canonical eval-run object model
- online eval interleaving and offline eval entrypoints over the same
  environment definition

This is how environment-bound compute becomes truthful instead of bespoke.

### 3. Multi-turn and tool-using environments need first-class abstractions

`INTELLECT-3` makes it explicit that agentic RL is not just next-token scoring.

Its environment stack models:

- multi-turn rollout loops
- tool calling
- stateful tool arguments
- sandbox-backed code execution
- code-test reward flows

OpenAgents should adapt that as future environment descriptors that can declare:

- turn model
- tool interface or protocol bindings
- stateful resource requirements
- sandbox/image/runtime requirements
- scoring mode and artifact expectations

This is the bridge from generic training to `Psionic Train` for real agent
workloads.

### 4. Sandboxes need RL-oriented throughput design

The `Prime Sandboxes` material in `INTELLECT-3` is especially relevant because
the repo already has a `psionic-sandbox` direction.

The useful lessons are:

- sandbox readiness should be push-driven where possible
- warm pools matter for repeated short-lived workloads
- image streaming or lazy pulling matters for dynamic environments
- control-plane bottlenecks can dominate execution if every action flows
  through a heavyweight API path

OpenAgents should adapt:

- pooled sandbox acquisition for training/eval lanes
- explicit ready/webhook or equivalent direct readiness signaling
- streamed image or artifact staging semantics
- separate maintenance vs hot-path execution control

This should land in Psionic-owned runtime contracts, not app-only glue.

### 5. Agentic SFT before RL should be part of the product plan

`INTELLECT-3`'s two-stage SFT story matters:

- general reasoning SFT
- agentic SFT for tools, long-horizon control, and format discipline
- then RL on top

OpenAgents should adapt the sequencing principle:

- future agentic RL lanes should assume a pre-RL alignment or SFT stage
- environment artifacts should be reusable for both SFT trace generation and RL
- long-context and tool-call formatting are part of training substrate, not just
  prompt engineering

This is especially relevant if Psionic Train later supports coding, research,
or browser/tool environments.

### 6. The orchestrator is the real center of gravity

`INTELLECT-3` strengthens `INTELLECT-2`'s lesson: the orchestrator is not just
plumbing. It owns:

- rollout scheduling
- batch assembly
- weight update propagation
- multi-client inference balancing
- environment invocation
- online eval interleaving

OpenAgents should therefore treat orchestrator behavior as first-class product
truth with inspectable state and receipts, not as throwaway background code.

### What not to copy from INTELLECT-3

OpenAgents should not copy:

- the exact Python module system for environments
- the exact Kubernetes sidecar/gateway architecture
- the specific MoE or Muon implementation as a repo-wide commitment

The reusable lesson is packageable environments plus RL-native orchestration and
sandbox throughput.

## What OpenAgents Should Adapt, Summarized

The strongest concrete adaptations are these.

### A. Make `Psionic Train` four explicit subsystems

`Psionic Train` should not be one vague crate.

It should have explicit contracts for:

- elastic membership and collective policy
- checkpoint/weight/data distribution
- trainer-orchestrator-inference control flow
- rollout validation and adjudication

### B. Make recovery posture machine-legible

Training runs should declare and record:

- join mode
- checkpoint source and digest
- stale-checkpoint cutoff
- failure and rejoin reason codes
- topology revision history

### C. Treat validator-aware rollouts as first-class proof surfaces

OpenAgents should widen proof work from:

- execution proof only

to also include:

- rollout integrity proof or commitment refs
- sampled validation outcomes
- environment-side score provenance
- trainer-batch inclusion evidence

### D. Make environments the center of training/eval composability

An environment package should be able to power:

- offline eval
- online eval
- SFT trace generation
- RL rollouts
- sandbox-backed agentic execution

### E. Productize RL-oriented sandbox throughput

`psionic-sandbox` should grow beyond "execute one remote job" toward:

- pooled environments
- fast readiness
- streamed images or artifacts
- code-test loops
- repeated multi-turn execution against stable state

## What OpenAgents Should Not Copy

OpenAgents should explicitly avoid these mistakes.

### 1. Do not copy Prime's deployment substrate as architecture truth

Kubernetes, nginx, firewall rules, and specific relay server choices are
implementation details, not architectural requirements.

### 2. Do not turn one research recipe into one market contract

`GRPO`, `Muon`, `FSDP2`, or one MoE implementation should not leak into kernel
truth as if they are canonical forever.

### 3. Do not blur trusted and untrusted roles

The papers are strongest when they separate trusted trainer logic, untrusted
rollout workers, validators, and coordination services. OpenAgents should keep
that split explicit.

### 4. Do not make training artifacts opaque

Checkpoints, policy weights, environment versions, and sandbox images should be
digest-bound and queryable, not just "whatever the trainer used."

## Proposed GitHub Issue Set For The Full Intellect-Style Training Paradigm

This list assumes the currently open Prime/Psionic issues are already complete
before any of the work below starts.

So this is not a set of edits to today's open issues.

It is the follow-on GitHub issue program required to fully implement the
training paradigm implied by `INTELLECT-1`, `INTELLECT-2`, and `INTELLECT-3`.

Where relevant, each issue can reference the completed `Wave 6` work
(`Environments`, `Evals`, `Psionic Train`, `Psionic Adapters`) plus the
completed proof and validator foundations as prerequisites.

The first five issues below are hard Rust-only prerequisites. They are required
before the later trainer/orchestrator/validator architecture can be honestly
called "all-Rust."

### 1. `Psionic Train: create the Rust-native training core crate`

Description:

- Create the actual `psionic-train` crate in the workspace.
- Add backward/autodiff or an equivalent explicit training graph, optimizer
  state ownership, checkpoint I/O, and fixed-budget trainer-loop contracts.
- Make this the reusable training substrate rather than hiding training logic
  inside apps, notebooks, or external runtimes.

References:

- Explicit prerequisite called out in
  `crates/psionic/docs/AUTORESEARCH_INTEGRATION_PLAN.md`.

### 2. `Psionic RL: define rollout artifacts, trainer batches, and policy-lineage contracts`

Description:

- Add reusable rollout record types covering task input, completion or
  trajectory, token ids, logprobs, rewards, advantages, and proof refs.
- Define trainer-batch assembly contracts and replay-safe serialization.
- Record policy revision lineage so off-policy accounting and validator review
  are possible.

References:

- Required because current Psionic has no reusable rollout artifact crate.

### 3. `Environments: define a Rust-native environment ABI and runtime contract`

Description:

- Define the Rust-native ABI for environment packages, rollout execution,
  rubric evaluation, tool-calling, and multi-turn task control.
- Ensure environment packages can be loaded, versioned, and composed without a
  Python runtime.
- Make this the only accepted environment contract for Rust-only training work.

References:

- Required if "no Python anywhere" is real rather than aspirational.

### 4. `Psionic Data: add Rust-native dataset, tokenizer, split, and packing contracts`

Description:

- Add dataset manifests, tokenizer and corpus digests, split declarations, and
  streamed iteration over training corpora.
- Define sequence-packing or batch-packing contracts for long-context SFT and
  RL workloads.
- Bind these objects into training and environment packages.

References:

- Follow-on to the `psionic-data` or `psionic-datasets` recommendation in the
  existing Psionic autoresearch plan.

### 5. `Psionic Eval: create the Rust-native eval and rubric runtime`

Description:

- Create the actual `psionic-eval` crate in the workspace.
- Add fixed scoring contracts, held-out eval runners, benchmark harness hooks,
  and durable eval summaries.
- Keep online and offline eval on the same environment contract used by the
  trainer.

References:

- Explicit prerequisite for a Python-free reward and quality loop.

### 6. `Psionic Train: define canonical run graph, topology revisions, and participant lifecycle`

Description:

- Define the canonical object model for one training run, including stable run
  ids, stage ids, participant roles, topology revisions, and replay-safe
  lifecycle events.
- Model join, graceful leave, crash, timeout, eviction, and rejoin as explicit
  reason-coded events.
- Record heartbeat and voluntary-departure semantics as part of training truth,
  not as log-only runtime behavior.

References:

- Follow-on to the completed `Psionic Train` and `psionic-cluster` groundwork.

### 7. `Psionic Train: implement checkpoint lineage, recovery modes, and catch-up receipts`

Description:

- Add explicit recovery postures such as blocking catch-up, overlapped catch-up,
  next-step admission, and resume-from-last-stable-checkpoint.
- Record checkpoint family, source, digest, assembled artifact digest, and
  recovery mode in run receipts.
- Make checkpoint catch-up a machine-legible part of training lineage.

References:

- Follow-on to completed checkpoint/datastream work and the current
  `ComputeCheckpointBinding` schema.

### 8. `Psionic Collectives: add bandwidth-aware elastic sync planning and quantized policy surfaces`

Description:

- Separate local high-bandwidth collective groups from global low-bandwidth
  synchronization groups.
- Feed transport observations into topology scoring and collective replanning.
- Make sync cadence, quantization posture, and collective policy explicit and
  receipt-bearing.

References:

- Follow-on to completed elastic-membership and quantized-collective issues.

### 9. `Psionic Datastream: add sharded policy-weight broadcast and freshness control`

Description:

- Extend datastream contracts to cover checkpoint and policy-weight broadcast,
  not only generic dataset streaming.
- Add shard manifests, pipelined delivery, assembled-artifact digest checks,
  freshness windows, and stale-artifact rejection.
- Support relay or mirror metadata for cases where direct peer transfer is not
  the right trust or network posture.

References:

- Follow-on to completed `Psionic Datastream` and artifact-residency work.

### 10. `Psionic Train: build the orchestrator state machine and trainer-batch assembly contracts`

Description:

- Define the orchestrator as a first-class subsystem with inspectable state.
- Own rollout scheduling, batch assembly, policy revision tracking, and online
  eval interleaving.
- Produce stable receipts for which policy revisions, environments, and worker
  contributions fed a trainer batch.

References:

- Follow-on to completed environment/eval foundations and the base `Psionic
  Train` substrate.

### 11. `Psionic Train: implement off-policy budget rules and stale-rollout pruning`

Description:

- Add explicit maximum policy age or revision drift budgets for asynchronous
  training.
- Reject or quarantine stale rollouts with reason-coded outcomes.
- Surface telemetry for accepted, discarded, and quarantined rollout batches.

References:

- Built on top of the orchestrator issue above.

### 12. `Psionic Train: define the inference-worker protocol for trustless rollout generation`

Description:

- Define worker heartbeats, task claims, sample-selection seeds, rollout upload
  schema, and stale-weight handling.
- Make rollout payloads and upload outcomes deterministic and validator-ready.
- Distinguish trusted trainer nodes from untrusted or semi-trusted rollout
  workers in the protocol itself.

References:

- Follow-on to completed provider, datastream, and cluster protocol work.

### 13. `Validator Service: add rollout-verification bundles and sampled adjudication protocols`

Description:

- Widen validator work beyond matrix challenges to include rollout integrity.
- Support activation-commitment or equivalent proof refs, termination checks,
  sampling-shape checks, schema/sanity checks, and sampled expensive
  verification over only a subset of rollouts.
- Emit validator verdict artifacts that can be attached to training or eval
  outcomes.

References:

- Follow-on to completed proof and validator-service foundations.

### 14. `Environments: define a package contract for SFT, RL, and eval`

Description:

- Standardize one environment package shape that can represent datasets,
  rollout entrypoints, rubrics, tool schemas, sandbox requirements, and
  environment metadata.
- Include difficulty metadata and environment policy refs so curriculum and
  filtering can become first-class infrastructure.
- Treat environment packages as independently versioned products, not folders in
  a trainer repo.

References:

- Follow-on to completed environment package and binding issues.

### 15. `Environments Registry: add package install, version pinning, composition, and eval parity`

Description:

- Build registry flows for package resolution, pinned versions, artifact
  dependencies, and mixed-environment composition.
- Support `EnvGroup`-style mixed training runs without requiring special-case
  orchestrator code for every environment set.
- Ensure the same package contract works for offline eval, online eval, SFT
  trace generation, and RL rollouts.

References:

- Follow-on to completed registry and evaluation-run groundwork.

### 16. `Psionic Sandbox: add RL-throughput primitives for pooled, repeated agentic execution`

Description:

- Add warm pools, push-based readiness, repeated command loops, and streamed
  image or artifact staging.
- Optimize for thousands of short-lived or multi-turn environment interactions,
  not only one-shot remote execution.
- Make sandbox pooling, acquisition latency, and reuse visible in receipts and
  operator state.

References:

- Follow-on to completed `Psionic Sandbox` extraction and background-job/file
  transfer work.

### 17. `Psionic Train: add SFT trace ingestion, stage transitions, and agentic pre-RL flows`

Description:

- Support multi-stage training programs that can move from general SFT to
  agentic SFT to RL with explicit checkpoint promotion between stages.
- Normalize tool-call traces, long-context stage metadata, and trace lineage so
  SFT assets can be reused by later RL environments.
- Treat stage transitions as explicit run events rather than ad hoc operator
  scripts.

References:

- Follow-on to completed environment and checkpoint lineage work.

### 18. `Psionic Train: implement curriculum, filtering, and non-zero-advantage gates`

Description:

- Add offline and online difficulty filters, trivial-rollout suppression, and
  gates for samples that carry no learning signal.
- Bind filtering policy to environment metadata and record it in run receipts.
- Make training-time sample selection observable and reproducible.

References:

- Follow-on to completed environment package and orchestrator issues.

### 19. `Psionic Train: add instability telemetry, halt policies, and risky-optimization gating`

Description:

- Surface gradient norm alerts, clipping-ratio trends, entropy drift,
  stale-rollout drop rate, checkpoint catch-up latency, topology churn, and
  environment or sandbox failure rates.
- Add machine-legible halt or quarantine policies when instability thresholds
  are crossed.
- Make risky runtime optimizations policy-gated rather than silently enabled.

References:

- Follow-on to the completed training control-plane and proof telemetry work.

### 20. `Kernel/Nexus: add training/eval receipt families, policy registries, and read models`

Description:

- Add canonical receipt and read-model support for training/eval runs once they
  become market-relevant products.
- Carry environment refs, checkpoint lineage, validator posture, rollout
  verification refs, and final accepted outcomes into economic truth.
- Add durable policy registries for allowed environment packages, validator
  policies, and checkpoint families.

References:

- Follow-on to completed kernel/proto widening and authority read-model work.

### 21. `Desktop and autopilotctl: expose training operator surfaces and diagnostics`

Description:

- Add operator-visible inspection for training runs, topology revisions,
  checkpoint lineage, stale-rollout drops, validator verdicts, environment
  versions, and sandbox-pool health.
- Keep training control and inspection inside the app-owned control plane rather
  than creating a disconnected operator stack.

References:

- Follow-on to completed compute-control and operator-surface work.

### 22. `Reference Program: run one end-to-end agentic SFT plus RL pilot on the full stack`

Description:

- Run one concrete reference program that exercises environment packaging,
  checkpoint lineage, sandbox pooling, validator-aware rollouts, online eval,
  and operator inspection together.
- Treat this as the acceptance gate for the full training paradigm rather than
  declaring success from isolated subsystem benchmarks.

References:

- Final integration issue after all items above are complete.

## Ownership Map For The Adaptation

The right owner split remains the same.

### `crates/psionic/*`

Should own:

- elastic membership and collective execution contracts
- checkpoint, weight, and dataset data-plane contracts
- orchestrator-facing training control contracts
- pooled sandbox lifecycle for training/eval
- runtime receipts and topology lineage

### `crates/openagents-kernel-core` and `crates/openagents-kernel-proto`

Should own:

- market-facing references to environment packages, checkpoint families, and
  validator requirements
- durable receipt shapes for training/eval products when those become economic
  objects

### `crates/openagents-validator-service`

Should own:

- rollout verification jobs
- sampled validator checks
- final validator verdict artifacts for challenge-eligible training/eval lanes

### `apps/nexus-control`

Should own:

- canonical acceptance of market-relevant training/eval outcomes
- policy and registry truth for environment, validator, and product references

### Desktop and control plane

Should own:

- operator inspection of training state
- health and topology visibility
- failure reasons, instability warnings, and environment or sandbox inspection

## Bottom Line

The Intellect papers do not change the direction of the Prime/Psionic plan.
They make it sharper.

The sharp reading is:

- `INTELLECT-1` says `Psionic Train` must be elastic and recovery-aware.
- `INTELLECT-2` says async RL needs a separate data plane and validator-aware
  rollout contracts.
- `INTELLECT-3` says environments and sandboxes are not side systems; they are
  the center of a serious post-training stack.

So the right adaptation for OpenAgents is not "copy Prime."

It is:

> build a Rust-native, receipt-bearing, validator-aware, environment-native
> training substrate where elastic topology, checkpoint lineage, rollout
> integrity, and sandbox throughput are all explicit parts of the contract.
