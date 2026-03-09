# Exo Unified Integration Plan For Psionic

> Status: drafted 2026-03-08 by merging
> `deep-research-exo.md`, `EXO_INTEGRATION_PLAN.md`, and the current
> `ROADMAP.md` state on `main`.
>
> Baseline assumption: the generic Psionic cutover is already landed on `main`, and
> Psionic can run a truthful local GPT-OSS path today. However, the full
> GPT-OSS/NVIDIA completion track is still in flight as of 2026-03-09:
> `#3244` through `#3248` remain open after `#3243` closed, and the single-node
> execution shape is still converging toward device-resident KV/attention/MoE
> plus graph-based prefill/decode. This document therefore treats clustered
> execution as gated on that local execution shape stabilizing, and it treats
> Metal GPT-OSS as deferred rather than assumed-ready.

## Objective

Integrate the useful parts of `~/code/exo` into Psionic in a way that produces a
truthful, Rust-native cluster execution substrate for OpenAgents.

The target end state is:

- Psionic keeps model, runtime, backend, capability, and evidence truth in
  `crates/psionic/*`
- Psionic gains a Rust-native cluster control plane inspired by Exo's architecture
- Psionic can do cluster-aware placement and scheduling first, then replicated and
  sharded execution when the backend substrate is ready
- any future Exo interoperability remains optional and never becomes the only
  execution path

## Source Hierarchy

The primary semantic references remain:

- `~/code/candle` for Rust runtime/backend structure and quantized storage ideas
- `~/code/tinygrad` for explicit runtime-plan, KV-cache, and execution-evidence
  patterns
- `~/code/llama.cpp` for GGUF/GPT-OSS/Harmony/NVIDIA behavior truth where
  applicable
- `~/code/ollama` for API-visible behavior and local-runtime semantics

`~/code/exo` is a secondary reference repo with four main uses:

- cluster discovery, membership, and topology orchestration
- ordered event-log and catchup design
- topology-aware placement and sharding heuristics
- GPT-OSS/Harmony parser fixtures and multi-API adapter patterns

For Apple-specific clustered execution semantics, Exo should be read precisely:

- Exo's first-class Apple path is MLX-backed, not a generic "Metal available"
  switch
- placement distinguishes concrete distributed communication modes:
  `MlxRing` over socket connectivity and `MlxJaccl` over RDMA/JACCL
- model cards and placement rules gate sharding eligibility explicitly
  (`supports_tensor`, model-specific exclusions, memory fit, topology fit)
- unsupported topology/backend combinations fail at placement time with an
  explicit error rather than silently falling back

Psionic should preserve those semantics when cluster execution reaches Apple, but
translate them into Psionic-owned backend/transport capability classes instead of
copying the MLX runtime dependency or names wholesale.

## Non-Negotiable Constraints

The integration must preserve these boundaries:

- Psionic remains Rust-first and library-first
- Psionic does not take a required Python, MLX, or Exo runtime dependency inside
  `crates/psionic/*`
- Psionic-owned execution must remain the shipped path for inference and evidence
- Exo may inform semantics, but it must not become the hidden backend
- clustered execution must be as machine-checkable as today's local Psionic runtime
- security posture must be explicit and tiered: ship the first cluster path
  under the same-network, operator-controlled local-cluster assumption Exo is
  optimized for, then add stronger membership/authentication requirements
  before widening any cluster claim toward compute-market or adversarial
  environments

In plain terms:

- port the ideas
- do not bind the runtime to Exo

## Trust Model And Discovery Scope

This plan should be read with an explicit scope ladder, not as one giant
"internet-wide cluster" plan.

### Scope 1: Local trusted LAN cluster

This is the first target and the one closest to Exo's current value:

- machines on the same network
- one operator or one cooperating household/lab
- cluster membership scoped by explicit namespace/admission configuration
- goal is faster-than-single-machine inference that can be sold honestly as a
  faster local cluster

For this scope, the immediate requirement is not full adversarial security. The
requirement is:

