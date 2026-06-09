# Omni Continual Learning Training Loop

Date: 2026-06-08
Status: big-picture bridge for Probe benchmark and training plans

## Thesis

Probe's benchmark and training apparatus is not a side research project. It is
the learning loop for the OpenAgents product: turn coding-agent work into
accepted outcomes, preserve evidence, classify failures, improve the runtime,
route better work to better providers, and pay contributors for useful work.

The immediate commercial wedge is Coding on Autopilot. The first product
strategy should be Blueprint-governed coding work: typed Program Signatures,
tool-menu policy, verifier discipline, evidence closeout, and promotion gates.
Codex can be one strong backend route when the task needs it, but it should not
be treated as the necessary first engine or the organizing architecture. Probe
is the owned runtime path. Benchmark Cloud measures the loop. Psionic trains
and evaluates model/runtime candidates. Pylon supplies distributed rollout
capacity for benchmark-driven optimization first and later distributed
training capacity for admitted Psionic/Qwen/LoRA lanes. OpenAgents product surface is the current
release and projection authority for the product surface, even when older Omni
docs use the Vortex name.

The continual learning plan only matters if it improves the buyer-facing loop:

```text
mission goal
-> coding workroom
-> Blueprint-governed Probe/workroom turn
-> selected backend route, including Codex when useful
-> diff, tests, preview, logs, receipts
-> human review
-> accepted or rejected outcome
-> route scorecard and failure lesson
-> benchmark or training candidate
-> gated promotion
```

## Source Material Reviewed

Root Omni docs:

- `docs/omni/README.md`
- `docs/omni/coding-on-autopilot-wedge-spec.md`
- `docs/omni/agent-cloud-edge-synthesis.md`
- `docs/omni/vortex-model-routing-training-loop-synthesis.md`
- `docs/omni/signature-marketplace-and-streaming-money.md`
- `docs/omni/vortex-public-proof-open-positioning-synthesis.md`

OpenAgents transcript sources:

- `openagents/docs/transcripts/README.md`
- `openagents/docs/transcripts/201.md` - Fracking Apple Silicon
- `openagents/docs/transcripts/203.md` - Pylon and Nexus
- `openagents/docs/transcripts/206.md` - Codex on Autopilot
- `openagents/docs/transcripts/216.md` - Psionic
- `openagents/docs/transcripts/218.md` - Probe
- `openagents/docs/transcripts/219.md` - Probe: Inference Modes
- `openagents/docs/transcripts/220.md` - Propaganda Podcast
- `openagents/docs/transcripts/221.md` - Pylon Launch
- `openagents/docs/transcripts/223.md` - Pay the People
- `openagents/docs/transcripts/224.md` - Distributed Training 101
- `openagents/docs/transcripts/225.md` - Developer Bounties

Probe benchmark docs:

