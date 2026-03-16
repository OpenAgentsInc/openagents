# 2026-03-15 RLM Psionic Economy Kernel Integration Audit

## Intent

This audit answers the same class of question as the GEPA audit, but for
`~/code/rlm`:

> after reading the current Psionic and kernel specs and the external RLM repo,
> how should OpenAgents adapt Recursive Language Models so they help Psionic and
> the economy kernel without breaking MVP scope or the current owner split?

The useful answer is not:

- "import the Python RLM runtime into Psionic core"
- "treat RLM as a replacement for Psionic inference/runtime crates"
- "let recursive execution bypass kernel authority, settlement, or receipts"
- "revive the archived coupled Pylon RLM bundle"

The useful answer is:

- treat RLM as a high-level recursive workload and orchestration pattern
- use Psionic as the machine-facing execution substrate under that pattern
- use the kernel as the accepted-outcome and settlement authority above that
  pattern
- productize RLM later as a demand engine and labor/computation composition
  lane, not as a shortcut around current architecture

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
- `docs/pylon/PYLON_PLAN.md`
- `docs/audits/2026-03-07-pylon-archive-and-transcript-audit.md`
- `crates/nostr/core/src/nip90/kinds.rs`

RLM sources reviewed:

- `~/code/rlm/README.md`
- `~/code/rlm/docs/architecture.md`
- `~/code/rlm/docs/getting-started.md`
- `~/code/rlm/docs/api/rlm.md`
- `~/code/rlm/rlm/core/rlm.py`
- `~/code/rlm/rlm/core/lm_handler.py`
- `~/code/rlm/rlm/core/types.py`
- `~/code/rlm/rlm/environments/base_env.py`
- `~/code/rlm/rlm/environments/local_repl.py`
- `~/code/rlm/rlm/environments/docker_repl.py`
- `~/code/rlm/rlm/environments/prime_repl.py`
- `~/code/rlm/rlm/logger/rlm_logger.py`
- `~/code/rlm/rlm/utils/prompts.py`
- `~/code/rlm/tests/test_subcall.py`
- `~/code/rlm/tests/test_multi_turn_integration.py`
- `~/code/rlm/tests/test_local_repl.py`
- `~/code/rlm/tests/repl/test_custom_tools.py`

## Executive Summary

RLM is not a model runtime.

It is an agentic inference shell that wraps an LM with:

- iterative REPL execution
- recursive sub-calls
- depth- and budget-bounded fan-out
- sandbox/environment abstraction
- trajectory logging

That makes it strategically relevant to OpenAgents, but only at the right
layer.

The most important architectural reading from this audit is:

> RLM should be adapted as a recursive labor-plus-compute orchestration pattern
> on top of Psionic and the kernel, not imported as the execution substrate
> itself.

Why:

- Psionic already owns machine-facing runtime truth, sandbox execution, cluster
  topology, artifact staging, and proof-bearing receipts
- the kernel already owns accepted outcomes, verification, liability, and
  settlement truth
- RLM today is Python-first, prompt-driven, and only lightly sandboxed in its
  default mode

So the valuable part is not the literal implementation.

The valuable part is the workload pattern:

- break a hard task into many bounded subcalls
- use code execution plus sub-model calls to inspect large context
- propagate depth, timeout, token, and budget limits through the subcall tree
- retain trajectories so operators can understand how the answer was built

This is exactly the kind of workload that can create real buy-side demand for a
distributed compute network.

The best OpenAgents reading is therefore:

1. RLM is a strong future demand engine for the compute market.
2. RLM is a strong future composition model for the labor market.
3. RLM should push Psionic to improve:
   - recursive session receipts
   - persistent sandbox sessions
   - long-context eval harnesses
   - fan-out scheduling and cluster orchestration
4. RLM should not push Psionic toward:
   - Python-first core execution
   - in-process soft sandboxing as production truth
   - ad hoc socket-based authority or billing