- explicit cluster admission
- explicit cluster identity
- explicit topology and execution evidence
- no accidental cross-cluster joining

### Scope 2: Operator-configured multi-subnet cluster

This is a later widening step:

- no reliance on mDNS alone
- configured peers or relays
- stronger membership and replay protection
- more explicit operational policy

### Scope 3: Future compute-market distributed cluster

This is not the initial shipping target.

Before Psionic makes this claim, it will need:

- authenticated node identity
- signed control-plane messages
- tamper-evident catchup and replay
- stronger command authorization and provenance policy

The important rule is:

- design the cluster APIs so Scope 3 is possible later
- optimize the first shipped implementation for Scope 1

## What This Plan Includes

This plan intentionally combines all recommendations from the two Exo docs:

- use Exo immediately as a reference repo
- port the highest-value semantics into Psionic tests and docs
- add a Rust-native `psionic-cluster` control-plane layer
- map Exo-style placement onto Psionic-native `ExecutionTopologyPlan`
- phase clustered execution from scheduling to replication to sharding
- extend provider and evidence contracts for cluster truth
- evaluate an optional Exo bridge only after the Rust-native substrate exists

## Current Starting Point

The starting point on `main` is:

- Psionic already has explicit topology planning and selected-device truth
- Psionic already has provider-visible capability and receipt surfaces for
  topology, batching, cache state, and delivery-proof inputs
- Psionic already has a truthful local GPT-OSS path, but the CUDA runtime refactor
  toward device-resident KV, attention, MoE, and graph-based execution is still
  active
- Psionic already has substantial loader and artifact substrate landed in
  `psionic-models` and `psionic-catalog`, including GGUF metadata/tensor loading,
  tokenizer loading, Harmony/GPT-OSS prompt/output handling, paged blob access,
  OCI ingestion into the local model store, artifact identity, provenance, and
  license facts
- OpenAgents no longer depends on the external Ollama daemon for the default
  local path
- Metal GPT-OSS should currently be treated as a deferred backend lane, not as
  a cluster-ready execution target

That means the Exo integration problem is no longer "make Psionic able to run a
real model." It is now:

- add a truthful cluster control plane
- add cluster-aware scheduling and execution
- keep evidence, capability, and policy truth explicit as those features widen

## Backend Readiness Gate For Cluster Execution

The cluster plan should not assume that every local backend lane is equally
ready for clustered execution.

For GPT-OSS specifically, the current gating rule should be:

- phases 0 through 7 can proceed because they are mainly control-plane,
  residency, planning, and evidence work
- phases 8 through 10 must target a stable single-node execution seam rather
  than the current moving GPT-OSS internals
- the first truthful clustered execution target should be one homogeneous
  backend/product lane, realistically CUDA first
- Apple/Metal GPT-OSS nodes should currently be treated as explicit refusal or
  unsupported candidates for cluster execution until the single-node Metal
  GPT-OSS path is real and validated

This preserves truthful planning and avoids coupling the cluster layer to a
backend path that is still being restructured.

The phrase to use here is not "deferred refusal." The concrete rule is:

- Metal GPT-OSS remains a deferred roadmap lane
- until that lane exists, cluster placement should refuse current Psionic Metal
  GPT-OSS nodes for GPT-OSS execution instead of pretending they are eligible

That matches Exo's own posture more closely: unsupported combinations are
rejected during placement rather than hidden behind ambiguous fallback.

## Exo Apple / MLX Semantics To Preserve In Psionic

When Psionic eventually adds clustered Apple execution, the Exo reference says the
cluster layer should model more than "Apple node" or "Metal backend."

Semantics worth preserving:

- communication-class distinction:
  Exo uses `MlxRing` for socket-based ring communication and `MlxJaccl` for
  RDMA-backed JACCL communication
- topology-aware admission:
  ring only needs neighbour connectivity, while JACCL requires all-to-all RDMA
  connectivity plus coordinator selection
- model-card sharding gates:
  placement checks `supports_tensor`, hidden-size divisibility for tensor
  parallelism, and model-specific refusal cases
