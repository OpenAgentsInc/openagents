# Psionic and Pylon

Status: historical cross-repository source map, 2026-09-19. Product and runtime
claims below belong to the reviewed repositories at that time. This checkout
does not gain their fleet, wallet, or earn behavior by retaining the survey.
Use the [master roadmap](roadmap.md) and [agent labor plan](agents/market-infrastructure.md)
for current integration priorities.

A map of the Psionic machine-learning stack and the Pylon node product: what
each one is, which repository owns it, and where the files live. Surveyed
2026-09-19 across `~/work/psionic`, `~/work/openagents`, and `~/work/coder`.

## What each one is

**Psionic** is the Rust-native ML and inference stack. It owns the
machine-facing execution substrate behind local inference, serving, training,
distributed execution, artifact truth, and clustered compute. It lives in its
own repository at `~/work/psionic` (`OpenAgentsInc/psionic`), extracted from
this repository's earlier shape. Episode 216 introduced it publicly as the
effort to rebuild the Python and C++ ML ecosystem in Rust, starting from
llama.cpp-class inference and growing into model training.

**Pylon** is the OpenAgents node software: a compute miner that a person or
an agent runs on a machine to sell compute for Bitcoin. A Pylon bundles
Psionic for inference, embeddings, and training work, plus a self-custodial
Lightning wallet through MoneyDevKit, so a fresh node can earn as soon as it
comes online. The name is the StarCraft unit: you construct additional
pylons, and the network warps in around them. Episode 221 launched it as the
user-facing provider node; episode 237 ("You Must Construct Additional
Pylons") made it the home base inside every Autopilot install.

The two meet at a typed boundary. Psionic publishes the manifests, signed job
envelopes, worker receipts, payment decisions, and settlement-proof checks
that Pylon consumes. Pylon owns the node product, the wallet, the install
flow, and the public release. Psionic does not custody wallet secrets or
execute payments; Treasury or Nexus does, and Psionic validates the proofs
they return.

## Where Psionic lives

Repository: `~/work/psionic`, remote `OpenAgentsInc/psionic`. A standalone
Cargo workspace (`members = ["crates/psionic-*"]`) with 51 crates. Its agent
contract is `~/work/psionic/AGENTS.md`.

### Crate map

Grouped per `docs/WORKSPACE_MAP.md` plus the families added since:

| Group | Crates | What they own |
| --- | --- | --- |
| Framework core | `psionic-core`, `psionic-ir`, `psionic-compiler`, `psionic-runtime`, `psionic-array`, `psionic-array-io`, `psionic-function-io` | Tensor, graph, runtime, and IO contracts. |
| Backend execution | `psionic-backend-cpu`, `psionic-backend-cuda`, `psionic-backend-metal`, `psionic-backend-amd-kfd`, `psionic-backend-amd-userspace`, `psionic-backend-tests` | Real backend behavior and backend-facing validation. |
| Neural network and transformer layers | `psionic-nn`, `psionic-nn-optimizers`, `psionic-transformer` | Layer and optimizer primitives above the backends. |
| Serving and model execution | `psionic-models`, `psionic-serve`, `psionic-provider`, `psionic-router`, `psionic-catalog` | Local server surfaces, model execution, routing, provider projection. `psionic-serve` ships `psionic-gpt-oss-server`, `psionic-mesh-lane`, and the generic OpenAI-compatible surface in `src/openai_http.rs`. |
| Cluster and distributed execution | `psionic-cluster`, `psionic-collectives`, `psionic-distributed`, `psionic-datastream`, `psionic-net`, `psionic-sandbox` | Topology, collectives, networking, sandboxing, distributed runtime truth. |
| Training, data, eval, optimization | `psionic-train`, `psionic-data`, `psionic-eval`, `psionic-adapters`, `psionic-optimize`, `psionic-environments`, `psionic-research` | Training contracts, datasets, evaluation, adapter lanes, environment packages, research lanes, bounded offline optimizer artifacts. |
| Apple and MLX surfaces | `psionic-apple-fm`, `psionic-mlx-compat`, `psionic-mlx-catalog`, `psionic-mlx-lm`, `psionic-mlx-serve`, `psionic-mlx-vlm`, `psionic-mlx-audio`, `psionic-mlx-bench`, `psionic-mlx-capi`, `psionic-mlx-recipes`, `psionic-mlx-workflows` | Apple-side model, serving, and MLX ecosystem integration. |
| Audio and speech | `psionic-csm-speech`, `psionic-vad` | Rust-only CSM speech generation and voice-activity detection. |
| Compatibility and research support | `psionic-compat`, `psionic-observe`, `psionic-tassadar-student` | Upstream compatibility shims, observability, and the Tassadar student-model lane. |

### Canonical docs

`~/work/psionic/docs/` holds about 400 documents. The ones that define the
system:

| Doc | Role |
| --- | --- |
| `README.md` | Entrypoint and track map. |
| `docs/ARCHITECTURE.md` | Canonical Psionic-wide system spec. |
| `docs/WORKSPACE_MAP.md` | Expanded crate, doc, and lane map. |
| `docs/INFERENCE_ENGINE.md` | Canonical inference and serving completion bar. |
| `docs/TRAIN_SYSTEM.md` | Canonical training subsystem spec. |
| `docs/FRAMEWORK_CORE_ACCEPTANCE_MATRIX.md` | Framework-core completion bar. |
| `docs/INFERENCE_MESH_OWNERSHIP.md` | Owner split for mesh identity, admission, routing, and management. |
| `docs/ROADMAP.md`, `docs/ROADMAP_CLUSTER.md`, `docs/ROADMAP_METAL.md`, `docs/ROADMAP_MLX.md`, `docs/ROADMAP_PARAMETERGOLF.md`, `docs/ROADMAP_TASSADAR.md` | Per-track roadmaps. |
| `docs/audits/` | Rationale and closeout records; not canonical current state. |

Psionic docs use a fixed status vocabulary: `implemented`,
`implemented_early`, `partial`, `partial_outside_psionic`, and `planned`.

### Main tracks

- Inference and local serving: local GPT-OSS server, generic
  OpenAI-compatible surface, bounded `qwen35`, `qwen38`, `gemma4`, and MedPsy
  lanes, a Gemma Metal lane on Apple Silicon, and the CSM speech lane.
- Hermes agent backend: Psionic behind the OpenAI-compatible
  `chat.completions` path (`docs/hermes/`).
- Parameter Golf: single-H100 and distributed `8xH100` training, submission,
  and evidence lanes.
- Cluster, swarm, and cross-provider compute: local mixed-hardware swarm,
  Google two-node swarm, sparse expert placement for `gemma4:26b`.
- Psion learned-model program: corpus, tokenizer, pretraining, trusted
  cluster, and decentralized contribution work.
- Tassadar executor lane: the exact-computation "LLM as computer" executor
  substrate, served evidence, and bounded public claim surfaces.

### Other top-level paths

| Path | What it holds |
| --- | --- |
| `fixtures/` | Committed evidence: run bundles, receipts, contract artifacts, and compatibility records. Treat as versioned substrate truth. Includes `fixtures/pylon/psionic/` (below). |
| `reports/` | Retained run and readiness reports. |
| `scripts/` | Repo-local checkers (`check-*.sh`), benchmark harnesses, and fixture generators (`build-*.sh`). |
| `TRAIN` | Operator entrypoint for the actual Psion pretraining lane. |
| `TRAIN_TASSADAR` | Wrapper that runs the `tassadar_train_operator` example. |
| `configs/legal/` | Legal-lane configuration. |
| `merge/legal-sft-round-001.json` | Retained merge descriptor for a Qwen legal SFT round. |
| `suites/` | Harvey-compatible benchmark task suites (`harvey_public_*.json`). |
| `tasks/synthetic/` | Synthetic task definitions, currently `legal-workflow-v1`. |
| `RELEASE_V0.2.0.md` | The `0.2.0` source boundary for the Pylon worker and scheduler path. |
| `V0.2_PYLON_RELEASE_AUDIT.md` | Readiness audit for that boundary. |

## Where Pylon lives

Pylon the product predates the current repository shapes. Its code shipped
inside the earlier OpenAgents monorepo — the same shape this repository's
`docs/transcripts/` archive records — and as the `@openagentsinc/pylon`
package that installs optional local inference. In the three repositories
surveyed here, no Pylon product source remains; what remains is the boundary
in `psionic`, the history in `docs/transcripts/`, and the vocabulary in
`coder`.

### The Psionic-side Pylon boundary

All of this lives in `~/work/psionic`:

| Path | What it does |
| --- | --- |
| `docs/PYLON_PSIONIC_MANIFESTS.md` | Canonical doc for the manifests Pylon consumes. Status `implemented_early`. |
| `crates/psionic-serve/src/pylon_release_manifest.rs` | Produces and validates the release and model-artifact manifests. Tests run with `cargo test -p psionic-serve pylon_manifest`. |
| `fixtures/pylon/psionic/` | The committed manifests: `release_manifest_{darwin_arm64,linux_x64,linux_arm64}_v0_3.json` for the `psionic-openai-server` binary, and `model_artifact_manifest_qwen35_{0_8b,2b}_q8_0_v0_3.json` for the two Qwen3.5 Q8_0 model rows. |
| `crates/psionic-train/src/qwen_legal_pylon_dispatch.rs` | The Qwen legal Pylon scheduler: `LocalOnly`, `Loopback`, `Tailnet`, and `Production` dispatch modes over signed Ed25519 job envelopes, node eligibility checks, and signed worker-receipt verification. |
| `crates/psionic-train/src/bin/qwen_legal_pylon_worker_server.rs` | The worker entrypoint: serves one signed job over TCP, writes outputs, signs the receipt, exits. |
| `crates/psionic-train/src/qwen_legal_training_placement.rs` | Places Qwen legal training jobs across admitted Pylon nodes. |
| `crates/psionic-train/src/pylon_launch_promise_gates.rs` | Projects the `psionic.pylon_launch_dashboard.v1` bundle separating GEPA live-import and Qwen Pylon training rows for Omega. |
| `crates/psionic-train/src/probe_gepa_rollout_coordinator.rs` | Imports live Omega/Pylon closeouts into the Probe GEPA rollout frontier; Pylon stays a metric-call rollout backend, not distributed training. |
| `RELEASE_V0.2.0.md` + `V0.2_PYLON_RELEASE_AUDIT.md` | The `0.2.0` boundary: signed dispatch, receipt verification, payment decisions, Treasury handoff batches, settlement-proof validation, telemetry, public-network contracts. |
| `scripts/check-v0.2-pylon-release.sh` | The executable boundary gate. |
| `scripts/check-qwen-legal-pylon-network-sft.sh` | Qwen legal Pylon network SFT fixture check. |
| `scripts/build-qwen35-08b-harvey-mfn-simulated-pylons-data.sh`, `scripts/check-qwen35-08b-harvey-mfn-simulated-pylons-run.sh` | Harvey MFN training data built as simulated Pylon worker outputs. |
| `docs/TRAIN_SYSTEM.md`, `docs/KHALA_M6_M7_COORDINATOR_PLAN.md`, `docs/QWEN_LEGAL_TRACKING_CLOSEOUT.md`, `docs/LEGAL_BENCHMARK_ENGINE.md`, `docs/PROBE_GEPA_ROLLOUT_COORDINATOR.md` | Subsystem specs that reference the Pylon lanes. |

The contract the boundary implements, in short:

1. A scheduler builds a `QwenLegalSignedJobEnvelope`, verifies its digest and
   its own Ed25519 signature, then dispatches over local, loopback, Tailnet,
   or production TCP.
2. The worker verifies the scheduler signature, runs the job, writes the
   expected outputs, signs the receipt, and returns it on the same
   connection.
3. The scheduler verifies the receipt digest and worker signature before
   marking the work payable.
4. Payable decisions generate Treasury handoff batches. Settlement proofs
   require a settlement time plus a payment hash or transaction proof;
   Psionic rejects duplicates, unknown authorizations, amount mismatches, bad
   digests, and proofs carrying secret material.

The inference-side manifests carry their own honesty bounds: the
`openagents.psionic.release_manifest.v0.3` and
`openagents.psionic.model_artifact_manifest.v0.3` schemas mark the rows
`inferenceOnly = true` with `trainingClaim` and `paidInferenceClaim` blocked,
and Pylon is expected to verify SHA-256 digests before placing any artifact
in its cache. Pylon is not expected to bundle Psionic binaries or weights.

### Pylon in the wider system

Recorded in `docs/transcripts/` and the workspace's own notes:

- **NIP-90 provider node.** A Pylon is a Nostr client and a NIP-90
  data-vending-machine service provider; work reaches it over Nostr job
  requests and settles in Bitcoin (episode 221).
- **Node software for the network.** Every Autopilot carried one and could
  construct more on any reachable machine; Tassadar ran on them as an
  indefinite Bitcoin-paid distributed training run (episode 237).
- **Khala dispatch target.** Khala, the OpenAI-compatible gateway, fans work
  out to models, tools, validators, and Pylon workers. The fleet dispatch
  path is Khala → Pylon → assignment, and `target_pylon_unavailable` is the
  failure when no eligible node answers (episodes 241, 244, 245).
- **Coding-capacity lane.** The same pipeline routed a user's own Codex and
  Claude capacity: a Pylon coding assignment flows to the caller's own coding
  agent under an own-capacity-only invariant (episodes 244, 245).
- **Verse object.** In the deleted three-dimensional Verse, a pylon was the
  world station standing for a serving node (`~/work/coder` glossary).

## Psionic inside the coder repository

`~/work/coder` (`OpenAgentsInc/coder`) runs its own Psionic program. It is a
separately scoped effort — the split-GPT-OSS inference program that executes
one request across layer stages on physical hosts — and its docs state
explicitly that they do not authorize deployment or claim changes in the
standalone Psionic repository.

| Path | What it is |
| --- | --- |
| `docs/psionic/` | The program docs: `README.md` (index), `architecture.md`, `stage-contract.md`, `ownership.md`, `roadmap.md` (P0–P8), `artifacts.md`, `hosts.md`, `numeric-policy.md`, `packages.md`, `wan.md`, `tokens-per-second.md`, and the hardware receipts (`baseline-receipt.md`, `mixed-20b-receipt.md`, `mixed-120b-receipt.md`, `gce-20b-receipt.md`, `gce-120b-receipt.md`, `p5-metal-receipt.md`, plus unfilled templates). `release-audit-0.5.0.md` grades what landed. |
| `crates/coder-stage` | The stage contract: `Stage` trait, session plans, activation frames, `CoordinatorCore` fencing. Model-independent. |
| `crates/coder-gptoss` | The GPT-OSS engine: GGUF manifest and digests, `CpuStage` reference executor, `MetalStage` (validated on Apple Silicon), `CudaStage`, `RocmStage` (built, no device run), package loader, `stage_budget`. |
| `bins/coder-inference-daemon` | The worker binary; `--engine stage` registers a stage offer and serves activations. |
| `bins/coder-fleet` | The fleet coordinator: registry, router, consumer endpoints, and the split coordinator under `--model-path`. |
| `bins/coder-fleet-adapter` | Adapter worker fronting OpenAI-compatible engines (vLLM, SGLang, llama.cpp server). |
| `ops/psionic-gce.sh`, `ops/psionic-amd.sh`, `ops/lib/psionic-host.sh` | Operator scripts for Compute Engine and AMD runs. |
| `ops/tests/psionic-gce.sh`, `ops/tests/psionic-amd.sh` | Their test harnesses. |
| `bench/psionic/wan/` | The WAN measurement bench behind `docs/psionic/tokens-per-second.md`. |
| `fleet/probes.toml`, `fleet/catalog.toml`, `docs/fleet/protocol-v1.md` | The verification-floor probes, the model catalog, and the worker protocol the split session rides. |
| `docs/GLOSSARY.md` | Current vocabulary: inference fleet, inference mesh, stage runtime, sharded session, stage worker, weight map. Historical section: Pylon, Tassadar, Khala, the Verse. |

The `coder` repo's plan proposes consolidating its fleet control with narrow
Psionic mesh contracts (`psionic-net`, `psionic-cluster`, `psionic-runtime`,
`psionic-models`, `psionic-serve` as boundary owners). `docs/psionic/
ownership.md` names which Coder code sits behind each proposed boundary
today. Extraction ownership is undecided and belongs to the standalone
Psionic maintainers.

## Psionic and Pylon inside this repository

This repository (`~/work/openagents`, `OpenAgentsInc/openagents`) holds no
Psionic or Pylon source. What it holds:

- `docs/transcripts/` — the retained episode archive. The index in
  `docs/transcripts/README.md` maps the arc: episodes 144 and 203 introduce
  Pylon and Pylon/Nexus compute coordination, episodes 216–217 introduce and
  benchmark Psionic, episode 221 launches Pylon as the provider node, and
  episodes 236–247 cover the Pylon v0.3/Tassadar launch, the Verse run board,
  Khala, and fleet delegation.
- No crate, script, or doc outside that archive implements or references the
  two systems. The `psionic` repository's own extraction note applies in
  reverse: do not pull code back from `psionic` unless asked.

## How the pieces fit

```text
Pylon node software                     Psionic (~/work/psionic)
┌──────────────────────────┐           ┌────────────────────────────┐
│ install + cache           │ consumes  │ release/model manifests     │
│ (verifies SHA-256)        │◄──────────│ (psionic-serve, fixtures/)  │
│ self-custodial wallet     │           │                             │
│ worker job loop           │ runs      │ qwen_legal_pylon_* signed   │
│ (NIP-90 / TCP worker)     │◄──────────│ job envelopes + receipts    │
└───────────┬──────────────┘           │ (psionic-train)             │
            │                          └──────────────┬─────────────┘
   Khala gateway fans work ──► assignment ──► verified receipt ──►
   payment decision ──► Treasury handoff ──► settlement proof check

Coder repo (~/work/coder): its own split-inference program under
docs/psionic/ proposes adopting narrow Psionic contracts; no code is shared
today.
```

## Cautions

- The transcripts are history, not spec. Nexus endpoints, package names, and
  launch claims in `docs/transcripts/` describe the earlier monorepo and are
  deprecated where they conflict with the current crates.
- `~/work/psionic` is open source, but its disclosure rules are strict:
  public surfaces use bounded claim language, and private roadmap wording
  does not cross into it. Keep that boundary when describing it elsewhere.
- The `coder` `docs/psionic/` program and the standalone `psionic` repository
  share a name and a direction, not code or status. Evidence in one does not
  certify the other.
