# Exo Core Cluster Deep Research and a Rust-First Integration Path for Mox

## Summary

Exo’s “core cluster” idea is not just “run inference on multiple machines.” It is a cohesive distributed-systems design: automatic peer discovery, a topology- and resource-aware cluster state, a master-election mechanism for unstable networks, and an event-sourcing replication model where a single “writer” orders events and every node folds the ordered event stream into a convergent shared state. Exo’s README frames the user-facing goal as “connect all your devices into an AI cluster,” enabling models larger than a single device and scaling performance with more devices, including explicit attention to RDMA-over-Thunderbolt setups and topology-aware sharding. citeturn36view0turn36view1turn36view2

Mox already has a surprisingly compatible foundation for cluster integration: it explicitly models device inventory qualifiers, supports multi-device execution topology plans (single-device, replicated, layer-sharded, tensor-sharded), and uses stable digests for topology identity—concepts that map directly to “cluster placement” and “truthful capability/receipt reporting” needs. citeturn30view0turn32view0 The most leverage comes from porting Exo’s cluster-control substrate (network discovery + pubsub, master election, ordered event log + catchup, topology model + placement heuristics) into a new Rust crate layer within `crates/mox/*`, and then wiring that to existing Mox provider and serving surfaces rather than grafting Exo’s Python runner stack into OpenAgents.

Licensing aligns for code adaptation: Exo is Apache License 2.0, and OpenAgents is also Apache License 2.0. citeturn40view0turn43view0

## Exo core cluster architecture

Exo explicitly documents an **Event Sourcing + Erlang-style message passing** architecture, with components communicating via “topics.” To support this in Python, Exo mentions a channel library built on `anyio` channels with inspiration from `tokio::sync::mpsc`. citeturn37view1 This is the first key “core cluster” takeaway: the cluster is treated as a replicated state machine driven by ordered events, not a loose set of RPC calls.

Exo splits the system into five major systems:

- **Master**: executes placement and orders events through a single writer.
- **Worker**: schedules work on a node and gathers system information.
- **Runner**: executes inference in an isolated process for fault tolerance.
- **API**: exposes state and commands to clients.
- **Election**: master election for unstable network conditions. citeturn37view1turn37view2

Exo’s core cluster message space is structured into five topics:

- **Commands**: imperative requests when “the event log isn’t sufficient” (including placement and catchup requests).
- **Local Events**: all nodes publish; master consumes and orders.
- **Global Events**: master publishes ordered events; all nodes fold them into `State`.
- **Election Messages**: negotiation before cluster establishment.
- **Connection Messages**: discovery layer emits mDNS-discovered hardware connection events. citeturn37view1turn37view2

Exo’s design guidance is crisp: events are “past tense” facts; commands are imperative; events ideally should not trigger side effects directly. citeturn37view1 This is important to preserve when adapting into Mox because it cleanly separates: (a) truth about the world (events/state), from (b) actions the system might attempt (commands), which aligns with Mox’s “evidence and receipts” philosophy.

## Exo implementations that embody the core cluster idea

Exo’s “core cluster” is not only in docs; the code makes specific architectural commitments that are highly portable to a Rust Mox implementation.

Exo’s README emphasizes the cluster-level behaviors that the core infrastructure must deliver: automatic device discovery, topology-aware model splitting based on “realtime view of your device topology” and link latency/bandwidth, and multi-device tensor parallelism. citeturn36view1turn36view2

### Networking substrate and cluster isolation

Exo’s Rust `networking` crate (already Rust, not Python) uses **libp2p** with **gossipsub** and a discovery behavior built on **mDNS** (+ ping), producing “discovered/expired” events and message streams. citeturn25view0turn38view1turn38view0

Two details are especially relevant to Mox:

- **Cluster isolation by namespace/versioning**: Exo defines a `NETWORK_VERSION` and an override environment variable `EXO_LIBP2P_NAMESPACE`, explicitly calling out that this prevents devices on different versions/namespaces from interacting. citeturn38view0turn36view0  
- **Private network handshake (pnet)**: Exo derives a pre-shared key from the namespace/version and upgrades the transport with libp2p `pnet` so the swarm runs on a private network (reducing collisions with “public libp2p nodes” and other versions). citeturn38view0