The recommended plan is:

1. define an RLM workload family above Psionic, not inside `psionic-core` or
   `psionic-runtime`
2. build a Psionic-native reference harness for recursive long-context tasks
3. add receipt-bearing subcall graphs, budgets, and result lineage
4. only then expose recursive fan-out as a compute/labor product lane

## What RLM Actually Is

Reading `~/code/rlm` directly, RLM is best understood as a recursive agent loop
rather than a new model family.

Its main loop is:

1. ask the root LM for the next step
2. extract ```repl``` code blocks from the response
3. execute those blocks inside a REPL environment
4. let the executed code call:
   - `llm_query()`
   - `llm_query_batched()`
   - `rlm_query()`
   - `rlm_query_batched()`
5. append REPL stdout/stderr and subcall results back into the conversation
6. continue until a final answer is produced or limits fire

That pattern is implemented in:

- `rlm/core/rlm.py`
- `rlm/core/lm_handler.py`
- `rlm/environments/*`

The repo's important technical features are:

- recursive subcalls with `max_depth`
- per-call LM handler on a localhost socket
- multiple environment backends:
  - `local`
  - `docker`
  - `modal`
  - `prime`
  - `daytona`
  - `e2b`
- propagation of:
  - remaining timeout
  - remaining budget
  - token limits
  - error thresholds
- trajectory metadata capture through `RLMLogger`
- optional persistent sessions for local-only multi-turn state
- history compaction once context reaches a threshold
- custom Python tools injected into the REPL

So the repo is not "just a prompt pattern."

It is a runtime for:

- interactive code-augmented reasoning
- long-context decomposition
- recursive task splitting
- nested LM orchestration

## What RLM Is Not

RLM is also not several things that matter to OpenAgents.

It is not:

- a low-level model runtime
- a Rust-native execution engine
- a canonical sandbox security boundary in its default `local` mode
- a replay-safe receipt system
- a canonical compute-market settlement or proof layer
- a validator/adjudication framework
- a distributed cluster substrate

The biggest implementation-level constraints are explicit in the code and docs:

- the default local environment uses in-process Python `exec()`
- the default local environment is a soft sandbox, not a hard security boundary
- persistence is only supported for the local environment
- recursive subcalls are coordinated through host-side handlers and ad hoc
  socket or HTTP brokers
- results are logged as trajectories, not canonical market receipts

That means RLM is useful as:

- a workload model
- an orchestration reference
- an operator/debugging pattern

It is not useful as:

- a direct replacement for Psionic
- an authority layer
- a production receipt format

## Why RLM Matters To OpenAgents Anyway

Even with those constraints, RLM matters for one big reason:

> it is a concrete buyer-side workload that naturally explodes one hard task
> into many smaller compute and reasoning jobs.

That directly connects to a historical OpenAgents theme already visible in the
repo:

- `crates/nostr/core/src/nip90/kinds.rs` still defines
  `KIND_JOB_RLM_SUBQUERY = 5940`
- the archived Pylon audit says RLM fan-out was the intended demand-side unlock
  for swarm compute
- `docs/pylon/PYLON_PLAN.md` explicitly rejects reviving RLM as part of the new
  Pylon scope, which means the idea still exists but is intentionally deferred

That is the right current reading:

> RLM is strategically important, but it is not MVP scope and it should not
> come back as a scope-creeping bundle.

This audit therefore treats RLM as:

- not MVP
- but still a high-value later compute/labor demand engine

## Where RLM Fits In The Current Owner Split

The clean owner split is:

- Autopilot or a future labor-side controller owns:
  - recursive planning
  - decomposition strategy
  - branching policy
  - answer synthesis
  - task UX
- Psionic owns:
  - sandbox execution
  - inference execution
  - model routing
  - artifact staging
  - cluster fan-out
  - execution receipts
