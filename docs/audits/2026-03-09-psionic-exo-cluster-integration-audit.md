# 2026-03-09 Psionic Exo Cluster Integration Audit

> Historical note: this audit is a point-in-time snapshot from 2026-03-09.
> Current product and architecture authority lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and the active `crates/psionic/docs/*` plans.

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- GitHub issue `#3249` via `gh issue view 3249 --comments`
- `docs/audits/2026-03-08-psionic-vs-llama-cpp-gpt-oss-performance-audit.md`
- `docs/audits/2026-03-09-psionic-gpt-oss-metal-gap-audit.md`
- `crates/psionic/docs/deep-research-perf.md`
- `crates/psionic/docs/deep-research-exo.md`
- `crates/psionic/docs/EXO_INTEGRATION_PLAN.md`
- `crates/psionic/docs/EXO_UNIFIED_INTEGRATION_PLAN.md`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-provider/src/lib.rs`
- `https://github.com/exo-explore/exo`
- `https://raw.githubusercontent.com/exo-explore/exo/main/TODO.md`

## Executive Summary

Issue `#3249` and the follow-up benchmark comments show real progress, but they
also make the dependency order clear.

- Psionic's exact GPT-OSS HTTP benchmark moved from roughly `35.26 tok/s` to
  roughly `101.32 tok/s` on the local RTX 4080 path.
- The same control path in `llama.cpp` remained roughly `166-187 tok/s` on
  clean host runs.
- The remaining gap is still not "just cache reuse" or "just logits readback."
  The issue thread and perf audits keep converging on the same answer:
  graph shape, kernel fusion, dispatch policy, and steady-state decode runtime
  reuse still dominate.

That matters for Exo integration. The Exo research is useful, but not as a
shortcut around `#3249`. Exo contributes a cluster control-plane design:
discovery, ordered state, election, topology, placement, and catchup. It does
not remove the need to finish Psionic's single-node graph/runtime seam first.

The practical recommendation is:

1. finish the single-node GPT-OSS decode architecture that `#3249` is asking
   for, at least for one truthful clustered backend/product lane,
2. add a Rust-native `psionic-cluster` control-plane crate inside
   `crates/psionic/*`,
3. ship cluster-aware scheduling first,
4. then replicated serving,
5. then real sharded execution,
6. and keep Metal GPT-OSS and any internet-wide cluster claim out of the first
   scope.

## What The Recent Benchmarks Actually Say

The benchmark history tied to `#3249` matters because it tells us what cluster
work can and cannot solve.

| Checkpoint | Psionic | `llama.cpp` | What changed | What it means |
| --- | ---: | ---: | --- | --- |
| Issue baseline | `35.26 tok/s` | `167.27 tok/s` | Current hot path still encoded as Rust-owned decode sequencing | The gap is architectural, not cosmetic |
| Early direct ports | `35.99 tok/s` | `166.74 tok/s` | First direct CUDA fusion ideas, fewer launches | Correct but insufficient |
| Q8_1 fast path + graph replay + prompt-cache work | `68.43-68.65 tok/s` | `167.11-168.72 tok/s` | Big jump from better decode path and cache residency | Necessary but still not parity |
| Exact repeated-prompt cache fast paths | `89-92 tok/s` | `166-170 tok/s` | Less host prompt-cache overhead, fused greedy output | Request overhead fell; device decode still dominant |
| Latest fused attention-output checkpoint | `101.32 tok/s` | `187.13 tok/s` | Attention output writes directly into `Q8_1`; launch count dropped from `9102` to `8214` | Another real win, but step wall stayed about `295.5 ms` for `37` tokens, so the remaining gap is still in heavy decode dispatch |

The issue comments and perf audits also ruled out several plausible branches:

- `f16` mirrors feeding cuBLAS tensor-op GEMV on this path
- simpler per-expert MMVQ/atomic MoE routing in place of the current selected-4
  custom path
- a grouped-query attention specialization for the exact GPT-OSS geometry
- a fused selected-4 MoE-down quantize path
- wider selected-4 gate/down CUDA blocks

The durable conclusion is that Psionic is no longer blocked on obvious host-side
paper cuts. It is blocked on the exact work `#3249` names:

- mirror the GPT-OSS / OpenAI-MoE graph structure more literally,
- mirror `llama.cpp` fusion and dispatch decisions more literally,
- make graph reuse and capture a runtime contract,
- keep more of steady-state decode device-resident.

Cluster work does not invalidate that. If anything, it makes it more important:
distributing a still-suboptimal single-node decode path across multiple machines
would spread the inefficiency, not remove it.

## What Exo Actually Contributes

The Exo research is still valuable, but its value is concentrated in the
cluster control plane.

The useful Exo ideas are:

- automatic peer discovery and cluster isolation
- typed topic separation for commands, local events, global events, election,
  and connection facts
- a leader/master role that orders cluster-wide events
- an ordered event log with catchup and gap recovery
- topology-aware placement based on live node and link facts
- explicit separation between cluster control, worker supervision, and runner
  execution

Those ideas fit Psionic well because Psionic already values deterministic state,
replayable evidence, and truthful capability reporting.

The parts Exo does not solve for us are just as important:

- it does not close Psionic's current CUDA GPT-OSS graph/runtime gap
- its main execution stack is Python/MLX-shaped, not Rust/GGUF/CUDA-shaped
- its trust posture is acceptable for a local lab cluster, not for an
  adversarial compute market
- its mDNS-centric discovery is good for same-network clusters, not for the
  wider network by itself
- its current README still treats Linux as CPU-only, which is another reason to
  keep Psionic's first clustered execution claim narrow and backend-specific

Exo also exposes two practical control-plane hints that map well to a Psionic
cluster model:

- namespace-based cluster isolation
- coordinator-only operation via `--no-worker`

So the right reading is:

- adapt Exo's cluster semantics,
- do not make Exo the shipped execution dependency,
- do not route Psionic's inference truth through Exo runners,
- do not confuse cluster orchestration with kernel/runtime execution.

## Why Psionic Is Already A Good Host For The Cluster Idea

The Exo research is actionable because Psionic already has the core types needed
to absorb it cleanly.

### 1. Psionic already models execution topology explicitly

`psionic-runtime` already defines:

- `DeviceInventoryQualifiers`
- `ExecutionTopologyKind::{SingleDevice, Replicated, LayerSharded, TensorSharded}`
- `ExecutionTopologyPlan`
- `ExecutionShardAssignment`
- `ExecutionTopologyPlan::stable_digest()`

That is exactly the right output seam for cluster placement. Exo's placement can
be treated as input research, while Psionic keeps the exported truth in its own
runtime types.

### 2. Provider surfaces already carry topology truth

`psionic-provider` already serializes:

- `selected_device_inventory`
- `selected_devices`
- `execution_topology`

in both capability envelopes and receipts.

That means clustered execution does not need a parallel evidence model. It can
extend the existing one and stay compatible with the current "explicit and
truthful" posture in the MVP docs.

### 3. Ownership boundaries already point to the right home

`docs/OWNERSHIP.md` makes the layering requirement clear:

- app surfaces own product UX and orchestration
- reusable execution substrate belongs in crates
- reusable provider-domain semantics stay out of app code

So the cluster control plane should live in `crates/psionic/*`, not inside
`apps/autopilot-desktop`, not inside `crates/wgpui`, and not as hidden logic in
`psionic-serve`.

## Recommended Integration Shape

The cleanest path is to treat Exo as a design reference for a new
`psionic-cluster` crate.

`psionic-cluster` should own:

- cluster identity and admission policy
- transport and discovery for the first scope
- typed cluster commands and events
- ordered event-log replication and catchup
- cluster state snapshots and stable digests
- topology and node telemetry facts
- placement that emits Psionic-native `ExecutionTopologyPlan`

It should not own:

- product UI
- wallet or payout behavior
- ad hoc app orchestration
- the underlying CUDA or Metal inference kernels
- a required Python or MLX runtime dependency

That split matches both the Exo research and the current ownership contract.

## Trust Boundary

- First shipped scope is a trusted same-network cluster.
- This is not yet a compute-market trust model.
- Exo's current posture is much closer to "cooperating devices on one operator's
  network" than to an adversarial market-facing fabric.