- `docs/benchmarks/README.md`
- `docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`
- `docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
- `docs/benchmarks/2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`

## What The Omni Docs Add

The Probe benchmark docs already define a technical loop. The Omni docs add the
product reason for that loop.

### Coding Is The First Accepted-Outcome Wedge

The Omni README and Coding on Autopilot spec narrow the first launch story:
start with coding because code is unusually verifiable. A coding agent can
produce a diff, tests, a preview, a PR, and a reviewable human acceptance
decision. That makes coding the first commercial proof for the larger Agent
Cloud.

For Probe, this means benchmark learning should optimize toward accepted
coding outcomes, not abstract model scores. Terminal-Bench, SWE/SWT-style
tasks, retained Probe fixtures, and real Autopilot workrooms are useful because
they measure the same operational shape: can the runtime produce correct,
reviewable work with evidence?

### Autopilot Is The Product, Blueprint Is The First Architecture

Episode 206 and the Coding on Autopilot spec frame Autopilot as the layer that
removes manual babysitting from coding work. The user should define the
mission and return to a mission briefing, not type `continue` every turn. The
durable first architecture is Blueprint: explicit Program Signatures, source
authority, tool-policy boundaries, acceptance criteria, receipts, and release
gates.

Episodes 218 and 219 frame Probe as the owned runtime direction: model
agnostic, embeddable, able to use Codex, Psionic/local models, Apple/local
models, and future backends. The benchmark learning loop should therefore
serve two horizons:

- near term: improve Blueprint-governed coding workrooms by improving Program
  Signature selection, tool-menu projection, closeout discipline, acceptance
  criteria, account/backend routing, and route scorecards;
- longer term: make Probe the owned runtime surface that can use Codex,
  Psionic/Qwen, Apple FM, local/swarm inference, or future backends while
  preserving the same product evidence contract.

### Guidance Becomes Blueprint, Blueprint Becomes A Marketable Capability

The older Guidance Module idea from Episode 206 is the between-turn autonomy
layer: continue, test, fix, stop, escalate, summarize, or prepare a briefing.
The current Blueprint system is the more durable implementation direction.

The signature marketplace Omni doc explains why this matters economically.
The marketable object is not a random prompt. It is a reviewed, versioned,
typed capability contract:

```text
Program Signature
-> module version
-> fixture/eval suite
-> Program Run evidence
-> acceptance receipt
-> attribution or payout
```

For Probe's continual learning loop, GEPA should optimize candidate text
bundles and Blueprint usage. It should not create product authority by itself.
Successful candidates become draft Blueprint or Probe module candidates with
fixtures, evidence, lineage, and release gates.

### Public Proof Is A Projection, Not Marketing Copy

The public proof Omni doc defines the claim ladder:

```text
planned -> modeled -> measured -> verified -> settled
```

Benchmark learning must obey that vocabulary. A retained Terminal-Bench
fixture improvement is measured internal evidence, not a public leaderboard
score. A validation split is not frozen holdout performance. A Pylon completed
rollout is not a settled payout. A GEPA-accepted candidate is not an active
runtime release.

This is why the benchmark docs keep repeating public claim boundaries. The
continual learning loop is a public-proof engine only when the artifacts are
redacted, scored, verified, and projected through the product authority layer.

The Forum is one of those projection surfaces. The public
`https://openagents.com/AGENTS.md` contract exposes public Forum reads and
registered-agent topic/reply writes, while the dedicated Artanis posting path
is an OpenAgents product surface operator runbook and internal bridge. A Probe benchmark campaign
may prepare public-safe Forum copy and refs, but the Forum post itself is a
public summary of retained evidence, not the benchmark scorer, payment
authority, training authority, or release gate.

## What The Transcript Arc Adds

### Pylon Is Paid Work, Not Decorative Uptime

The Pylon and distributed training episodes sharpen the provider rule. Pylon
starts by getting machines online, but the mature system pays for useful work:
assigned jobs, validation, checkpoints, artifacts, receipts, and accepted
results. Online node count is not the economic unit.

For the benchmark apparatus, this means Pylon should run:

- GEPA rollout metric calls;
- Terminal-Bench and retained fixture attempts;
- validation and holdout reruns;
- Qwen/LoRA training shards when admitted;
- evaluator and verifier work where a low-capability device can still
  contribute;
- public-safe receipt generation.

The worker should advertise what it can honestly do. The scheduler should pay
or credit work only after the result survives validation and policy gates.

### Psionic Is The Training Factory

Episode 216 introduces Psionic as the Rust-native ML stack and training
ambition. Episode 224 explains the distributed training primitives:

- run identity;
- assignment window;
- current checkpoint;
- local work;
- synchronization or merge;
- validation;
- contribution accounting;
- recovery after dropout.

For Probe, Psionic should own optimizer and model-training truth:

- GEPA-style optimizer coordinator or manifests;
- candidate frontier and lineage;
- LoRA/SFT/DPO/GRPO jobs;
- adapter merge and registration;
- Pylon worker receipts;
- training report and promotion evidence.

Probe should not become the training stack. Probe should emit the traces,
decisions, tool events, benchmark artifacts, and closeout data that Psionic can
turn into candidate improvements.

### Probe Is Multi-Inference From Day One

