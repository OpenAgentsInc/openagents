# Psionic Serving Topology Acceptance Matrix

> Status: canonical `PSI-255` / `#3560` closure doc, updated 2026-03-14 after
> landing the executable matrix runner in
> `scripts/release/check-psionic-topology-acceptance-matrix.sh`.

This document turns Psionic's current serving-topology claims into one explicit,
operator-runnable acceptance matrix.

It is intentionally narrower than `ROADMAP.md`: it does not widen Psionic's
claims. It states exactly which local and clustered topology classes are
currently supported, which are explicitly refused, and which remain unsupported.

This matrix is about `crates/psionic/*` only. It does not change MVP product
authority in `docs/MVP.md`, and it does not move product, wallet, or market
authority out of the owners defined in `docs/OWNERSHIP.md`.

## Canonical Runner

Run the full matrix from the repo root:

```bash
scripts/release/check-psionic-topology-acceptance-matrix.sh
```

Targeted entrypoints:

```bash
scripts/release/check-psionic-topology-acceptance-matrix.sh --local-only
scripts/release/check-psionic-topology-acceptance-matrix.sh --cluster-only
scripts/release/check-psionic-topology-acceptance-matrix.sh --only replicated
scripts/release/check-psionic-topology-acceptance-matrix.sh --only tensor
scripts/release/check-psionic-topology-acceptance-matrix.sh --include-benchmarks
```

Supported suites:

- `local-baseline`
- `routing-cache`
- `pd-modes`
- `contracts`
- `whole-request`
- `replicated`
- `pipeline`
- `layer`
- `tensor`
- `unsupported`
- `benchmarks`

`benchmarks` is optional because `PSI-256` owns the broader benchmark-program
split. `PSI-255` only requires executable correctness and capability matrices.

## Status Legend

- `Supported`: Psionic has a planner or runtime path, explicit capability
  declaration, evidence or receipt truth, and a machine-runnable validation
  suite.
- `Supported with bounds`: Psionic supports the class only under explicit cache,
  transport, or topology pinning constraints that are validated and surfaced in
  receipts.
- `Refused`: Psionic has an explicit machine-checkable refusal path for the
  combination and must not silently downgrade into an implied claim.
- `Unsupported`: Psionic does not declare the class in its serving lanes,
  topology kinds, or provider receipts today.

## Canonical Mapping

The matrix uses these current codebase mappings:

- `DP` means clustered replica routing:
  `ClusterExecutionLane::ReplicaRouted` and
  `ExecutionTopologyKind::Replicated`
- `PP` means public-network pipeline-parallel serving:
  `ClusterExecutionLane::PipelineSharded` and
  `ExecutionTopologyKind::PipelineSharded`
- `Layer-sharded` remains a distinct current cluster lane:
  `ClusterExecutionLane::LayerSharded` and
  `ExecutionTopologyKind::LayerSharded`
- `TP` means tensor-parallel serving:
  `ClusterExecutionLane::TensorSharded` and
  `ExecutionTopologyKind::TensorSharded`
- `PD` means the explicit prefill/decode modes in
  `PrefillDecodeExecutionMode`
- `EP` is currently unsupported for serving because there is no
  `ClusterExecutionLane::ExpertParallel` and no
  `ExecutionTopologyKind::ExpertParallel` in the current serving contracts

## Topology Matrix

| Class | Local serving | Clustered serving | PD mode(s) | Cache truth | Routing truth | Artifact truth | Current status | Validation suites |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Single-runtime baseline | Supported | Not applicable | `disaggregated_colocated` inside one runtime owner | request-owned paged KV plus shared-prefix cache with tenancy and sampler boundaries | explicit router headers and warm/cache route selection even on one worker fleet surface | local delivery proof, execution-plan digest, cold vs warm provenance | Supported | `local-baseline`, `routing-cache`, `pd-modes` |
| `DP` / replica-routed | Unsupported as a local topology | Supported | `disaggregated_colocated` on the winning replica | replica-local prefix and KV reuse; invalidates on route change | warm-aware and cache-aware replica selection; route pinned | served artifact must already be resident or scheduling must refuse/mark staging truth explicitly | Supported with bounds | `routing-cache`, `contracts`, `whole-request`, `replicated` |
| `PP` / pipeline-sharded | Unsupported locally | Supported | `disaggregated_kv_transfer` | topology-pinned cache truth across stage topology | stage order, handoff latency envelope, and public-network route facts are explicit | handoff links and stage topology must stay honest; sharded evidence is surfaced | Supported with bounds | `pd-modes`, `contracts`, `pipeline` |
| Layer-sharded | Unsupported locally | Supported | `disaggregated_kv_transfer` | topology-pinned stage-local cache truth | shard placement and handoff-link quality are explicit | shard manifests, artifact residency, and handoff evidence are validated | Supported with bounds | `pd-modes`, `contracts`, `layer` |
| `TP` / tensor-sharded | Unsupported locally | Supported | `disaggregated_kv_transfer` | topology-pinned collective cache truth | collective-mesh transport and topology pinning are explicit | tensor-shard manifest alignment and mesh suitability are validated | Supported with bounds | `pd-modes`, `contracts`, `tensor` |
| `EP` / expert-parallel | Unsupported | Unsupported | none | none | none | none | Unsupported | `unsupported` |

