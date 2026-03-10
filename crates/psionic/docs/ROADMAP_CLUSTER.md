# Psionic Cluster Roadmap

> Status: updated 2026-03-10 after reading `ROADMAP.md`,
> `ROADMAP_METAL.md`, `EXO_UNIFIED_INTEGRATION_PLAN.md`, and
> `../../../docs/audits/2026-03-09-psionic-exo-cluster-integration-audit.md`,
> after confirming that the cluster-adjacent substrate `PSI-148`,
> `PSI-160` through `PSI-175`, and `PSI-179` through `PSI-183` is already
> landed on `main`, after confirming that the active NVIDIA local-runtime gate
> remains `#3276` -> `#3288` -> `#3248`, after confirming that the active
> native Metal GPT-OSS gate remains `#3286` -> `#3285` -> `#3269` -> `#3262`,
> after landing `PSI-184` / `#3289` in `64c2a8fc6` and `PSI-185` / `#3290` in
> `f2e758720`, after landing `PSI-186` / `#3291` in `cc60eea89`, after
> opening `PSI-188` through `PSI-197` as `#3297` through `#3306`, after
> landing `PSI-187` / `#3292` through `PSI-190` / `#3299` in `2acc2ecf6`,
> after confirming that `#3300` is now the next open cluster queue item, and
> after checking live GitHub issue search so this roadmap reflects the current
> GitHub queue rather than local placeholders.
>
> This is the live roadmap for truthful Psionic cluster support in
> `crates/psionic/*`. It is intentionally narrower than
> `crates/psionic/docs/ROADMAP.md`: it is about making cluster control,
> placement, scheduling, and later multi-node execution honest and
> machine-checkable, not about the full Psionic replacement program.

Agent execution instruction: implement this roadmap in dependency order, not by
raw local ID ordering and not by whichever backend issue looks most exciting at
the moment. Work the GitHub-backed queue below one issue at a time, keep the
local `PSI-*` IDs aligned with the real issue numbers, and update this document
after each cluster issue lands so it reflects the new GitHub state, landed
commit hash, current backend gate, and remaining cluster queue.

Reference-first instruction: cluster work must not be implemented from memory.
Choose the reference that owns the layer being changed:

- start with `~/code/exo` for cluster-control architecture, ordered event
  routing, catchup, membership, topology-aware placement, and node-role ideas
- start with `~/code/tinygrad` for execution-plan caching, replayable runtime
  policy, queueing, and machine-checkable execution evidence
- start with `~/code/llama.cpp` and `~/code/gpt-oss` for backend-specific
  distributed execution truth, model eligibility constraints, and the exact
  local GPT-OSS execution lane that cluster work must not overclaim
- start with `crates/psionic/docs/ROADMAP.md` for local-runtime readiness,
  capability/evidence surfaces, and the current CUDA execution gate
- start with `crates/psionic/docs/ROADMAP_METAL.md` for the current Apple
  refusal boundary; do not treat Metal as cluster-eligible just because generic
  Metal substrate exists

Psionic-only execution rule: these references are design, behavior, and
performance oracles only. Do not shell out to, proxy through, FFI-wrap, or
otherwise delegate cluster execution to Exo or any other external runtime when
closing issues in this roadmap. The shipped cluster path must remain Psionic-
owned end to end.

## Objective

Make Psionic's own cluster support truthful enough to become a real OpenAgents
execution surface:

- cluster control lives in `crates/psionic/*`, not app glue
- cluster identity, membership, topology, placement, and scheduling are
  explicit and replayable
- the first shipped cluster scope is a trusted same-network LAN cluster with
  explicit admission, not an internet-wide compute-market cluster
- cluster behavior is exposed through existing Psionic capability, receipt, and
  evidence seams rather than through a hidden side channel
- scheduling lands before replication, and replication lands before real
  sharded execution
- any later Exo interoperability remains optional and never becomes the only
  execution path

This is not a plan to "add generic distributed systems" to Psionic.

This is also not a license to widen backend claims. Cluster support only counts
when the selected backend lane is already truthful locally and the cluster layer
can prove what topology was promised, selected, and delivered.

## Ownership Rules

This roadmap must continue to respect `docs/OWNERSHIP.md`:

- `crates/psionic/*` owns reusable cluster identity, transport, ordered state,
  topology, placement, scheduling, evidence, and execution truth
- `apps/autopilot-desktop` owns app-level cluster controls, UX, onboarding,
  status presentation, and product orchestration
