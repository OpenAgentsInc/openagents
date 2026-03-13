# 2026-03-13 Prime Ecosystem Rust Integration Plan

> Historical note: this document is a point-in-time architecture plan from
> 2026-03-13. Current authority still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and `docs/kernel/`.

## Intent

This document replaces the earlier “Prime sits alongside us” framing.

That was the wrong lens for the question you are actually asking.

The useful question is:

> which parts of the Prime Intellect ecosystem are worth folding into
> OpenAgents as OpenAgents-owned Rust systems, and where should they land?

Assumption for this plan:

- infinite money
- infinite time
- no requirement to reuse PI code directly
- strong preference for OpenAgents-owned Rust implementations
- willingness to widen Psionic well beyond “local inference engine” when that is
  the right owner

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/economy-kernel-proto.md`
- `docs/kernel/data-market.md`
- `docs/kernel/labor-market.md`
- `docs/kernel/liquidity-market.md`
- `docs/kernel/prediction-markets.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/PROVIDER_INTEGRATION.md`
- `crates/psionic/docs/BACKENDS.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-provider/src/lib.rs`
- `crates/psionic/psionic-cluster/src/lib.rs`
- `crates/openagents-kernel-core/src/{compute,data,labor,liquidity,risk,authority,receipts}.rs`
- `crates/openagents-kernel-proto/README.md`
- `apps/nexus-control/src/{kernel,economy}.rs`
- `~/code/pi/README.md`
- `~/code/pi/protocol/{README.md,Cargo.toml}`
- `~/code/pi/prime-iroh/{README.md,Cargo.toml}`
- `~/code/pi/pccl/README.md`
- `~/code/pi/prime-diloco/README.md`
- `~/code/pi/prime-vllm/README.md`
- `~/code/pi/prime-pipeline/README.md`
- `~/code/pi/prime-rl/README.md`
- `~/code/pi/verifiers/README.md`
- `~/code/pi/community-environments/README.md`
- `~/code/pi/research-environments/README.md`
- `~/code/pi/evalchemy/README.md`
- `~/code/pi/genesys/README.md`
- `~/code/pi/toploc/README.md`
- `~/code/pi/toploc-validator/README.md`
- `~/code/pi/gpu-challenge/README.md`
- `~/code/pi/threadpark/README.md`
- `~/code/pi/threadpool/README.md`
- `~/code/pi/pi-quant/README.md`
- `~/code/pi/datasetstream/README.md`
- `~/code/pi/sglang/README.md`
- `~/code/pi/cloud-lora/README.md`
- `~/code/pi/smart-contracts/README.md`
- `~/code/prime/README.md`
- `~/code/prime/packages/prime-sandboxes/README.md`
- `~/code/prime/packages/prime-evals/README.md`
- `~/code/prime/packages/prime-mcp-server/README.md`
- `~/code/prime/packages/prime-tunnel/README.md`

## Executive Summary

With infinite resources, OpenAgents should not merely “integrate with Prime.”
It should internalize the best architectural ideas from the Prime Intellect
ecosystem into an OpenAgents-native Rust stack.

The right end-state is:

- `Psionic` expands into the reusable machine execution substrate for local,
  clustered, remote, and eventually training-class workloads.
- `openagents-kernel-core` / `openagents-kernel-proto` / `apps/nexus-control`
  absorb the economic and market authority layer that prices, verifies,
  collateralizes, settles, and insures those workloads.
- new environment/eval/data-plane surfaces absorb the best ideas from
  `verifiers`, `prime-rl`, `community-environments`, `research-environments`,
  `genesys`, and `evalchemy`.
- new proof and attestation crates absorb the best ideas from `toploc`,
  `toploc-validator`, and `gpu-challenge`.
- app/tooling layers absorb the best ideas from `prime`, `prime-evals`,
  `prime-mcp-server`, `prime-sandboxes`, and `prime-tunnel`.

If we do that well, OpenAgents becomes:

- a compute substrate
- a labor substrate
- a verification substrate
- a risk substrate
- a data substrate

all terminating in one economy kernel.

## The Core Architectural Reading

The PI ecosystem is not one product. It is a rough first draft of a full
machine economy stack.

Broken down by function:

- `protocol`, `prime-iroh`, `pccl`, `prime-vllm`, `prime-pipeline`,
  `prime-diloco`
  - distributed compute, transport, collectives, elastic membership, remote
    execution, pipelining
- `prime`, `prime-sandboxes`, `prime-evals`, `prime-mcp-server`,
  `prime-tunnel`
  - operator control plane, remote runtime lifecycle, evaluation artifact
    handling, MCP wrapping, tunnel/session management
- `verifiers`, `prime-rl`, `community-environments`,
  `research-environments`, `genesys`, `evalchemy`
  - environment definitions, rubrics, synthetic data, eval workflows, async
    RL/training orchestration
- `toploc`, `toploc-validator`, `gpu-challenge`
  - verifiable inference, challenge protocols, compact proof artifacts
- `threadpark`, `threadpool`, `pi-quant`, `datasetstream`
  - native low-level scheduling, quantization, and data-plane components

That maps almost perfectly onto the five-market economy-kernel model:

- `Compute`
- `Data`
- `Labor`
- `Liquidity`
- `Risk`

The main implication is that this work should not be framed as “Prime
integration.” It should be framed as:

> a long-horizon Rust-native OpenAgents rebuild of the valuable parts of the PI
> machine-economy stack.

## Kernel-Mapped Integration Plan

## 1. Compute Market

This is where the PI ecosystem is most obviously valuable, and where Psionic
should expand the most aggressively.

### Valuable PI references

- `protocol`
- `prime-iroh`
- `pccl`
- `prime-vllm`
- `prime-pipeline`
- `prime-diloco`
- `sglang`
- `threadpark`
- `threadpool`
- `pi-quant`
- `datasetstream`
- `cloud-lora`
- parts of `prime` / `prime-sandboxes`

### What OpenAgents should build in Rust

### 1.1 Psionic network and transport layer

Reference:

- `protocol`
- `prime-iroh`

Goal:

- make Psionic capable of explicit P2P and cluster transport beyond the current
  trusted-LAN seam
- support peer discovery, NAT-aware transport, direct connections, relay
  fallback, and stable session identity

Recommended new Psionic ownership:

- `psionic-net`
- or `psionic-cluster` widened substantially

What it should own:

- peer identity
- transport sessions
- topology facts
- link health
- transport capabilities
- stable node membership views

### 1.2 Psionic collectives and shared-state sync

Reference:

- `pccl`
- `prime-diloco`

Goal:

- add Rust-native collective communication and shared-state synchronization
- support dynamic join/leave, world resizing, and topology-aware collective
  planning

Recommended new Psionic ownership:

- `psionic-collectives`
- or new submodules under `psionic-runtime` and `psionic-cluster`

What it should own:

- all-reduce and reduce-scatter contracts
- shared state synchronization
- revisioned cluster state
- topology-aware routing
- bandwidth-aware collective plans

### 1.3 Psionic distributed serving and pipeline execution

Reference:

- `prime-vllm`
- `prime-pipeline`
- `sglang`

Goal:

- make Psionic a real distributed serving runtime, not only a single-node local
  engine
- support pipeline, replica, and tensor/layer-sharded inference under one
  runtime truth model

Recommended Psionic expansion:

- widen `psionic-serve`
- widen `psionic-cluster`
- widen `psionic-runtime`

What it should own:

- stage placement
- shard contracts
- replica routing
- prefix-cache compatibility
- startup/prefill/decode telemetry
- scheduler policy

### 1.4 Psionic bounded sandbox execution

Reference:

- `prime-sandboxes`
- `prime`

Goal:

- turn the already-modeled `psionic.sandbox_execution` lane into a serious
  reusable runtime family
- make bounded remote or local sandbox execution first-class compute supply

Recommended Psionic expansion:

- new `psionic-sandbox` crate
- or widened `psionic-provider` + `psionic-runtime` + `psionic-serve`

What it should own:

- bounded execution profiles
- filesystem/network/process limits
- background job contracts
- artifact transfer contracts
- attach/expose/session semantics
- sandbox evidence receipts

### 1.5 Psionic elastic job and training substrate

Reference:

- `prime-diloco`

Goal:

- eventually let Psionic handle long-running elastic distributed jobs, not only
  inference-class jobs

Recommended ownership:

- later `psionic-train`
- or widened `psionic-cluster` with training lanes

What it should own:

- heartbeat-based membership
- async checkpoint staging
- peer recovery
- resumable cluster jobs
- training-class delivery proofs

### 1.6 Psionic native utility layer

Reference:

- `threadpark`
- `threadpool`
- `pi-quant`
- `datasetstream`

Goal:

- build Rust-native equivalents for high-throughput queueing, host scheduling,
  quantization helpers, and token/data streaming

Recommended ownership:

- support crates under `crates/psionic/`
- possibly `psionic-utils`, `psionic-host`, `psionic-quant`, `psionic-data`

## 2. Data Market

The PI ecosystem is weaker here than Compute, but still valuable.

### Valuable PI references

- `verifiers`
- `community-environments`
- `research-environments`
- `genesys`
- `datasetstream`
- parts of `prime`

### What OpenAgents should build in Rust

### 2.1 Environment and dataset registry

Reference:

- `verifiers`
- `community-environments`
- `research-environments`

Goal:

- make environments, task specs, harnesses, and evaluation packages first-class
  assets in the Data Market

Recommended ownership:

- new crates outside Psionic
- likely `openagents-envs` and `openagents-evals`
- kernel-facing integration in `openagents-kernel-core`

What it should own:

- environment manifests
- versioned asset packaging
- dependency metadata
- signed publication receipts
- installability and compatibility contracts

### 2.2 Synthetic data and task-generation substrate

Reference:

- `genesys`

Goal:

- make synthetic task and dataset generation a first-class data production lane
- bind generated data to verification and provenance

Recommended ownership:

- new data/eval crates
- kernel data-market integration

What it should own:

- generation jobs
- verification jobs
- task schemas
- dataset provenance
- reward and rubric metadata

### 2.3 High-throughput data-plane delivery

Reference:

- `datasetstream`

Goal:

- add streaming delivery for tokenized datasets, shards, and benchmark corpora

Recommended ownership:

- new `openagents-data-plane` crate
- possibly Psionic support if execution nodes consume it directly

## 3. Labor Market

The PI environment/eval stack is highly relevant here.

### Valuable PI references

- `verifiers`
- `prime-rl`
- `community-environments`
- `research-environments`
- `evalchemy`
- `genesys`
- long-tail inspiration from `gpt-engineer`, `parsel`, and similar repos in the
  mirror

### What OpenAgents should build in Rust

### 3.1 Work environments as first-class labor contracts

Reference:

- `verifiers`

Goal:

- make “dataset + harness + rubric” a first-class way to define machine work
- let buyer jobs reference reusable environment packages instead of ad hoc task
  text

Recommended ownership:

- new environment/eval crates
- kernel labor-market bindings in `openagents-kernel-core`

What it should own:

- environment ids and versions
- rubric contracts
- harness definitions
- replay-safe task packaging
- worker/verifier compatibility metadata

### 3.2 Async orchestration for labor loops

Reference:

- `prime-rl`

Goal:

- absorb the separation of orchestrator, trainer, inference, and eval roles
- use that to structure future OpenAgents labor, practice, and benchmark loops

Recommended ownership:

- app/tooling and new orchestration crates
- not directly in `psionic-runtime`, except where reusable execution semantics
  are involved

### 3.3 Unified benchmark and eval substrate

Reference:

- `evalchemy`
- `verifiers`

Goal:

- make evaluations and benchmark runs first-class labor and risk inputs
- unify local eval, distributed eval, and receipted benchmark publication

Recommended ownership:

- `openagents-evals`
- kernel risk and labor integration

## 4. Risk Market

This is the highest-upside PI area after Compute.

### Valuable PI references

- `toploc`
- `toploc-validator`
- `gpu-challenge`
- verifier/rubric structures from `verifiers`

### What OpenAgents should build in Rust

### 4.1 Proof-bearing execution artifacts

Reference:

- `toploc`

Goal:

- add compact proof artifacts to inference and execution receipts
- turn provenance from narrative text into machine-checkable proof material

Recommended ownership:

- new `psionic-proof` crate
- integration with `psionic-runtime`, `psionic-provider`, and
  `openagents-kernel-core`

What it should own:

- activation-derived proof digests
- proof serialization
- verifier-facing proof metadata
- proof generation and verification APIs

### 4.2 Validator services and challenge infrastructure

Reference:

- `toploc-validator`
- `gpu-challenge`

Goal:

- build validator services that can challenge provider claims, inference runs,
  and bounded compute deliveries
- tie challenge results into Coverage, Claim, and RiskSignal objects

Recommended ownership:

- proof crates
- kernel risk-market authority
- `apps/nexus-control` for authority mutation and projection

### 4.3 Risk pricing from real verification signals

Reference:

- `toploc`
- `gpu-challenge`
- `verifiers`

Goal:

- use proof success rates, verifier disagreement, and challenge outcomes as
  first-class Risk Market signals

Recommended ownership:

- `openagents-kernel-core`
- `openagents-kernel-proto`
- `apps/nexus-control`

## 5. Liquidity Market

This is where the PI ecosystem is least directly useful.

### Valuable PI references

- indirect structure only from `smart-contracts`
- indirect operator surface lessons from `prime`

### What OpenAgents should do

- do not force PI concepts into the core Lightning/Hydra direction
- take only the high-level economic ideas:
  - pools
  - domains
  - collateralized participation
  - rewards and slashing concepts

The implementation should remain OpenAgents-native and Bitcoin/Lightning-first.
PI does not appear to have a directly reusable liquidity substrate for our
economy kernel.

## Control Plane And Product Surfaces Worth Rebuilding

These should not be jammed into Psionic, but they are still valuable.

### Valuable PI references

- `prime`
- `prime-evals`
- `prime-mcp-server`
- `prime-tunnel`
- `prime-sandboxes`

### What OpenAgents should build

### 1. OpenAgents operator CLI

Reference:

- `prime`

Goal:

- evolve `autopilotctl` into a full operator CLI for local runtime, cluster,
  environments, proofs, benchmarks, and market state

### 2. OpenAgents MCP server

Reference:

- `prime-mcp-server`

Goal:

- expose safe operator-grade control and inspection to agent clients over MCP

### 3. OpenAgents eval artifact service

Reference:

- `prime-evals`

Goal:

- create explicit evaluation and benchmark artifact publication, batching, and
  finalization flows

### 4. OpenAgents session/tunnel services

Reference:

- `prime-tunnel`
- `prime-sandboxes`

Goal:

- add explicit remote attach/session/tunnel contracts for cluster nodes,
  sandboxes, validators, and operator access

## Ownership Plan

The question is not “should we copy PI?” It is “which OpenAgents layer should
own which rebuilt capability?”

## Expand Psionic aggressively when the capability is reusable execution truth

Psionic should own rebuilt versions of:

- transport/session substrate
- collectives and shared-state sync
- clustered serving and sharding runtime
- bounded sandbox execution runtime
- proof generation hooks and execution evidence
- elastic cluster jobs and later training-class runtime
- host-side native helper kernels and schedulers

This is a much larger Psionic than we have today. That is appropriate.

## Keep market authority and pricing in kernel crates

`openagents-kernel-core` / `openagents-kernel-proto` / `apps/nexus-control`
should own rebuilt versions of:

- compute products and capacity instruments
- environment and dataset asset registration
- labor contracts referencing environments and rubrics
- risk signals derived from proofs and validators
- claim, coverage, settlement, and collateral logic

## Create new non-Psionic crates for environment/eval/data-plane work

Psionic should not absorb everything.

Likely new crate families:

- `openagents-envs`
- `openagents-evals`
- `openagents-data-plane`
- `openagents-proof` if proof work outgrows Psionic coupling

## Keep app/tooling responsibility in app layers

`apps/autopilot-desktop`, `autopilotctl`, and future tooling crates should own:

- CLI and MCP wrappers
- local workspace setup
- operator UX
- environment authoring UX
- benchmark TUI or dashboards

## Kernel Authority Deltas Required

The PI-inspired runtime work only becomes economically real if it terminates in
kernel objects, policies, and receipts. That means the following market-layer
expansion is mandatory.

### Compute kernel deltas

Needed additions or productization:

- richer `ComputeProduct` families for:
  - local inference
  - clustered inference
  - bounded sandbox execution
  - elastic long-running jobs
  - training-class jobs
- delivery proofs that include:
  - topology
  - transport posture
  - collective mode
  - proof references
  - benchmark class
- capacity objects that can express:
  - single-node supply
  - clustered supply
  - reserved or forward capacity
  - recovery and failover posture

### Data kernel deltas

Needed additions or productization:

- `DataAsset` classes for:
  - environment packages
  - benchmark bundles
  - synthetic datasets
  - verifier bundles
  - model or adapter artifacts
- delivery bundles that can reference streamed or sharded data-plane delivery
- permission policies for environment and dataset access

### Labor kernel deltas

Needed additions or productization:

- `WorkUnit` types that can bind to versioned environments and rubrics
- `Submission` payloads that can reference:
  - environment version
  - verifier bundle
  - synthetic generation lineage
  - proof references
- `Verdict` logic that can consume both rubric outputs and proof/challenge
  outcomes

### Risk kernel deltas

Needed additions or productization:

- `RiskSignal` sources tied to:
  - proof success/failure
  - challenge pass/fail
  - verifier disagreement
  - cluster instability
  - benchmark drift
- coverage offers and bindings that understand proof-bearing compute deliveries
- claim flows for sandbox, cluster, and long-running job failures

### Proto and authority deltas

This implies broadening:

- `openagents-kernel-proto`
- `apps/nexus-control`
- receipt projection and snapshot publication

so the new Psionic and environment/eval surfaces do not stay app-local or
runtime-local forever.

## Recommended New OpenAgents Rust Surfaces

If we were green-lighting the whole program, I would open these tracks:

### Psionic family

- `psionic-net`
- `psionic-collectives`
- `psionic-sandbox`
- `psionic-proof`
- `psionic-host`
- `psionic-data`
- later `psionic-train`

### Kernel family

- expand `openagents-kernel-core` compute/data/labor/risk
- expand `openagents-kernel-proto`
- widen `apps/nexus-control` authority and projection routes

### New ecosystem crates

- `openagents-envs`
- `openagents-evals`
- `openagents-data-plane`
- maybe `openagents-benchmark`

## Infinite-Horizon Implementation Order

Given the kernel spec and the PI repo landscape, the rational order is:

### Phase 1: Psionic cluster substrate

Build first:

- transport
- topology
- collectives
- shared-state sync
- clustered serving scheduler

Reference:

- `protocol`
- `prime-iroh`
- `pccl`
- `prime-vllm`
- `prime-pipeline`

Reason:

- this upgrades the Compute Market substrate directly
- it also makes later Risk and Labor work more meaningful

### Phase 2: Psionic proof substrate

Build next:

- proof formats
- validator services
- challenge protocols
- proof-bearing receipts

Reference:

- `toploc`
- `toploc-validator`
- `gpu-challenge`

Reason:

- this is the shortest path from “execution happened” to “execution is
  underwritable”

### Phase 3: Environment and eval substrate

Build next:

- environment manifests
- rubrics
- harness packaging
- synthetic data generation
- unified evaluation publication

Reference:

- `verifiers`
- `prime-rl`
- `community-environments`
- `research-environments`
- `genesys`
- `evalchemy`

Reason:

- this upgrades both Labor and Data markets
- it also strengthens verification and synthetic practice loops already called
  out in `economy-kernel.md`

### Phase 4: Sandbox and remote execution productization

Build next:

- first-class bounded sandbox runtime
- attach/expose/session mechanics
- transferable artifact and job handles

Reference:

- `prime-sandboxes`
- `prime-tunnel`

Reason:

- this turns a planned compute family into a real reusable substrate

### Phase 5: Operator surface unification

Build next:

- full CLI
- MCP server
- eval artifact service
- operator dashboards

Reference:

- `prime`
- `prime-evals`
- `prime-mcp-server`

### Phase 6: Training-class expansion

Build later:

- elastic training runtime
- distributed checkpoint recovery
- long-lived job markets

Reference:

- `prime-diloco`
- `prime-rl`

Reason:

- strategically valuable
- not needed before the compute/labor/risk substrate is already strong

## Specific Repo-To-OpenAgents Mapping

| PI repo | What is valuable | Rebuild target |
| --- | --- | --- |
| `protocol` | worker/validator/orchestrator/discovery split | `psionic-net`, `psionic-cluster`, kernel compute authority |
| `prime-iroh` | reliable P2P transport abstraction | `psionic-net` |
| `pccl` | collectives, shared-state sync, dynamic membership | `psionic-collectives` |
| `prime-vllm` | sharding and pipelined serving semantics | `psionic-cluster`, `psionic-serve` |
| `prime-pipeline` | stage-boundary benchmarking and async pipeline ideas | `psionic-cluster`, benchmark tooling |
| `prime-diloco` | elastic membership, recovery, async checkpoints | later `psionic-train` |
| `verifiers` | environment = dataset + harness + rubric | `openagents-envs`, kernel labor/data integration |
| `prime-rl` | orchestrator/trainer/inference/eval split | `openagents-envs`, orchestration crates, later training |
| `community-environments` | environment marketplace pattern | `openagents-envs` registry |
| `research-environments` | curated environment program | `openagents-envs` publisher/reviewer model |
| `genesys` | synthetic data generation + verifier loop | `openagents-data-plane`, `openagents-evals` |
| `evalchemy` | unified eval runner and output conventions | `openagents-evals` |
| `toploc` | compact proof-bearing inference artifacts | `psionic-proof` |
| `toploc-validator` | operational validation service | proof services + kernel risk authority |
| `gpu-challenge` | verifier challenge protocol for untrusted compute | proof services + risk market |
| `threadpark` | low-level scheduling primitives | `psionic-host` or internal utils |
| `threadpool` | low-overhead worker execution | `psionic-host` or internal utils |
| `pi-quant` | host-side quant/dequant kernels | `psionic-quant` |
| `datasetstream` | streamed token/data delivery | `openagents-data-plane` |
| `prime` | operator CLI and package boundaries | `autopilotctl` expansion, tooling crates |
| `prime-sandboxes` | runtime lifecycle contract shape | `psionic-sandbox`, operator APIs |
| `prime-evals` | batched artifact publication | `openagents-evals` service |
| `prime-mcp-server` | thin MCP control wrapper | OpenAgents MCP server |
| `prime-tunnel` | explicit tunnel/session contract | attach/tunnel services |
| `sglang` | serving scheduler and cache-aware runtime ideas | `psionic-serve` and benchmarks |
| `cloud-lora` | adapterized remote serving concept | future model/artifact market and compute catalog |
| `smart-contracts` | pools/domains/slashing ideas only | high-level market research, not implementation base |

## Bottom Line

The valuable conclusion is not “Prime is separate from us.”

The valuable conclusion is:

> the Prime Intellect ecosystem contains a large amount of architecture that is
> worth re-creating inside OpenAgents as OpenAgents-owned Rust infrastructure.

If we are serious about folding that value into our ecosystem, the priority
order should be:

1. widen Psionic into cluster transport, collectives, distributed serving, and
   sandbox execution
2. build proof-bearing execution and validator services
3. build an environment/eval/data-plane substrate tied into the Labor and Data
   markets
4. productize operator surfaces with CLI, MCP, eval publication, and sessions
5. only then go after training-class infrastructure

That path is fully compatible with `docs/kernel/economy-kernel.md`.

In fact, it is one of the cleanest ways to make the kernel real:

- Compute gets a serious execution substrate.
- Data gets assets, datasets, and environment packaging.
- Labor gets reusable work environments and verifier contracts.
- Risk gets proof-bearing receipts and challenge services.
- Liquidity remains OpenAgents-native, but now has better underlying signals to
  price.