The discovery behavior is also opinionated: it tracks “durable peer connections,” dials discovered-but-not-connected peers immediately, disconnects expired peers, and retries on a fixed interval. citeturn38view1 This is precisely the kind of “unreliable LAN reality” engineering that a Mox cluster feature will need (especially for home/office clusters with changing Wi‑Fi and sleep/wake patterns).

One caution: Exo’s transport layer includes a comment that “Noise is faster than TLS + we don’t care much for security.” citeturn38view0 That stance is incompatible with OpenAgents’ compute-market threat model; if ported, it should be treated as a performance-oriented baseline that must be hardened (see the risks section).

### Topic-based typed routing and publish policies

On the Python side, Exo defines typed topics and a router that serializes/deserializes payloads and interacts with the libp2p swarm through bindings. Topics include explicit publish policies such as “Never” for topics that should remain local-only (e.g., connection messages). citeturn15view0turn15view1

Exo’s router code also contains pragmatic delivery semantics for network reality: a “no peers found” error results in dropping the message; several gossipsub queue-full conditions drop messages (with different log levels); and oversized messages are dropped with an error. citeturn15view1 This discipline matters because it motivates why Exo leans on event-log catchup instead of assuming reliable pubsub delivery.

### Ordered event routing, acknowledgments, and catchup

Exo’s `EventRouter` design is one of the most reusable “core cluster” pieces for Mox:

- It maintains **ordered delivery** of global events (master-ordered) using an `OrderedBuffer`.
- It tracks events “out for delivery” and supports sending a **NACK / RequestEventLog** (explicitly a `RequestEventLog` command) when there is a gap in observed event indexes.
- It rate-limits NACKs using exponential backoff so a single missing event does not cause a broadcast storm. citeturn15view2turn37view1

This is a proven recipe for building a cluster state that converges even if the underlying transport drops messages or nodes temporarily disconnect.

### Event application and replicated state

Exo’s state replication is explicit and strict about event ordering. The `apply(state, event)` function asserts that event indices are contiguous, updating `last_event_applied_idx`, and the per-event handlers update maps of cluster facts (tasks, runners, instances, topology edges, node telemetry, etc.). citeturn33view0

One of the strong points here is that Exo treats topology and node telemetry as **first-class replicated facts**. For example, when a node times out, Exo removes the node from topology and cleans up all per-node mappings (memory, disk, system profile, network, thunderbolt, rdma control status), optionally recomputing thunderbolt bridge cycles. citeturn33view0 This is exactly the kind of “cluster truth hygiene” that prevents ghost devices and incorrect placement decisions.

### Master election for unstable conditions

Exo’s architecture doc calls out a dedicated election system “in unstable networking conditions.” citeturn37view1turn37view2 The election implementation is designed around exchanging election messages and negotiating a master before the cluster is fully established (and then being able to recover leadership when network conditions change). citeturn37view1

Even without line-by-line detail in this report, the architectural positioning matters for Mox: if Mox forms a local cluster, it will need a leader to serialize certain global decisions (placement decisions, ordered logs, cluster-wide admission policy), or it must adopt a different consensus approach.

### Topology-aware placement and “cycles” as a primitive

Exo’s core cluster concept goes beyond “list nodes”: it tries to find good ways to split a model based on topology and resources. The README explicitly states Exo’s sharding/parallelism decisions consider resources and network latency/bandwidth. citeturn36view2turn36view1

In code (master placement utilities), Exo uses the cluster topology graph to compute candidate subgraphs (“cycles”) and then selects placements based on constraints such as number of nodes and resource sufficiency (e.g., memory capacity), with different sharding modes implying different constraints. citeturn14view1turn14view2 (The placement logic is extensive; the key takeaway is that Exo elevates topology substructures to a placement primitive, rather than assuming a homogeneous fully connected set.)

## Mox primitives that make cluster integration feasible

Mox’s codebase already has the abstractions you want before trying to add clustering: explicit device identity, topology plans, and provider-visible truth surfaces.

### Mox’s crate decomposition and “engine-first” posture

The `crates/mox/` README describes Mox as the “native Rust compute engine for OpenAgents,” and lists sub-crates such as `mox-core`, `mox-ir`, `mox-compiler`, `mox-runtime`, `mox-models`, `mox-serve`, and backend crates. citeturn13view0 This separation matters: the cluster layer should live in `crates/mox/*` (engine substrate) and expose library-first APIs, rather than becoming entangled with desktop orchestration.

