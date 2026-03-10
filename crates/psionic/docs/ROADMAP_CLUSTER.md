# Psionic Cluster Roadmap

> Status: updated 2026-03-10 after reading `ROADMAP.md`,
> `ROADMAP_METAL.md`, `EXO_UNIFIED_INTEGRATION_PLAN.md`, and
> `../../../docs/audits/2026-03-09-psionic-exo-cluster-integration-audit.md`,
> after confirming that the cluster-adjacent substrate `PSI-148`,
> `PSI-160` through `PSI-175`, and `PSI-179` through `PSI-183` is already
> landed on `main`, after confirming that the former NVIDIA local-runtime gate
> `#3276` -> `#3288` -> `#3248` is now closed on GitHub, after confirming that the active
> native Metal GPT-OSS gate remains `#3286` -> `#3285` -> `#3269` -> `#3262`,
> after landing `PSI-184` / `#3289` in `64c2a8fc6` and `PSI-185` / `#3290` in
> `f2e758720`, after landing `PSI-186` / `#3291` in `cc60eea89`, after
> opening `PSI-188` through `PSI-197` as `#3297` through `#3306`, after
> landing `PSI-187` / `#3292` through `PSI-190` / `#3299` in `2acc2ecf6`,
> after landing `PSI-191` / `#3300` in `ad6891b82`, after confirming that
> `PSI-192` / `#3301` in `327944c08`, after landing `PSI-193` / `#3302` in
> `d88d284c5`, after landing `PSI-194` / `#3303` in `fa7523ada`, after
> landing `PSI-195` / `#3304` in `1cdcf3058`, after landing `PSI-196` /
> `#3305` in `7124eefd7`, after landing `PSI-197` / `#3306` in `d424ab1cf`,
> after opening `PSI-198` through `PSI-201` as `#3307` through `#3310` for the
> operator-managed multi-subnet follow-on queue, after landing `PSI-198` /
> `#3307` in `011e0452c`, after landing `PSI-199` / `#3308` in `87d428e43`,
> after landing `PSI-200` / `#3309` in `86a2c920a`, after landing `PSI-201` /
> `#3310` in `ac9dd2285`, after opening `PSI-202` through `PSI-205` as
> `#3311` through `#3314` for the coordinator-authority multi-subnet follow-on
> queue, after landing `PSI-202` / `#3311` in `1e65c56c9`, after landing
> `PSI-203` / `#3312` in `ddc092cbb`, after landing `PSI-204` / `#3313` in
> `313fbdc25`, after landing `PSI-205` / `#3314` in `4732fbc26`, after
> opening `PSI-206` through `PSI-209` as `#3315` through `#3318` for the
> command-authorization and payout-provenance follow-on queue, after landing
> `PSI-206` / `#3315` in `e6888aaa0`, after landing `PSI-207` / `#3316` in
> `7b7b681f7`, after opening `PSI-210` through `PSI-213` as `#3319` through
> `#3322` for the compute-market trust hardening follow-on queue, after
> landing `PSI-210` / `#3319` in `37fb246f1`, after landing `PSI-211` /
> `#3320` in `4a21d6947`, after landing `PSI-212` / `#3321` in
> `d0f3e7891`, after landing `PSI-213` / `#3322` in `b0601f662`, after
> opening `PSI-214` through `PSI-216` as `#3323` through `#3325` for the
> wider-network discovery follow-on queue, after landing `PSI-214` / `#3323`
> in `1102bffa4`, after landing `PSI-215` / `#3324` in `47410298a`, after
> landing `PSI-216` / `#3325` in `7c0f34503`, after opening `PSI-217`
> through `PSI-219` as `#3329` through `#3331` for the post-E1 follow-on
> queue, after landing `PSI-217` / `#3329` in `7aa76a2a9`, after landing
> `PSI-218` / `#3330` as the explicit decision memo in `EXO_INTEROPERABILITY_DECISION.md`,
> after landing `PSI-219` / `#3331` in `98dc1bdc3`, after opening
> `PSI-220` through `PSI-222` as `#3332` through `#3334` for the cluster
> benchmark-receipt follow-on queue, after landing `PSI-220` / `#3332` in
> `4f64525b4`, after landing `PSI-221` / `#3334` in `a524658b8`, after
> landing `PSI-222` / `#3333` in `3fe872c96`, after opening `PSI-223`
> through `PSI-225` as `#3335` through `#3337` for the declared cluster
> capability-profile follow-on queue, after landing `PSI-223` / `#3335` in
> `37183c6cb`, after landing `PSI-224` / `#3336` in `9aad9af8d`, after
> landing `PSI-225` / `#3337` in `efa52005e`, after opening `PSI-226`
> through `PSI-228` as `#3341`, `#3339`, and `#3340` for the advertised
> capability-profile publication follow-on queue, and after checking live
> GitHub issue search so this roadmap reflects
> the current GitHub queue rather than local placeholders.
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
    `PSI-197` / [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306) are landed on `main`
  - the first multi-subnet follow-on queue is now landed on `main`
    - `PSI-198` / [#3307](https://github.com/OpenAgentsInc/openagents/issues/3307) is landed on `main`
    - `PSI-199` / [#3308](https://github.com/OpenAgentsInc/openagents/issues/3308) is landed on `main`
    - `PSI-200` / [#3309](https://github.com/OpenAgentsInc/openagents/issues/3309) is landed on `main`
    - `PSI-201` / [#3310](https://github.com/OpenAgentsInc/openagents/issues/3310) is landed on `main`
  - the coordinator-authority multi-subnet queue is now landed on `main`
    - `PSI-202` / [#3311](https://github.com/OpenAgentsInc/openagents/issues/3311) is landed on `main`
    - `PSI-203` / [#3312](https://github.com/OpenAgentsInc/openagents/issues/3312) is landed on `main`
    - `PSI-204` / [#3313](https://github.com/OpenAgentsInc/openagents/issues/3313) is landed on `main`
    - `PSI-205` / [#3314](https://github.com/OpenAgentsInc/openagents/issues/3314) is landed on `main`
  - the command authorization and payout provenance queue is now landed on `main`
    - `PSI-206` / [#3315](https://github.com/OpenAgentsInc/openagents/issues/3315) is landed on `main`
    - `PSI-207` / [#3316](https://github.com/OpenAgentsInc/openagents/issues/3316) is landed on `main`
    - `PSI-208` / [#3317](https://github.com/OpenAgentsInc/openagents/issues/3317) is landed on `main`
    - `PSI-209` / [#3318](https://github.com/OpenAgentsInc/openagents/issues/3318) is landed on `main`
  - the compute-market trust hardening follow-on queue is now landed on `main`
    - `PSI-210` / [#3319](https://github.com/OpenAgentsInc/openagents/issues/3319) is landed on `main`
    - `PSI-211` / [#3320](https://github.com/OpenAgentsInc/openagents/issues/3320) is landed on `main`
    - `PSI-212` / [#3321](https://github.com/OpenAgentsInc/openagents/issues/3321) is landed on `main`
    - `PSI-213` / [#3322](https://github.com/OpenAgentsInc/openagents/issues/3322) is landed on `main`
  - the wider-network discovery follow-on queue is now landed on `main`
    - `PSI-214` / [#3323](https://github.com/OpenAgentsInc/openagents/issues/3323) is landed on `main`
    - `PSI-215` / [#3324](https://github.com/OpenAgentsInc/openagents/issues/3324) is landed on `main`
    - `PSI-216` / [#3325](https://github.com/OpenAgentsInc/openagents/issues/3325) is landed on `main`
  - the post-E1 follow-on queue is now landed on `main`
    - `PSI-217` / [#3329](https://github.com/OpenAgentsInc/openagents/issues/3329) is landed on `main`
    - `PSI-218` / [#3330](https://github.com/OpenAgentsInc/openagents/issues/3330) is landed on `main`
    - `PSI-219` / [#3331](https://github.com/OpenAgentsInc/openagents/issues/3331) is landed on `main`
  - the benchmark-receipt follow-on queue is now open on GitHub
    - `PSI-220` / [#3332](https://github.com/OpenAgentsInc/openagents/issues/3332) is landed on `main`
    - `PSI-221` / [#3334](https://github.com/OpenAgentsInc/openagents/issues/3334) is landed on `main`
    - `PSI-222` / [#3333](https://github.com/OpenAgentsInc/openagents/issues/3333) is landed on `main`
  - the declared cluster capability-profile follow-on queue is now landed on `main`
    - `PSI-223` / [#3335](https://github.com/OpenAgentsInc/openagents/issues/3335) is landed on `main`
    - `PSI-224` / [#3336](https://github.com/OpenAgentsInc/openagents/issues/3336) is landed on `main`
    - `PSI-225` / [#3337](https://github.com/OpenAgentsInc/openagents/issues/3337) is landed on `main`
  - the advertised capability-profile publication follow-on queue is active on `main`
    - `PSI-226` / [#3341](https://github.com/OpenAgentsInc/openagents/issues/3341) is landed on `main`
    - `PSI-227` / [#3339](https://github.com/OpenAgentsInc/openagents/issues/3339) is open
    - `PSI-228` / [#3340](https://github.com/OpenAgentsInc/openagents/issues/3340) is open
- the current backend execution gates are still real and must remain visible
  - former NVIDIA gate: `#3276` -> `#3288` -> `#3248` is closed on `main`
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
- `PSI-191` / [#3300](https://github.com/OpenAgentsInc/openagents/issues/3300)
  - landed in `ad6891b82`
  - `psionic-cluster` now provides a deterministic whole-request remote
    scheduler that consumes authoritative membership, telemetry, transport, and
    artifact-residency facts, emits truthful single-node
    `ExecutionTopologyPlan` output plus runtime-owned cluster execution
    evidence, and surfaces explicit machine-checkable refusal and degraded-path
    diagnostics
- `PSI-192` / [#3301](https://github.com/OpenAgentsInc/openagents/issues/3301)
  - landed in `327944c08`
  - `psionic-cluster` now provides explicit cluster serving policy for queue
    discipline, prefill-versus-decode fairness, cancellation propagation,
    slow-node backpressure, and reroute behavior on top of truthful
    whole-request scheduling, while `psionic-runtime` now carries serving
    policy digests and fallback reasons for those cluster-routing outcomes
- `PSI-193` / [#3302](https://github.com/OpenAgentsInc/openagents/issues/3302)
  - landed in `d88d284c5`
  - `psionic-cluster` now provides truthful replicated serving for one lane,
    including stable replica-lane identity, warm-state snapshots, lifecycle
    policy digests, warm-standby versus selected routing truth, deterministic
    refusal when warm replica count is insufficient, and replicated
    `ExecutionTopologyPlan` output; `psionic-runtime` and `psionic-provider`
    now surface replica-state digests, replica-node evidence, and replicated
    topology/device truth consistently through delivered execution, capability,
    and receipt surfaces
- `PSI-194` / [#3303](https://github.com/OpenAgentsInc/openagents/issues/3303)
  - landed in `fa7523ada`
  - `psionic-cluster` now provides a first homogeneous CUDA layer-sharded lane
    with deterministic multi-node placement, explicit activation and KV handoff
    facts plus estimated bytes-per-token, refusal when shard geometry,
    transport, or artifact readiness is insufficient, and truthful
    `ExecutionTopologyPlan::layer_sharded` output; `psionic-runtime` and
    `psionic-provider` now preserve shard handoff evidence and layer-sharded
    topology truth through delivered execution and receipt surfaces
- `PSI-195` / [#3304](https://github.com/OpenAgentsInc/openagents/issues/3304)
  - landed in `1cdcf3058`
  - `psionic-cluster` now provides a first homogeneous CUDA tensor-sharded
    lane with deterministic tensor-axis partitioning, explicit model-
    eligibility and mesh-transport policy truth, transport-policy digests,
    tensor-collective handoff evidence, refusal when backend, geometry, or
    mesh policy is insufficient, and truthful
    `ExecutionTopologyPlan::tensor_sharded` output; `psionic-runtime` and
    `psionic-provider` now preserve tensor partition facts through delivered
    execution and receipt surfaces
- `PSI-196` / [#3305](https://github.com/OpenAgentsInc/openagents/issues/3305)
  - landed in `7124eefd7`
  - `psionic-cluster` now ships a reusable integration validation matrix, a
    restart/rejoin transport test, fault-injected recovery/scheduling/
    replication/sharding coverage, a release benchmark gate script for cluster
    planners, and an operator runbook in
    `crates/psionic/docs/CLUSTER_VALIDATION_RUNBOOK.md`
- `PSI-197` / [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306)
  - landed in `d424ab1cf`
  - `psionic-cluster` now exposes machine-checkable trust posture via
    `ClusterTrustPolicy` and `ConfiguredClusterPeer`, persists node signing
    identity beside node ID and epoch, authenticates configured peers with
    signed control-plane messages, rejects duplicate replay counters, and
    extends transport coverage plus the validation runbook to prove signed
    configured-peer discovery, unknown-peer refusal, tamper refusal, and
    replay refusal without pretending the default cluster posture is now
    internet-safe
- `PSI-198` / [#3307](https://github.com/OpenAgentsInc/openagents/issues/3307)
  - landed in `011e0452c`
  - `psionic-cluster` now ships a persisted `ClusterOperatorManifest` with a
    stable rollout digest, JSON load/store, manifest-to-`LocalClusterConfig`
    conversion, and transport coverage proving authenticated configured-peer
    nodes can boot from manifests instead of ad hoc code-only config
- `PSI-199` / [#3308](https://github.com/OpenAgentsInc/openagents/issues/3308)
  - landed in `87d428e43`
  - `ordered_state` now ships authenticated catchup and snapshot envelopes with
    stable recovery digests, signer/requester verification, replay refusal, and
    tests proving signed recovery succeeds while tampered or replayed recovery
    envelopes are refused explicitly
- `PSI-200` / [#3309](https://github.com/OpenAgentsInc/openagents/issues/3309)
  - landed in `86a2c920a`
  - `psionic-cluster` now exposes explicit configured-peer dial policy and
    health snapshots with backoff, degraded and unreachable reachability
    posture, and transport coverage proving configured peers degrade honestly
    when absent and recover cleanly when they later join
- `PSI-201` / [#3310](https://github.com/OpenAgentsInc/openagents/issues/3310)
  - landed in `ac9dd2285`
  - `psionic-cluster` now exposes trust-bundle versioning, accepted rollout
    overlap windows, previous-key acceptance for configured peers, and
    machine-checkable rollout diagnostics for accepted overlap and stale-bundle
    refusal, with transport coverage proving key rotation and stale-bundle
    drift are surfaced honestly
- `PSI-202` / [#3311](https://github.com/OpenAgentsInc/openagents/issues/3311)
  - landed in `1e65c56c9`
  - `ordered_state` now exposes explicit coordinator lease policy, lease-aware
    leadership truth, effective-versus-stale coordinator queries, stable
    stale-leader diagnostics, lease-aware state digests, and runbook-backed
    validation for operator-managed multi-subnet coordinator freshness claims
- `PSI-203` / [#3312](https://github.com/OpenAgentsInc/openagents/issues/3312)
  - landed in `ddc092cbb`
  - `ordered_state` now exposes a reusable election-term vote ledger, explicit
    conflicting-vote refusal, explicit same-term split-brain leader refusal,
    and an authoritative-state guard that refuses conflicting
    `LeadershipReconciled` events instead of silently switching coordinators in
    one term
- `PSI-204` / [#3313](https://github.com/OpenAgentsInc/openagents/issues/3313)
  - landed in `313fbdc25`
  - current clustered execution evidence now carries coordinator term, commit
    index, fence token, and authority digest truth through `psionic-runtime`,
    `psionic-cluster`, and `psionic-provider`, while sharded and whole-request
    schedules now attach authority digests so stale coordinators cannot present
    current commit authority implicitly after failover
- `PSI-205` / [#3314](https://github.com/OpenAgentsInc/openagents/issues/3314)
  - landed in `4732fbc26`
  - `cluster_validation_matrix` and the operator runbook now cover stale-leader
    diagnostics, same-term split-brain refusal, and fenced coordinator failover
    rotation, so the coordinator-authority queue now has explicit validation
    drills and fail conditions instead of code-only claims
- `PSI-206` / [#3315](https://github.com/OpenAgentsInc/openagents/issues/3315)
  - landed in `e6888aaa0`
  - `ordered_state` now exposes typed cluster-command authority scopes,
    operator-managed authorization policy digests, machine-checkable refusal
    codes, and coordinator-override versus self/peer/member authorization
    decisions with stable digests and unit coverage for coordinator-only, self-
    scoped, peer-scoped, and membership-status-gated command submission
- `PSI-207` / [#3316](https://github.com/OpenAgentsInc/openagents/issues/3316)
  - landed in `7b7b681f7`
  - `IndexedClusterEvent`, `ClusterSnapshot`, and `ClusterState` now retain
    command authorization provenance for memberships, links, telemetry,
    artifact residency, and leadership facts; compaction, catchup, and
    snapshot recovery preserve that truth; and unit coverage proves replay and
    snapshot-install recovery keep provenance intact

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
  residency truth, cluster execution evidence seams, truthful remote
  whole-request scheduling, explicit cluster queue/fairness/backpressure
  policy, truthful replicated serving for one lane, and first homogeneous CUDA
  layer-sharded and tensor-sharded lanes with explicit handoff truth, but
  there is still no widened-backend sharding path
- the first honest cluster scope remains a trusted same-network LAN cluster
  with explicit namespace/admission policy, not an adversarial compute-market
  fabric
- there is now an explicit operator-managed configured-peer posture for wider
  networks, but it is opt-in, signed, replay-protected, and still not an
  internet-wide adversarial trust model
- real cluster execution claims must remain gated on a stable local backend lane
  rather than on design-doc optimism
  - first truthful lane is now homogeneous CUDA GPT-OSS, with `#3276`,
    `#3288`, and `#3248` closed on `main`
  - current Metal GPT-OSS nodes remain explicit refusal candidates until the
    Metal roadmap queue closes

That means the next cluster work is not "make sharding happen somehow." It is:

- keep the replicated and layer-sharded lanes truthful and measurable
- keep tensor sharding bounded by explicit transport and model-eligibility
  refusal boundaries
- reuse the existing evidence seams instead of inventing a side channel
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

Tracked by `PSI-191` through `PSI-193`, now landed on `main`.

Current truth:

- Psionic can now choose one best remote node for whole-request execution and
  express the result as truthful single-node topology and cluster evidence
- cluster serving policy is now explicit for queue discipline, decode fairness,
  cancellation propagation, slow-node backpressure, and reroute/refusal
  behavior across whole-request candidates
- replicated serving now exists for one lane with stable replica identity,
  warm-state snapshots, lifecycle-policy digests, and explicit selected versus
  warm-standby routing truth reflected in runtime and provider evidence

Required outcome:

- keep replicated routing truthful as the baseline operational scale-out mode
  while the next phase adds real sharded execution rather than overloading
  replica evidence with sharding claims
- preserve explicit degraded and refusal diagnostics when replica warm-state or
  lifecycle policy is insufficient for honest replicated service

### Real sharded execution

Tracked by `PSI-194` / [#3303](https://github.com/OpenAgentsInc/openagents/issues/3303)
and `PSI-195` / [#3304](https://github.com/OpenAgentsInc/openagents/issues/3304).

Current truth:

- one homogeneous CUDA layer-sharded lane now exists with deterministic
  placement, explicit activation and KV handoff truth, bytes-per-token
  estimates, and provider/runtime evidence for layer boundaries and handoff
  transport
- one homogeneous CUDA tensor-sharded lane now also exists with deterministic
  tensor-axis partitioning, explicit model-eligibility truth, explicit mesh
  transport policy, tensor-collective handoff evidence, and refusal of
  unsupported backend, ineligible tensor geometry, or unsuitable shard mesh

Required outcome:

- keep both sharded lanes bounded by the new validation matrix, benchmark gate,
  and operator runbook rather than letting the roadmap outrun the evidence
- continue refusing unsupported cluster sharding explicitly instead of
  collapsing to whole-request or replica-routed claims

### Validation, security, and rollout

Tracked by landed `PSI-196` / [#3305](https://github.com/OpenAgentsInc/openagents/issues/3305)
and landed `PSI-197` / [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306).

Current truth:

- there is now a reusable cluster validation matrix, fault-injected coverage,
  a release benchmark gate, and an operator runbook for both the shipped
  trusted-LAN scope and the widened authenticated configured-peer posture
- authenticated cluster membership now exists through machine-checkable trust
  policy, configured peers, signed control-plane messages, and replay
  protection
- adversarial or compute-market trust claims are still out of scope

Required outcome:

- keep the validation and benchmark assets authoritative for both trust
  postures instead of letting rollout claims outrun the tests
- keep wider trust claims bounded to operator-managed configured peers until a
  new GitHub-backed queue proves anything stronger

### Operator-managed multi-subnet follow-on

Tracked by landed `PSI-198` / [#3307](https://github.com/OpenAgentsInc/openagents/issues/3307)
through `PSI-201` / [#3310](https://github.com/OpenAgentsInc/openagents/issues/3310).

Current truth:

- authenticated configured-peer posture now has a reusable operator manifest
  and rollout digest instead of relying entirely on hand-built Rust config
- catchup and snapshot payloads are now signed, digest-checked, and replay-
  checked for authenticated recovery paths
- configured peers now carry explicit dial policy, backoff, and degraded or
  unreachable reachability truth instead of looking like implicit LAN retries
- trust-bundle versioning, previous-key overlap, and stale-bundle rollout
  diagnostics are now explicit and machine-checkable for operator-managed
  configured-peer clusters

Required outcome:

- keep the operator runbook authoritative for manifest, recovery, dial-health,
  and rotation drills rather than widening trust claims without evidence

### Coordinator authority and failover follow-on

Tracked by landed `PSI-202` / [#3311](https://github.com/OpenAgentsInc/openagents/issues/3311)
through landed `PSI-205` / [#3314](https://github.com/OpenAgentsInc/openagents/issues/3314).

Current truth:

- operator-managed configured-peer clusters now have manifest, signed recovery,
  dial-health, and trust-rollout truth
- `ordered_state` now has explicit coordinator lease policy, lease-aware
  leadership records, effective-versus-stale coordinator queries, stable
  stale-leader diagnostics, reusable election-term vote ledger, and same-term
  split-brain refusal on `main`
- clustered execution evidence now also carries coordinator term, commit index,
  fence token, and authority digest truth on `main`
- operator validation drills for fenced coordinator turnover now exist in the
  validation matrix and runbook on `main`
- that means wider operator-managed clusters still depend on implicit
  multi-subnet assumptions only to the extent that any stronger future claim
  now needs a new GitHub-backed queue rather than a placeholder extension here

Required outcome:

- keep the new failover drill authoritative while the next queue adds typed
  command authorization and payout-grade provenance rather than silently
  inferring who was allowed to mutate authoritative state

### Command authorization and payout provenance follow-on

Tracked by landed `PSI-206` / [#3315](https://github.com/OpenAgentsInc/openagents/issues/3315)
through `PSI-209` / [#3318](https://github.com/OpenAgentsInc/openagents/issues/3318),
with the full D3 queue now landed on `main`.

Current truth:

- authenticated operator-managed clusters now have signed transport, replay
  protection, manifest/dial/rollout truth, coordinator lease state, split-
  brain refusal, and fenced commit authority on `main`
- `ClusterCommand` now carries typed authority scopes and is paired with an
  operator-managed authorization policy, stable command/policy digests,
  coordinator override, and machine-checkable refusal diagnostics on `main`
- authoritative ordered events, snapshots, and recovered cluster state now also
  retain command-authorization provenance for the current facts they expose
- clustered execution evidence and settlement-linkage inputs now also retain
  bounded command/admission provenance, including scheduler membership,
  selected-node membership, artifact-residency authorization, and leadership
  fence truth
- the validation matrix and operator runbook now also cover allowed/refused
  authorization flows, payout-facing settlement provenance, and the sharded
  provenance merge path
- no open cluster roadmap issues remain under the current D3 scope; any
  stronger claim now requires a new explicit follow-on queue

Required outcome:

- preserve command provenance through ordered history, catchup, and snapshot
  flows so replay can explain who requested a mutation and under which policy
- keep runtime/provider execution and settlement evidence aligned with the same
  bounded provenance truth, then add validation gates for those claims

### Compute-market trust hardening follow-on

Tracked by landed `PSI-210` / [#3319](https://github.com/OpenAgentsInc/openagents/issues/3319)
and `PSI-211` / [#3320](https://github.com/OpenAgentsInc/openagents/issues/3320),
plus landed `PSI-212` / [#3321](https://github.com/OpenAgentsInc/openagents/issues/3321)
and `PSI-213` / [#3322](https://github.com/OpenAgentsInc/openagents/issues/3322).

Current truth:

- the D1 through D3 queues made operator-managed clusters explicit, signed, and
  provenance-aware, but they did not make current postures market-safe
- current cluster trust postures now include trusted-LAN, authenticated
  configured-peer, and attested configured-peer admission, which is stronger
  than the earlier operator-managed posture but still not a wider compute-
  market discovery fabric
- `ClusterTrustPolicy` now exposes a machine-checkable compute-market refusal
  contract, runtime/provider/cluster now expose a signed cluster evidence
  bundle export, attested configured-peer admission now exists as an explicit
  seam, and cluster policy/config now surface explicit discovery posture plus a
  bounded non-LAN discovery assessment
- any compute-market distributed-cluster language would still outrun the code
  unless a future wider-network discovery queue replaces the current
  machine-checkable refusal boundary with a real discovery fabric

Required outcome:

- keep current non-market-safe postures refusal-capable instead of doc-only
- keep clustered execution evidence bound into signed exportable bundles before
  talking about audit or dispute handling outside operator-managed posture
- keep attestation-aware admission explicit and refusal-capable for market-
  facing node identity claims
- keep current non-LAN discovery posture and refusal boundary explicit before
  widening cluster claims toward a compute-market fabric
- open a fresh GitHub-backed queue instead of mutating D4 in place when wider-
  network discovery implementation work actually starts

## GitHub-Backed Roadmap Items

Phases C1 through C6 are now all landed on GitHub/main. The local `PSI-*` IDs
below still come from the 2026-03-09 cluster audit, but this roadmap now maps
them to their real GitHub issue numbers directly. The next multi-subnet
follow-on queues now also have real GitHub issue numbers instead of placeholder
notes.

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
| `PSI-191` | [#3300](https://github.com/OpenAgentsInc/openagents/issues/3300) | Closed | Add whole-request remote scheduling for one-node execution | `psionic-cluster`, `psionic-runtime` | Landed in `ad6891b82`: authoritative cluster facts now drive deterministic whole-request remote scheduling with truthful single-node topology, selected device identity, degraded-path notes, and machine-checkable refusal diagnostics. |
| `PSI-192` | [#3301](https://github.com/OpenAgentsInc/openagents/issues/3301) | Closed | Add queue policy, fairness, cancellation, and backpressure rules | `psionic-cluster`, `psionic-runtime` | Landed in `327944c08`: cluster serving policy is now explicit and replayable, with queue discipline, decode fairness, cancellation propagation, slow-node backpressure, reroute/refusal outcomes, serving-policy digests, and fallback-reason evidence layered on top of truthful whole-request scheduling. |

### Phase C4: replicated serving for one validated backend lane

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-193` | [#3302](https://github.com/OpenAgentsInc/openagents/issues/3302) | Closed | Ship replicated cluster serving for one validated backend lane | `psionic-cluster`, `psionic-runtime`, `psionic-serve`, `psionic-provider` | Landed in `d88d284c5`: Psionic now has a truthful replicated-serving lane with replica warm-state and lifecycle-policy truth, deterministic warm-replica routing and refusal behavior, replicated execution topology output, and consistent capability/receipt evidence for replica selection and standby state. |

### Phase C5: first truthful sharded execution lane

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-194` | [#3303](https://github.com/OpenAgentsInc/openagents/issues/3303) | Closed | Add homogeneous CUDA layer-sharded execution | `psionic-backend-cuda`, `psionic-runtime`, `psionic-cluster`, `psionic-provider` | Landed in `fa7523ada`: Psionic now has a first homogeneous CUDA layer-sharded lane with deterministic shard placement, explicit activation/KV handoff evidence and bytes-per-token estimates, truthful `ExecutionTopologyPlan::layer_sharded` reporting, provider receipt propagation, and refusal coverage for non-CUDA, unsuitable inter-shard links, and insufficient artifact readiness. |
| `PSI-195` | [#3304](https://github.com/OpenAgentsInc/openagents/issues/3304) | Closed | Add homogeneous CUDA tensor-sharded execution and transport policy | `psionic-backend-cuda`, `psionic-runtime`, `psionic-cluster`, `psionic-provider` | Landed in `1cdcf3058`: Psionic now has a first homogeneous CUDA tensor-sharded lane with explicit tensor-axis partition geometry, model-eligibility truth, transport-policy digests, tensor-collective handoff evidence, and refusal coverage for unsupported backend, ineligible geometry, and unsuitable shard mesh transport. |

### Phase C6: validation, security, and rollout hardening

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-196` | [#3305](https://github.com/OpenAgentsInc/openagents/issues/3305) | Closed | Add cluster validation, fault-injection, and performance gates | docs/tests/validation plus cluster crates | Landed in `7124eefd7`: Psionic now ships a reusable cluster validation matrix, restart/rejoin transport coverage, fault-injected recovery/scheduling/replication/sharding tests, a release benchmark gate script, and an operator runbook so cluster claims stay repeatable and evidence-backed. |
| `PSI-197` | [#3306](https://github.com/OpenAgentsInc/openagents/issues/3306) | Closed | Harden cluster trust beyond the first LAN scope | `psionic-cluster`, security/docs | Landed in `d424ab1cf`: Psionic now exposes machine-checkable trust posture, authenticated configured-peer membership, signed control-plane messages, replay protection, and runbook-backed validation for widened operator-managed cluster posture without retroactively claiming internet-wide safety. |

### Phase D1: operator-managed multi-subnet follow-on

These issues remain outside the completed first trusted-cluster scope. They are
the next honest queue if Psionic widens from operator-managed configured peers
toward a more operationally robust multi-subnet substrate.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-198` | [#3307](https://github.com/OpenAgentsInc/openagents/issues/3307) | Closed | Add operator cluster manifest and trust-bundle digests | `psionic-cluster`, security/docs | Landed in `011e0452c`: Psionic now persists cluster rollout inputs as a `ClusterOperatorManifest` with a stable digest, JSON load/store, manifest-derived config, and manifest-backed authenticated transport coverage. |
| `PSI-199` | [#3308](https://github.com/OpenAgentsInc/openagents/issues/3308) | Closed | Add tamper-evident catchup and snapshot envelopes | `psionic-cluster`, ordered-state/tests | Landed in `87d428e43`: Psionic now signs recovery payloads, verifies cluster/requester/signer identity and recovery digests, rejects replayed envelopes, and documents the signed recovery drill in the validation runbook. |
| `PSI-200` | [#3309](https://github.com/OpenAgentsInc/openagents/issues/3309) | Closed | Add explicit multi-subnet peer dial policy and health truth | `psionic-cluster`, transport/docs | Landed in `86a2c920a`: Psionic now exposes configured-peer dial policy in the trust surface, tracks per-peer reachability and backoff truth, and validates degraded-to-reachable transitions in transport tests and the runbook. |
| `PSI-201` | [#3310](https://github.com/OpenAgentsInc/openagents/issues/3310) | Closed | Add membership key rotation and rollout diagnostics | `psionic-cluster`, security/docs | Landed in `ac9dd2285`: Psionic now exposes trust-bundle version overlap, previous-key rotation windows, and rollout diagnostics for accepted overlap and stale-bundle refusal, with operator runbook coverage for both rotation and drift detection. |

### Phase D2: coordinator authority and failover follow-on

These issues are the next honest queue for operator-managed multi-subnet
clusters now that manifest, recovery, dial-health, and rotation truth exist on
`main`.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-202` | [#3311](https://github.com/OpenAgentsInc/openagents/issues/3311) | Closed | Add coordinator lease policy and stale-leader diagnostics | `psionic-cluster`, ordered-state/tests/docs | Landed in `1e65c56c9`: coordinator leadership now carries explicit lease policy and heartbeat ticks, `ClusterState` exposes effective-versus-stale leadership queries plus stale-leader diagnostics, snapshot digests now reflect lease turnover, and the operator runbook now has a coordinator lease drill. |
| `PSI-203` | [#3312](https://github.com/OpenAgentsInc/openagents/issues/3312) | Closed | Add vote ledger and split-brain refusal semantics | `psionic-cluster`, ordered-state/tests | Landed in `ddc092cbb`: Psionic now has a reusable multi-term election ledger, deterministic refusal of conflicting vote grants and conflicting same-term leader heartbeats, and an authoritative-state guard that rejects conflicting same-term `LeadershipReconciled` events instead of silently changing leaders. |
| `PSI-204` | [#3313](https://github.com/OpenAgentsInc/openagents/issues/3313) | Closed | Add failover fencing tokens and commit authority truth | `psionic-cluster`, `psionic-runtime`, `psionic-provider`, docs | Landed in `313fbdc25`: Psionic now derives stable coordinator fence tokens and authority digests from ordered leadership truth, threads commit-authority evidence through runtime/provider execution context, and attaches authority digests to whole-request and sharded schedules so stale coordinators cannot look current after failover. |
| `PSI-205` | [#3314](https://github.com/OpenAgentsInc/openagents/issues/3314) | Closed | Add coordinator failover validation drills and runbook gates | docs/tests/validation plus cluster crates | Landed in `4732fbc26`: `cluster_validation_matrix` now covers stale-leader diagnostics, split-brain refusal, and failover fence rotation, while the operator runbook now has an explicit coordinator failover drill and exit gate for fenced coordinator claims. |

### Phase D3: command authorization and payout provenance follow-on

These issues are the next honest queue for operator-managed multi-subnet
clusters now that signed transport and coordinator authority truth exist on
`main`, but command authorization and payout-grade provenance still do not.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-206` | [#3315](https://github.com/OpenAgentsInc/openagents/issues/3315) | Closed | Add typed cluster command authorization policy and refusal diagnostics | `psionic-cluster`, ordered-state/tests | Landed in `e6888aaa0`: `ordered_state` now exposes typed command authority scopes, operator-managed authorization policy digests, explicit coordinator override, stable authorization facts, and machine-checkable refusal codes with coverage for coordinator-only, self-node, link-peer, and membership-status-gated command submission. |
| `PSI-207` | [#3316](https://github.com/OpenAgentsInc/openagents/issues/3316) | Closed | Preserve command provenance through authoritative cluster events | `psionic-cluster`, recovery/tests | Landed in `7b7b681f7`: `IndexedClusterEvent`, `ClusterSnapshot`, and `ClusterState` now retain command-authorization provenance for the current authoritative facts they expose, while compaction, catchup, and snapshot recovery preserve that provenance and the new replay/recovery tests prove it survives state rebuilds. |
| `PSI-208` | [#3317](https://github.com/OpenAgentsInc/openagents/issues/3317) | Closed | Extend cluster execution and settlement evidence with command provenance truth | `psionic-runtime`, `psionic-provider`, `psionic-cluster` | Landed in `24dd4aee8`: `ClusterExecutionContext` and settlement-linkage inputs now retain bounded command/admission provenance, whole-request and sharded cluster planners now emit that truth from authoritative membership/residency/leadership facts, and provider receipts now serialize payout-facing cluster provenance for audit or later dispute handling. |
| `PSI-209` | [#3318](https://github.com/OpenAgentsInc/openagents/issues/3318) | Closed | Add cluster authorization and payout-provenance validation gates | docs/tests/validation plus cluster crates | Landed in `715539147`: `cluster_validation_matrix` now covers allowed versus refused command flows plus whole-request and sharded payout-provenance surfaces, while the cluster validation runbook now defines the authorization/payout provenance drill and exit gate for stronger audit or dispute claims. |

### Phase D4: compute-market trust hardening follow-on

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-210` | [#3319](https://github.com/OpenAgentsInc/openagents/issues/3319) | Closed | Define compute-market trust posture and refusal diagnostics | `psionic-cluster`, docs/tests | Landed in `37fb246f1`: `ClusterTrustPolicy` now derives a stable `ClusterComputeMarketTrustAssessment` with explicit refusal reasons for current non-market-safe trust postures and the remaining D4 hardening gaps. |
| `PSI-211` | [#3320](https://github.com/OpenAgentsInc/openagents/issues/3320) | Closed | Add signed cluster evidence bundle export | `psionic-runtime`, `psionic-provider`, `psionic-cluster` | Landed in `4a21d6947`: Psionic now has stable `ClusterEvidenceBundlePayload` and `SignedClusterEvidenceBundle` types, receipt-export helpers in `psionic-provider`, and cluster-identity verification against control-plane signing keys. |
| `PSI-212` | [#3321](https://github.com/OpenAgentsInc/openagents/issues/3321) | Closed | Add attested node-identity admission seams | `psionic-cluster`, docs/tests | Landed in `d0f3e7891`: Psionic now has explicit attested configured-peer posture, persisted node-attestation evidence, configured-peer attestation requirements, and machine-checkable refusal diagnostics for missing or mismatched attestation during market-facing cluster admission. |
| `PSI-213` | [#3322](https://github.com/OpenAgentsInc/openagents/issues/3322) | Closed | Add non-LAN discovery posture diagnostics | `psionic-cluster`, docs/tests | Landed in `b0601f662`: Psionic now carries explicit `ClusterDiscoveryPosture`, a stable `ClusterNonLanDiscoveryAssessment`, config/node helpers that report current discovery truth, and validation coverage that keeps LAN-only, operator-managed configured-peer, and explicitly requested-but-unimplemented wider-network discovery claims machine-checkably bounded. |

### Phase E1: wider-network discovery substrate follow-on

This queue is now landed on `main`. It replaced the earlier explicit discovery
refusal boundary with bounded wider-network discovery truth without turning the
discovery substrate into the cluster control plane itself.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-214` | [#3323](https://github.com/OpenAgentsInc/openagents/issues/3323) | Closed | Add signed cluster introduction envelopes and policy digests | `psionic-cluster`, docs/tests | Landed in `1102bffa4`: Psionic now has `ClusterDiscoveryCandidate`, signed `SignedClusterIntroductionEnvelope`, explicit `ClusterIntroductionPolicy` digests, verification/refusal diagnostics for untrusted or malformed introduction artifacts, and manifest/config surfaces that keep future wider-network introductions separate from admitted membership truth. |
| `PSI-215` | [#3324](https://github.com/OpenAgentsInc/openagents/issues/3324) | Closed | Add bounded discovery-candidate ledger and admission reconciliation | `psionic-cluster`, ordered-state/tests | Landed in `47410298a`: Psionic now keeps `ClusterDiscoveredCandidateRecord` state and provenance separate from admitted membership, exposes deterministic candidate status transitions for introduced/accepted/refused/expired discovery truth, and replays, compacts, and recovers explicit candidate admission into membership without silently widening cluster membership. |
| `PSI-216` | [#3325](https://github.com/OpenAgentsInc/openagents/issues/3325) | Closed | Add wider-network discovery validation drills and rollout gates | docs/tests/validation plus cluster crates | Landed in `7c0f34503`: `cluster_validation_matrix` now carries an explicit wider-network discovery gate covering signed introduction intake, untrusted-source refusal, expiry, and admission reconciliation, while `CLUSTER_VALIDATION_RUNBOOK.md` now defines the wider-network discovery drill and rollout boundary before any broader discovery claims. |

### Phase F1: optional Exo interoperability follow-on

This is now the active post-E1 queue. It must remain bounded: Exo may inform
placement, but Psionic must keep final scheduling authority, execution, and
evidence truth.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-217` | [#3329](https://github.com/OpenAgentsInc/openagents/issues/3329) | Closed | Add bounded Exo placement-hint adapter for remote scheduling | `psionic-cluster`, runtime evidence/tests | Landed in `7aa76a2a9`: Psionic now has a bounded `ExoPlacementHint` seam that can bias tie-breaking among already-eligible whole-request candidates, surfaces accepted or ignored hint diagnostics in selection notes and runtime cluster evidence, and retains final placement authority plus eligibility truth inside Psionic-owned scheduling and receipts. |
| `PSI-218` | [#3330](https://github.com/OpenAgentsInc/openagents/issues/3330) | Closed | Make an explicit keep/discard decision on optional Exo interoperability | docs/tests plus cluster crates | Landed in [`EXO_INTEROPERABILITY_DECISION.md`](./EXO_INTEROPERABILITY_DECISION.md): the repo now explicitly keeps only the bounded `ExoPlacementHint` seam from `#3329` and discards the broader Exo orchestrator bridge, required runtime dependency, and execution delegation story. |

### Phase F2: post-Metal communication-class eligibility follow-on

This queue is now landed on `main`. The Metal roadmap queue
`#3286` -> `#3285` -> `#3269` -> `#3262` still blocks any honest Apple cluster
claim; what landed here is the explicit eligibility contract and refusal
surface, not Metal cluster readiness.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-219` | [#3331](https://github.com/OpenAgentsInc/openagents/issues/3331) | Closed | Add communication-class eligibility and keep Apple cluster refusal explicit | `psionic-cluster`, runtime/provider evidence/tests | Landed in `98dc1bdc3`: `psionic-runtime` now carries explicit cluster communication-class eligibility evidence, whole-request and replica lanes now retain backend communication truth in receipts/evidence, sharded planners now refuse by required communication class instead of by backend label alone, and current Metal cluster execution remains explicitly refused with diagnostics pointing at the still-open Metal roadmap gate. |

### Phase G1: cluster benchmark receipt follow-on

This queue is now landed on `main`. It closed the remaining gap between the
landed benchmark gates and the roadmap's requirement that cluster performance
claims be tied to explicit machine-checkable benchmark receipts.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-220` | [#3332](https://github.com/OpenAgentsInc/openagents/issues/3332) | Closed | Add typed cluster benchmark receipts and gate JSON schema | `psionic-cluster`, tests | Landed in `4f64525b4`: `psionic-cluster` now exposes typed `ClusterBenchmarkReceipt` models plus topology/recovery benchmark contexts and stable digest helpers, while the benchmark gates now emit receipt-shaped JSON instead of anonymous summary blobs and release-gate artifacts now preserve benchmark identity, budget truth, context, and pass/fail outcome. |
| `PSI-221` | [#3334](https://github.com/OpenAgentsInc/openagents/issues/3334) | Closed | Wire cluster benchmark gate script and outputs to typed receipts | `psionic-cluster`, scripts/docs | Landed in `a524658b8`: the cluster benchmark gate script now documents typed benchmark receipts instead of generic summaries, validates the stable receipt filenames and core schema fields after the release gate runs, emits explicit receipt artifact paths for CI and operator consumers, and the runbook now points at receipt artifacts rather than anonymous summary JSON. |
| `PSI-222` | [#3333](https://github.com/OpenAgentsInc/openagents/issues/3333) | Closed | Add benchmark receipt validation drill and roadmap closeout | docs/tests/validation plus cluster crates | Landed in `3fe872c96`: `CLUSTER_VALIDATION_RUNBOOK.md` now defines an explicit benchmark receipt drill with exact commands, expected receipt files, and failure interpretation, while this roadmap now closes the G1 queue explicitly instead of leaving typed benchmark receipts as an open-ended follow-on note. |

### Phase H1: declared cluster capability-profile follow-on

This queue is now landed on `main`. It closes the remaining post-G1 gap between
typed cluster execution evidence, planner eligibility, and operator-facing
validation by making declared capability profiles authoritative for clustered
lanes and by adding an explicit validation drill for those claims.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-223` | [#3335](https://github.com/OpenAgentsInc/openagents/issues/3335) | Closed | Add runtime-owned cluster execution capability profile | `psionic-runtime`, tests | Landed in `37183c6cb`: `psionic-runtime` now exposes typed `ClusterExecutionCapabilityProfile` and `ClusterExecutionLane` models, stable profile digests, and profile-derived `ClusterCommunicationEligibility` helpers so clustered lane support can be declared explicitly instead of starting from backend-name heuristics alone. |
| `PSI-224` | [#3336](https://github.com/OpenAgentsInc/openagents/issues/3336) | Closed | Make cluster planners consume declared execution capability profiles | `psionic-cluster`, `psionic-runtime`, `psionic-provider` | Landed in `9aad9af8d`: whole-request, replicated, layer-sharded, and tensor-sharded planners now consume declared `ClusterExecutionCapabilityProfile` truth instead of widening lane support from backend labels; `ClusterCommunicationEligibility` now carries the stable capability-profile digest it was derived from; and provider/runtime evidence surfaces now preserve that declared-profile digest alongside cluster execution context. |
| `PSI-225` | [#3337](https://github.com/OpenAgentsInc/openagents/issues/3337) | Closed | Add capability-profile validation drill and roadmap closeout | docs/tests/validation plus cluster crates | Landed in `efa52005e`: `CLUSTER_VALIDATION_RUNBOOK.md` now defines an explicit capability-profile drill with exact runtime, cluster, and provider commands plus failure interpretation, and this roadmap now closes the H1 queue explicitly instead of leaving declared capability-profile validation as a vague follow-on. |

### Phase H2: advertised capability-profile publication follow-on

This is the current active post-H1 queue. It closes the next remaining gap
between declared clustered-lane truth and provider-side publication by making
advertised capability surfaces publish declared cluster execution capability
profiles before any request is planned.

| Local ID | GitHub | State | Issue | Scope | Why it exists |
| --- | --- | --- | --- | --- | --- |
| `PSI-226` | [#3341](https://github.com/OpenAgentsInc/openagents/issues/3341) | Closed | Publish declared cluster execution capability profiles in runtime capability surfaces | `psionic-runtime`, `psionic-provider`, tests | Landed in `9ebb90a3e`: `BackendSelection` now exposes an optional advertised `cluster_execution_capability_profile`, capability-side runtime/provider serialization now round-trips that declared truth before any request executes, and provider tests keep that advertised profile distinct from realized `cluster_execution` evidence. |
| `PSI-227` | [#3339](https://github.com/OpenAgentsInc/openagents/issues/3339) | Open | Thread advertised cluster capability profiles through provider capability envelopes | `psionic-provider`, `psionic-cluster`, tests | The capability-side model is not enough unless provider capability exports actually populate it and keep it aligned with the planner-side declared profiles for remote, replicated, and sharded lanes. |
| `PSI-228` | [#3340](https://github.com/OpenAgentsInc/openagents/issues/3340) | Open | Add advertised capability-profile validation drill and roadmap closeout | docs/tests/validation plus cluster crates | Once advertised profiles publish through capability surfaces, the runbook and roadmap need an explicit operator drill and closeout so those new claims are repeatable instead of implicit. |

## Recommended Order

The shortest honest path from today's `main` is:

1. Treat C1 through C6 as landed on `main`, with the first trusted-cluster
   scope closing in `d424ab1cf`.
2. Treat D1 as landed on `main`, with the operator-managed multi-subnet follow-
   on queue closing in `ac9dd2285`.
3. Treat D2 as landed on `main`, with the coordinator-authority follow-on queue
   closing in `4732fbc26`.
4. Treat D3 as landed on `main`, with the authorization and payout-provenance
   queue closing in `715539147`.
5. Treat D4 as landed on `main`, with the compute-market hardening queue now
   closing in `b0601f662`.
6. Treat E1 as landed on `main`, with the wider-network discovery queue now
   closing in `7c0f34503`.
7. Treat the former local CUDA truth gate `#3276` -> `#3288` -> `#3248` as
   closed on `main`, and treat F1 as landed on `main`.
8. Treat F2 as landed on `main` in `98dc1bdc3`: communication-class
   eligibility is now explicit, and current Metal nodes remain refused for
   cluster execution while the Metal roadmap queue stays open.
9. Treat G1 as landed on `main` in `3fe872c96`, so benchmark-backed
   performance claims now have typed receipts, a script-level output contract,
   and an operator drill instead of a vague follow-on gap.
10. Treat H1 as landed on `main`, with the capability-profile validation drill
    and queue closeout now anchored in `efa52005e`.
11. Work the rest of H2 in order: land `#3339`, then `#3340`, now that
    `#3341` has made declared cluster capability truth machine-checkable on
    capability-side runtime/provider models before any request executes.
12. Keep current authenticated configured-peer posture explicit and bounded;
   it is operator-managed, not market-safe.
13. If stronger trust or wider network claims are needed beyond H2, open a new
    GitHub-backed queue instead of extending this roadmap with local placeholders.
14. Keep current Metal GPT-OSS nodes refused for cluster execution until the
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
