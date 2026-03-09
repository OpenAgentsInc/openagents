# Exo Unified Integration Plan For Mox

> Status: drafted 2026-03-08 by merging
> `deep-research-exo.md`, `EXO_INTEGRATION_PLAN.md`, and the current
> `ROADMAP.md` state on `main`.
>
> Baseline assumption: the generic Mox cutover and GPT-OSS/NVIDIA completion
> track are already landed on `main`, so this document starts from "Mox can run
> the truthful local GPT-OSS path" and defines the remaining work to integrate
> the full set of Exo-derived recommendations without surrendering Mox's
> Rust-first runtime truth.

## Objective

Integrate the useful parts of `~/code/exo` into Mox in a way that produces a
truthful, Rust-native cluster execution substrate for OpenAgents.

The target end state is:

- Mox keeps model, runtime, backend, capability, and evidence truth in
  `crates/mox/*`
- Mox gains a Rust-native cluster control plane inspired by Exo's architecture
- Mox can do cluster-aware placement and scheduling first, then replicated and
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

## Non-Negotiable Constraints

The integration must preserve these boundaries:

- Mox remains Rust-first and library-first
- Mox does not take a required Python, MLX, or Exo runtime dependency inside
  `crates/mox/*`
- Mox-owned execution must remain the shipped path for inference and evidence
- Exo may inform semantics, but it must not become the hidden backend
- clustered execution must be as machine-checkable as today's local Mox runtime
- security posture must be stronger than Exo's current "fast local cluster"
  assumptions

In plain terms:

- port the ideas
- do not bind the runtime to Exo

## What This Plan Includes

This plan intentionally combines all recommendations from the two Exo docs:

- use Exo immediately as a reference repo
- port the highest-value semantics into Mox tests and docs
- add a Rust-native `mox-cluster` control-plane layer
- map Exo-style placement onto Mox-native `ExecutionTopologyPlan`
- phase clustered execution from scheduling to replication to sharding
- extend provider and evidence contracts for cluster truth
- evaluate an optional Exo bridge only after the Rust-native substrate exists

## Current Starting Point

The starting point on `main` is:

- Mox already has explicit topology planning and selected-device truth
- Mox already has provider-visible capability and receipt surfaces for
  topology, batching, cache state, and delivery-proof inputs
- Mox already has the local GPT-OSS/NVIDIA path landed
- OpenAgents no longer depends on the external Ollama daemon for the default
  local path

That means the Exo integration problem is no longer "make Mox able to run a
real model." It is now:

- add a truthful cluster control plane
- add cluster-aware scheduling and execution
- keep evidence, capability, and policy truth explicit as those features widen

## Desired End State

Full integration of the Exo recommendations is complete only when all of the
following are true:

- Exo's useful semantics are represented by Mox-owned types, tests, and docs
- Mox has a Rust-native cluster transport, membership, and ordered state model
- Mox can build a truthful cluster-wide placement plan from live topology and
  resource facts
- Mox can schedule work across nodes without hiding where execution happened
- Mox can support replicated cluster serving
- Mox can support at least one truthful sharded execution path, or explicitly
  refuse unsupported cluster sharding with stable diagnostics
- provider capabilities and receipts expose cluster topology, node selection,
  delivered execution shape, and degraded/refused reasons
- cluster networking, event ordering, and catchup are fault tested
- the team has made an explicit keep-or-discard decision on optional Exo
  interoperability

## Implementation Plan

## Phase 0: Lock The Reference Contract

Goal:

- make Exo an intentional secondary reference rather than an informal source of
  ideas

Steps:

1. Add this unified plan to the active Mox docs set.
2. Keep `deep-research-exo.md` as the architectural deep dive and
   `EXO_INTEGRATION_PLAN.md` as the narrower audit note, but treat this
   document as the implementation order.
3. Update Mox contributor guidance so Exo is consulted when work touches:
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

## Phase 1: Harvest Exo Semantics Into Mox-Owned Tests And Fixtures

Goal:

- import the high-value Exo behavior into Mox without importing Exo runtime
  dependencies

Steps:

1. Port GPT-OSS/Harmony parser fixture cases from Exo into Mox tests.
2. Port or recreate the key parser scenarios:
   commentary tool-call recipient placement,
   thinking-then-tool-call flow,
   truncated tool-call termination,
   plain-text non-tool-call output.