- single-node downgrade behavior:
  Exo forces single-node placement onto the simpler pipeline/ring path instead
  of pretending tensor/JACCL still applies
- explicit runtime knobs and risk surfaces:
  Exo runner bootstrap sets `MLX_METAL_FAST_SYNCH`, and its MLX distributed
  code treats synchronization/deadlock behavior as a first-class operational
  concern

What Psionic should copy from that:

- explicit transport / communication classes in cluster capability and
  placement types
- explicit backend-specific admission and refusal diagnostics
- explicit model eligibility flags for sharding modes
- explicit coordinator / connectivity evidence in receipts and validation

What Psionic should not copy from that:

- MLX as a required runtime dependency
- MLX-specific environment variables as public Psionic abstractions
- the `MlxRing` / `MlxJaccl` names as the final Psionic API if backend-agnostic
  names are clearer

## What Cluster Work Should Reuse From Existing Psionic Loader And Artifact Work

Cluster integration should build on the loader/artifact work that is already
shipped, not rebuild it.

Already-landed Psionic substrate that cluster work should reuse directly:

- GGUF metadata and tensor loading
- tokenizer reconstruction and chat-template handling
- GPT-OSS/Harmony tokenizer and parser surfaces
- paged blob access and mmap-or-buffered local blob reads
- local model-store integrity and OCI ingestion
- served-artifact identity tuples
- artifact provenance and license gating
- cache invalidation and artifact-drift refusal behavior

That means cluster planning must treat these as inputs:

- artifact digest and identity
- local residency state
- provenance and license policy
- backend/toolchain compatibility

It should not re-open "how to load the model" as a cluster concern.

## Architectural Clarifications To Make Explicit

These items were implicit across the two earlier Exo docs and should now be
first-class requirements.

### Cluster identity

Define:

- `ClusterId`
- `ClusterNodeId`
- `NodeEpoch`
- `ClusterNamespace`

Use them directly in:

- leader election
- event-log ownership
- catchup and replay
- placement provenance
- provider receipts

### Node roles

Support explicit roles from the start:

- control-plane-only node
- execution-capable node
- mixed node

This is the Psionic equivalent of Exo's coordinator-only or `--no-worker` mode and
prevents cluster control and cluster execution from being accidentally coupled.
It should also be explicit which roles may:

- lead
- place
- cache
- execute

### Artifact logistics

Separate placement from residency.

For every node and artifact, model explicit states such as:

- `already_present`
- `needs_copy`
- `needs_pull`
- `refused`

And make receipts able to say whether execution used:

- pre-existing local artifacts
- cluster-staged artifacts
- OCI-pulled artifacts

### Serving policy risks

Make these cluster-serving behaviors explicit instead of burying them in
runtime defaults:

- prefill-vs-decode fairness
- continuous batching admission policy
- cluster-wide cancellation behavior
- backpressure when a slow or overloaded node holds a shard

### State durability

The event log must have explicit lifecycle policy:

- indexed ordered log
- snapshot frequency
- replay-from-snapshot rules
- schema/version migration rules
- maximum catchup window before full resync is required

### Evidence mapping

Clustered execution should emit explicit machine-checkable digests for:

- cluster plan digest
- node-set digest
- shard-map digest
- per-node artifact digest
- per-node backend/toolchain digest
- per-node warm/cold state
- aggregate degraded/fallback reason

## Desired End State

Full integration of the Exo recommendations is complete only when all of the
following are true:

- Exo's useful semantics are represented by Psionic-owned types, tests, and docs
- Psionic has a Rust-native cluster transport, membership, and ordered state model
- Psionic has explicit persistent cluster identity, node identity, node epoch, and
  node role modeling
- Psionic can build a truthful cluster-wide placement plan from live topology and
  resource facts
- Psionic separates placement from artifact residency and can state whether an
  artifact was already present, copied, pulled, or refused on each node
- Psionic can schedule work across nodes without hiding where execution happened
- Psionic can support replicated cluster serving
- Psionic can support at least one truthful sharded execution path, or explicitly
  refuse unsupported cluster sharding with stable diagnostics