- Required later hardening includes authenticated membership.
- Required later hardening includes signed control-plane messages.
- Required later hardening includes replay protection.
- Required later hardening includes tamper-evident catchup.
- Required later hardening includes explicit cluster admission policy.

This trust boundary should be stated directly in product and runtime language.
Otherwise the system will sound broader than the current security posture
actually supports.

## Cluster Identity And Node Roles

The audit should be more concrete here than "figure out identity later."

Recommended minimum model:

- `ClusterId`
- `NodeId`
- `NodeEpoch`
- `NodeRole::{CoordinatorOnly, ExecutorOnly, Mixed}`

That maps cleanly onto the Exo ideas already visible today:

- namespace-scoped cluster isolation
- coordinator-only nodes via `--no-worker`

`NodeEpoch` matters because clustered placement, warm-state reporting, and
receipt truth all become ambiguous if a rebooted or restarted node can be
mistaken for the prior instance.

## Artifact Residency And Staging

Placement and residency are separate decisions.

The cluster planner should be able to conclude "this node is topologically
correct" while the artifact layer concludes "this node is not yet ready to
execute." The runtime contract should make that visible instead of silently
turning staging into an implementation detail.

Recommended residency states:

- `resident`
- `copy_required`
- `pull_required`
- `refused`

This is one of the clearest places where Exo's TODO maps directly onto Psionic:
offline model copy, model streaming from peer devices, and better handling of
already-present local model folders are all artifact-staging concerns, not
placement concerns.

Psionic should keep these authoritative:

- artifact digest
- provenance
- license policy
- supply and refusal policy

Cluster receipts should say whether execution used a resident artifact or a
cluster-staged artifact and how that artifact became available.

## Transport And Link Policy

Exo is unusually explicit that topology quality matters. Its current README and
TODO call out topology-aware auto-parallel, RDMA over Thunderbolt, displaying
connection type, and preferring better links when they are available.

Psionic should not bury that inside opaque scheduler heuristics.

Cluster topology needs machine-checkable link facts such as:

- transport class
- latency posture
- bandwidth posture
- stability posture

Placement and receipts should also expose:

- why a link was chosen
- what failover rule applied
- whether execution degraded onto a lower-quality link

Transport preference should be runtime policy, not implicit folklore.

## Serving Policy And Fairness

The cluster document should say directly that orchestration quality is not just
about placement. It is also about queue discipline.

Exo's TODO already calls out a real batching failure mode:
new prefill can block decode for the current batch. Psionic should therefore
make serving policy first-class from the start.

Cluster scheduling work needs explicit rules for:

- prefill-vs-decode fairness
- queue discipline
- cancellation propagation
- slow-node backpressure
- degraded-replica routing

Without that, a cluster can look impressive on paper while still underperforming
badly under real mixed traffic.

## State Durability And Recovery

Ordered event history is a strong foundation, but it is not enough by itself.

If `psionic-cluster` adopts an Exo-like ordered state model, it should make
these rules explicit from day one:

- the ordered event log is the authoritative cluster-state history
- snapshots and compaction are explicit
- replay windows and replay bounds are explicit
- partition healing and leader failover behavior are explicit
- node rejoin semantics are explicit
- state versioning and migration are separate from model/runtime versioning

This is the difference between "event sourced" as an architectural slogan and
"event sourced" as an operationally supportable system.

## Cluster Evidence Additions

Clustered execution should extend the existing Psionic capability and receipt
surfaces rather than inventing a parallel truth system.

Recommended additions:

- `cluster_plan_digest`
- `selected_nodes`
- `selected_node_inventory`
- `shard_assignments`
- `artifact_residency`
- `transport_class`
- `degraded_or_fallback_history`
- `per_node_warm_state`
- `cluster_policy_digest`

This is the minimum level of specificity needed if clustered execution is later
used for provider receipts, payout arguments, or validation gates.

## Definition Of Done For First Cluster Scope

The first cluster scope should be deliberately narrower than Exo's full
marketing surface.

Definition of done for the first truthful Psionic cluster scope:

