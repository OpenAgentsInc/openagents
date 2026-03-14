# Prime Ecosystem Compute Integration Spec

Status: proposed  
Date: 2026-03-13

Companion docs:

- `docs/audits/2026-03-13-prime-relation-and-psionic-adaptation-audit.md`
- `docs/plans/compute-market-full-implementation-plan.md`
- `docs/plans/compute-market-launch-truth-checklist.md`
- `docs/kernel/economy-kernel.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`

## Intent

This document answers one concrete question:

> if OpenAgents wants its own Rust-native version of the valuable compute
> architecture across Prime Intellect's ecosystem, what exactly should we build,
> where should it land in this repo, and in what order should it be implemented?

This is not a vendor-integration plan. It assumes:

- we are not adopting Prime or PI code directly
- we do want to rebuild the useful patterns in OpenAgents-owned Rust
- Psionic should expand far beyond today's local-runtime scope
- the current MVP loop still matters and must remain the visible wedge
- the kernel remains the economic authority, even as the execution substrate gets
  much larger

## Executive Thesis

The right long-horizon move is to turn today's local compute-provider lane into
the first truthful slice of a much broader compute stack with five major layers:

1. `apps/autopilot-desktop` stays the operator and product surface.
2. `openagents-provider-substrate` stays a narrow provider descriptor and
   inventory-control layer.
3. `crates/psionic/*` becomes the reusable compute substrate for local,
   clustered, remote, sandboxed, and eventually training-class execution.
4. `openagents-kernel-core`, `openagents-kernel-proto`, and
   `apps/nexus-control` become the economic truth layer for pricing, inventory,
   obligations, delivery proof, challenge outcomes, and settlement.
5. New environment, eval, and proof services become first-class compute
   infrastructure rather than "adjacent tooling".

The Prime/PI ecosystem contributes the strongest patterns in five areas:

- wider-network transport and peer discovery
- elastic collectives and distributed execution
- public-network pipelined inference and training
- bounded remote/sandbox execution with operator surfaces
- evaluation, proof, and validator systems that make compute economically safe

## Hard Constraints From The Current Repo

This spec is long-horizon, but it still has to respect the retained repo truth.

### Product authority

`docs/MVP.md` still defines the visible product promise:

- Desktop-first
- `Go Online -> paid job -> wallet tick up -> withdraw`
- compute-provider first

Nothing in this plan should make the visible MVP loop harder to understand.
Most of the deeper compute machinery should land underneath that loop before it
ever becomes a large user-facing surface.

### Ownership authority

`docs/OWNERSHIP.md` still applies:

- `apps/autopilot-desktop` owns app wiring, UX flows, Mission Control, Provider
  Control, and desktop control
- `crates/openagents-provider-substrate` owns narrow reusable provider health,
  product derivation, inventory controls, and lifecycle helpers
- `crates/psionic/*` owns reusable execution substrate
- `openagents-kernel-*` plus `apps/nexus-control` own authoritative economic
  state and receipts

That means we should not respond to Prime by dumping control-plane or cluster
logic into the desktop app, and we should not let provider-substrate balloon
into a second execution engine.

## Current OpenAgents Compute Reality

The right implementation plan starts from what already exists.

### 1. Desktop already has an app-owned compute control surface

The current app is not just a GUI wrapper around invisible backend state.

Relevant files:

- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/kernel_control.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

Important current truths:

- the desktop already owns the local-runtime seam
- the desktop already owns Provider Control and Mission Control
- the desktop already has an app-owned JSON/event control plane
- `autopilotctl` already gives us a thin operator CLI over the running app
- the desktop already registers online compute inventory with the kernel

That is valuable. Prime's CLI/SDK/operator lessons should extend this control
surface, not replace it with a separate "real" control plane.

### 2. Provider-substrate already owns reusable inventory semantics

Relevant files:

- `crates/openagents-provider-substrate/src/lib.rs`
- `crates/openagents-provider-substrate/src/sandbox.rs`
- `crates/openagents-provider-substrate/src/sandbox_execution.rs`
- `crates/psionic/psionic-sandbox/src/lib.rs`

Important current truths:

- provider backend health is already normalized across GPT-OSS and Apple FM
- provider compute products already exist as a reusable abstraction
- provider-substrate still owns the provider-facing descriptor layer
- bounded sandbox runtime detection and execution now live in Psionic behind a
  compatibility shim
- sandbox profiles, runtime kinds, capability summaries, and evidence are
  already modeled

This means we are not starting from zero on "Prime sandboxes"-like behavior.
However, it also means we need to avoid letting this crate absorb too much
runtime depth. Long-term, the heavy execution substrate should migrate into
Psionic-owned crates while provider-substrate remains the descriptor layer.

### 3. Psionic already contains real cluster and proof-oriented substrate

Relevant files:

- `crates/psionic/psionic-cluster/src/lib.rs`
- `crates/psionic/psionic-cluster/src/ordered_state.rs`
- `crates/psionic/psionic-cluster/src/scheduler.rs`
- `crates/psionic/psionic-cluster/src/replicated_serving.rs`
- `crates/psionic/psionic-cluster/src/layer_sharded.rs`
- `crates/psionic/psionic-cluster/src/tensor_sharded.rs`
- `crates/psionic/psionic-provider/src/lib.rs`
- `crates/psionic/psionic-serve/src/lib.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`

Important current truths:

- `psionic-cluster` already has a trusted-LAN control-plane substrate
- it already has ordered-state concepts, signed introductions, discovery
  candidates, scheduling requests, and topology-aware execution plans
- it already has explicit replicated, layer-sharded, and tensor-sharded
  planning surfaces
- `psionic-provider` already exports capability envelopes, cluster evidence
  bundles, settlement linkage, and delivery-proof surfaces
- `psionic-serve` already carries execution-plan digests and optional clustered
  execution context

This is the most important architectural fact for the whole plan:

> OpenAgents already has the seed of its own Prime-like compute substrate.

The work is to widen and productize it, not to invent the idea from scratch.

### 4. Kernel compute objects exist, but they are still launch-slice thin

Relevant files:

- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-proto/README.md`
- `apps/nexus-control/src/kernel.rs`

Important current truths:

- the kernel already has `ComputeProduct`, `CapacityLot`, `CapacityInstrument`,
  `DeliveryProof`, and `ComputeIndex`
- launch validation currently only recognizes local text-generation products
- `ComputeExecutionKind` is still effectively a local-inference-only shape
- capability envelopes are useful, but too thin for clustered execution,
  sandbox execution, proof posture, or environment binding
- `nexus-control` already has durable compute authority mechanics and compute
  runtime policy gates

This means the economic substrate is ready to widen, but it is not yet modeling
the execution complexity that Prime/PI makes useful.

## Prime / PI Reference Corpus And What To Rebuild

The table below is the shortest useful summary of the compute-relevant PI stack.

| Reference repo | Valuable pattern | Rust landing zone in OpenAgents | Rebuild priority |
| --- | --- | --- | --- |
| `protocol` | worker/orchestrator/validator split for decentralized compute | `psionic-net`, `psionic-cluster`, `apps/nexus-control`, new validator services | very high |
| `prime-iroh` | reliable P2P sessions with direct connect, hole punching, relay fallback | `psionic-net` | very high |
| `pccl` | elastic collectives, shared state sync, dynamic membership, topology optimization | `psionic-collectives`, `psionic-cluster` | very high |
| `prime-vllm` | public-network pipeline parallel inference, pre-sharding, intermediate-result transport | `psionic-serve`, `psionic-cluster`, `psionic-catalog` | very high |
| `prime-pipeline` | research harness for synchronous and asynchronous pipelined inference under variable network conditions | `psionic-serve`, benchmark harnesses, conformance docs | high |
| `prime-diloco` | elastic device mesh, live checkpoint recovery, async distributed checkpointing, quantized cross-node sync | `psionic-collectives`, `psionic-train`, `psionic-checkpoint` | high |
| `prime` | operator CLI/SDK split for availability, pods, sandboxes, environments | `autopilotctl`, future `nexusctl`, shared control schemas | medium-high |
| `prime-sandboxes` | typed sandbox lifecycle API, remote exec ergonomics, background jobs, file transfer | `psionic-sandbox`, desktop control, kernel compute contracts | high |
| `prime-mcp-server` | MCP wrapper over compute control operations | future `openagents-compute-mcp` crate | medium-high |
| `prime-tunnel` | secure tunnel abstraction for exposing local services | `psionic-net` or dedicated control-plane tunnel crate | medium |
| `verifiers` | environment = dataset + harness + rubric | new environments/evals crates, risk integration | very high |
| `prime-evals` | eval artifact ingestion and sample/metric lifecycle | `openagents-evals`, kernel projections | high |
| `community-environments` | registry of reusable environments | `openagents-environments` | medium-high |
| `research-environments` | first-party maintained environment suite | `openagents-environments-research` | medium |
| `prime-rl` | orchestrator/trainer/inference split for async RL and large-scale post-training | `psionic-train` plus environment/eval services | medium-high |
| `toploc` | compact proof artifacts from activation locality fingerprints | `psionic-proof`, `DeliveryProof` metadata | high |
| `gpu-challenge` | cheap challenge protocol for GPU work using Freivalds plus commitments | validator service, risk/adjudication pipeline | high |
| `datasetstream` | efficient streaming dataset server/client for tokenized corpora | `psionic-datastream`, training/eval data plane | medium-high |
| `pi-quant` | fast low-level quantization kernels and multithreaded CPU quant ops | `psionic-runtime`, backend crates, collectives | medium-high |
| `threadpark` | cross-platform thread parking primitive | low-level runtime utility crate if needed | medium |
| `threadpool` | lightweight task execution pool built on explicit wake/park | low-level runtime scheduling substrate if needed | medium |
| `cloud-lora` | remote adapter hosting and elastic throughput for LoRA products | later `psionic-adapters` / `psionic-serve` | medium |
| `genesys` | synthetic reasoning data generation and verification pipeline | `openagents-synthetic-data` plus verifiers integration | medium |
| `evalchemy` | broad benchmark harness and standardized output management | `openagents-evals` benchmark adapters | medium |

## What We Should Actually Build

The rest of this document names the target system more concretely.

## 1. Widen Psionic Into The Full Compute Substrate

Psionic should become the reusable compute substrate for all of the following:

- local inference
- clustered inference
- bounded sandbox execution
- artifact staging and distribution
- proof-bearing execution evidence
- environment-linked eval execution
- later training-class distributed jobs

That does not mean "Psionic owns everything". It means Psionic becomes the
execution substrate other layers can program against.

### 1.1 Proposed Psionic crate expansion

Likely final crate map:

- `psionic-net`
  - wider-network peer identity, session establishment, relay fallback, tunnel
    support, transport metrics
- `psionic-cluster`
  - ordered state, membership, placement, topology, cluster control facts
- `psionic-collectives`
  - elastic all-reduce, reduce-scatter, broadcast, shared-state sync,
    world-resize logic
- `psionic-serve`
  - single-node and clustered serving products
- `psionic-sandbox`
  - bounded sandbox runtime, remote execution, file transfer, artifact capture,
    background jobs
- `psionic-proof`
  - execution proof schema, proof adapters, validator payloads, challenge
    interfaces
- `psionic-datastream`
  - checkpoint and dataset streaming
- `psionic-checkpoint`
  - async checkpointing, live recovery, sidecar/state transfer
- `psionic-train`
  - later training and async post-training substrate
- `psionic-adapters`
  - later adapter and LoRA packaging/hosting

Not all of these need to be created immediately. The important thing is the
architectural direction.

### 1.2 What Psionic should not own

Psionic should still not own:

- pane UX
- wallet UX
- app-specific lifecycle orchestration
- canonical market authority
- buyer-facing procurement flows

Those remain in app or kernel layers.

## 2. Keep `openagents-provider-substrate` Narrow But Smarter

This crate is already the right place for:

- provider backend health normalization
- provider product derivation
- inventory rows and toggles
- reusable provider lifecycle helpers

It should expand only in ways consistent with that role.

### It should gain

- richer product descriptors for cluster-backed and sandbox-backed products
- reusable inventory templates for local, clustered, and sandbox offers
- provider-facing health summaries for cluster membership and proof readiness
- policy-derived advertisability checks for products that require proof posture
  or validator availability

### It should not gain

- network transport
- cluster state machine logic
- proof computation
- sandbox process engines as a long-term owner
- market settlement logic

The current sandbox execution code here is useful seed material. Long-term, the
execution engine should migrate into `psionic-sandbox`, with this crate keeping
only provider-facing descriptors and status summaries.

## 3. Build A Wider-Network Psionic Transport Layer

The clearest gap between current OpenAgents and the best PI compute patterns is
wider-network transport.

`psionic-cluster` is currently a trusted-LAN substrate with strong early shape:

- signed identities
- admission config
- discovery candidates
- introduction envelopes
- ordered state

The next step is to widen it from trusted-LAN to resilient internet-grade
transport.

### Prime patterns to adapt

- `prime-iroh`: direct connection when possible, NAT traversal when needed,
  relay fallback when unavoidable
- `protocol`: explicit discovery/orchestrator/worker roles
- `prime-tunnel`: secure service exposure without asking operators to manage
  raw ingress directly

### Target OpenAgents shape

We should add a transport layer that can represent:

- peer identity
- stable node identity separate from ephemeral sessions
- candidate introduction provenance
- direct-connect path
- relay path
- tunnel path
- transport health and bandwidth observations
- stream classes with explicit delivery guarantees

### Why this matters to the compute market

Without this layer, we can only honestly sell local compute or trusted-LAN
cluster compute. With it, we can sell:

- clustered inference across public networks
- remote whole-request execution
- future globally distributed training products

## 4. Make Ordered Cluster State The Execution Truth, Not The Money Truth

Prime's distributed systems are useful, but we should not copy their authority
model blindly.

OpenAgents should distinguish two different truths:

- Psionic ordered state is authoritative for cluster execution facts
- kernel authority is authoritative for economic facts

That distinction is critical.

### Ordered execution truth should cover

- membership
- health
- topology
- placement
- staging state
- collective world revision
- checkpoint availability
- cluster proof bundle composition

### Kernel truth should cover

- product registration
- lots and inventory
- accepted obligations
- delivery-proof acceptance
- challenge outcomes
- collateral draws
- settlement and indices

This split lets us rebuild the best of `protocol`, `prime-iroh`, `pccl`, and
`prime-diloco` without accidentally making the execution substrate the money
authority.

## 5. Productize Clustered Serving As First-Class Compute Products

Current kernel product validation still treats launch compute as local
text-generation products. That is useful but too narrow.

We should productize clustered serving in a way that stays machine-legible.

### Product families we should eventually support

Near-term truthful families:

- local text generation
- local embeddings
- sandbox container execution
- sandbox python execution
- sandbox node execution
- sandbox posix execution

Medium-term truthful families:

- clustered whole-request inference
- clustered replica-routed inference
- clustered layer-sharded inference
- clustered tensor-sharded inference
- clustered embeddings
- batch evaluation jobs

Longer-horizon families:

- training SFT
- training async RL
- training distributed sync / DiLoCo-like products
- hosted adapter / LoRA serving

### What the product model needs beyond today's enums

Today's `ComputeCapabilityEnvelope` is a good seed, but it needs more structure.

We should add kernel-level concepts for:

- `ComputeTopologyKind`
  - `single_node`
  - `remote_whole_request`
  - `replicated`
  - `pipeline_sharded`
  - `layer_sharded`
  - `tensor_sharded`
  - `sandbox_isolated`
  - `training_elastic`
- `ComputeProvisioningKind`
  - `desktop_local`
  - `cluster_attached`
  - `remote_sandbox`
  - `reserved_cluster_window`
- `ComputeProofPosture`
  - `none`
  - `delivery_proof_only`
  - `topology_and_delivery`
  - `toploc_augmented`
  - `challenge_eligible`
- `ComputeArtifactResidency`
  - where model/data/checkpoint bytes live and how they were staged
- `ComputeEnvironmentBinding`
  - dataset ref
  - rubric ref
  - environment ref
  - evaluator policy ref
- `ComputeCheckpointBinding`
  - checkpoint family
  - recovery posture
  - latest checkpoint source

### Important design rule

Do not overload `ComputeFamily` with topology, proof, or provisioning concerns.

`ComputeFamily` should stay about what is being sold:

- inference
- embeddings
- sandbox execution
- evaluation
- training
- adapter hosting

Topology, proof, and staging should be adjacent fields, not hidden inside
family names.

## 6. Fold Sandbox Execution Into The Compute Market Properly

Prime's sandbox stack is valuable because it makes remote execution feel like a
product, not a shell script.

OpenAgents already has a strong local start, now extracted into
`crates/psionic/psionic-sandbox` with `openagents-provider-substrate` kept as
the provider-facing descriptor layer. The right next step is to make it a
first-class compute family.

### Why it matters

Sandbox compute is the cleanest bridge between:

- buyer demand for remote task execution
- labor contracts
- risk controls
- proof-bearing delivery

It is also where compute and labor start to fuse most directly.

### Target sandbox product shape

Each sandbox product should declare:

- runtime family
- execution class
- profile digest
- filesystem policy
- network policy
- secrets policy
- resource ceilings
- artifact-output policy
- accelerator policy
- proof posture
- environment compatibility

### Ownership split

- `psionic-sandbox` should own the actual runtime engine and evidence bundle
- `openagents-provider-substrate` should own provider-facing summaries and
  advertised product descriptors
- `apps/autopilot-desktop` should own the pane and control interactions
- the kernel should own lots, instruments, delivery proof, challenge state, and
  settlement

## 7. Add Proof-Bearing Execution As A First-Class Contract

Prime's most valuable long-horizon contribution is not "GPUs" or "pods". It is
the insistence that compute needs validator-grade evidence if it is going to
become a real market.

OpenAgents should adopt that deeply.

### Proof layers we should support

#### Base layer: execution delivery proof

This already exists in seed form in Psionic and the kernel:

- execution-plan digest
- metered quantity
- accepted quantity
- capability envelope comparison

We should widen it for:

- topology digest
- selected node set
- transport class
- artifact residency digest
- cluster policy digest
- runtime identity digest
- validator references

#### Optional proof layer: activation fingerprints

`toploc` is valuable because it provides cheap, compact, hardware-tolerant proof
artifacts. We should build an OpenAgents-owned Rust equivalent for inference
products where the economics justify it.

#### Optional proof layer: challenge protocol

`gpu-challenge` is valuable because it gives us a path for cheap adversarial
spot-checking instead of only trusting provider-asserted receipts.

This should become:

- a validator service
- a challenge queue
- an adjudication result
- a risk input
- a settlement gate for proof-sensitive products

### Where proof data should land

- Psionic owns proof generation inputs and bundle assembly
- validator services own challenge execution
- kernel `DeliveryProof` owns accepted proof references and status
- risk market objects own coverage, claims, and bond draw outcomes related to
  challenge failures

## 8. Treat Environments And Evals As Compute Infrastructure

Prime's environment and eval stack is not a side quest. It is compute
infrastructure because it defines what a compute job is allowed to do, how it
is scored, and which evidence bundle matters.

### Valuable patterns

- `verifiers`: environment = dataset + harness + rubric
- `prime-evals`: create/push/finalize eval lifecycle
- `community-environments` and `research-environments`: registry and packaging
- `genesys`: synthetic generation and verification pipeline
- `evalchemy`: benchmark orchestration and standardized outputs
- `prime-rl`: training/eval/orchestrator separation

### OpenAgents adaptation

We should add a Rust-owned environment and eval substrate with:

- environment registry
- environment package descriptor
- dataset refs
- rubric refs
- expected artifact refs
- benchmark adapter refs
- eval run records
- per-sample result artifacts
- synthetic-data generation jobs

### Why this belongs in a compute spec

Because a serious compute market eventually needs products like:

- "run this evaluation bundle"
- "serve this environment-compatible inference lane"
- "execute this sandbox task under this rubric"
- "train under this environment suite"

Without environments and evals, compute is only generic capacity. Prime's
ecosystem shows that the real value is in capacity plus harness plus scoring.

## 9. Bring Prime's Training Patterns In Later, But Explicitly

This spec is compute-first, not training-first. But if we have infinite time and
money, we should still design for training-class execution now.

### Valuable Prime training patterns

- `prime-diloco`: elastic device mesh, live recovery, async checkpointing,
  bandwidth-aware global sync
- `prime-rl`: orchestrator/trainer/inference split
- `datasetstream`: training data plane
- `pi-quant`: quantized communication kernels
- `cloud-lora`: adapter-based deployment products

### OpenAgents adaptation

Long-term Psionic should support:

- elastic cluster membership
- async checkpoint and recovery
- streamed dataset delivery
- quantized collective transport
- training obligations with checkpoint-bearing delivery proof
- adapter outputs as tradable artifacts

This should come after clustered inference, sandbox products, proof systems, and
environment/eval substrate. Training should not distort the near-term compute
market implementation order.

## 10. Kernel And Proto Deltas Required For The Real End-State

The compute market objects must widen materially if this plan is going to fit.

### 10.1 `ComputeProduct`

Needs to add or normalize:

- topology kind
- provisioning kind
- proof posture
- validator requirements
- artifact residency policy
- environment binding
- checkpoint binding
- cluster admission policy ref
- collective policy ref
- sandbox profile ref
- adapter policy ref

### 10.2 `CapacityLot`

Needs to support:

- local offers
- cluster-backed offers
- sandbox-window offers
- future reserved cluster windows
- offer-side proof requirements
- staging or artifact warmness hints

### 10.3 `CapacityInstrument`

Needs to support:

- spot physical clustered allocations
- spot sandbox allocations
- forward physical cluster windows
- reservation rights for scarce products
- challenge holdbacks before final settlement

### 10.4 `DeliveryProof`

Needs to widen substantially with:

- topology kind
- topology digest
- selected node digests or stable ids
- transport class
- artifact residency refs
- execution-plan digest
- cluster evidence bundle ref
- sandbox evidence bundle ref
- proof refs
- challenge refs
- environment binding refs
- evaluator result refs

### 10.5 `ComputeIndex`

Needs separate families for:

- local inference price references
- clustered inference price references
- sandbox execution price references
- later training-window price references

And it needs correction and methodology governance that understands proof and
challenge outcomes.

## 11. Operator And Product Surface Plan

Prime's operator stack is worth rebuilding, but we should do it in an
OpenAgents-shaped way.

### Desktop and `autopilotctl` stay the operator seed

The current app-owned control plane is the correct seed for:

- local runtime truth
- provider online/offline truth
- inventory truth
- cluster attach/detach actions
- sandbox run inspection
- proof inspection
- buy-mode and starter-demand coordination

### We should extend it with

- cluster status and topology commands
- proof and challenge inspection commands
- sandbox lifecycle commands
- environment installation and eval commands
- cluster benchmark and soak-test commands

### We should later add

- an MCP server over the same control contracts
- a shared compute-control schema crate if desktop, headless provider, and
  future services need one canonical JSON shape
- an eventual `nexusctl` for authority and validator operations

### Important rule

Do not create a separate hidden operator stack that bypasses the desktop control
surfaces the product already uses. Prime's CLI ergonomics are worth copying; its
"separate world from the app" pattern is not.

## 12. Recommended Phase Order

The correct order is:

1. widen the schema and ownership seams so future work lands cleanly
2. widen Psionic transport and cluster control beyond trusted-LAN
3. make clustered serving honest and productized
4. move sandbox execution into a real compute-family implementation
5. widen proof, challenge, and validator surfaces
6. make environments and evals kernel-linked compute infrastructure
7. only then widen into full training-class products

This ordering is important because clustered execution without proof, or proof
without kernel-linked delivery objects, creates false confidence instead of a
real market.

## Sequenced GitHub Issue Backlog

This is the recommended issue order for fully implementing the stack above.

## Wave 0: Schema And Boundaries

1. **Compute: ratify Prime-ecosystem compute ownership map**
   - Status: implemented on 2026-03-13 via GitHub issue `#3485`.
   - Write the architectural ADR that locks the owner split for desktop, provider-substrate, Psionic, kernel authority, validators, and environment/eval services.
   - This prevents the inevitable boundary bleed once cluster and sandbox work accelerates.