### Multi-device topology plans already exist in `mox-runtime`

`mox-runtime` defines reusable data structures that align closely with Exo’s placement outputs:

- `DeviceInventoryQualifiers` for stable device identity and topology keys. citeturn30view0  
- `ExecutionTopologyPlan` with explicit `ExecutionTopologyKind` modes: `SingleDevice`, `Replicated`, `LayerSharded`, `TensorSharded`. citeturn30view0  
- `ExecutionShardAssignment` tying a `shard_id` and logical partition (whole model, replica index, layer range, tensor range) to a stable `ExecutionDevicePlacement`. citeturn30view0  
- `ExecutionTopologyPlan::stable_digest()` for stable comparison and evidence. citeturn30view0  

This is a strong precondition for importing Exo’s “topology-aware auto parallel” idea into Mox: Exo produces cluster placements, and Mox has a structured way to represent them as a topology plan with stable identity.

### Provider capability envelopes already anticipate topology

`mox-provider` already wires topology and multi-device information into provider-visible capability envelopes. For example, `SandboxExecutionCapabilityEnvelope` includes:

- `selected_device_inventory` and `selected_devices`
- `execution_topology: Option<ExecutionTopologyPlan>`
- `backend_toolchain` identity and selection truth citeturn32view0turn30view0  

Even if you are not immediately doing distributed inference, adding “cluster-aware scheduling” can still benefit by truthfully reporting what device inventory and topology were used (a home cluster, a single host, etc.). If Exo-style placement is integrated, Mox can report it through these existing surfaces rather than inventing a parallel reporting channel.

## Mapping Exo’s core cluster concepts onto Mox

The key to integrating Exo’s approach into Mox is recognizing that Exo’s “core cluster” is primarily a **cluster control plane**. The compute plane (actual inference kernels) can remain Mox-native and incrementally gain distributed execution later.

### Conceptual mapping

Exo uses a leader (master) to order events and centralize certain global decisions, while every node maintains a convergent “State” by folding ordered events. citeturn37view1turn33view0turn15view2 Mox already relies on stable digests, explicit identities, and policy-driven receipts—so the natural mapping is:

- **Exo global event stream** → **Mox cluster control log** that records cluster membership, device inventories, model residency decisions, and placement plans in a replayable, indexed sequence (akin to Exo’s `last_event_applied_idx` discipline). citeturn33view0turn15view2  
- **Exo topology-aware placement** → **Mox `ExecutionTopologyPlan` synthesis**, where Exo placement decisions become Mox topology plans, using `LayerSharded` and `TensorSharded` partitions and stable device placements. citeturn30view0turn36view1turn14view1  
- **Exo discovery + connection messages** → **Mox cluster membership and link facts**, initially to define cluster membership and later to incorporate link-quality for placement, echoing Exo’s “realtime view of topology” requirement. citeturn38view1turn37view1turn36view2  

### What is most reusable to port into Rust

If the goal is “integrate the core cluster idea,” the highest-value parts to adapt are:

- Exo’s **libp2p + gossipsub networking crate**, including namespace isolation and private network configuration. citeturn38view0turn38view1turn36view0  
- Exo’s **event-router catchup design** (ordered buffer + NACK `RequestEventLog` with backoff), because it prevents subtle “split-brain state” in a lossy pubsub environment. citeturn15view2turn37view1  
- Exo’s **strict event-application discipline** (`last_event_applied_idx` contiguous index assumption), which makes replay, debugging, and correctness much easier. citeturn33view0  
- Exo’s **master election positioning** for unstable networking conditions, because some leader-based behavior is still the simplest way to produce a single ordered cluster control log. citeturn37view1turn37view2  
- Exo’s **topology-aware placement heuristics**, but mapped into Mox’s topology plan vocabulary and integrated with Mox’s existing memory planning/residency and backend capability truth (rather than Exo’s runner semantics). citeturn36view2turn30view0  

## Recommended Mox integration architecture

This section proposes a concrete way to integrate Exo’s core cluster idea into Mox while respecting Mox’s crate boundaries and staying Rust-first.