- trusted same-network cluster only
- explicit `ClusterId`, `NodeId`, role, and admission policy
- whole-request remote scheduling works and is reflected in receipts
- artifact residency and staging state are explicit
- cluster policy and degraded/fallback history are explicit
- replicated serving works for one validated backend/product lane
- unsupported backends are refused explicitly rather than silently included

Not in the first done definition:

- internet-wide cluster claims
- adversarial compute-market trust claims
- "all backends are cluster-capable" claims
- Metal GPT-OSS as an eligible clustered execution lane
- broad tensor-sharding claims before one validated homogeneous path exists

## Proposed GitHub Issue Breakdown

After consulting `crates/psionic/docs/ROADMAP.md`, the dependency order should
stay honest:

- the live Psionic execution queue is still the single-node throughput-parity
  track `#3249 -> #3247 -> #3248`
- real clustered execution should not jump ahead of that queue
- one exception is a control-plane-only "hello world" cluster connection issue,
  because it can establish the crate seam and local cluster identity model
  without pretending distributed execution is already ready

These are the suggested next issues to open on GitHub. They are written in the
same `PSI-*` style as the current roadmap, but they are suggestions in this
audit, not claims that the issues already exist.

| Order | Suggested ID | Suggested GitHub Issue Name | Description |
| --- | --- | --- | --- |
| 1 | `PSI-184` | `[psionic][cluster] Stand up a hello-world local cluster connection in psionic-cluster` | Create the initial `psionic-cluster` crate, add local cluster config, and prove that two Psionic nodes on the same trusted network can discover each other, exchange a typed hello/ping handshake, report `ClusterId` and `NodeId`, and surface explicit coordinator-versus-executor role truth without claiming any scheduling or execution behavior yet. |
| 2 | `PSI-185` | `[psionic][cluster] Define cluster identity, node epoch, and admission policy` | Make `ClusterId`, `NodeId`, `NodeEpoch`, and `NodeRole::{CoordinatorOnly, ExecutorOnly, Mixed}` first-class runtime types, add persistent local identity storage, add explicit namespace/admission configuration, and refuse cross-cluster joins or stale-node epochs instead of silently accepting ambiguous membership. |
| 3 | `PSI-186` | `[psionic][cluster] Add typed cluster commands, events, and authoritative ordered state` | Implement the control-plane vocabulary Exo makes useful: typed commands, local events, global ordered events, election messages, and connection facts, plus a Psionic-owned ordered event log and cluster-state model with stable digests so cluster truth is replayable and inspectable. |
| 4 | `PSI-187` | `[psionic][cluster] Add catchup, snapshots, compaction, and recovery semantics` | Extend the ordered state model with explicit catchup requests, replay bounds, snapshots, compaction, leader failover behavior, partition healing, node rejoin rules, and schema/version migration boundaries so the cluster state machine is operationally supportable rather than only conceptually event-sourced. |
| 5 | `PSI-188` | `[psionic][cluster] Publish topology, link-class, and node telemetry facts` | Make cluster topology explicit by publishing node resource facts, backend readiness, link class, latency, bandwidth, and stability posture into the replicated cluster state, and add stable topology digests so placement decisions can be justified from shared facts rather than hidden scheduler heuristics. |
| 6 | `PSI-189` | `[psionic][cluster] Add artifact residency and cluster staging truth` | Separate placement from artifact readiness by exposing `resident`, `copy_required`, `pull_required`, and `refused` artifact states, wire cluster-aware staging decisions to the existing Psionic artifact/provenance/license surfaces, and make peer-copy versus OCI-pull behavior explicit instead of burying it in runner logic. |
| 7 | `PSI-190` | `[psionic][cluster] Extend capability and receipt evidence for clustered execution` | Extend existing `psionic-provider` and `psionic-runtime` evidence seams with cluster-specific truth such as `cluster_plan_digest`, `selected_nodes`, `selected_node_inventory`, `shard_assignments`, `artifact_residency`, `transport_class`, `degraded_or_fallback_history`, `per_node_warm_state`, and `cluster_policy_digest`. |
| 8 | `PSI-191` | `[psionic][cluster] Add whole-request remote scheduling for one-node execution` | Use the new cluster state to choose the best remote node for an entire request, express the outcome as a truthful single-node `ExecutionTopologyPlan`, and surface explicit selection reasons, node/device identity, and refusal diagnostics without yet claiming replicated or sharded inference. |
| 9 | `PSI-192` | `[psionic][cluster] Add queue policy, fairness, cancellation, and backpressure rules` | Make cluster serving policy explicit by implementing queue discipline, prefill-versus-decode fairness, cancellation propagation, slow-node backpressure, and degraded replica routing so the cluster can serve mixed workloads without hidden starvation or scheduler-induced regressions. |
| 10 | `PSI-193` | `[psionic][cluster] Ship replicated cluster serving for one validated backend lane` | Turn `ExecutionTopologyKind::Replicated` into a real served behavior for one validated backend/product lane, add replica warm-state and load/unload policy, and make cluster routing across warm replicas explicit in receipts and capability reporting. |
| 11 | `PSI-194` | `[psionic][cluster][cuda] Add homogeneous CUDA layer-sharded execution` | After the single-node CUDA GPT-OSS parity stack closes, implement layer-sharded execution across homogeneous CUDA nodes, make activation and KV handoff explicit, and extend placement and receipts so layer-sharded cluster execution is truthful, measurable, and refusal-capable when topology or artifact readiness is insufficient. |
| 12 | `PSI-195` | `[psionic][cluster][cuda] Add homogeneous CUDA tensor-sharded execution and transport policy` | Add the first real tensor-sharded cluster path on homogeneous CUDA nodes, including explicit transport requirements, link-policy-aware placement, tensor-axis partition evidence, and stable refusal semantics when the required inter-node transport or model eligibility constraints are not satisfied. |
| 13 | `PSI-196` | `[psionic][cluster] Add cluster validation, fault-injection, and performance gates` | Add integration coverage and runbooks for hello-world connectivity, membership refusal, catchup, partition/rejoin, artifact staging, remote scheduling, replication, and sharding, plus benchmark gates so cluster claims remain evidence-backed instead of aspirational. |
| 14 | `PSI-197` | `[psionic][cluster][security] Harden cluster trust beyond the first LAN scope` | Add authenticated membership, signed control-plane messages, replay protection, multi-subnet/configured-peer posture, and stronger admission policy so the cluster substrate can widen beyond the initial trusted same-network scope without pretending Exo's current LAN-oriented trust assumptions already solve that problem. |