- `crates/wgpui*` must not absorb Psionic cluster business logic
- cluster work must not move wallet, payout, mission, or app-pane behavior into
  `crates/psionic/*`

## Why This Roadmap Exists

`crates/psionic/docs/ROADMAP.md` already tracks the full Psionic program, and
`crates/psionic/docs/EXO_UNIFIED_INTEGRATION_PLAN.md` already captures the full
cluster design shape. What is still missing is a roadmap-style, dependency-
ordered cluster queue that matches the format used by the main and Metal
roadmaps.

As of 2026-03-10, the current issue reality is:

- the first dedicated cluster queue now exists on GitHub
  - `PSI-184` / [#3289](https://github.com/OpenAgentsInc/openagents/issues/3289) is landed on `main`
  - `PSI-185` / [#3290](https://github.com/OpenAgentsInc/openagents/issues/3290) is landed on `main`
  - `PSI-186` / [#3291](https://github.com/OpenAgentsInc/openagents/issues/3291) is landed on `main`
  - `PSI-187` / [#3292](https://github.com/OpenAgentsInc/openagents/issues/3292) is landed on `main`
- the next cluster phases now also exist on GitHub
  - `PSI-188` / [#3297](https://github.com/OpenAgentsInc/openagents/issues/3297) through
    `PSI-190` / [#3299](https://github.com/OpenAgentsInc/openagents/issues/3299) are landed on `main`
  - `PSI-191` / [#3300](https://github.com/OpenAgentsInc/openagents/issues/3300) through
    `PSI-197` / [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306) remain open
- the current backend execution gates are still real and must remain visible
  - NVIDIA: `#3276` -> `#3288` -> `#3248`
  - Metal: `#3286` -> `#3285` -> `#3269` -> `#3262`

That is now concrete enough that cluster work deserves its own roadmap.

## Shipped On Main

`main` already includes the cluster-adjacent substrate this roadmap will build
on:

- `PSI-148` / [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232)
  - minimum hardware validation matrix and claim IDs
- `PSI-160` / [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220)
  - local-serving isolation policy truth
- `PSI-161` / [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171)
  - backend-neutral fallback lattice
- `PSI-162` / [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233)
  - served-artifact identity and reproducibility tuples
- `PSI-163` / [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234)
  - cache invalidation and persisted-state upgrade policy
- `PSI-164` / [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235)
  - provenance and license gating for local artifacts and compute-market supply
- `PSI-171` / [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223)
  - selected-device inventory qualifiers and backend-toolchain truth
- `PSI-172` / [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224)
  - execution-profile, queue-policy, and throughput-class truth
- `PSI-173` / [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225)
  - `ExecutionTopologyPlan`, `selected_devices`, and stable topology digests
- `PSI-174` / [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226)
  - execution-plan cache policy, kernel-cache policy, and compile-path evidence
- `PSI-175` / [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227)
  - delivery-proof and settlement-linkage inputs
- `PSI-179` through `PSI-183`
  - truthful local GPT-OSS/NVIDIA enablement now exists on `main`
- `PSI-184` / [#3289](https://github.com/OpenAgentsInc/openagents/issues/3289)
  - landed in `64c2a8fc6`
  - initial `psionic-cluster` crate, trusted-LAN namespace/admission config,
    typed UDP `hello`/`ping` handshake, generated `ClusterId`/`NodeId`,
    surfaced node-role truth, and integration coverage proving seeded local
    nodes discover each other without claiming scheduling or execution behavior
- `PSI-185` / [#3290](https://github.com/OpenAgentsInc/openagents/issues/3290)
  - landed in `f2e758720`
  - first-class `ClusterNamespace`, `AdmissionToken`, `ClusterAdmissionConfig`,
    `NodeEpoch`, and file-backed identity persistence; explicit cross-cluster,
    admission-mismatch, and stale-epoch refusal diagnostics; and integration
    coverage proving node identity survives restart while epoch truth advances
- `PSI-186` / [#3291](https://github.com/OpenAgentsInc/openagents/issues/3291)
  - landed in `cc60eea89`
  - typed `ClusterCommand`, `LocalClusterEvent`, `ClusterEvent`,
    `ClusterElectionMessage`, and `ClusterConnectionFact` schemas plus a
    Psionic-owned `ClusterEventLog`, contiguous indexed apply discipline,
    replayable `ClusterState`/`ClusterSnapshot`, stable state digests, and unit
    coverage for ordering, replay, and out-of-order refusal
- `PSI-187` / [#3292](https://github.com/OpenAgentsInc/openagents/issues/3292)
  - landed in `2acc2ecf6`
  - catchup requests and responses, compacted snapshots, bounded replay tails,
    full-resync versus snapshot-install recovery dispositions, and unit
    coverage proving rejoin and schema-mismatch recovery semantics
- `PSI-188` / [#3297](https://github.com/OpenAgentsInc/openagents/issues/3297)
  - landed in `2acc2ecf6`
  - cluster topology, link-class, transport, backend-readiness, and node
    telemetry facts now live in authoritative cluster state with separate
    topology digests and replay coverage
- `PSI-189` / [#3298](https://github.com/OpenAgentsInc/openagents/issues/3298)
  - landed in `2acc2ecf6`
  - artifact residency and cluster staging truth now live beside topology
    facts as separate digests and explicit residency status records rather than
    hidden scheduler assumptions
- `PSI-190` / [#3299](https://github.com/OpenAgentsInc/openagents/issues/3299)
  - landed in `2acc2ecf6`
  - runtime-owned `ClusterExecutionContext` now flows through
    `psionic-runtime`, `psionic-serve`, and provider capability and receipt
    surfaces with policy digests, selected nodes, residency posture, transport
    class, and fallback history

This is a real baseline. The cluster roadmap is not starting from zero.

## Current Reality

The checked-in repo is not yet a cluster runtime, but it is already shaped for
one:

- Psionic already has explicit topology and device-selection truth via
  `ExecutionTopologyPlan`, `selected_devices`, stable digests, and provider
  receipt/capability surfaces
- Psionic already has artifact identity, cache invalidation, provenance,
  hardware validation, and delivery-proof substrate that a cluster lane can
  extend instead of replacing
- there is now a `psionic-cluster` crate with trusted-LAN hello/ping
  discovery, persistent node identity, explicit admission policy,
  machine-checkable join refusals, replayable ordered state, catchup,
  snapshots, compaction, recovery, topology and telemetry facts, artifact
  residency truth, and cluster execution evidence seams, but there is still no
  truthful remote-node scheduling path
- the first honest cluster scope is still a trusted same-network LAN cluster
  with explicit namespace/admission policy, not an adversarial compute-market
  fabric
- real cluster execution claims must remain gated on a stable local backend lane
  rather than on design-doc optimism
  - first likely lane: homogeneous CUDA GPT-OSS after `#3276`, `#3288`, and
    `#3248`
  - current Metal GPT-OSS nodes remain explicit refusal candidates until the
    Metal roadmap queue closes

That means the next cluster work is not "make sharding happen somehow." It is:

- create a real control plane
- make placement and scheduling truthful
- reuse existing evidence seams
- widen execution claims only after the corresponding backend truth exists

## Lessons Now Baked Into This Roadmap

This roadmap explicitly adopts the conclusions from:

- `crates/psionic/docs/EXO_UNIFIED_INTEGRATION_PLAN.md`
- `../../../docs/audits/2026-03-09-psionic-exo-cluster-integration-audit.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/ROADMAP_METAL.md`

### Exo: control-plane truth, not runtime substitution

Exo contributes the right architecture ideas:

- typed topic separation
- ordered event logs with catchup
- leader-ordered global state
- namespace-based cluster isolation
- topology-aware placement
- coordinator-only versus execution-capable roles

Practical roadmap consequence:

- cluster work should port these semantics into Psionic-owned types and tests
- cluster work must not make Exo a required runtime dependency

### Main Psionic roadmap: cluster work must reuse shipped substrate

The main roadmap already landed the reusable pieces cluster support needs:

- device inventory qualifiers
- execution-topology planning
- artifact identity and cache invalidation
- provider and receipt evidence
- backend validation and fallback policy

Practical roadmap consequence:

- cluster support should extend those contracts
- cluster support should not invent a second hidden truth system

### Metal roadmap: cluster work must not paper over backend readiness

The Metal roadmap now makes the refusal boundary explicit:

- current Metal GPT-OSS still has correctness and architecture blockers
- same-host throughput closure is not yet honest enough for cluster eligibility

Practical roadmap consequence:

- current Metal GPT-OSS nodes should be refused for cluster execution
- do not use cluster work to smuggle Metal GPT-OSS readiness claims into the
  product

## What Still Blocks A Real Cluster Lane

### Cluster identity, membership, and ordered truth

Tracked by `PSI-184` through `PSI-187`, now landed on `main`.

Current truth:

- `psionic-cluster` now owns reusable cluster transport, identity, and ordered
  state substrate
- persistent `ClusterId`, `NodeId`, `NodeEpoch`, and node-role truth now exist
- authoritative ordered history, catchup, snapshots, compaction, and recovery
  semantics are now explicit and replayable

Required outcome:

- the next phases should consume this control-plane substrate rather than
  rebuilding cluster truth inside scheduling code

### Topology, residency, and evidence mapping

Tracked by `PSI-188` through `PSI-190`, now landed on `main`.

Current truth:

- authoritative cluster state now carries topology, link-class, transport, and
  node telemetry facts with stable digests
- artifact residency and placement readiness are now separate cluster truths
- provider capabilities and receipts can now carry cluster-specific digests,
  selected nodes, residency posture, transport class, and fallback history

Required outcome:

- the next scheduling phase should consume these facts to make remote-node
  routing decisions truthful rather than purely local

### Cluster-aware scheduling and serving policy

Tracked by proposed `PSI-191` through `PSI-193`.

Current truth:

- Psionic can choose devices inside one node, but it cannot yet admit on one
  node and execute on another
- fairness, cancellation, backpressure, and degraded replica routing are still
  local-serving concepts, not cluster-serving policy

Required outcome:

- the first useful cluster behavior is truthful remote whole-request scheduling,
  followed by replicated serving for one validated lane

### Real sharded execution

Tracked by proposed `PSI-194` and `PSI-195`.

Current truth:

- there is no Psionic-owned cross-node activation, KV, or synchronization
  substrate
- `ExecutionTopologyPlan` can describe sharding, but the runtime does not yet
  deliver it across nodes

Required outcome:

- at least one homogeneous sharded CUDA path becomes real, or the system
  refuses unsupported cluster sharding explicitly

### Validation, security, and rollout

Tracked by proposed `PSI-196` and `PSI-197`.

Current truth:

- there is no cluster validation matrix, fault-injection suite, or hardening
  path
- there is no authenticated cluster membership story beyond a local trusted-LAN
  assumption

Required outcome:

- cluster claims become evidence-backed and supportable before scope widening

## GitHub-Backed Roadmap Items

Phases C1 through C6 are now opened on GitHub. The local `PSI-*` IDs below
still come from the 2026-03-09 cluster audit, but this roadmap now maps them to
their real GitHub issue numbers directly.

### Phase C0: shipped cluster-adjacent baseline

Already on `main`:

- [#3232](https://github.com/OpenAgentsInc/openagents/issues/3232)
- [#3220](https://github.com/OpenAgentsInc/openagents/issues/3220)
- [#3171](https://github.com/OpenAgentsInc/openagents/issues/3171)
- [#3233](https://github.com/OpenAgentsInc/openagents/issues/3233)
- [#3234](https://github.com/OpenAgentsInc/openagents/issues/3234)
- [#3235](https://github.com/OpenAgentsInc/openagents/issues/3235)
- [#3223](https://github.com/OpenAgentsInc/openagents/issues/3223)
- [#3224](https://github.com/OpenAgentsInc/openagents/issues/3224)
- [#3225](https://github.com/OpenAgentsInc/openagents/issues/3225)
- [#3226](https://github.com/OpenAgentsInc/openagents/issues/3226)
- [#3227](https://github.com/OpenAgentsInc/openagents/issues/3227)
- [#3239](https://github.com/OpenAgentsInc/openagents/issues/3239) through
  [#3241](https://github.com/OpenAgentsInc/openagents/issues/3241)

### Phase C1: local-cluster control-plane foundation

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-184` | [#3289](https://github.com/OpenAgentsInc/openagents/issues/3289) | Closed | Stand up a hello-world local cluster connection in `psionic-cluster` | `psionic-cluster`, docs/tests | Landed in `64c2a8fc6`: established the crate seam and proved that seeded local Psionic nodes can discover each other, exchange typed hello/ping state, and report explicit role truth without claiming execution behavior. |
| `PSI-185` | [#3290](https://github.com/OpenAgentsInc/openagents/issues/3290) | Closed | Define cluster identity, node epoch, and admission policy | `psionic-cluster`, `psionic-runtime`, docs | Landed in `f2e758720`: persistent local node identity, explicit namespace/admission config, role-visible node epoch truth, and machine-checkable refusal of admission mismatch, cluster mismatch, and stale-node ambiguity. |
| `PSI-186` | [#3291](https://github.com/OpenAgentsInc/openagents/issues/3291) | Closed | Add typed cluster commands, events, and authoritative ordered state | `psionic-cluster` | Landed in `cc60eea89`: typed control-plane schemas, contiguous indexed-event apply rules, replayable authoritative cluster state, and stable digests that later receipts and diagnostics can reference. |
| `PSI-187` | [#3292](https://github.com/OpenAgentsInc/openagents/issues/3292) | Closed | Add catchup, snapshots, compaction, and recovery semantics | `psionic-cluster`, storage/tests | Landed in `2acc2ecf6`: event history now supports bounded replay, compacted snapshots, snapshot-install versus full-resync recovery, and rejoin coverage. |

### Phase C2: topology, staging, and evidence truth

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-188` | [#3297](https://github.com/OpenAgentsInc/openagents/issues/3297) | Closed | Publish topology, link-class, and node telemetry facts | `psionic-cluster`, `psionic-runtime`, `psionic-provider` | Landed in `2acc2ecf6`: authoritative state now carries topology and telemetry facts with explicit link classes, readiness posture, and stable topology digests. |
| `PSI-189` | [#3298](https://github.com/OpenAgentsInc/openagents/issues/3298) | Closed | Add artifact residency and cluster staging truth | `psionic-cluster`, `psionic-models`, `psionic-catalog`, `psionic-provider` | Landed in `2acc2ecf6`: cluster state now tracks artifact residency separately from topology with explicit staging status and dedicated residency digests. |
| `PSI-190` | [#3299](https://github.com/OpenAgentsInc/openagents/issues/3299) | Closed | Extend capability and receipt evidence for clustered execution | `psionic-runtime`, `psionic-provider`, `psionic-serve` | Landed in `2acc2ecf6`: provider and serve surfaces now expose cluster digests, selected nodes, residency posture, transport class, and fallback history through a runtime-owned evidence type. |

### Phase C3: cluster-aware single-node scheduling and policy

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-191` | [#3300](https://github.com/OpenAgentsInc/openagents/issues/3300) | Open | Add whole-request remote scheduling for one-node execution | `psionic-cluster`, `psionic-serve`, `psionic-provider` | The first useful cluster behavior is choosing the best node for the whole request while keeping execution local to that node. |
| `PSI-192` | [#3301](https://github.com/OpenAgentsInc/openagents/issues/3301) | Open | Add queue policy, fairness, cancellation, and backpressure rules | `psionic-cluster`, `psionic-runtime`, `psionic-serve` | A cluster that ignores fairness and slow-node behavior will underperform while still looking correct on paper. |

### Phase C4: replicated serving for one validated backend lane

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-193` | [#3302](https://github.com/OpenAgentsInc/openagents/issues/3302) | Open | Ship replicated cluster serving for one validated backend lane | `psionic-cluster`, `psionic-runtime`, `psionic-serve`, `psionic-provider` | Replication is the first operationally useful scale-out story and should land before true sharding. |

### Phase C5: first truthful sharded execution lane

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-194` | [#3303](https://github.com/OpenAgentsInc/openagents/issues/3303) | Open | Add homogeneous CUDA layer-sharded execution | `psionic-backend-cuda`, `psionic-runtime`, `psionic-cluster`, `psionic-provider` | Layer sharding is the safest first real multi-node execution claim after the local CUDA lane is stable. |
| `PSI-195` | [#3304](https://github.com/OpenAgentsInc/openagents/issues/3304) | Open | Add homogeneous CUDA tensor-sharded execution and transport policy | `psionic-backend-cuda`, `psionic-runtime`, `psionic-cluster`, `psionic-provider` | Tensor sharding requires explicit transport requirements, model eligibility gates, and refusal semantics instead of aspirational topology claims. |

### Phase C6: validation, security, and rollout hardening

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-196` | [#3305](https://github.com/OpenAgentsInc/openagents/issues/3305) | Open | Add cluster validation, fault-injection, and performance gates | docs/tests/validation plus cluster crates | Cluster claims need a real validation matrix, not just unit-test confidence. |
| `PSI-197` | [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306) | Open | Harden cluster trust beyond the first LAN scope | `psionic-cluster`, security/docs | The first shipped cluster scope is LAN-trusted only; wider cluster claims need explicit authentication, replay protection, and stronger admission rules. |

## Recommended Order

The shortest honest path from today's `main` is:

1. Treat C1 and C2 as landed on `main` in `2acc2ecf6`.
2. Keep the opened later-phase queue aligned to the roadmap and only pull work
   forward when its dependency notes are actually satisfied:
   `#3300` -> `#3301` -> `#3302` -> `#3303` -> `#3304` -> `#3305` -> `#3306`.
3. Keep the active local CUDA throughput queue
   `#3276` -> `#3288` -> `#3248` in flight in parallel; do not let cluster work
   become an excuse to stop finishing the local lane.
4. Treat closure of that local CUDA lane as the gate for `#3300` onward.
5. Execute `#3300` then `#3301` then `#3302`.
6. Execute `#3303` then `#3304` only for one homogeneous CUDA lane.
7. Execute `#3305` and `#3306` before widening scope beyond the first
   trusted-LAN cluster claim.
8. Keep current Metal GPT-OSS nodes refused for cluster execution until the
   Metal roadmap queue `#3286` -> `#3285` -> `#3269` -> `#3262` closes.

Why this order:

- control-plane truth first, because cluster execution without cluster truth is
  not supportable
- topology, residency, and evidence before scheduling, because otherwise the
  scheduler has nothing auditable to stand on
- remote whole-request scheduling before replication, because it gives immediate
  value without pretending cross-node compute is already real
- replication before sharding, because it is safer operationally and easier to
  prove honestly
- hardening before scope widening, because trusted-LAN cluster claims are not
  the same thing as compute-market distributed-cluster claims

## Definition Of Done For The First Cluster Scope

This roadmap's first truthful cluster scope is complete only when all of the
following are true:

- the shipped feature is explicitly a trusted same-network LAN cluster, not an
  adversarial or internet-wide cluster
- `psionic-cluster` exists and owns cluster identity, ordered state, topology,
  placement, and catchup truth
- `ClusterId`, `NodeId`, `NodeEpoch`, and node-role truth are persistent and
  refusal-capable
- ordered cluster-state history, snapshots, compaction, and rejoin semantics
  are explicit
- topology, transport class, artifact residency, and policy digests are visible
  in capability and receipt surfaces
- whole-request remote scheduling works and is reflected truthfully in
  `ExecutionTopologyPlan` and provider receipts
- replicated serving works for one validated backend/product lane
- unsupported backends are refused explicitly rather than silently included
- current Metal GPT-OSS nodes remain refused for cluster execution until the
  Metal roadmap itself is complete

## Additional Definition Of Done For Psionic As A Cluster Substrate

The broader cluster program is not complete until all of the following are
true:

- at least one truthful homogeneous sharded CUDA path exists, or unsupported
  sharded paths are refused with stable diagnostics
- cluster validation covers membership refusal, disconnect/rejoin, catchup,
  artifact staging, remote scheduling, replication, and sharding
- cluster performance claims are tied to explicit benchmark receipts
- authenticated membership, signed control-plane messages, replay protection,
  and stronger admission policy exist before any multi-subnet or market-facing
  scope widening
- downstream OpenAgents systems can tell exactly what cluster topology was
  promised, selected, delivered, and degraded

## Likely Follow-On After This Roadmap

There are three likely follow-ons that should remain outside the first cluster
scope:

- optional Exo interoperability experiments, only after Psionic's own cluster
  substrate is credible
- operator-configured multi-subnet clusters with configured peers and stronger
  membership policy
- Apple clustered execution, but only after the native Metal GPT-OSS roadmap is
  complete and cluster placement can express communication-class eligibility
  honestly

## Non-Goals

- making Exo a required runtime dependency for `crates/psionic/*`
- proxying or delegating cluster execution through Exo and calling that
  Psionic cluster support
- treating Nostr, relays, or Nexus as the cluster control plane
- claiming sharded cluster execution when the system is only doing remote
  whole-request scheduling
- treating the first cluster scope as if it already solved adversarial
  compute-market security
- making current Metal GPT-OSS nodes cluster-eligible before the Metal roadmap
  itself is complete
- reopening local model loading, tokenizer, or artifact-format truth as if
  cluster work replaced the shipped Psionic loader/runtime substrate
- moving app UX or pane orchestration from `apps/autopilot-desktop` into
  `crates/psionic/*`