2. **Compute: widen kernel taxonomy for topology, provisioning, proof, and environment binding**
   - Status: implemented on 2026-03-13 via GitHub issue `#3486`.
   - Extend `openagents-kernel-core/src/compute.rs` with the missing concepts described in this spec.
   - The deliverable is a kernel object model that can express clustered, sandboxed, and proof-sensitive compute without inventing ad hoc metadata blobs.

3. **Compute: widen proto packages and authority contracts to match the new taxonomy**
   - Status: implemented on 2026-03-13 via GitHub issue `#3487`.
   - Extend `openagents-kernel-proto` and the corresponding authority routes/read models so the wire layer matches the real market.
   - This should include product, lot, instrument, delivery, index, validator, and environment-binding surfaces.

4. **Compute: normalize launch product IDs into a Psionic-owned family tree**
   - Status: implemented on 2026-03-13 via GitHub issue `#3488`; migration note in `docs/kernel/compute-product-id-migration.md`.
   - Replace the current launch-slice product naming with a forward-compatible product namespace that can represent local, clustered, and sandbox products cleanly.
   - Preserve compatibility shims for current MVP products while introducing the new canonical IDs.

## Wave 1: Wider-Network Transport

5. **Psionic Net: extract a wider-network transport crate from `psionic-cluster`**
   - Status: implemented on 2026-03-13 via GitHub issue `#3489`.
   - Split transport/session concerns from ordered-state concerns so internet-grade transport can evolve without entangling the whole cluster crate.
   - The new crate should own peer identity, session establishment, stream classes, and transport observations.