- provider capabilities and receipts expose cluster topology, node selection,
  delivered execution shape, and degraded/refused reasons
- clustered serving policy is explicit about fairness, batching, cancellation,
  and backpressure
- the event log has explicit snapshot, replay, compaction, and resync rules
- cluster networking, event ordering, and catchup are fault tested
- the team has made an explicit keep-or-discard decision on optional Exo
  interoperability

## Implementation Plan

## Phase 0: Lock The Reference Contract

Goal:

- make Exo an intentional secondary reference rather than an informal source of
  ideas

Steps:

1. Add this unified plan to the active Psionic docs set.
2. Keep `deep-research-exo.md` as the architectural deep dive and
   `EXO_INTEGRATION_PLAN.md` as the narrower audit note, but treat this
   document as the implementation order.
3. Update Psionic contributor guidance so Exo is consulted when work touches:
   cluster discovery, ordered event routing, placement, GPT-OSS/Harmony parser
   behavior, or multi-API adapter design.
4. Record intentional source-of-truth precedence:
   `llama.cpp` wins for GGUF/GPT-OSS/Harmony execution truth,
   `ollama` wins for API-visible semantics,
   Exo wins only for cluster-control design unless a narrower layer is named.

Deliverables:

- docs update
- contributor guidance note

Definition of done:

- future Exo-related work has an explicit reference rule, not an ad hoc one

## Phase 1: Harvest Exo Semantics Into Psionic-Owned Tests And Fixtures

Goal:

- import the high-value Exo behavior into Psionic without importing Exo runtime
  dependencies

Steps:

1. Port GPT-OSS/Harmony parser fixture cases from Exo into Psionic tests.
2. Port or recreate the key parser scenarios:
   commentary tool-call recipient placement,
   thinking-then-tool-call flow,
   truncated tool-call termination,
   plain-text non-tool-call output.
3. Capture Exo's GPT-OSS-specific EOS/default handling as documented test
   fixtures where it matches the local model reality.
4. Translate Exo placement examples into Psionic planner tests:
   cycle filtering by memory,
   smallest-cycle selection,
   ring-vs-RDMA coordinator selection heuristics,
   layer-allocation proportionality edge cases,
   single-node downgrade to the simpler communication class,
   and explicit refusal for unsupported backend/topology/model combinations.
5. Port Exo KV-prefix-cache hit/update ideas into Psionic test cases where they
   match Psionic's cache policy model.
6. Document every intentional deviation where Psionic chooses a different semantic.

Deliverables:

- new Psionic tests
- fixture corpus additions
- docs note for intentional deviations

Definition of done:

- the important Exo-derived semantics are encoded in Psionic tests
- Psionic can preserve those behaviors without needing Exo at runtime

## Phase 2: Create A Rust-Native Cluster Crate Layer

Goal:

- define the Psionic-native control-plane substrate that will own cluster truth

Recommended crate structure:

- `psionic-cluster`
- optional small subcrates later for transport or persistent log internals if
  needed

Core types to add:

- `ClusterId`
- `ClusterNodeId`
- `NodeEpoch`
- `NodeRole`
- `ClusterTrustMode`
- `ClusterNamespace`
- `ClusterMembershipRecord`
- `ClusterLink`
- `ClusterCommand`
- `ClusterEvent`
- `IndexedClusterEvent`
- `ClusterState`
- `ClusterSnapshot`
- `ClusterCatchupRequest`
- `ClusterCatchupResponse`

Steps:

1. Define the crate boundary and keep it engine-first and reusable.
2. Define the typed command/event model, mirroring Exo's useful separation:
   commands, local facts, ordered global facts, election traffic, connection
   traffic.
3. Define a contiguous indexed-event apply discipline like Exo's event model.
4. Define persistent cluster and node identity rules instead of relying on
   session-local IDs.
5. Define node roles and admission rules up front so control-plane-only nodes
   are a first-class shape.
