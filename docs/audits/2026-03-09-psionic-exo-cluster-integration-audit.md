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