6. **Psionic Net: add direct-connect, NAT-traversal, and relay fallback session establishment**
   - Status: implemented on 2026-03-13 via GitHub issue `#3490`.
   - Rebuild the useful `prime-iroh` session semantics in Rust with OpenAgents-owned types and telemetry.
   - The deliverable is reliable peer connectivity across ordinary public-network conditions.

7. **Psionic Net: persist peer introductions, trust bundles, and candidate history**
   - Status: implemented on 2026-03-13 via GitHub issue `#3491`.
   - Durable introduction and trust-bundle storage is required if wider-network cluster membership is going to be auditable and restart-safe.
   - This should reuse the existing signed-introduction shape already emerging in `psionic-cluster`.

8. **Psionic Net: add secure tunnel support for exposing selected local services**
   - Status: implemented on 2026-03-13 via GitHub issue `#3492`.
   - Build the equivalent of `prime-tunnel` for the services OpenAgents actually needs to expose.
   - Keep it scoped to explicit, policy-gated control-plane and inference endpoints rather than generic ingress magic.

## Wave 2: Cluster Control And Scheduling

9. **Psionic Cluster: make ordered state durable and catch-up capable**
   - Status: implemented on 2026-03-13 via GitHub issue `#3493`.
   - Persist ordered cluster facts, event indices, and snapshots so cluster state can survive restarts and late joins.
   - This is the minimum requirement for trustworthy multi-node execution.