- the kernel and Nexus own:
  - work-unit truth
  - evaluation and verification truth
  - accepted outcomes
  - settlement and liability truth

That means RLM belongs primarily:

- above Psionic as a recursive workload controller
- below the product shell only where operator/runtime controls are needed

It does not belong:

- inside `psionic-core`
- inside `psionic-ir`
- inside `psionic-runtime` as fundamental runtime semantics
- inside kernel authority

## The Best Benefits To Psionic

RLM benefits Psionic most by forcing the subtree to support a realistic
recursive agent workload, not by contributing its Python implementation.

### 1. A concrete recursive workload family for Psionic

Psionic currently names product-level work classes such as:

- inference
- embeddings
- sandbox execution
- clustered serving
- artifact staging
- training-class coordination

RLM suggests an additional important workload family:

> recursive long-context reasoning over a mixture of served inference and
> sandbox execution.

This is not a new low-level runtime work class.

It is a higher-level execution composition that stresses several existing
Psionic layers at once:

- `psionic-serve`
- `psionic-sandbox`
- `psionic-router`
- `psionic-net`
- `psionic-cluster`

That makes it a useful acceptance target and benchmark family for Psionic.

### 2. Better persistent sandbox-session semantics

RLM's `persistent=True` local mode is one of its most useful ideas:

- a session keeps state across turns
- later turns can inspect prior variables and histories
- the task stops feeling stateless

OpenAgents should not copy the current implementation literally, because it is
local-only and Python/in-process.

But Psionic does benefit from the product requirement it exposes:

> some high-value agentic workloads want bounded persistent execution sessions,
> not only one-shot sandbox jobs.

That argues for strengthening `psionic-sandbox` around:

- session-scoped state
- resume-safe snapshots
- bounded persistent pools
- explicit lifecycle receipts for session open/step/close

### 3. Better nested receipt and budget propagation

One of the strongest RLM ideas is that child calls inherit:

- remaining timeout
- remaining budget
- token limits
- recursion depth

That pattern maps naturally onto Psionic and kernel work.

Psionic can benefit from adding explicit recursive execution facts such as:

- parent execution id
- child execution id
- inherited budget ceiling
- inherited timeout ceiling
- cumulative resource usage
- subcall result refs

These should be receipts and proof-bearing references, not only trajectory
logs.

### 4. Better operator-visible trajectories and debugging

RLM's logger and visualizer are some of the cleanest parts of the repo.

The visual value is simple:

- see root prompts and responses
- see executed code blocks
- see child LM calls
- see timing and usage

Psionic already wants a cross-library observability and debug surface. RLM
provides a good concrete UI pattern for:

- nested execution trees
- timeline views
- code/output panes
- usage summaries

That is especially valuable for:

- sandbox-heavy agentic workloads
- recursive decomposition failures
- validator explanation and replay

### 5. Better fan-out and batching pressure on cluster/router design

RLM's recursive and batched query model pushes directly on:

- admission control
- queue fairness
- placement
- cache affinity
- parent/child call locality
- partial failure handling

That makes it a strong workload to justify and validate later Psionic work on:

- router placement
- clustered serving
- cache-aware scheduling
- service-tunnel and network identity

### 6. Better environment/eval packages for long-context reasoning

Psionic docs already push toward:

- reusable environment packages
- benchmark packages
- held-out eval runs
- repeat-run aggregation

RLM gives a concrete task family that should become one of those packages:

- huge context
- chunking decisions
- recursive summarization
- selective retrieval
- hierarchical synthesis

This is stronger than using vague "long context" benchmarks because it measures
the actual behavior OpenAgents cares about: how well a recursive agent uses
compute and context together.

### 7. Better workload pressure for repeated agentic iteration receipts

`psionic-sandbox` already has repeated agentic iteration receipts. RLM is an
excellent real target for those surfaces because each loop step already has:

- LM response
- code execution
- stdout/stderr
- child-call tree
- possible partial answer