6. Define snapshot and compaction hooks up front so the log does not grow
   unbounded by accident.
7. Define a stable digest for cluster state snapshots and topology plans so
   receipts can refer to them.

Deliverables:

- `psionic-cluster` crate skeleton
- typed schemas
- state machine apply path
- unit tests for ordering and replay

Definition of done:

- Psionic has a place to put cluster truth that is not hidden inside `psionic-serve` or
  app code

## Phase 3: Transport, Membership, And Secure Cluster Formation

Goal:

- build a real transport and discovery substrate, informed by Exo and scoped
  first for same-network local clusters

Steps:

1. Reuse or adapt the Exo Rust networking ideas around:
   libp2p,
   gossipsub,
   mDNS discovery,
   ping-based durable connectivity,
   namespace isolation,
   private network scoping.
2. For the first shipped cluster path, implement explicit local-cluster
   admission policy:
   namespace scoping,
   cluster join policy,
   operator-configured admission secret or equivalent,
   explicit no-cross-cluster joining behavior.
3. Support at least two discovery modes:
   zero-config LAN discovery,
   explicit configured peers for non-mDNS environments.
4. Represent namespace/versioning as explicit cluster configuration rather than
   environment-only magic.
5. Add durable node identity rather than Exo's current effectively-ephemeral
   generated identity path.
6. Reserve message schema fields for later signed/authenticated membership
   expansion before any future compute-market-wide cluster claim.

Deliverables:

- transport layer in Rust
- membership/auth model
- discovery configuration
- integration tests for join/leave/rejoin

Definition of done:

- a Psionic local cluster can form and identify its members truthfully under the
  same-network operator-controlled trust assumption

## Phase 4: Ordered Log, Catchup, And Leader Election

Goal:

- make cluster state converge under lossy or unstable conditions

Steps:

1. Implement a single ordered global event stream for cluster-wide decisions.
2. Add leader election or equivalent ordering authority for:
   placement decisions,
   membership ordering,
   cluster-wide state transitions.
3. Port Exo's useful catchup pattern:
   ordered buffers,
   missing-index detection,
   explicit catchup request,
   exponential-backoff retry.
4. Add persisted event log storage with replay tests.
5. Add snapshot frequency, replay-from-snapshot, and compaction policy with
   deterministic restore tests.
6. Define a maximum catchup window before full resync is required.
7. Define split-brain and stale-leader behavior explicitly and test it.

Deliverables:

- leader/orderer implementation
- ordered event log
- catchup path
- replay/snapshot tests

Definition of done:

- dropped messages or temporary disconnects do not silently fork cluster truth

## Phase 5: Cluster Telemetry And Topology Facts

Goal:

- make placement decisions from explicit replicated facts, not local guesswork

Steps:

1. Add per-node telemetry events for:
   device inventory,
   backend readiness,
   memory budget,
   storage budget,
   health state,
   load state,
   node role.
2. Add link-fact events for:
   connection up/down,
   transport class,
   measured latency,
   measured bandwidth,
   RDMA capability where applicable.
3. Define a stable cluster-topology digest and stable topology snapshot format.
4. Keep topology facts separate from placement decisions so diagnostics stay
   machine-checkable.
5. Add fault cleanup behavior for disappeared nodes and stale links.

Deliverables:

- topology schema
- node telemetry schema
- cluster topology digest
- cleanup behavior tests

Definition of done:

- cluster placement can be justified from explicit shared state

## Phase 6: Artifact Residency And Logistics

Goal:

- make artifact presence and staging explicit before placement and execution

Steps:

1. Add per-node artifact residency state keyed by served-artifact identity.
2. Separate placement feasibility from residency feasibility.
3. Model at least these residency outcomes:
   already present,
   needs copy,
   needs pull,
   refused.
4. Decide the first transport for staging:
   local peer-to-peer copy,
   OCI pull,
   or explicit refusal when neither is allowed.
5. Require digest checking for any staged artifact path.
6. Extend receipts so they can say whether execution used:
   pre-existing local artifacts,
   cluster-staged artifacts,
   freshly pulled artifacts.