10. **Psionic Cluster: graduate discovery candidates into internet-grade admission policy**
   - Status: implemented on 2026-03-13 via GitHub issue `#3494`.
   - Extend the current discovery-candidate and introduction-envelope machinery into a full admission and revocation flow.
   - Support attestation requirements, trust posture, and explicit refusal reasons for market-facing nodes.

11. **Psionic Cluster: implement remote whole-request scheduling over wider-network transport**
   - Status: implemented on 2026-03-13 via GitHub issue `#3495`.
   - Productize the existing scheduler concepts so one node can admit a request and place it on another truthful node.
   - This is the first non-local compute execution lane worth selling.

12. **Psionic Cluster: productize replicated serving placement and routing**
   - Status: implemented on 2026-03-13 via GitHub issue `#3496`.
   - Turn the existing replicated-serving substrate into a real execution mode with explicit topology, routing, and evidence.
   - This creates the first honest clustered low-latency serving product.

13. **Psionic Cluster: productize layer-sharded serving**
   - Status: implemented on 2026-03-13 via GitHub issue `#3497`.
   - Widen the current layer-sharded planning path into an end-to-end execution lane with topology digests and delivery evidence.
   - This is the first meaningful step toward larger-than-one-node inference products.

14. **Psionic Cluster: productize tensor-sharded serving and collective planning**
   - Status: implemented on 2026-03-13 via GitHub issue `#3498`.
   - Turn the current tensor-sharded planner into an execution-backed product that depends on explicit collective eligibility and topology proof.
   - This is where clustered inference begins to converge with later training-class transport needs.