That is a near-perfect fit for richer step-level receipt families.

## The Best Benefits To The Economy Kernel

RLM helps the economy kernel most as a demand engine and composition pattern.

### 1. Compute market demand engine

This is the biggest potential win.

The compute market's biggest product risk is always cold-start demand.

RLM can create a strong demand story because a single top-level task can fan
out into:

- many sub-inference requests
- many summarization/classification passes
- sandbox-bound parsing and transformation tasks
- recursive child tasks with different price/performance profiles

That is especially attractive for a distributed network because the child tasks
are often:

- independent
- parallelizable
- latency-sensitive but bounded
- heterogeneous in quality and price requirements

That is a better fit for a swarm compute market than many monolithic jobs.

### 2. Labor market composition model

RLM is also a labor-market pattern.

The top-level user asks for one outcome.

Under the hood, the system creates:

- a root planning task
- many subordinate subqueries
- local or remote compute work
- a final synthesis

That maps naturally to labor-market ideas like:

- one root `WorkUnit`
- subordinate task graph
- subordinate `Submission`/`Verdict` style checks
- one accepted root outcome after composition

So RLM is not only a compute workload. It is also a concrete way to decompose
machine labor into smaller verifiable units.

### 3. Data market pull for permissioned context

RLM is unusually good at making the value of the data market obvious.

Why:

- the whole pattern assumes rich context inspection
- chunking and selective subquerying are central
- some of the most valuable recursive workloads depend on private corpora

That means RLM naturally consumes:

- permissioned files
- document bundles
- retrieval indices
- chat/context history
- structured local project state

So it creates a plausible demand path for `DataAsset` and `AccessGrant` style
flows later on.

### 4. Liquidity market pressure through micro-budgets

Liquidity is not the first reason to pursue RLM, but it matters.

A recursive workload wants:

- bounded top-level budget
- bounded child budgets
- maybe different price ceilings for different branches

That is a useful future pressure test for:

- micro-envelopes
- reserve partitioning
- route planning for many small sub-payments

The important point is still a boundary point:

RLM should consume those liquidity primitives, not replace or govern them.

### 5. Risk-market and verification pressure

RLM also creates a clearer risk model than monolithic tasks because the system
can inspect:

- where the answer came from
- which branches disagreed
- which subcalls used which models
- where confidence was weak
- where sandbox code failed

That opens useful risk-layer opportunities:

- branch-level challenge policies
- confidence- or disagreement-aware verification
- correlation tracking across child calls
- selective expensive checks only on risky branches

Again, the risk market benefits from the workload shape. It should not inherit
the RLM runtime as authority.

## The Biggest Mismatches With OpenAgents

The current RLM implementation also conflicts with OpenAgents in important
ways.

### 1. Python-first runtime is the wrong substrate for Psionic

OpenAgents is explicitly Rust/WGPUI-first on the retained path, and Psionic is
explicitly the reusable Rust-native execution subtree.

RLM today is:

- Python runtime
- Python environments
- Python `exec`
- Python state persistence
- Python-oriented customization and tool injection

That is fine as an external reference repo.

It is not fine as the substrate of truth inside Psionic.

### 2. LocalREPL is not a production sandbox

The docs say this directly and the code confirms it:

- LocalREPL executes code via in-process `exec()`
- it strips some dangerous builtins
- it is a soft sandbox, not a hard boundary

OpenAgents cannot use that as the truthful production compute boundary for:

- provider execution
- market-facing receipts
- buyer trust
- validator trust

RLM's local mode is a convenience/development mode, not a canonical security
posture.

### 3. Handler/broker communication is ad hoc, not canonical transport truth

RLM uses:

- localhost TCP socket handlers
- per-environment proxy HTTP servers
- environment-specific forwarding shims

That is pragmatic, but it is not the kind of typed transport or session-claims
discipline Psionic wants for networked execution truth.