### Add a `mox-cluster` control-plane crate in `crates/mox/`

`mox-cluster` would implement:

- **Transport + discovery**: reuse/adapt Exo’s Rust `networking` crate patterns (libp2p swarm with discovery + gossipsub, namespace isolation, private network handshake). citeturn38view0turn38view1  
- **Cluster topics**: mirror Exo’s topic separation (commands/local events/global events/election/connection messages), but with Mox-controlled schemas and `serde` encoding. Exo’s architecture demonstrates why this partitioning is useful: commands for imperative actions; local events for facts produced by all nodes; global events for the ordered cluster control log; election messages; connection messages. citeturn37view1turn15view0  
- **Ordered event log + catchup**: port Exo’s `EventRouter` approach—ordered buffers plus NACK/catchup requests—to avoid “assuming reliable pubsub.” citeturn15view2turn15view1  
- **Cluster state machine**: a Mox-native `ClusterState` with an `apply(indexed_event)` discipline similar to Exo’s contiguous index enforcement. citeturn33view0  

In Rust terms, you want a replayable, append-only log of typed `ClusterEvent`s that can be serialized into provider receipts and used for deterministic auditing.

### Define cluster identity in a way that composes with Mox’s device identity

Mox already has `DeviceInventoryQualifiers` and stable device IDs. citeturn30view0 Exo has a `NodeId` concept and per-node telemetry. citeturn33view0turn26view0 The simplest Mox approach is to define:

- A **cluster node identity** (host-level identity; ideally tied to OpenAgents identity systems rather than a random ephemeral ID).
- A **device placement identity** that extends `DeviceInventoryQualifiers` with a host/node identity prefix.

This lets `ExecutionTopologyPlan::stable_digest()` remain meaningful in a cluster context, because device placements include stable identifiers. citeturn30view0

### Make placement produce Mox-native `ExecutionTopologyPlan`

Exo’s README promise is “topology-aware auto parallel,” choosing how to split models based on link characteristics and resources. citeturn36view2turn36view1 Mox already has the output type that should result from placement: `ExecutionTopologyPlan` with explicit `LayerSharded` and `TensorSharded` partitions. citeturn30view0

So the ported placement layer should produce:

- `ExecutionTopologyPlan::single_device(...)` when one node/device suffices.
- `ExecutionTopologyPlan::layer_sharded(...)` when splitting by layer ranges.
- `ExecutionTopologyPlan::tensor_sharded(...)` when splitting by tensor-axis ranges (Exo’s “tensor parallelism” maps naturally here). citeturn30view0turn36view1

If Exo’s placement uses topology substructures like “cycles,” treat those as intermediate solver artifacts; the exported interface should remain Mox’s stable topology plan and digest.

### Wire cluster topology and selection truth through existing provider envelopes

Because `mox-provider` already has optional `execution_topology` in capability envelopes and includes selected devices, you can surface cluster-aware execution truth without changing downstream consumers immediately. citeturn32view0turn30view0

A cluster-aware Mox execution path should populate:

- `selected_devices` as the participating devices (across nodes).
- `execution_topology` with the plan and digest.
- delivery proof/evidence fields per request, potentially aggregated from per-node execution proofs (depending on whether work is sharded or “scheduled on one node”). citeturn32view0turn30view0

### Keep compute execution Mox-native and phase distributed execution carefully

Exo’s cluster includes a Runner system that executes inference in an isolated process. citeturn37view1turn37view2 Mox is explicitly an in-repo Rust engine; you should not import Exo’s runner design as-is. Instead:

- Phase one: **cluster-aware scheduling, not distributed inference**. Pick the best node/device to run the model (based on memory, backend readiness, etc.), and run inference via Mox locally on that chosen node.
- Phase two: **replicated execution** (data parallel) where multiple nodes can serve the same model (use `ExecutionTopologyKind::Replicated`). citeturn30view0turn36view1  
- Phase three: **true sharded execution** (layer/tensor sharding across nodes), which requires a dedicated transport for activations/KV and careful backend integration. Exo’s README highlights “MLX distributed” for distributed communication; Mox will need an equivalent (likely backend-specific) communication substrate if it aims to do cross-host tensor parallel. citeturn36view1

## Implementation sequence and verification strategy