Recommended dependency notes:

- `PSI-184` can start before `#3249/#3247/#3248` finish because it is limited to
  hello-world cluster bring-up and must not claim real execution.
- `PSI-191` onward should depend on the current single-node CUDA parity track
  closing for the first validated execution lane.
- `PSI-194` and `PSI-195` should stay explicitly CUDA-first unless a later
  roadmap update makes another backend equally truthful.

## Path Forward

### Phase 0: Treat `#3249` as a hard gate for real clustered execution

Before "full Exo-style integration" means anything, Psionic needs one stable
single-node execution seam that cluster scheduling can trust. For now, that is
the CUDA GPT-OSS lane.

Required before serious sharded-cluster claims:

- graph-shaped GPT-OSS decode/prefill representation
- llama.cpp-like fusion and dispatch decisions for the real hot path
- stable decode-graph identity and reuse evidence
- device-resident decode ownership where possible

This does not mean cluster work must wait entirely. It means cluster phases
should begin with control-plane work and remote whole-node scheduling, not with
distributed tensor transport.

### Phase 1: Build a local-cluster control plane first

First shipping scope should be a trusted same-network cluster, not an
internet-wide compute-market cluster.

Implement:

- explicit cluster namespace and admission config
- persistent cluster identity and node identity
- transport/discovery suitable for same-network peers
- ordered global event stream with catchup and replay
- cluster topology and resource facts as replicated state

This is the point where Exo is most directly useful.

### Phase 2: Add cluster-aware scheduling before sharding

The first useful cluster behavior should be:

- discover several eligible Psionic nodes,
- decide which node should run the whole request,
- record why that node was selected,
- return receipts that expose the node and topology used.