So the pattern is useful, but the transport implementation should not be
copied into core OpenAgents architecture.

### 4. Trajectory logs are not enough for market truth

RLM has trajectories and JSONL logs.

OpenAgents needs:

- digest-bound artifacts
- typed receipts
- immutable references
- accepted-outcome authority

RLM logs are valuable operator evidence, but they are not sufficient as
canonical market truth.

### 5. Recursion semantics are not yet economic semantics

RLM propagates local budgets and limits, but it does not define:

- settlement per child call
- parent/child contract semantics
- liability assignment
- challenge rights
- accepted-outcome publication

The kernel still needs to own that.

### 6. Scope creep is a real historical risk here

The repo already has explicit warnings about earlier Pylon scope drift:

- provider node
- buyer client
- wallet shell
- host runtime
- browser bridge
- Codex shell
- RLM bundle

So even if RLM is strategically attractive, reviving it carelessly would be a
regression against current MVP discipline.

## What OpenAgents Should Adapt From RLM

These are the good parts worth adapting.

### 1. Recursive task decomposition as a product pattern

The most important adaptation is conceptual:

> recursive workloads are a plausible, high-value buyer of distributed compute.

This should influence compute/labor roadmap thinking even before any concrete
productization.

### 2. Budget propagation through the subcall tree

This should become explicit in Psionic and later kernel-linked receipts:

- top-level budget
- inherited child budget
- spent so far
- remaining allowance
- branch failure due to exhausted budget

### 3. Persistent bounded workspaces

The implementation should be different, but the product requirement is real:

- some tasks are much better with session memory and persistent variables
- stateful task decomposition is a real workload class

### 4. Trajectory visualization for nested agent runs

The visualizer pattern is useful and should influence:

- Autopilot debug surfaces
- `autopilotctl` diagnostics
- future Psionic operator UIs

### 5. Long-context compaction and summary checkpoints

RLM's compaction mechanism is not sufficient as-is, but the need it surfaces is
real:

- recursive workloads may need intermediate summary checkpoints
- those checkpoints should be typed artifacts and receipts in OpenAgents

### 6. Environment abstraction

RLM's abstraction over:

- local
- Docker
- cloud sandboxes

is directionally useful. OpenAgents should preserve that kind of environment
independence, but with Psionic-native runtime and receipt semantics.

## What OpenAgents Should Not Copy From RLM

These are the parts to reject or sharply limit.

### 1. In-process Python `exec()` as a default trusted lane

Do not copy this into Psionic product truth.

### 2. Socket/HTTP broker glue as the canonical network model

Useful as a prototype, wrong as long-term transport truth.

### 3. Prompt-only control of recursive planning

OpenAgents should progressively move recursive planning into more typed
contracts and policy surfaces, not leave the whole system as prompt discipline.

### 4. Trajectory logs as the only replay surface

OpenAgents needs receipts and digest-bound artifacts, not just logs.

### 5. Product bundling

Do not revive RLM as:

- provider runtime
- buyer shell
- wallet shell
- bridge
- Codex shell
- product bundle

Keep it narrow.

## Suggested Integration Roadmap

The right roadmap is phased and boundary-aware.

### Phase 0: Name the lane and keep it out of MVP scope

Goals:

- acknowledge RLM as a later strategic demand engine
- keep it explicitly out of the current MVP/provider extraction scope
- avoid accidental product drift

Concretely:

- do not put RLM in `apps/pylon` scope
- do not treat it as current compute-market MVP work
- keep any early experiments behind research or operator surfaces

### Phase 1: Build an OpenAgents-native RLM reference workload harness

Goals:

- translate the useful workload pattern into existing Psionic objects
- avoid importing the external runtime wholesale

Concretely:

- create one reference benchmark/environment package for recursive long-context
  tasks
- run it through:
  - `psionic-sandbox`
  - `psionic-serve`
  - `psionic-eval`