Deliverables:

- residency state model
- artifact logistics policy
- digest-checked staging path or explicit refusal
- receipt integration

Definition of done:

- cluster planning and execution can talk honestly about where model bytes came
  from

## Phase 7: Placement Planner To Psionic-Native ExecutionTopologyPlan

Goal:

- turn Exo-style cluster heuristics into Psionic-native placement output

Steps:

1. Build a cluster planner that consumes `ClusterState` and emits
   `ExecutionTopologyPlan`.
2. Start with these truthful outputs:
   `SingleDevice`,
   `Replicated`,
   `LayerSharded`,
   `TensorSharded`.
3. Use Exo's cycle/topology heuristics as planner internals where useful, but
   do not expose cycles as the public API.
4. Integrate Psionic constraints into planning:
   backend readiness,
   artifact identity,
   artifact residency,
   provenance/license gating,
   memory admission,
   policy refusals,
   performance qualifiers,
   communication-class eligibility,
   and model sharding eligibility.
5. Emit explicit refusal diagnostics when no valid placement exists.
6. Emit planner explanations and stable plan digests so receipts can explain the
   chosen topology.

Deliverables:

- planner implementation
- refusal diagnostics
- placement tests
- plan digest integration

Definition of done:

- cluster placement results are Psionic-native, stable, and explainable

## Phase 8: Cluster-Aware Scheduling With Single-Node Execution First

Goal:

- ship useful cluster behavior before attempting distributed tensor transport

Steps:

1. Add remote execution selection so a request can be admitted on one node and
   executed on another Psionic node.
2. Keep the execution path Psionic-native on the selected node through a stable
   local execution seam, not by coupling cluster scheduling to backend-specific
   GPT-OSS internals that are still in flux.
3. Surface in receipts:
   selected node,
   selected devices,
   execution topology,
   promised vs delivered execution location.
4. Add cluster-aware warm/load/unload and keepalive policy.
5. Add cluster admission control and queue policy.
6. Make prefill-vs-decode fairness explicit.
7. Define continuous batching admission policy explicitly.
8. Add cluster-wide cancellation, retry, and degraded/fallback semantics.
9. Add backpressure behavior for slow coordinator or slow execution nodes.
10. For GPT-OSS, start with a homogeneous backend class and explicit backend
    admission rules rather than mixed-backend scheduling guesses.
11. When Apple cluster execution exists, model communication-class admission
    explicitly in the Exo spirit:
    socket/ring-like paths versus RDMA/all-to-all paths, not a single generic
    "Metal cluster" bucket.

Deliverables:

- remote scheduling path
- cluster-aware lifecycle policy
- capability/receipt changes
- end-to-end tests

Definition of done:

- Psionic can use a cluster to pick the best execution node without pretending it is
  already doing sharded execution
- the first shipping path can refuse nodes whose local backend/product lane is
  not yet cluster-ready, including current Metal GPT-OSS nodes

## Phase 9: Replicated Cluster Serving

Goal:

- make clustered serving operationally useful before true sharding

Steps:

1. Add replicated model residency across multiple nodes.
2. Start with homogeneous replication for one truthful backend/product lane,
   not mixed CUDA+Metal GPT-OSS replication.
3. Add cluster-aware balancing and queue routing across replicas.
4. Add replica health and backpressure reporting.
5. Define how shared artifact identity and version drift are handled across
   replicas.
6. Ensure receipts distinguish:
   cluster available replicas,
   selected replica,
   degraded replica routing,
   refused replica routing.

Deliverables:

- replicated residency model
- routing policy
- observability and receipt extensions
- fault-injection tests

Definition of done:

- Psionic can truthfully serve the same model from multiple nodes in one cluster
- cluster replication does not imply that every backend with local loading
  support is already an eligible GPT-OSS execution backend
- future Apple replication should inherit the same communication-class and
  topology gates Exo applies to MLX ring and JACCL modes

## Phase 10: True Sharded Cluster Execution