Episode 219 demos the important product thesis: Probe should combine multiple
inference routes behind one Blueprint-governed runtime. Codex is one possible
account-backed or Codex-style backend; Psionic/Qwen local or remote models,
Apple/local inference modes, and Pylon/swarm compute should sit in the same
surface. Episode 201 adds the Apple Silicon edge-compute thesis: some
coding-agent work can move from cloud APIs to local devices over time.

The benchmark loop should measure that migration honestly. It should compare:

- Blueprint-governed Probe runs using Codex when useful;
- Probe using Codex-style backends;
- Probe using Apple FM for bounded local tool use;
- Probe using Qwen/Psionic models;
- Probe using Pylon/swarm compute;
- mixed strategies where local models handle low-risk tool, summarization,
  routing, or verifier-support steps.

The goal is not to make Codex the default yardstick or to pretend every local
model can replace a frontier backend immediately. The goal is route memory:
know which work can run through local or swarm routes, which work benefits
from Codex or another frontier backend, which work can be split, and what each
route costs.

### OpenAgents Wants A Revenue-Sharing Capability Market

The transcript guide and signature marketplace docs connect bounties,
contributor payments, plugin marketplaces, Pylon compute, data markets, and
paid skills into one pattern: useful capabilities should earn.

For Probe learning, this implies attribution records:

- which Program Signature helped;
- which GEPA candidate improved the run;
- which Pylon workers produced valid rollouts;
- which Psionic adapter or checkpoint was used;
- which verifier or fixture suite caught the failure;
- which provider route delivered the accepted outcome.

Those records are how the learning apparatus becomes an economy instead of an
internal eval harness.

## The Four Continual Learning Loops

The full system needs four loops that share evidence but have different
promotion gates.

### 1. Product Outcome Loop

This is the buyer-facing Autopilot loop.

```text
mission
-> workroom
-> Blueprint-governed Probe/workroom turns
-> selected backend route
-> artifacts
-> briefing
-> human review
-> accepted or rejected outcome
```

Metrics:

- acceptance rate;
- human review minutes;
- turns per accepted outcome;
- cost per accepted outcome;
- retry count;
- route scorecard;
- artifact completeness;
- public/private proof state.

This loop proves the commercial wedge.

### 2. Benchmark Learning Loop

This is the public benchmark-cloud loop.

```text
task
-> runner assignment
-> Probe attempt with selected backend route
-> verifier result
-> failure classification
-> retained fixture or live score
-> candidate proposal
-> rerun
-> promotion evidence
```

Metrics:

- verifier reward;
- cost;
- duration;
- policy findings;
- failure family;
- retained regression delta;
- validation and holdout split results;
- artifact and proof bundle completeness.

This loop makes the product improve without relying on vibes.

### 3. GEPA/Blueprint Optimization Loop

This is the first optimizer loop.

```text
candidate text bundle
-> Pylon rollout batch
-> evaluator side information
-> GEPA reflection
-> new candidate
-> retained and validation gates
-> shadow Blueprint candidate
```

The candidate bundle can include:

- Probe system prompt;
- backend prompt addendum;
- terminal benchmark global playbook;
- signature-selection policy;
- tool-menu policy;
- failure-family playbooks;
- patch/test policy;
- closeout policy.

Promotion boundary:

```text
optimizer_accepted != active
```

GEPA can find better candidates. OpenAgents product surface/Blueprint release gates decide what can
ship.

This loop is distributed optimization, not distributed neural-network
training. Pylons can run many independent Probe benchmark rollouts for a
candidate bundle, return verifier results and receipts, and let the GEPA
coordinator update the candidate frontier. The model weights do not change in
this loop.

### 4. Psionic Model/Adapter Training Loop

This is the heavier model loop.

```text
clean traces
-> split-aware training data
-> LoRA/SFT/DPO/GRPO run
-> adapter merge
-> retained eval
-> validation eval
-> holdout/live sweep
-> shadow or reject
```

This should start after the GEPA loop produces clean data:

- successful rollouts;
- failed rollouts;
- candidate diffs;
- verifier outcomes;
- failure-family labels;
- selected signatures;
- tool-menu decisions;
- closeout evidence quality.

