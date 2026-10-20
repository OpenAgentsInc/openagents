# What Psionic should learn from Mesh-LLM

Reviewed on 2026-10-20. Mesh-LLM source revision:
[`d059e25055d42df3b319a3dd462aaed6bdd95cc9`](https://github.com/Mesh-LLM/mesh-llm/tree/d059e25055d42df3b319a3dd462aaed6bdd95cc9).
Psionic comparison revision: `83fe8b7f6b3cd437ae9b7af213cf9f04875dd793`
in this monorepo. This is a source review, not a performance comparison.

## Recommendation

Study Mesh-LLM primarily for distributed deployment and model packaging.
Its most useful lessons complement the [DeepInfer review](deepinfer-lessons.md):
DeepInfer informs efficient device execution; Mesh-LLM informs how to place,
load, qualify, and operate model stages across machines.

Keep Psionic's native Rust engine, cluster state, and artifact contracts.
Extend them with stage-specific packages, workload-aware placement, a complete
activation boundary contract, and explicit serving readiness. Mesh-LLM's Rust
host does not mean its numerical engine is Rust: Skippy relies on a patched
llama.cpp runtime. Adopting that runtime would be a separate engine decision,
not a prerequisite for applying these lessons. [M1] [M2] [M10]

| Order | Work | Reason |
| --- | --- | --- |
| 1 | Qualify a two-stage execution path against full-model execution | A placement plan is not evidence that the chosen model boundary computes correctly. |
| 2 | Connect shard manifests to minimal stage materialization | Workers should fetch and load only their assigned tensors and required shared assets. |
| 3 | Make stage loading and route publication one controlled lifecycle | Partially ready or superseded stage chains must not receive new requests. |
| 4 | Replace equal layer counts with explicit latency and throughput objectives | Memory fit, single-request speed, and saturated throughput prefer different placements. |
| 5 | Add measured cache-aware routing and qualified wire compression | These improve an established execution path; neither establishes its correctness. |

## Review scope and implementation boundaries

The clone contains two product trees: Mesh owns networking, routing, and the
host; Skippy owns model execution, packages, and serving. This review inspected
the production split planner adapter, coordinator, stage loading, package
admission, activation protocol, cache routing, runtime manifests, and
correctness tooling. No dependencies, native runtime, or model weights were
installed. No node joined a public mesh, and no inference benchmark ran.

Several boundaries matter when interpreting the source:

- **Routing and splitting are different operations.** Sending a complete
  request to a peer that holds a model differs from forwarding intermediate
  activations through several stages. Mesh supports both. [M1]
- **There are two planning components.** `skippy-topology` provides pure
  boundary and family-policy planning. Mesh production placement uses the
  resource planner in `skippy-coordinator`, called by the host's
  `runtime/split_planning.rs`. The package/name-based adapter in
  `inference/skippy/topology.rs` is test-only. Do not assume every capability
  of the standalone planner runs in the production placement path. [M7] [M9] [M22] [M29]
- **Gossip is not the entire coordination model.** Mesh discovery can be
  decentralized while a particular split has a coordinator, claims, and
  readiness requirements. Those serving decisions need stronger rules than
  an advertisement that a peer exists. [M5] [M6]
- **A support roster is not a benchmark result.** Architecture admission is
  tied to a native build recipe; it does not prove every quantization,
  placement, context length, or heterogeneous device combination. [M19]

## Psionic already has the right foundations

| Area | Inspected Psionic implementation | Implication |
| --- | --- | --- |
| Whole-request placement | `schedule_remote_whole_request` checks ready membership, capability eligibility, artifact requirements, and candidate refusals. [P1] | Retain these checks when comparing local execution, remote execution, and splitting. |
| Pipeline placement | `schedule_pipeline_sharded_execution` builds ordered stage and handoff records, checks links, and can validate a sharded manifest. It accepts CUDA in this path. Its `split_layers` helper divides layers evenly. [P2] | Improve this planner instead of creating a competing placement subsystem. Do not infer mixed Metal/CUDA execution from the presence of both backends elsewhere. |
| Communication policy | Pipeline placement has bandwidth and handoff-latency limits; tensor placement requires an RDMA-class full mesh under its default CUDA policy. [P2] [P3] | Keep pipeline traffic and collective-heavy tensor parallelism under different link requirements. |
| Artifacts | `ShardedModelManifest` binds shard identities and partitions to a served artifact and validates against a topology. `DatastreamManifest` describes digested objects and chunks. [P4] [P5] | Build stage packaging and delivery on these contracts. Their existence alone does not prove minimal stage fetching or loading. |
| Transport and recovery | `psionic-net` records direct, NAT-traversed, and relayed paths. Cluster state records link health and supports ordered catchup/snapshot recovery. [P6] [P7] | Measure the selected path and keep authoritative cluster recovery; do not replace it with gossip-only serving decisions. |
| Prefix compatibility | Shared prefix matching binds model, weights, tokenizer, template, backend, and tenant identity. [P8] | Cache routing hints must preserve those compatibility and isolation checks. |

These are source-level findings about the inspected paths. This review does
not establish that Psionic has completed multi-host execution for every
represented plan, or that Mesh-LLM's advertised platform matrix transfers to it.

## 1. Package what a stage owns, not the whole model

Mesh's package format resolves a stage admission descriptor containing a
package identity, exact resident tensor IDs, and sidecar references. Resolution
validates the package, requires sorted and valid references, identifies the
metadata and tensor artifacts needed, and counts distinct stored tensor bytes.
This makes stage ownership an artifact-level decision rather than an informal
layer-range label. [M3]

For Psionic, extend `ShardedModelManifest` and delivery planning so each stage
can resolve its exact weights, tokenizer/template requirements, projector
assets if needed, and any shared tensors. Keep semantic model identity separate
from a derived stage file's identity. Validate contiguous coverage, missing
assets, duplicate ownership, and shared/tied weights before loading.

Use datastream digests and chunk validation for transfer. Avoid making workers
download the full GGUF and discard most tensors. Measure downloaded bytes,
materialized bytes, host RAM, device memory, and time to readiness separately.
Package bytes and available VRAM do not account for every runtime allocation.

Mesh's peer artifact transfer is a useful reference for bounded trust:
the inspected policy disables it by default for public meshes, restricts its
source identity, and verifies manifest and artifact SHA-256 values. Reuse the
principles through OpenAgents' existing authorization and artifact APIs;
do not copy its environment flags or introduce a second discovery service.
[M21]

## 2. Define the complete computation crossing a stage boundary

A stage boundary may need more than a hidden-state vector. Residual paths,
token-dependent inputs, shared KV relationships, and recurrent state can make
an arbitrary layer cut invalid or require additional tensors.

Skippy's native ABI patch describes graph-derived stage boundaries, and its
activation protocol carries a directory of typed parts. Each part has an
identity, element type, dimensions, strides, token axis, and payload range;
the frame identifies the frontier and producing layer range. The decoder
checks sizes and descriptor consistency rather than treating the message as
an untyped byte array. [M10] [M11] [M13]

Psionic should derive a boundary contract from the actual model execution
graph or model implementation. Bind it to the served artifact, stage range,
backend execution format, and protocol version. Reject a consumer whose inputs
do not match. Keep recurrent/KV ownership explicit: moving a future activation
to another machine does not also move a live sequence's state.

The standalone `skippy-topology` family-policy code provides useful examples
of shared-KV constraints and required sidebands. Treat those as examples to
reimplement for Psionic's models, not production capabilities inferred from
model names. Validate legal boundaries before considering their performance.
[M22] [M26]

Start with a supported dense model and raw, precisely described activations.
Compare full-model and two-stage logits on identical input tokens, then test
multi-step generation and cached continuation. Add recurrent, shared-KV, MoE,
and multimodal cases only after their additional state is accounted for.

## 3. Prepare stages before publishing the route

The inspected Mesh loader starts downstream stages in reverse order. It can
check exact local-source inventory, sends a load request, and waits for stage
startup responses. The replacement coordinator loads a candidate before
sending a replacement event; on acceptance, it updates the active generation
and stops the previous one. [M4] [M6]

Apply that lifecycle to Psionic's existing stage plans:

1. Resolve and verify artifacts and runtime compatibility.
2. Reserve memory and create a specific topology generation.
3. Load downstream stages and verify their boundary contracts.
4. Confirm every required stage is ready before advertising the entry route.
5. On failure, withdraw availability and release partial allocations.

Keep coordinator ownership separate from serving readiness. Mesh requires a
majority of planned stages to accept its coordinator claim; that quorum is
not permission to execute with a required stage missing. Its fence binds the
claim to model/package identity, coordinator, term, topology, participant hash,
and lease. [M5]

Preserve Psionic's ordered cluster authority when adding equivalent stale-work
protection. Test delayed loads, duplicate commands, reordered stops, coordinator
loss, and a recovered peer receiving an old topology. Do not transplant Mesh's
four-hour lease or wall-clock assumptions without evaluating expiry, clock
skew, restart persistence, and competing participant sets. Its host also has an
explicit term-zero/no-coordinator bypass for legacy/local loads; that should
not silently become a remote serving fallback. [M5] [M27] [M28]

Preparing a replacement before route publication is valuable, but it does not
prove uninterrupted live-sequence migration. Define whether existing requests
drain, fail, or restore state when ownership changes, and measure the temporary
memory needed to keep old and new stages present.

## 4. Optimize for the workload, not an even layer count

Mesh's coordinator takes per-layer weight sizes, KV cost, recurrent state,
context, lanes, memory headroom, and optional transfer and decode-speed inputs.
Its performance solver distinguishes two objectives over a fixed node order:

- **Single-request latency:** minimize the sum of stage decode times.
- **Concurrent throughput:** minimize the slowest stage's decode time.

The solver uses a weight-bytes/decode-rate estimate and memory constraints;
these are planning estimates, not measurements of every request. The optional
balancing mode also needs speed inputs for all placed nodes. [M7] [M8]

Psionic's even layer split is a useful baseline, but layers can differ in cost,
and devices differ in speed and memory. Add per-layer resource estimates and
backend-qualified measured costs. Prefer whole-model local or remote execution
when it meets the request's requirements; use splitting to gain otherwise
unavailable capacity or a demonstrated workload benefit.

For a single autoregressive sequence, reason about:

`time per output token = sum(stage compute) + serialized transfers + return/sampling work`

For concurrent requests, the slowest stage can limit throughput, but actual
batching, queueing, shared links, and memory can change the bottleneck. Keep
prefill and decode objectives separate. Predicting decode weight traffic does
not fully predict long-context prefill.

Charge actual activation and sideband bytes, host/device copies, and prediction
return traffic to the network model. Mesh's own comparison document uses
wording suggesting zero inter-device communication during decode; the staged
protocol and loader show that activations still cross machines. The benefit
is avoiding per-layer collectives, not eliminating stage traffic. [M23] [M4] [M11]

Retain Psionic's link eligibility checks while adding an end-to-end latency
budget. A connected relay path can be correct and too slow for the requested
interaction. Record observed direct/relay status and transfer performance;
configured bandwidth and a successful handshake are insufficient evidence.

## 5. Route by saved work with bounded cache evidence

Mesh's routing primitive compares a candidate's queue delay, restore time,
and suffix-prefill cost with estimated cold prefill. A large prefix match
does not automatically win. Inventory is bounded and short-lived, advertises
positive hits, and uses rotating public salts over prefix digests. Its own
trust documentation explicitly treats peer evidence as a hint, not proof of
cache residency or a private secret. [M14] [M15] [M16]

This is a useful addition to Psionic's existing placement and prefix-cache
contracts. Compare estimated completion cost only among authorized, compatible,
ready candidates. Scope evidence to artifact, tokenizer/template, backend,
tenant, and state format. Reject stale evidence, fall back when a claimed hit
is gone, and record whether the advertised saving occurred.

Do not equate an advertisement with pinned KV. Do not interpret a rotating
digest as confidentiality against a serving peer. The worker performing
inference processes request data; encrypted transport protects the link,
not the data from that worker. Keep public and owner-authorized placement
choices under explicit policy, and avoid sending raw prompt tokens in cache
advertisements.

## 6. Negotiate compression without weakening the output contract

The reviewed code is more specific than the split guide's broad raw-f32
description. `Fixed` uses its configured codec. `AutoLosslessV1` requires a
raw-f32 fallback and admits raw-f32, BF16, or F16; the producer's selection
checks complete round-trip equality of the realized frame. S8 helpers exist,
but that automatic policy excludes S8. Helper availability is not runtime
qualification. [M12] [M13] [M25]

For Psionic, first establish a raw transport baseline. Then evaluate exact
compression with codec negotiation, decoded-size limits, and fallback. Include
selection, allocation, encoding, decoding, and copy time in the benchmark;
saving bytes alone may lose on a fast LAN.

Treat lossy activation quantization as a separate mode with explicit quality
evaluation and cache identity. Exact wire reconstruction also does not imply
identical numerical execution across CUDA and Metal kernels. Qualify those
backend combinations independently.

## 7. Bind qualification to the artifact and runtime that ran

Mesh's split admission roster validates its llama.cpp pin, patch-queue digest,
and ABI before admitting an architecture. Native runtime manifests separately
record checksums and compatibility metadata; the resolver checks the expected
ABI. Its correctness runner compares staged execution with full-model
execution and includes state-handoff tests. [M17] [M18] [M19] [M20]

For Psionic, retain a compact qualification record for each supported
artifact/backend/stage-boundary combination. Include weights, tokenizer,
runtime build, boundary schema, cache format, and wire policy. Avoid marking
an entire model family supported because one representative generated text.
Reuse existing fixtures and backend tests; a standalone qualification tool is
useful only if it exercises the same loader and execution path that serves
requests.

Keep Rust libraries usable without implicitly downloading a model or building
every accelerator backend. Mesh's separation of host and versioned native
runtime artifacts is useful packaging guidance; Psionic need not introduce a
dynamic C ABI merely to reproduce that distribution boundary.

## Smallest useful experiments

| Experiment | Comparison | Required evidence |
| --- | --- | --- |
| Two-stage correctness | Full model versus two stages on identical tokens | Logits, greedy output, multi-step continuation, cache restore, and boundary rejection tests. |
| Minimal stage package | Full GGUF handling versus assigned-tensor materialization | Verified identity, fetched bytes, host/device peak memory, startup time, and missing-sidecar behavior. |
| Placement objective | Equal layers versus latency and throughput policies on unequal devices | Per-stage compute and transfer time, queue-inclusive p50/p95/p99 latency, committed tokens/s, and measured versus estimated cost. |
| Lifecycle failure | Healthy chain versus stage loss and delayed old commands | Route withdrawal, cancellation completion, no stale stage takeover, partial-load cleanup, and declared request recovery behavior. |
| Cache and wire optimization | Cold/warm routing and raw/exact-compressed activations | Realized cache savings, bounded memory, complete transport cost, numerical parity, and fallback behavior. |

Use an isolated private setup, pinned artifacts, and a small supported model
first. Compare one local node, whole-request remote execution, and a two-stage
chain before increasing node count. Separate capacity gains from speed gains.
Do not use Psionic's represented plan shapes or Mesh's published support matrix
as substitutes for completed runs on the selected devices.

## Sources and attribution

Mesh-LLM links are pinned to the reviewed commit. Psionic links refer to the
monorepo source at the comparison revision above. The root Mesh-LLM license is
Apache-2.0, while workspace package metadata declares `MIT OR Apache-2.0`;
check the applicable files and dependencies before any source adaptation.
This review imports no product code. [M2] [M24]

[M1]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/README.md
[M2]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/Cargo.toml
[M3]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-package-format/src/stage_admission.rs
[M4]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/runtime/local_split/loading.rs
[M5]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-coordinator/src/lib.rs
[M6]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/runtime/local_split/coordinator.rs
[M7]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-coordinator/src/topology.rs
[M8]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-coordinator/src/topology/performance.rs
[M9]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/runtime/split_planning.rs
[M10]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/llama_cpp/patches/0001-skippy-define-graph-derived-stage-ABI.patch
[M11]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-protocol/src/binary/types.rs
[M12]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-protocol/src/config.rs
[M13]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-protocol/src/binary/activation.rs
[M14]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-routing/src/cache_aware.rs
[M15]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-routing/src/cache_inventory.rs
[M16]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-routing/README.md
[M17]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-native-runtime/src/manifest.rs
[M18]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-native-runtime/src/resolver.rs
[M19]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-api/src/split_certification.rs
[M20]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-correctness/README.md
[M21]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/models/artifact_transfer.rs
[M22]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-topology/README.md
[M23]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/docs/EXO_COMPARISON.md
[M24]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/LICENSE
[M25]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/docs/SKIPPY_SPLITS.md
[M26]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/skippy/crates/skippy-topology/src/family_capability.rs
[M27]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/inference/skippy/stage/mod.rs
[M28]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/runtime/local_split.rs
[M29]: https://github.com/Mesh-LLM/mesh-llm/blob/d059e25055d42df3b319a3dd462aaed6bdd95cc9/mesh/crates/mesh-llm-host-runtime/src/inference/skippy/topology.rs
[P1]: ../../crates/psionic/crates/psionic-cluster/src/scheduler.rs
[P2]: ../../crates/psionic/crates/psionic-cluster/src/pipeline_sharded.rs
[P3]: ../../crates/psionic/crates/psionic-cluster/src/tensor_sharded.rs
[P4]: ../../crates/psionic/crates/psionic-runtime/src/lib.rs
[P5]: ../../crates/psionic/crates/psionic-datastream/src/lib.rs
[P6]: ../../crates/psionic/crates/psionic-net/src/lib.rs
[P7]: ../../crates/psionic/crates/psionic-cluster/src/ordered_state.rs
[P8]: ../../crates/psionic/crates/psionic-serve/src/lib.rs