15. **Psionic Cluster: add artifact staging and residency planning**
   - Status: implemented on 2026-03-13 via GitHub issue `#3499`.
   - Model where model bytes, adapters, checkpoints, and datasets live before a request is admitted.
   - This should produce machine-checkable residency facts that later land in delivery proofs.

## Wave 3: Serving, Sharding, And Data Plane

16. **Psionic Serve: add sharded-model manifests and pre-shard artifact handling**
   - Status: implemented on 2026-03-13 via GitHub issue `#3500`.
   - Rebuild the useful parts of `prime-vllm` sharding and pre-shard handling in Psionic terms.
   - The output should be explicit shard manifests and artifact refs, not implicit filesystem conventions.

17. **Psionic Serve: support public-network pipeline-parallel inference**
   - Status: implemented on 2026-03-13 via GitHub issue `#3501`.
   - Recreate the valuable `prime-vllm` and `prime-pipeline` execution modes with explicit prefill/decode timing, transport lanes, and intermediate-result routing.
   - This must integrate with `psionic-cluster` topology and proof surfaces rather than bypassing them.

18. **Psionic Serve: add clustered prefix/KV compatibility and cache truth**
   - Status: implemented on 2026-03-13 via GitHub issue `#3502`.
   - Model whether replicated or sharded lanes can honestly share cache or prefix state.
   - Expose that in capability envelopes so the market is not pretending all clustered inference has the same performance profile.

19. **Psionic Datastream: add streamed dataset and checkpoint delivery**
   - Status: implemented on 2026-03-13 via GitHub issue `#3503`.
   - Build the equivalent of `datasetstream` as a Rust data plane for tokenized corpora, eval bundles, and checkpoints.
   - This is required for future training lanes and for serious environment/eval execution.

20. **Psionic Runtime: import fast quantization and low-level scheduling lessons**
   - Status: implemented on 2026-03-13 via GitHub issue `#3504`.
   - Rebuild the useful `pi-quant`, `threadpark`, and `threadpool` ideas only where Rust-native runtime bottlenecks justify them.
   - This issue should produce explicit performance hooks rather than speculative low-level rewrites everywhere.

## Wave 4: Sandbox And Operator Surface

21. **Psionic Sandbox: extract the runtime engine from provider-substrate**
   - Status: implemented on 2026-03-13 via GitHub issue `#3505`.
   - Move long-term ownership of sandbox execution into a dedicated Psionic crate while preserving compatibility with existing provider-substrate APIs.
   - The old crate should remain the descriptor layer; the new crate should own execution.

22. **Psionic Sandbox: implement container, python, node, and posix execution runners**
   - Status: implemented on 2026-03-13 via GitHub issue `#3506`.
   - Rebuild the current local sandbox execution shapes as a reusable runtime with clear profile digests, file transfer rules, and evidence bundles.
   - This is the foundation for a Prime-sandboxes-quality Rust API.

23. **Psionic Sandbox: add remote background jobs, file transfer, and artifact retrieval**
   - Status: implemented on 2026-03-13 via GitHub issue `#3507`.
   - Match the useful lifecycle semantics from `prime-sandboxes`: create, wait, run, poll, upload, download, and clean up.
   - Every one of those operations should have machine-legible receipts and failure reasons.

24. **Compute Control: extend desktop control and `autopilotctl` for cluster and sandbox operations**
   - Status: implemented on 2026-03-13 via GitHub issue `#3508`.
   - Add commands and snapshot fields for cluster membership, topology, sandbox jobs, proof status, and challenge status.
   - This keeps operator truth inside the app-owned control plane instead of spawning a separate hidden system.

25. **Compute Control: add an OpenAgents compute MCP server**
   - Status: implemented on 2026-03-13 via GitHub issue `#3509`.
   - Rebuild the useful `prime-mcp-server` pattern over OpenAgents-owned control schemas.
   - It should expose inventory, lots, jobs, cluster status, and sandbox operations, but not bypass authority policy.

## Wave 5: Proof, Validators, And Risk

26. **Psionic Proof: define canonical execution-proof bundles for local, clustered, and sandbox compute**
   - Status: implemented on 2026-03-13 via GitHub issue `#3510`.
   - Standardize the bundle shape for execution-plan digest, topology digest, artifact residency, runtime identity, and any optional proof add-ons.
   - This is the core interface between execution and economic settlement.

27. **Psionic Proof: add activation-fingerprint proof adapters**
   - Status: implemented on 2026-03-13 via GitHub issue `#3511`.
   - Rebuild the useful `toploc` idea as an OpenAgents-owned proof module for products where activation-based proofs are economically worth it.
   - Keep the proof posture explicit so products can declare whether they require, support, or ignore this layer.

28. **Validator Service: implement Freivalds/Merkle-style GPU challenge protocol**
   - Status: implemented on 2026-03-13 via GitHub issue `#3512`.
   - Rebuild the useful `gpu-challenge` pattern as an OpenAgents validator service with challenge queues, commitments, and result receipts.
   - The output should be usable by settlement and risk systems, not just a benchmark script.