The model loop should train open/local models such as Qwen3.6 adapters through
Psionic. Apple FM can be a local Probe backend, but the docs should not claim
Apple FM itself is fine-tuned unless an Apple-supported API permits that exact
operation.

## Route Scorecards Are The Glue

The Omni model-routing synthesis says the product should win by picking the
right route for each accepted outcome and preserving why. Probe benchmark
learning should therefore emit route scorecards for both workrooms and
benchmarks.

Minimum route scorecard fields:

```text
selected_model_or_agent
selected_runner
selected_provider
selected_isolation_profile
selected_verifier
expected_cost
observed_cost
expected_latency
observed_latency
privacy_tier
trust_tier
selected_signatures
tool_menu
candidate_hash
rejected_routes
route_reason
post_closeout_route_score
```

Rejected routes are evidence. If a task used Codex instead of a local Probe
backend, or SHC instead of public Pylon, or GCP instead of SHC, the record
should say whether the blocker was privacy, trust, missing capability, weak
benchmark evidence, quota, latency, cost, or policy.

## How This Changes The Benchmark Roadmap

The benchmark folder now has a clearer sequence, and
`docs/benchmarks/plan.md` is the source of truth for issue creation:

1. Build Probe closeout foundations first.
2. Build the public benchmark-cloud target out of private Cloud
   source material.
3. Add Psionic GEPA candidate manifests and coordinator support.
4. Adapt Pylon/OpenAgents product surface work slices for benchmark metric calls with explicit
   paid/unpaid state.
5. Run GEPA Stage 0 and Stage 1 before LoRA or broad model training.
6. Use SHC as the first serious live Terminal-Bench validation environment.
7. Project Artanis campaign state and route scorecards through OpenAgents product surface.
8. Promote only through OpenAgents product surface/Blueprint gates and public-proof-safe projection.

This is narrower than "train models around the clock" and stronger than
"optimize benchmark prompts." It is a product learning system.

## Public Claim Boundaries

Allowed early claims:

- OpenAgents is building the public Benchmark Cloud apparatus.
- Probe can emit benchmark/runtime evidence.
- Pylon can run admitted GEPA rollout jobs, and later admitted training jobs
  when those lanes have explicit Psionic/model-training authority.
- GEPA candidates improved a named retained or validation split, if the split,
  candidate hash, verifier, and artifact state are shown.
- Psionic trained or evaluated a named adapter path, if the checkpoint, split,
  artifact, and boundary are shown.

Blocked claims:

- "Probe beats Terminal-Bench" from retained fixtures.
- "Pylon training work is settled" without payment/reconciliation receipts.
- "GEPA on Pylons is distributed model training" when it is only distributed
  rollout optimization over text candidates.
- "GEPA candidate is production" without release gates.
- "Codex is unnecessary" or "local models replaced frontier backends" without
  route scorecards and accepted-outcome evidence.
- "Apple FM was fine-tuned" without an exact supported fine-tuning path.
- "OpenAgents Cloud is live" from a benchmark harness alone.

## First Public Proof To Aim For

The first proof should be small and complete:

```text
Probe retained benchmark learning run
-> public benchmark-cloud split manifest
-> GEPA text-bundle candidate
-> Pylon Stage 0/1 rollout receipts
-> verifier results
-> policy findings
-> candidate hash and lineage
-> rejected routes
-> no automatic promotion
-> OpenAgents product surface-readable proof projection
```

That proof connects the whole story:

- Coding on Autopilot needs less babysitting.
- Probe supplies the runtime contract.
- Blueprint-governed decisions improve through measured evidence.
- Pylon supplies distributed work.
- Psionic receives clean traces for later training.
- OpenAgents product surface projects only what is safe to claim.

## Bottom Line

The continual learning apparatus is the product memory of OpenAgents. It tells
the system which coding strategies work, which routes are worth using, which
workers produce valid evidence, which Blueprint modules deserve promotion, and
which model adapters are ready to shadow.

Benchmarks are not the goal. Accepted outcomes are the goal. Benchmarks are the
controlled environment where Probe learns before those lessons are allowed to
affect paid Autopilot workrooms.