Goal:

- add real multi-node execution only after the control plane and scheduling are
  solid

Execution order:

1. layer-sharded execution
2. tensor-sharded execution
3. any more exotic topology after those are stable

Steps:

1. Define a backend-specific distributed communication substrate for activations,
   KV, and synchronization.
2. Start with one backend/platform combination and make it truthful before
   widening claims. Given the current local-runtime state, that should be gated
   on the CUDA single-node execution track stabilizing first, especially the
   graph/runtime seam.
3. Extend compiled plans with cross-node communication edges.
4. Extend runtime evidence with:
   shard map,
   per-node kernel/transfer summaries,
   per-node cache behavior,
   distributed prefill/decode timing,
   cross-node bytes moved.
5. Add parity and behavioral validation between:
   single-node baseline,
   replicated baseline where applicable,
   sharded execution path.
6. Refuse unsupported sharding modes explicitly instead of silently collapsing
   to single-node execution.
7. Defer Metal GPT-OSS sharding until there is a real single-node Apple path;
   do not use cluster work to paper over missing local Metal execution.
8. When Apple sharding is attempted later, require explicit communication-class
   validation analogous to Exo's ring-vs-JACCL split instead of treating Apple
   clustered execution as one undifferentiated backend mode.

Deliverables:

- backend-specific distributed execution substrate
- sharded execution path
- parity and behavior tests
- explicit refusal paths

Definition of done:

- Psionic can truthfully execute at least one real sharded cluster path, or truthfully
  refuse unsupported ones

## Phase 11: Provider, Capability, Receipt, And Evidence Expansion

Goal:

- make clustered execution first-class in OpenAgents capability and receipt
  surfaces

Steps:

1. Extend capability envelopes with:
   cluster membership class,
   transport class,
   topology class,
   cluster size,
   sharding capability,
   replica count,
   cluster queue/admission posture.
2. Extend receipts and provenance with:
   selected nodes,
   delivered nodes,
   topology digest,
   placement digest,
   node-set digest,
   shard-map digest,
   per-node artifact identity,
   per-node artifact digest,
   per-node backend/toolchain digest,
   per-node load state,
   per-node degraded/refusal data,
   promised-vs-delivered topology comparison.
3. Reuse existing Psionic execution-topology and delivery-proof structures where
   possible instead of inventing side channels.
4. Ensure cluster evidence is machine-checkable enough for compute-market use.

Deliverables:

- provider schema extensions
- provenance/receipt schema extensions
- compatibility tests

Definition of done:

- clustered execution truth is visible to downstream consumers without needing
  app-local reconstruction

## Phase 12: Optional Exo Interoperability Spike

Goal:

- test whether Exo is worth keeping as an optional orchestrator peer after the
  Rust-native substrate exists

This phase is optional and must not block the earlier phases.

Allowed scope:

- Exo may provide discovery, election, or placement input
- execution on each node must still be performed by Psionic
- Psionic capabilities and receipts must remain the source of truth

Spike options:

1. Exo orchestrates Psionic worker nodes through an explicit adapter seam.
2. Psionic ingests Exo-discovered topology or placement hints but keeps the final
   placement decision internal.

Decision criteria:

- keep the bridge only if it reduces real integration cost without weakening
  evidence truth or introducing a required Python/MLX dependency
- otherwise discard the bridge and keep only the ported design ideas

Deliverables:

- limited-scope spike
- keep/discard decision memo

Definition of done:

- the team makes an explicit decision rather than drifting into accidental Exo
  dependency

## Phase 13: Security Hardening, Fault Injection, And Rollout

Goal:

- validate the cluster substrate as a real OpenAgents execution surface

Steps:

1. Add fault-injection coverage for:
   dropped pubsub messages,
   stale leaders,
   split brain,
   disconnect/rejoin,
   node timeout,
   stale catchup responses,
   partial replica outage.
2. Add security tests for:
   unauthorized peers,
   namespace mismatch,
   tampered cluster messages,
   replayed cluster commands/events.