- define what counts as success:
  - final answer quality
  - branch efficiency
  - budget adherence
  - receipt completeness

This phase is about measurement, not productization.

### Phase 2: Add nested execution receipts and session identity

Goals:

- make recursive execution first-class in Psionic evidence

Concretely:

- add parent/child receipt linkage for sandbox and served subcalls
- add session-scoped execution ids for recursive workflows
- add typed budget and timeout propagation facts
- add summary-checkpoint artifacts for long-context compaction

This is where RLM starts improving Psionic substrate directly.

### Phase 3: Add persistent bounded workspaces to `psionic-sandbox`

Goals:

- support stateful agentic sessions safely and replayably

Concretely:

- add bounded persistent sandbox sessions
- add session snapshot and restore receipts
- distinguish:
  - one-shot jobs
  - persistent recursive sessions
- keep the default security posture explicit and machine-legible

This phase should not inherit RLM's in-process local semantics.

### Phase 4: Add recursive workload procurement over the compute market

Goals:

- make RLM-like fan-out a real buyer-side demand pattern

Concretely:

- revive the idea behind job kind `5940` as a narrow recursive subquery family
- let a root workload fan out into subordinate compute jobs
- use compute products and delivery proofs for each child execution
- retain one parent execution graph that ties the subtree together

This should be typed market behavior, not an ad hoc local runtime.

### Phase 5: Add hierarchical labor-market truth

Goals:

- make root-task to subtask composition legible in the kernel

Concretely:

- define root labor work units that can reference subordinate compute/labor
  work
- allow verdicts and accepted outcomes to bind to a subordinate execution tree
- preserve per-branch evidence without requiring every branch to be a
  user-visible first-class product

This is where RLM becomes a true labor-market composition pattern.

### Phase 6: Add risk and liquidity overlays only after the above are real

Goals:

- safely handle micro-budgets and challenge posture

Concretely:

- bounded subcall envelopes
- branch-level challenge rights and verification plans
- aggregated spend and failure reporting
- correlation-aware validator policies for recursive trees

Only after the core execution and accepted-outcome path is real.

### Phase 7: Consider a Rust-native recursive controller, not a Python transplant

If the workload proves valuable, the next step should be:

- a Rust-native recursive controller or orchestration layer
- likely above Psionic runtime but close to Psionic sandbox/serve
- still distinct from kernel authority

The rule should be:

> adopt the recursive workload model, not the current Python implementation as
> the architectural core.

## Recommended Priority Order

If OpenAgents wants to realize the value of RLM without destabilizing current
work, the order should be:

1. benchmark/eval harness for recursive long-context workloads
2. nested receipt linkage and session identity
3. persistent bounded sandbox sessions
4. compute-market recursive subquery procurement
5. hierarchical labor-market outcomes
6. risk/liquidity overlays
7. Rust-native recursive controller

Do not start with:

- importing the external Python repo into product code
- reviving the old Pylon RLM bundle
- wallet/bridge/Codex coupling
- kernel-first settlement schemas for a workload the runtime cannot yet explain

## Bottom Line

RLM is a real strategic input for OpenAgents, but not because its current code
should become Psionic.

It matters because it demonstrates a concrete future workload:

- recursive
- long-context
- agentic
- fan-out heavy
- budgeted
- parallelizable

That workload can create exactly the kind of buy-side demand the compute market
will eventually need.

The right architectural reading is:

> OpenAgents should adapt RLM as a recursive orchestration and demand pattern
> above Psionic, use Psionic to execute and evidence the subcalls, and use the
> kernel to accept, verify, and settle the resulting outcomes.

If handled that way, RLM can push the stack in the right direction:

- stronger sandbox session semantics
- richer nested receipts
- better operator observability
- better long-context eval harnesses
- a compelling future demand engine for compute and labor

If handled the wrong way, it would just reintroduce the exact scope drift and
runtime ambiguity the current repo is trying to eliminate.