This gives immediate value without pretending that distributed decode already
exists.

In Psionic terms, this means emitting:

- a single-node `ExecutionTopologyPlan` that names the chosen remote node/device,
- truthful selected-device and selected-node evidence,
- explicit refusal when a node is not eligible.

### Phase 3: Add replicated cluster serving

After single-node remote scheduling is stable, add
`ExecutionTopologyKind::Replicated` as a real runtime behavior:

- multiple nodes can host the same model
- placement can choose between warm replicas
- cluster queueing, cancellation, load/unload, and keepalive become explicit
- capability envelopes advertise replication truthfully

This is a much safer first scale-out story than cross-node tensor sharding.

### Phase 4: Add true sharded execution only on a homogeneous, validated lane

When the local execution seam is stable enough, start with one concrete sharded
target, likely CUDA GPT-OSS.

Recommended order:

1. layer sharding across homogeneous CUDA nodes
2. tensor sharding only after activation/KV transport is explicit and measured
3. any more exotic topology after those are stable

Metal GPT-OSS should stay out of this phase until the single-node Metal lane is
real and validated. The `2026-03-09` Metal audit already makes that point.

### Phase 5: Harden from local-cluster truth toward market-safe truth

Only after the local trusted-cluster path is real should we widen the trust
model.

That later hardening includes:

- stronger membership authentication
- signed cluster control-plane messages
- tamper-evident catchup and replay
- multi-subnet or configured-peer discovery
- performance gates and fault-injection coverage

## Open Questions

These are the design questions still worth answering before implementation
sprawl starts.

### 1. What is the first cluster scope?

Recommendation: explicitly scope the first shipped feature as a trusted local
cluster. Do not blur that into "provider cluster" or "compute-market cluster."

### 2. What transport owns cluster truth?

Recommendation: keep cluster control separate from Nostr/Nexus market transport.
Nostr is the market plane. Cluster control needs a low-latency, stateful,
catchup-capable control plane.

### 3. How should node identity relate to OpenAgents identity?

We need a clear answer for:

- cluster node identity
- device identity within a node
- cluster admission authority
- how that identity appears in provider receipts

### 4. What is the first distributed execution claim?

Recommendation: replicated serving before tensor sharding, unless the product
goal explicitly prioritizes "bigger-than-one-box" support over operational
simplicity.

### 5. How are artifacts staged and verified across nodes?

The existing Psionic loader, artifact, provenance, and license surfaces should
remain authoritative. Cluster work should reuse them, not invent a second
artifact pipeline.

### 6. What exact receipt fields are required for truthful cluster payouts?

If a future provider is paid for cluster execution, receipts likely need more
than just `execution_topology`. They may need:

- selected node set
- cluster plan digest
- artifact residency facts
- degraded/fallback history
- cluster policy/admission posture

### 7. Which backend is allowed to be first-class?

Recommendation: treat CUDA GPT-OSS as the likely first truthful clustered lane.
Refuse current Metal GPT-OSS placement instead of silently pretending it is
eligible.

## Recommended Near-Term Decisions

- Keep Exo as a secondary design reference, not a required runtime dependency.
- Treat `#3249` and the adjacent CUDA graph/runtime work as the execution gate
  for real cluster claims.
- Start cluster work with a new `psionic-cluster` crate in `crates/psionic/*`.
- Scope the first cluster feature to trusted same-network scheduling and
  replication.
- Reuse `ExecutionTopologyPlan` and existing provider receipt/capability
  surfaces as the contract for cluster truth.
- Defer Metal GPT-OSS cluster eligibility until the single-node Metal lane is
  real.

## Bottom Line

The recent perf work and the Exo research are aligned, not in conflict.

The perf work says Psionic still needs a better single-node decode architecture.
The Exo work says cluster value comes from a real control plane with ordered
state, topology, and placement. Put together, the path forward is straightforward:

- finish the single-node runtime seam,
- add a Rust-native cluster control plane,
- make scheduling truthful first,
- add replication next,
- and only then attempt true multi-node sharding.

That is the shortest path from the current Psionic setup to a full cluster
integration that stays honest about what is actually happening.