3. Before widening beyond the local trusted LAN assumption, add:
   authenticated node identity,
   signed control-plane messages,
   tamper-evident catchup and replay protection,
   stronger cluster admission policy.
4. Add a hardware and network validation matrix for:
   single host,
   two-node LAN,
   multi-node LAN,
   RDMA-capable cluster if supported,
   refusal paths when transport/backend claims are not met.
5. Add operational runbooks:
   cluster bring-up,
   namespace configuration,
   membership debugging,
   catchup recovery,
   cluster placement debugging,
   degraded/refused execution diagnosis.
6. Add performance gates before widening cluster claims.

Deliverables:

- fault-injection suite
- security validation
- hardware/network matrix
- runbooks

Definition of done:

- the cluster feature is supportable and its claims are evidence backed

## Recommended Execution Order

From today's `main`, the shortest honest order is:

1. Phase 0: reference contract
2. Phase 1: tests and fixtures
3. Phase 2: `psionic-cluster` crate
4. Phase 3: transport and membership
5. Phase 4: ordered log and election
6. Phase 5: topology and telemetry
7. Phase 6: artifact residency and logistics
8. Phase 7: placement planner
9. Gate execution phases on a stable local single-node runtime seam for the
   target backend/product lane
10. Phase 8: cluster-aware single-node scheduling
11. Phase 9: replicated serving
12. Phase 10: true sharded execution
13. Phase 11: provider/evidence expansion in parallel with phases 8-10, with
    the required schema work landing before any public claim widening
14. Phase 13: hardening and rollout
15. Phase 12: optional Exo interoperability spike only when the Rust-native
    substrate is already credible

For GPT-OSS today, that gate means:

- proceed freely through phases 0-7
- do not begin serious cluster execution work on Metal GPT-OSS yet
- treat CUDA as the likely first truthful clustered execution backend if the
  local `#3244`-`#3246` work lands cleanly
- do not wait for late-stage kernel parity work before continuing backend-agnostic
  cluster-control phases

## What Not To Do

Do not:

- make Exo a required runtime dependency for `crates/psionic/*`
- proxy Psionic execution through Exo and call that integration complete
- widen cluster capability claims before the corresponding evidence surfaces
  exist
- treat the first local-cluster shipping path as if it already solved
  adversarial compute-market cluster security
- claim sharded cluster execution when the system is only doing remote
  scheduling
- start clustered execution work on top of a local backend/product path whose
  execution seam is still being rewritten
- copy Exo's relaxed transport-security posture into a compute-market-facing
  Psionic cluster
- leak Apple/MLX assumptions into Psionic's backend truth
- treat deferred Metal GPT-OSS support as if it were an eligible cluster
  execution lane
- erase Exo's useful Apple placement semantics by collapsing communication mode,
  topology fit, and model sharding eligibility into a single "Metal capable"
  boolean
- add cluster complexity to app code when the substrate belongs in `crates/psionic/*`

## Deliverables Checklist

Full integration of the Exo recommendations requires all of these deliverables:

- unified Exo docs set in `crates/psionic/docs`
- Psionic-owned Exo-derived parser and planner tests
- `psionic-cluster` crate
- secure cluster transport and membership
- ordered event log and catchup
- leader ordering or equivalent ordering authority
- topology and telemetry facts
- artifact residency/logistics
- Psionic-native planner to `ExecutionTopologyPlan`
- remote scheduling path
- replicated serving path
- at least one truthful sharded execution path or explicit refusal coverage
- provider and receipt schema support for cluster truth
- validation matrix, runbooks, and security/fault testing
- explicit keep/discard decision on optional Exo interoperability

## Final Definition Of Done

This unified plan is complete only when:

- Exo's useful recommendations have been absorbed into Psionic's docs, tests, and
  Rust-native cluster substrate
- Psionic owns the shipped cluster execution truth end to end
- downstream OpenAgents systems can tell exactly what cluster topology was
  promised, selected, and delivered
- any Exo interoperability that remains is optional, bounded, and honestly
  documented