29. **Kernel: widen `DeliveryProof` to carry topology, sandbox, and validator references**
   - Status: implemented on 2026-03-13 via GitHub issue `#3513`.
   - Extend kernel delivery objects to hold the evidence that clustered and sandboxed execution actually need.
   - This is the main bridge between Psionic proof bundles and economic truth.

30. **Nexus: add validator scheduling, adjudication, and challenge-result projections**
   - Create the authority-side machinery that receives proof refs, queues challenges, accepts outcomes, and projects challenge health into read models.
   - This should surface enough state for operator, buyer, and provider UIs.

31. **Risk: add compute collateral, bond draw, and challenge-linked claim flows**
   - Connect challenge failures, proof absences, and delivery rejections to explicit economic consequences.
   - This is the point where compute stops being a best-effort marketplace and becomes an actually underwritten one.

## Wave 6: Environments, Evals, Synthetic Data, And Training

32. **Environments: define a Rust-native environment package and registry**
   - Rebuild the useful `verifiers` and environment-registry patterns with OpenAgents-owned package descriptors and registry APIs.
   - An environment package should bind dataset refs, harness requirements, rubric refs, artifact expectations, and policy metadata.

33. **Environments: bind environment refs into compute products and delivery proofs**
   - Allow compute products, lots, and jobs to declare the environments they support or require.
   - This is how capacity becomes market-relevant to real eval and training flows.

34. **Evals: add evaluation-run creation, sample ingestion, and finalize flows**
   - Rebuild the valuable `prime-evals` lifecycle in Rust with strong read models and stable artifact references.
   - Every eval should be queryable as a first-class compute output, not just a JSON blob on disk.

35. **Synthetic Data: add generation-plus-verification pipelines**
   - Rebuild the useful `genesys` pattern for synthetic data generation and asynchronous verification.
   - This should terminate in environment and eval objects, not float beside them.

36. **Benchmark Adapters: add broad benchmark harness integrations**
   - Pull in the useful `evalchemy` idea through adapters for benchmark suites where OpenAgents needs standardized outputs.
   - Keep benchmark orchestration separate from core environment semantics so the registry does not become a benchmark junk drawer.

37. **Psionic Train: implement async checkpointing, live recovery, and elastic membership**
   - Rebuild the strongest `prime-diloco` and `prime-rl` training substrate ideas in Psionic terms.
   - This issue should stop short of a huge product surface and focus on execution truth first.

38. **Psionic Train: add quantized collectives and elastic device-mesh semantics**
   - Build the Rust-native equivalent of the most valuable training communication patterns, including quantized sync where it is truthful.
   - This is where `psionic-collectives`, `psionic-datastream`, and `psionic-checkpoint` converge.

39. **Psionic Adapters: add LoRA and adapter artifact packaging and hosted serving**
   - Rebuild the useful `cloud-lora` pattern for OpenAgents-owned adapter products.
   - This should land only after the artifact, delivery-proof, and serving substrate is already credible.

## Wave 7: Market Productization

40. **Desktop: add advanced provider inventory surfaces for local, clustered, and sandbox products**
   - Expose the supply truth already derived by Psionic and provider-substrate without overwhelming the MVP one-button path.
   - Mission Control stays simple; advanced panes show the full inventory truth.

41. **Desktop: add buyer RFQ and quote-selection flows for clustered and sandbox compute**
   - Build human-legible buyer procurement for the new compute families rather than relying only on hidden automation.
   - The resulting flow should terminate in canonical lots and instruments.

42. **Kernel: add forward physical and reservation products for clustered compute**
   - Once spot clustered compute is credible, add reservation and forward-window instruments for scarce cluster products.
   - This should reuse the widened delivery and proof posture rather than inventing separate futures-only logic.

43. **Nexus: publish compute indices for local, clustered, sandbox, and later training lanes**
   - Add market-wide price references and methodology that understand delivery proof quality and challenge outcomes.
   - This is required before any serious derivative or reservation market can be honest.

44. **Desktop and control plane: surface proof, challenge, and settlement history**
   - Providers, buyers, and operators need visibility into why compute was accepted, rejected, challenged, or settled.
   - This should be available both in the GUI and through `autopilotctl`.

45. **Compute Launch Program: build an end-to-end validation matrix and soak harness**
   - Create the comprehensive validation program that proves local, clustered, sandbox, proof-bearing, and environment-bound compute lanes behave as advertised.
   - This should include desktop flows, headless flows, kernel receipts, cluster fault tests, and validator fault tests.

## Closing Read

If we execute this plan, OpenAgents ends up with its own Rust-native version of
the most valuable compute ideas in Prime's ecosystem:

- wider-network cluster transport
- elastic collectives
- pipelined and sharded inference
- bounded sandbox execution
- environment-linked evaluation substrate
- proof-bearing execution
- validator and risk hooks
- later training-class distributed compute

And we get there by widening the code we already have, not by pretending the
current repo is empty.