A staged plan keeps the integration honest and testable, and ensures Mox remains truthful as a compute-market substrate.

### Establish cluster formation and membership truth

Implement Rust-side discovery and pubsub using Exo’s approach: durable discovery behavior (mDNS + retry dial/disconnect), plus private network scoping via namespace/version. citeturn38view1turn38view0turn36view0

Verification should include automated tests that simulate:

- node join/leave
- intermittent dropped messages (gossipsub queue-full equivalents)
- convergence of `ClusterState` via ordered events and catchup requests (NACK → event log replay), matching Exo’s explicit need for catchup in lossy conditions. citeturn15view2turn15view1

### Implement ordered state replication and leader election

Port two Exo guarantees:

- a single ordered global event stream (via leader/master ordering) citeturn37view1  
- strict application ordering (`last_event_applied_idx` contiguous progression) citeturn33view0  

Implementing this in Rust enables deterministic replay and audit. Even if the leader election algorithm differs from Exo’s implementation details, preserve Exo’s **architectural contract**: a leader exists to order events, and nodes can recover leadership under unstable network conditions. citeturn37view2turn37view1

### Add topology and resource telemetry as events

Exo treats node telemetry (memory, disk, network interfaces, RDMA/thunderbolt status) and topology edge changes as replicated facts. citeturn33view0turn37view1 Mox should follow the same model:

- publish periodic node resource events (device inventories, backend readiness, free memory)
- publish topology link events (initially “connected/disconnected,” later latency/bandwidth if measured)

This creates the substrate for truthful placement and future compute-market capabilities.

### Port placement to output `ExecutionTopologyPlan`

Port Exo’s topology-aware placement, but make the output strictly a Mox `ExecutionTopologyPlan` plus Mox-visible “why” diagnostics:

- “selected devices” and “topology kind”
- stable digest of the plan citeturn30view0  
- explicit refusal diagnostics when no placement satisfies constraints

This keeps placement decisions visible and auditable, and it plugs into existing `mox-provider` capability and receipt surfaces that already carry `execution_topology`. citeturn32view0turn30view0

## Risks, constraints, and suggested deviations from Exo

### Security posture needs tightening

Exo’s networking transport comments explicitly deprioritize security (“we don’t care much for security”) while selecting Noise because it is faster than TLS. citeturn38view0 For a compute-market substrate, this is a material mismatch. At minimum, a Mox port should:

- authenticate cluster membership (shared secret is not enough if nodes can be compromised)
- prevent unauthorized peers from injecting commands/events
- protect event logs and catchup responses from tampering

Exo’s private network `pnet` and signed gossipsub messages are useful primitives, but they are not equivalent to a full threat-model-driven security design. citeturn38view0

### mDNS discovery is LAN-scoped

Exo’s discovery layer is designed around mDNS (with tuning for TTL and query intervals). citeturn38view1 That is ideal for “my devices on the same network,” but it will not discover nodes across NATs/subnets without additional routing/relay mechanisms. If Mox clustering is meant to be “local cluster” only, mDNS is fine. If it’s meant to support distributed compute-market clusters, you will likely need a second discovery backend and explicit configuration.

### Event log growth and compaction

Exo’s approach relies on catchup and replay; Mox will need explicit log retention and snapshotting/compaction policies to prevent unbounded growth, while preserving audit needs. The discipline in Exo’s `apply` (contiguous ordering, state derived from events) makes snapshotting feasible. citeturn33view0turn15view2

### Distributed inference is the hardest step and should be gated

Exo’s core cluster concept markets topology-aware sharding and tensor parallelism. citeturn36view1turn36view2 In Mox, the safest path is:

- ship a cluster control plane first (membership, telemetry, placement, scheduling)
- add sharded inference only when Mox has a backend-specific distributed communication substrate (comparable in role to Exo’s reliance on “MLX distributed”) citeturn36view1  
- ensure provider receipts and execution evidence remain truthful about what topology and devices truly executed the job, leveraging existing `ExecutionTopologyPlan` and evidence surfaces citeturn30view0turn32view0  

This avoids the failure mode where the system “claims cluster inference” but silently falls back to a single node or an unverified execution mode—exactly the kind of “truth drift” Mox’s current architecture is designed to prevent. citeturn30view0turn32view0