## PD Mode Matrix

`PSI-255` also requires prefill/decode execution to be explicit across local and
clustered serving rather than implied by scheduler internals.

| PD mode | Local lanes | Cluster lanes | Status | Validation |
| --- | --- | --- | --- | --- |
| `disaggregated_colocated` | local continuous-batching CPU and GPT-OSS serving surfaces | `RemoteWholeRequest`, `ReplicaRouted` | Supported | `local-baseline`, `pd-modes`, `replicated` |
| `disaggregated_kv_transfer` | Unsupported locally | `PipelineSharded`, `LayerSharded`, `TensorSharded` | Supported with bounds | `pd-modes`, `pipeline`, `layer`, `tensor` |
| `disaggregated_kv_transfer` on `RemoteWholeRequest` | Not applicable | explicit refusal | Refused | `pd-modes` |

## Unsupported And Refused Combinations

These combinations must stay explicit:

- Expert-parallel serving is unsupported.
  Rationale: current serving topology kinds and clustered serving lanes stop at
  replica, pipeline, layer-sharded, and tensor-sharded execution. Psionic does
  have MoE model execution work, but it does not yet expose an expert-parallel
  serving planner, route policy, cache contract, or provider receipt shape.
- Local `DP`, `PP`, layer-sharded, and `TP` are unsupported.
  Rationale: local Psionic serving is still one runtime owner per request path,
  with explicit local PD support but no shipped multi-device local planner.
- Clustered Metal dispatch remains refused.
  Rationale: the current cluster planners still point at the active Metal queue
  `#3286` -> `#3285` -> `#3269` -> `#3262` and must not silently claim cluster
  execution on Metal until that queue closes.
- Route-stable cache reuse across replica changes is refused.
  Rationale: replica-routed cache truth is explicitly replica-local and
  invalidates when route identity changes.
- Topology-stable cache reuse across pipeline, layer-sharded, and tensor-sharded
  changes is refused.
  Rationale: sharded cache truth is topology-pinned; topology changes invalidate
  warm reuse rather than silently carrying cache claims across a new mesh.

## Suite-to-Behavior Mapping

| Suite | What it proves |
| --- | --- |
| `local-baseline` | local continuous batching, paged KV ownership, prefix-cache hit/miss and refusal truth, and local cold/warm artifact provenance |
| `routing-cache` | cache-aware and warm-aware routing, tenant-safe cache affinity, and machine-checkable route/prefix headers |
| `pd-modes` | local `disaggregated_colocated` reporting, explicit capability-profile PD derivation, and refusal of unsupported clustered PD combinations |
| `contracts` | stable capability-profile and manifest contracts for replicated/layer/tensor topologies |
| `whole-request` | remote whole-request staging truth, degraded-candidate truth, and settlement-facing provenance preservation |
| `replicated` | `DP` warm-replica selection, route-pinned cache truth, and replica execution receipts |
| `pipeline` | `PP` stage topology, public-network handoff truth, and pipeline execution receipts |
| `layer` | layer-sharded handoff quality, bounded degraded handoff truth, and layer-sharded receipts |
| `tensor` | `TP` collective mesh truth, tensor-range evidence, and tensor-sharded receipts |
| `unsupported` | explicit refusal boundaries that keep unsupported cluster claims visible instead of implicit |
| `benchmarks` | optional ignored release gates for the currently supported clustered topology lanes |

## Definition Of Done For This Matrix

This matrix remains truthful only if all of the following stay true:

- every supported row has at least one runnable suite in the canonical script
- every supported row links capability/profile truth to execution or receipt
  truth rather than only to planner output
- cache, routing, and artifact identity checks remain part of the supported-row
  suites where those concepts matter
- unsupported or refused rows remain explicit in docs and in runnable refusal
  checks where a refusal path exists

If a future topology claim does not fit this matrix, add a new row, a new suite
in the canonical script, and a new refusal or support rationale before widening
any claim in `README.md`, `ROADMAP.md`, or provider capability publication.