3. Capture Exo's GPT-OSS-specific EOS/default handling as documented test
   fixtures where it matches the local model reality.
4. Translate Exo placement examples into Mox planner tests:
   cycle filtering by memory,
   smallest-cycle selection,
   ring-vs-RDMA coordinator selection heuristics,
   layer-allocation proportionality edge cases.
5. Port Exo KV-prefix-cache hit/update ideas into Mox test cases where they
   match Mox's cache policy model.
6. Document every intentional deviation where Mox chooses a different semantic.

Deliverables:

- new Mox tests
- fixture corpus additions
- docs note for intentional deviations

Definition of done:

- the important Exo-derived semantics are encoded in Mox tests
- Mox can preserve those behaviors without needing Exo at runtime

## Phase 2: Create A Rust-Native Cluster Crate Layer

Goal:

- define the Mox-native control-plane substrate that will own cluster truth

Recommended crate structure:

- `mox-cluster`
- optional small subcrates later for transport or persistent log internals if
  needed

Core types to add:

- `ClusterNodeId`
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
4. Define snapshot and compaction hooks up front so the log does not grow
   unbounded by accident.
5. Define a stable digest for cluster state snapshots and topology plans so
   receipts can refer to them.

Deliverables:

- `mox-cluster` crate skeleton
- typed schemas
- state machine apply path
- unit tests for ordering and replay

Definition of done:

- Mox has a place to put cluster truth that is not hidden inside `mox-serve` or
  app code

## Phase 3: Transport, Membership, And Secure Cluster Formation

Goal:

- build a real transport and discovery substrate, informed by Exo but hardened
  for Mox's threat model

Steps:

1. Reuse or adapt the Exo Rust networking ideas around:
   libp2p,
   gossipsub,
   mDNS discovery,
   ping-based durable connectivity,
   namespace isolation,
   private network scoping.
2. Replace Exo's relaxed local-cluster security posture with a Mox-appropriate
   one:
   authenticated peers,
   signed/verified control-plane messages,
   tamper-resistant catchup responses,
   explicit trust domain configuration.
3. Support at least two discovery modes:
   zero-config LAN discovery,
   explicit configured peers for non-mDNS environments.
4. Represent namespace/versioning as explicit cluster configuration rather than
   environment-only magic.
5. Add durable node identity rather than Exo's current effectively-ephemeral
   generated identity path.

Deliverables:

- transport layer in Rust
- membership/auth model
- discovery configuration
- integration tests for join/leave/rejoin

Definition of done:

- a Mox cluster can form and identify its members truthfully and securely

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
5. Add snapshot and compaction policy with deterministic restore tests.
6. Define split-brain and stale-leader behavior explicitly and test it.

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
   load state.
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

## Phase 6: Placement Planner To Mox-Native ExecutionTopologyPlan

Goal:

- turn Exo-style cluster heuristics into Mox-native placement output

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
4. Integrate Mox constraints into planning:
   backend readiness,
   artifact identity,
   provenance/license gating,
   memory admission,
   policy refusals,
   performance qualifiers.
5. Emit explicit refusal diagnostics when no valid placement exists.
6. Emit planner explanations and stable plan digests so receipts can explain the
   chosen topology.

Deliverables:

- planner implementation
- refusal diagnostics
- placement tests
- plan digest integration

Definition of done:

- cluster placement results are Mox-native, stable, and explainable

## Phase 7: Cluster-Aware Scheduling With Single-Node Execution First

Goal:

- ship useful cluster behavior before attempting distributed tensor transport

Steps:

1. Add remote execution selection so a request can be admitted on one node and
   executed on another Mox node.
2. Keep the execution path Mox-native on the selected node.
3. Surface in receipts:
   selected node,
   selected devices,
   execution topology,
   promised vs delivered execution location.
4. Add cluster-aware warm/load/unload and keepalive policy.
5. Add cluster admission control and queue policy.
6. Add cancellation, retry, and degraded/fallback semantics at cluster scope.

Deliverables:

- remote scheduling path
- cluster-aware lifecycle policy
- capability/receipt changes
- end-to-end tests

Definition of done:

- Mox can use a cluster to pick the best execution node without pretending it is
  already doing sharded execution

## Phase 8: Replicated Cluster Serving

Goal:

- make clustered serving operationally useful before true sharding

Steps:

1. Add replicated model residency across multiple nodes.
2. Add cluster-aware balancing and queue routing across replicas.
3. Add replica health and backpressure reporting.
4. Define how shared artifact identity and version drift are handled across
   replicas.
5. Ensure receipts distinguish:
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

- Mox can truthfully serve the same model from multiple nodes in one cluster

## Phase 9: True Sharded Cluster Execution

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
   widening claims.
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

Deliverables:

- backend-specific distributed execution substrate
- sharded execution path
- parity and behavior tests
- explicit refusal paths

Definition of done:

- Mox can truthfully execute at least one real sharded cluster path, or truthfully
  refuse unsupported ones

## Phase 10: Provider, Capability, Receipt, And Evidence Expansion

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
   per-node artifact identity,
   per-node load state,
   per-node degraded/refusal data,
   promised-vs-delivered topology comparison.
3. Reuse existing Mox execution-topology and delivery-proof structures where
   possible instead of inventing side channels.
4. Ensure cluster evidence is machine-checkable enough for compute-market use.

Deliverables:

- provider schema extensions
- provenance/receipt schema extensions
- compatibility tests

Definition of done:

- clustered execution truth is visible to downstream consumers without needing
  app-local reconstruction

## Phase 11: Optional Exo Interoperability Spike

Goal:

- test whether Exo is worth keeping as an optional orchestrator peer after the
  Rust-native substrate exists

This phase is optional and must not block the earlier phases.

Allowed scope:

- Exo may provide discovery, election, or placement input
- execution on each node must still be performed by Mox
- Mox capabilities and receipts must remain the source of truth

Spike options:

1. Exo orchestrates Mox worker nodes through an explicit adapter seam.
2. Mox ingests Exo-discovered topology or placement hints but keeps the final
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

## Phase 12: Security Hardening, Fault Injection, And Rollout

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
3. Add a hardware and network validation matrix for:
   single host,
   two-node LAN,
   multi-node LAN,
   RDMA-capable cluster if supported,
   refusal paths when transport/backend claims are not met.
4. Add operational runbooks:
   cluster bring-up,
   namespace configuration,
   membership debugging,
   catchup recovery,
   cluster placement debugging,
   degraded/refused execution diagnosis.
5. Add performance gates before widening cluster claims.

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
3. Phase 2: `mox-cluster` crate
4. Phase 3: transport and membership
5. Phase 4: ordered log and election
6. Phase 5: topology and telemetry
7. Phase 6: placement planner
8. Phase 7: cluster-aware single-node scheduling
9. Phase 8: replicated serving
10. Phase 10: provider/evidence expansion in parallel with phases 7-9
11. Phase 9: true sharded execution
12. Phase 12: hardening and rollout
13. Phase 11: optional Exo interoperability spike only when the Rust-native
    substrate is already credible

## What Not To Do

Do not:

- make Exo a required runtime dependency for `crates/mox/*`
- proxy Mox execution through Exo and call that integration complete
- widen cluster capability claims before the corresponding evidence surfaces
  exist
- claim sharded cluster execution when the system is only doing remote
  scheduling
- copy Exo's relaxed transport-security posture into a compute-market-facing
  Mox cluster
- leak Apple/MLX assumptions into Mox's backend truth
- add cluster complexity to app code when the substrate belongs in `crates/mox/*`

## Deliverables Checklist

Full integration of the Exo recommendations requires all of these deliverables:

- unified Exo docs set in `crates/mox/docs`
- Mox-owned Exo-derived parser and planner tests
- `mox-cluster` crate
- secure cluster transport and membership
- ordered event log and catchup
- leader ordering or equivalent ordering authority
- topology and telemetry facts
- Mox-native planner to `ExecutionTopologyPlan`
- remote scheduling path
- replicated serving path
- at least one truthful sharded execution path or explicit refusal coverage
- provider and receipt schema support for cluster truth
- validation matrix, runbooks, and security/fault testing
- explicit keep/discard decision on optional Exo interoperability

## Final Definition Of Done

This unified plan is complete only when:

- Exo's useful recommendations have been absorbed into Mox's docs, tests, and
  Rust-native cluster substrate
- Mox owns the shipped cluster execution truth end to end
- downstream OpenAgents systems can tell exactly what cluster topology was
  promised, selected, and delivered
- any Exo interoperability that remains is optional, bounded, and honestly
  documented
