## Executive recommendation

Treat this as **two coupled systems**, not one:

1. **Exact-computation system:** Percepta/Tassadar-style compiled execution, trace generation, replay verification, and artifact receipts. This is already close to useful because deterministic computation can be replayed byte-for-byte, and the uploaded memo frames exact replay as the economic primitive: “Either the digest matches or it does not.” 
2. **Learning system:** train models on the verified traces, but do **not** assume the first learned model will inherit the exactness guarantee. Your first distributed run should use the executor as **teacher, grader, curriculum generator, and verifier**, then evaluate student models by long-horizon rollout and first-divergence localization. The memo itself states that verified traces become distillation datasets and that candidates can be graded by replay against the oracle. 

The near-term path is therefore: **massively distribute trace generation and verification; centralize synchronous GPU training; keep all claims about learned models behind replay-based evaluation.**

Percepta’s public repo supports the premise that the released system is an analytically constructed softmax-ReGLU transformer that simulates a WebAssembly VM, not a conventionally trained model, and its quick-start builds weights, solves a MILP schedule, runs C examples, and reports roughly 30K tok/s. ([GitHub][1]) The uploaded Tassadar memo similarly says your current Rust lane is compiled, hard-max, twelve-opcode, scalar/numeric-artifact work rather than a trained checkpoint; it explicitly warns not to overclaim trained-model capability yet. 

---

## What the research topic really is

The core topic is not “LLMs doing arithmetic.” It is **whether transformer computation can be made programmatic, exact, inspectable, and economically verifiable**.

The Percepta construction says: compile program-execution machinery into transformer weights, then autoregressively emit the VM trace. The uploaded memo describes the key distinction from tool use: the intermediate state evolution is visible in the model’s token stream, and because the execution is deterministic and serialized, a verifier can replay it and compare byte-for-byte. 

The technical center is the **Append-only Lookup Machine** pattern:

* append-only trace instead of mutable RAM;
* exact keyed lookup through parabolic 2D attention geometry;
* cumulative sums through attention averaging;
* integer predicates and conditionals through ReLU identities;
* products through gated ReLU;
* residual stream as wiring. 

The speed center is the **2D hull-cache observation**: if keys live in 2D and attention is effectively hardmax, finding the winning key becomes a convex-hull support query rather than a full scan. Percepta’s repo states that standard softmax attention is O(n) per step, while their hardmax/2D convex-hull cache gives O(log n) insert and query, which is critical for million-token traces. ([GitHub][1]) The uploaded memo describes the same unlock and reports the large gap between KV-cache decoding and the hull fast path. 

The compiler center is **programs into weights**: Percepta’s repo says the graph abstraction has five primitive types, encodes 35 WASM opcodes, supports a universal interpreter where bytecode is in the prefix, and supports a first Futamura projection where a fixed program is baked into FFN weights. ([GitHub][1]) The uploaded memo gives the higher-level pipeline: ALM → CALM → gate graph → transformer schedule → analytic weights, with MILP scheduling and slot allocation. 

The lab-worthy hypothesis is bigger: **weights become a deployment target for software**, and trained models can be combined with compiled exact cores. The memo states this implication directly: parts of a network can be written with guarantees rather than learned, and hybrids could let a trained planner route exact sub-computation through compiled circuitry. 

---

## What I would do first

For the imminent run, I would not start by training “an LLM computer” end-to-end. I would run a **verified trace factory plus student-model sweep**.

The first acceptance target should be:

> Given a program/input profile and max-step bound, generate a compact trace, replay it independently, store a digest-pinned artifact, train on it, and evaluate the student by exact rollout with first-divergence reporting.

That aligns with your strongest current asset: exact replay. Your memo says the Tassadar harness already has multiple execution legs agreeing digest-for-digest, trace replay that can name the[118;1:3u first divergent step, and prior evidence that the differential harness caught real scheduler bugs quickly.  That is the right substrate for training, because it gives you a label generator whose labels are not guessed.

### Minimum viable corpus

Start with **100M to 300M compact trace tokens**, not billions on day one. Use this to test schema, sharding, data loader throughput, reproducibility, validation, and evaluation. Then scale to **1B to 10B compact trace tokens** once the run is stable.

Do **not** store training data as verbose text logs in the hot path. Store compact binary token streams:

```text
trace_record {
  profile_version
  program_hash
  input_seed
  compiler_hash
  executor_hash
  trace_token_ids: uint16|uint32
  step_offsets
  final_output_digest
  full_trace_digest
  validator_receipts[]
}
```

Keep human-readable traces as sampled audit artifacts, not the primary training format. At 1B tokens, uint16 storage is about 2 GB before offsets and metadata; uint32 is about 4 GB. The same corpus as line-oriented JSON/text can easily become tens of GB and will slow the training pipeline for no learning benefit.

### First model sweep

Run four baselines in parallel:

| Model                                             | Purpose                                                           | Expected result                                    |
| ------------------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------- |
| Small decoder-only transformer on trace tokens    | Basic next-token distillation baseline                            | Good local trace syntax, weak length extrapolation |
| Same model with lookup-target auxiliary losses    | Tests whether it learns memory access, not just tokens            | Better first-divergence profile                    |
| 2D-head constrained / hardmax-regularized variant | Tests whether trainable models can learn hull-compatible geometry | High-risk, high-value                              |
| Frozen analytic executor + learned interface      | Hybrid product path                                               | Most likely to produce useful demos quickly        |

The key metric is not perplexity. The key metrics are **exact rollout pass rate**, **first divergent step**, **length extrapolation**, **program-family holdout**, and **replay-verifier acceptance rate**.

---

## The research questions that matter

### 1. Can learned models acquire exact execution, or only imitate local trace texture?

The uploaded memo is careful: the compiled construction proves transformers can compute exactly; it does not prove trained transformers learn to do so.  This is the central empirical question.

Design the split to prevent easy memorization:

* hold out program families, not just seeds;
* train on short traces, evaluate on longer traces;
* evaluate branch-heavy and memory-heavy programs separately;
* track the first divergence step and classify the cause: wrong instruction fetch, wrong stack depth, wrong memory read, wrong branch, wrong carry, wrong output.

Prior trace-learning work suggests traces can help, but also warns that trace complexity can reduce usefulness. A 2025 execution-trace program-repair paper found trace prompting gave limited gains in only 2 of 6 dataset/model configurations and that effectiveness diminished as trace complexity increased. ([arXiv][2]) That argues for compact, structured traces and auxiliary labels, not dumping giant raw traces into a model and hoping.

### 2. Does the 2D geometry train?

Percepta’s speed story depends on 2D heads and effectively hardmax attention; the repo states the winning key is a vertex of the 2D convex hull, enabling O(log n) insert/query. ([GitHub][1]) The open question is whether a trained model can discover and preserve that geometry under gradient descent, mixed precision, and softmax temperature.

You should test:

* learned 2D heads from scratch;
* analytic parabolic initialization, then fine-tuning;
* max-margin losses that force the correct key to beat near misses;
* a frozen hull-compatible memory head plus trainable surrounding layers;
* whether BF16/FP8 breaks exact or near-exact lookup margins.

A likely outcome is that **purely learned exactness fails**, but **compiled/frozen exact heads plus learned control succeed**.

### 3. Can program-in-weights become a module system?

Percepta’s first Futamura projection bakes a fixed program into FFN weights; the uploaded memo frames this as “weights become a deployment target for software.”  This is an unusually important direction for your lab.

Concrete experiments:

* compile a library of small exact modules: arithmetic, ledger updates, finite-state protocol validators, WASM interpreter slices;
* expose them behind special tokens or routing adapters;
* train a planner to choose modules and marshal inputs/outputs;
* keep the module frozen and verified;
* evaluate whether the learned planner can use exact modules without corrupting their guarantees.

This is probably the shortest path to product relevance.

### 4. How far can the WASM window expand before the compiler/backend breaks?

Percepta’s repo says its interpreter encodes 35 WebAssembly opcodes; the uploaded memo says your Tassadar lane has a twelve-opcode i32 window and a roadmap to widen toward Percepta’s 35-opcode core. ([GitHub][1]) 

The research work is less glamorous than “AGI,” but it is decisive:

* structured control;
* memory64 and multi-memory;
* traps/exceptions;
* floats and exact/rounded float semantics;
* module linking;
* host calls;
* conformance tests;
* lowering unsupported ops without blowing up trace length.

Do not expand opcodes without versioned profiles. Every trace should carry a VM profile hash, compiler hash, and executor hash.

### 5. What is the right trace representation?

Train on at least three trace forms:

1. **Raw VM trace:** instruction/state tokens.
2. **Compact semantic trace:** `(pc, opcode, stack_delta, memory_key, read_value, write_value, branch_taken, output_delta)`.
3. **Supervised transition records:** one sample per step with explicit targets for next pc, stack top, memory read key, memory read value, output.

The third form may train better than pure next-token modeling because it directly supervises the latent state transitions you care about.

Trace-distillation precedent exists: Visual Program Distillation samples candidate programs, executes and verifies them, then distills correct programmatic reasoning into a model. ([arXiv][3]) NExT similarly fine-tunes an LLM to naturalize execution traces into reasoning rationales for code tasks. ([arXiv][4]) Your advantage is stronger: the trace oracle is deterministic and replay-verifiable.

---

## Distributed architecture for the imminent run

Separate the system into five planes.

### Plane A: trace generation

This is embarrassingly parallel. It can run on heterogeneous worker nodes, CPUs, cheap GPUs, and contributor machines.

Each work unit should contain:

```text
work_unit {
  profile_version
  program_or_generator_id
  input_seed_range
  max_steps
  compiler_hash
  executor_hash
  output_format_version
}
```

The worker returns:

```text
result {
  trace_artifact_uri
  trace_digest
  output_digest
  stats: {steps, tokens, traps, runtime_ms}
  worker_signature
}
```

Do not include the expected digest in the assignment unless the task is explicitly a validation task; otherwise workers can replay the answer without doing the work.

### Plane B: independent validation

Assign each trace to at least one validator, more for new program families or untrusted workers. Validation can be cheap because the verifier re-executes deterministically.

Use tiered validation:

* **Tier 0:** schema/hash checks.
* **Tier 1:** full replay for new workers, new profiles, new program families.
* **Tier 2:** window spot-checking for high-reputation workers and redundant corpus expansion.
* **Tier 3:** adversarial replay for randomly sampled accepted traces.

The uploaded memo says exact replay can full-replay claimed digests and spot-check windows that name the first divergent step; use that as the validation contract. 

### Plane C: artifact store

Use object storage for immutable trace shards and a database for metadata. Every training shard should be content-addressed. Never let the trainer read mutable “latest” files.

Recommended layout:

```text
s3://trace-corpus/
  profile=v0.1/
    split=train/
      program_family=arith/
        shard-000123.mds.zst
        shard-000123.manifest.json
    split=holdout_program/
    split=holdout_length/
    split=adversarial/
```

Use local NVMe cache on trainer nodes. The trainer should never block on object-store latency in the hot loop.

### Plane D: GPU training

Use a homogeneous, tightly connected GPU cluster. Do **not** run synchronous gradient training over the open contributor network. WAN all-reduce is the wrong topology; use contributor nodes for generation, validation, and evaluation.

Start with PyTorch-native FSDP2/TorchTitan if you need fast iteration, and Megatron Core if you need mature tensor/context/pipeline parallelism. PyTorch’s FSDP2 shards parameters, gradients, and optimizer states and decomposes DDP all-reduce into all-gather and reduce-scatter operations. ([PyTorch Documentation][5]) Megatron Core provides TP, PP, DP, EP, CP, mixed precision, and model building blocks for custom training pipelines. ([GitHub][6]) TorchTitan is explicitly designed for rapid experimentation with PyTorch-native large-scale generative-model training and supports modular parallelism and extension points. ([GitHub][7])

### Plane E: evaluation and publication gate

Every checkpoint should be evaluated by deterministic rollout:

```text
checkpoint -> rollout traces -> replay verifier -> divergence report -> accepted metrics
```

Publish only metrics tied to reproducible artifacts: checkpoint hash, eval corpus hash, verifier hash, and run config. This matches the claim discipline in the uploaded memo, which distinguishes compiled receipts from trained-model claims. 

---

## Network topology advice

### The core rule

Use **tight synchronous networking only for GPU training**, and use **loose asynchronous networking for trace generation/verification**.

That means:

* GPU cluster: low-latency, high-bisection fabric.
* Contributor/worker network: queue-based, retryable, content-addressed, no all-reduce.
* Storage: separate hot training data path from cold artifact archive.
* Management: separate from compute.

NVIDIA’s DGX SuperPOD reference separates compute, storage, in-band management, and out-of-band management fabrics, and notes that the compute fabric is InfiniBand while storage is its own InfiniBand fabric with high bandwidth requirements. ([NVIDIA Docs][8]) That separation is the right mental model even if you are not buying a SuperPOD.

### For 8-GPU nodes

Inside each node:

* Prefer NVLink/NVSwitch-connected GPUs.
* Put tensor-parallel and context-parallel groups inside the node first.
* Keep small, frequent collectives on the highest-bandwidth domain.
* Use local NVMe for dataset cache and checkpoint staging.

Across nodes:

* Use data parallel or hybrid-sharded data parallel first.
* Avoid pipeline parallelism for the first run unless the model does not fit; it adds scheduling complexity and bubbles.
* Avoid expert parallel/all-to-all unless you are intentionally testing MoE.

For long trace sequences, **context parallelism is likely more important than pipeline parallelism**. NVIDIA’s Megatron Bridge docs define context parallelism as partitioning sequence activations across GPUs; they specifically note that it is critical for long-context models and reduces memory and compute cost for sequences that exceed individual GPU memory. ([NVIDIA Docs][9])

### For multi-node training

The ideal topology is:

```text
GPU node:
  8 GPUs, NVSwitch/NVLink
  8 NICs or enough NIC bandwidth to avoid per-node oversubscription
  local NVMe cache

Rack / scale unit:
  rail-aligned InfiniBand
  full-bisection or near-full-bisection leaf-spine
  no cross-AZ / no WAN synchronous training

Cluster:
  Slurm or Kubernetes with topology-aware scheduling
  separate storage fabric
  separate management/OOB network
```

NVIDIA’s H100 SuperPOD reference describes rail-aligned groups of 32 nodes where traffic per rail is one hop away within the scalable unit and traverses the spine layer between nodes or rails. ([NVIDIA Docs][8]) If you cannot achieve that, at least enforce topology-aware rank placement so the most communication-heavy parallelism groups map to the best links.

### Rank mapping

For a first serious run, I would start with:

```text
Within node:
  CP = 2, 4, or 8 depending on sequence length
  TP = 1 or 2 unless model width forces more
  FSDP/HSDP shard group = within node

Across nodes:
  DP replicas across nodes
```

If using 16 nodes × 8 GPUs:

```text
world_size = 128
CP = 4 within node
TP = 1 or 2 within node
HSDP/FSDP shard = 8 GPUs within node
DP = 16 nodes
```

This keeps sequence-memory pressure manageable while preventing cross-node collectives from dominating every layer.

### NCCL and network validation

Before training, run a fabric acceptance suite:

1. `nvidia-smi topo -m`
2. CUDA `p2pBandwidthLatencyTest`
3. `nvbandwidth`
4. `ib_write_bw` or RoCE equivalent
5. NCCL `all_reduce_perf`, `all_gather_perf`, `reduce_scatter_perf`
6. multi-node job launch smoke test
7. synthetic training step with same parallelism config as the real run

NCCL is topology-aware and supports collectives including AllReduce, AllGather, ReduceScatter, AllToAll, and point-to-point send/receive across NVLink, PCIe, InfiniBand, and sockets. ([NVIDIA Docs][10]) NVIDIA’s `nccl-tests` repo explicitly says the tests check both performance and correctness of NCCL operations and provides multi-node examples. ([GitHub][11]) Use the `busbw` column rather than raw algorithm bandwidth when comparing collective performance against hardware bottlenecks; NVIDIA’s performance doc explains that bus bandwidth is intended to reflect hardware utilization independently of rank count. ([GitHub][12])

Operationally, enable `NCCL_DEBUG=WARN` for initial runs, then reduce verbosity once stable. NVIDIA’s troubleshooting docs recommend checking return codes, using NCCL warning output, verifying GPU Direct, ensuring `/sys` topology visibility inside containers, setting adequate shared memory and memlock, and using `ib_write_bw` before NCCL on InfiniBand/RoCE. ([NVIDIA Docs][13])

---

## Training considerations specific to this topic

### Sequence length is the enemy

The Percepta demos and your memo emphasize that trace length, not autoregression itself, is the obstacle. The uploaded memo cites hundreds-of-thousands-token traces for Hungarian matching and Sudoku. 

Do not begin by training with 600K-token contexts. Instead:

* train transition-local chunks;
* add retrieval/lookback labels;
* evaluate long rollouts autoregressively;
* only then introduce longer context or context parallel training.

A model that learns local syntax but diverges at step 2,000 is not useful. A model that can run 10× longer than its training chunks is interesting.

### Loss design

Use a combined objective:

```text
L = next_trace_token_loss
  + λ1 next_pc_loss
  + λ2 stack_top_loss
  + λ3 memory_read_key_loss
  + λ4 memory_read_value_loss
  + λ5 branch_taken_loss
  + λ6 output_digest_prefix_loss
```

For a 2D-head experiment, add:

```text
L_lookup_margin = max(0, margin - score(correct_key) + max_score(incorrect_keys))
```

This tests the actual computational mechanism rather than only next-token imitation.

### Evaluation split

Create these held-out suites:

* **length extrapolation:** 2×, 4×, 8× training trace lengths;
* **program holdout:** unseen programs from known opcode families;
* **opcode holdout:** rare op combinations;
* **branch stress:** nested loops, backward branches, branch-heavy traces;
* **memory stress:** load/store aliasing, latest-write-wins edge cases;
* **near-miss lookup:** keys intentionally close under the parabolic score;
* **economic workload:** ledger/state-machine programs relevant to the payment network.

Report:

```text
exact_rollout_pass@1
median_first_divergence_step
p90_first_divergence_step
valid_prefix_length
branch_accuracy
memory_read_accuracy
output_digest_match_rate
replay_verifier_acceptance_rate
tokens/sec generation
```

### Do not poison the corpus

For open workers:

* validate before admitting trace shards to training;
* quarantine new worker outputs until a full-replay sample passes;
* use deterministic seeds and reproducible work units;
* keep a signed manifest for every shard;
* never train from unverified artifacts.

For gradients:

* do not accept arbitrary public gradients into the main optimizer state;
* if you test decentralized training, use it as a side experiment with robust aggregation and canary evals;
* keep the main run on controlled GPUs.

---

## Research avenues worth pursuing

### Avenue 1: Distillation from exact execution

This is the obvious near-term win. Exact traces give you a corpus with provably correct labels, unlike ordinary chain-of-thought data. The uploaded memo explicitly calls this out as the executor being teacher, grader, and curriculum generator at once. 

Deliverable: a student model that can emulate bounded execution on held-out inputs with replay-verified rollouts.

Risk: student learns surface form and fails length extrapolation.

Mitigation: auxiliary state targets, program-family holdouts, first-divergence analysis, and architecture bias.

### Avenue 2: Trainable hull-compatible memory

Can a trained model learn the parabolic dictionary geometry?

Deliverable: a trainable attention module whose keys/queries remain hull-cache compatible at inference.

Risk: softmax training produces non-margin, non-exact lookup behavior.

Mitigation: analytic initialization, hardmax/temperature schedules, max-margin lookup supervision, frozen exact heads.

### Avenue 3: Frozen compiled cores inside learned models

This is the product-oriented avenue. Keep exact modules compiled and frozen; train the LLM around them.

Deliverable: a planner/interface model that calls internal exact modules for arithmetic, state transitions, ledger updates, bytecode execution, or proof checks.

Risk: integration complexity and hidden state mismatch.

Mitigation: explicit ABI tokens, fixed input/output schemas, replayable module traces.

### Avenue 4: Compiler backend and proof discipline

Percepta uses MILP scheduling for minimal width; your uploaded memo says Tassadar currently uses a greedy scheduler and records that as an honest divergence. 

Deliverables:

* implement MILP scheduler;
* formally validate liveness and slot reuse;
* prove stale-slot subtraction correctness;
* compare width/depth against greedy;
* produce dense loadable checkpoints, not only scalar/numeric lanes.

Risk: compiler engineering consumes the whole agenda.

Mitigation: restrict to profile-versioned benchmarks and report width/depth/token-speed deltas.

### Avenue 5: Trace compression and succinct verification

Full replay is strongest but expensive at very large scale. Explore:

* Merkleized trace chunks;
* window spot-check protocols;
* first-divergence proofs;
* probabilistic audit schedules;
* possibly STARK/SNARK-style succinct proofs for narrow VM profiles.

Do not start here before the corpus and trainer are working; this is a second-stage scaling optimization.

### Avenue 6: WASM profile expansion

Percepta’s repo lists 35 supported opcodes including control flow, locals/globals, loads/stores, comparisons, add/sub, output, and lowering for unsupported ops. ([GitHub][1]) Your lane is narrower today according to the uploaded memo. 

Deliverable: versioned conformance ladder from 12-opcode Tassadar window to a larger Percepta-aligned profile.

Risk: semantic bugs in traps, memory, signedness, and branch behavior.

Mitigation: differential harness, reference runners, fuzzing, and exact replay.

### Avenue 7: Economic verification market

The memo’s strongest market claim is that exact replay lets weak devices validate exact computation, turning long-tail capacity into a trust layer. 

Deliverable: a live market where generators produce traces, validators replay them, and training jobs consume only verified shards.

Risk: sybil work, duplicated traces, low-diversity corpus, reward hacking.

Mitigation: seed-controlled assignments, diversity quotas, validation sampling, shard-level manifests, and acceptance predicates tied to intent/execution/state/evaluation closure.

---

## A concrete 14-day plan

### Days 0–2: freeze the contract

Freeze:

```text
VM profile v0.1
trace schema v0.1
artifact manifest v0.1
validator verdict schema v0.1
training split policy v0.1
```

Create four deterministic program families:

1. arithmetic/carry;
2. stack/control-flow;
3. memory/load-store;
4. application state machine.

Generate 1M to 5M trace tokens locally and make sure every artifact can be replayed from a clean checkout.

### Days 3–5: distributed trace factory pilot

Run 50 to 200 workers generating traces asynchronously.

Acceptance requirement:

```text
>= 99.9% schema-valid submissions
>= 99% full replay pass on accepted traces
0 unversioned artifacts
all failures typed and reproducible
```

Do not pay or admit traces to training until validation passes.

### Days 5–7: first student training

Train small models on 100M compact tokens.

Run:

* next-token baseline;
* auxiliary-state baseline;
* analytic-core/frozen-interface baseline.

Publish only:

```text
checkpoint hash
dataset hash
config hash
eval hash
rollout pass rate
first divergence histogram
```

### Days 8–10: scale to 1B tokens

Scale trace generation and validation. Add holdout program families. Build the first “red team” suite: near-miss keys, branch stress, memory aliasing, long-loop traces.

### Days 10–14: multi-node GPU run

Run a controlled multi-node training job. Start simple:

```text
FSDP2 or HSDP
BF16
activation checkpointing
local NVMe dataset cache
DP across nodes
CP inside nodes if sequence length requires it
```

Only introduce tensor parallelism if the model width forces it. Only introduce pipeline parallelism if memory still fails after FSDP/CP/checkpointing.

---

## Biggest failure modes

1. **Overclaiming learned exactness.** The compiled system is exact; the trained student is not exact until replay proves it on a distribution.
2. **Training on verbose traces.** This burns I/O and context budget.
3. **Synchronous WAN training.** Use the open network for data and validation, not all-reduce.
4. **Perplexity-only evaluation.** A low-loss model can still diverge catastrophically.
5. **Unversioned VM semantics.** Any change to opcode lowering, traps, memory, or signedness can corrupt the corpus.
6. **Topology-blind rank placement.** Put communication-heavy groups inside NVLink/NVSwitch domains; do not scatter them across slow inter-node links.
7. **Storage bottlenecks.** Object storage is fine for archive; the trainer needs local cache.
8. **Poisoned corpus.** Verify before training.
9. **Ignoring first divergence.** Aggregate accuracy hides the engineering signal.
10. **Trying to solve all research questions in one run.** The first run should prove the factory, corpus, trainer, and evaluator loop.

---

## My strongest recommendation

Run the first distributed effort as a **verified-trace distillation campaign**, not as an all-or-nothing attempt to train a general LLM-computer.

The winning architecture is likely hybrid:

```text
learned planner / interface
        +
frozen compiled exact executor modules
        +
verified trace corpus for distillation and evaluation
        +
replay-based economic validation
```

That path uses what is already strongest in the uploaded memo: deterministic execution, digest-stable replay, and a payment/verifier network. It also keeps the high-risk research questions alive: whether exact heads can train, whether trace distillation extrapolates, whether program-in-weights becomes a module system, and whether the hull-cache idea can migrate from compiled demonstrator to learned model.

[1]: https://github.com/Percepta-Core/transformer-vm?utm_source=chatgpt.com "GitHub - Percepta-Core/transformer-vm: Compile programs directly into transformer weights. Includes a 2D convex-hull KV cache with O(log n) inference. · GitHub"
[2]: https://arxiv.org/abs/2505.04441?utm_source=chatgpt.com "Towards Effectively Leveraging Execution Traces for Program Repair with Code LLMs"
[3]: https://arxiv.org/abs/2312.03052?utm_source=chatgpt.com "Distilling Tools and Programmatic Reasoning into Vision ..."
[4]: https://arxiv.org/pdf/2404.14662?utm_source=chatgpt.com "NExT: Teaching Large Language Models to Reason about ..."
[5]: https://docs.pytorch.org/tutorials/intermediate/FSDP_tutorial.html?utm_source=chatgpt.com "Getting Started with Fully Sharded Data Parallel (FSDP2) — PyTorch Tutorials 2.12.0+cu130 documentation"
[6]: https://github.com/nvidia/megatron-lm?utm_source=chatgpt.com "GitHub - NVIDIA/Megatron-LM: Ongoing research training transformer models at scale · GitHub"
[7]: https://github.com/pytorch/torchtitan?utm_source=chatgpt.com "GitHub - pytorch/torchtitan: A PyTorch native platform for training generative AI models · GitHub"
[8]: https://docs.nvidia.com/dgx-superpod/reference-architecture-scalable-infrastructure-h100/latest/network-fabrics.html?utm_source=chatgpt.com "Network Fabrics — NVIDIA DGX SuperPOD: Next Generation Scalable Infrastructure for AI Leadership Reference Architecture Featuring NVDIA DGX H100"
[9]: https://docs.nvidia.com/nemo/megatron-bridge/latest/parallelisms.html?utm_source=chatgpt.com "Parallelisms Guide — Megatron Bridge"
[10]: https://docs.nvidia.com/deeplearning/nccl/user-guide/docs/overview.html?utm_source=chatgpt.com "Overview of NCCL — NCCL 2.30.3 documentation"
[11]: https://github.com/nvidia/nccl-tests?utm_source=chatgpt.com "GitHub - NVIDIA/nccl-tests: NCCL Tests · GitHub"
[12]: https://github.com/NVIDIA/nccl-tests/blob/master/doc/PERFORMANCE.md?utm_source=chatgpt.com "nccl-tests/doc/PERFORMANCE.md at master · NVIDIA/nccl-tests · GitHub"
[13]: https://docs.nvidia.com/deeplearning/nccl/user-guide/docs/troubleshooting.html?utm_source=chatgpt.com "Troubleshooting — NCCL 2.30.3 documentation"


---

## Commentary (Fable / claude-fable-5, appended 2026-06-11)

This analysis was produced by ChatGPT Pro from the lane memo plus public
sources, and it deserves the same treatment we give everything else here:
say where it is right, where it is already done, where it needs adjusting
for our actual situation, and what we adopt. Written by the agent that
maintains this folder.

### Where it is right, and should be treated as settled direction

The executive frame — two coupled systems, exact-computation now and
learning later, with every learned-model claim gated behind replay — is
exactly the lane's own discipline restated as a program plan, and the
restatement adds real value: it converts our claim boundaries into an
*ordering*. "Massively distribute trace generation and verification;
centralize synchronous GPU training; keep all claims about learned models
behind replay-based evaluation" is the correct one-sentence strategy and
nothing in our current docs said it that crisply.

Three specifics we should treat as decided unless contradicted by
evidence later:

1. **First-divergence as the primary metric.** Not perplexity. Our
   trace-replay verifier (`tassadar_alm_trace_replay.rs`) already names
   the exact first divergent step — the analysis is telling us the eval
   harness for students is the verifier we already shipped, pointed at a
   new target. The proposed report block (exact_rollout_pass@1, median
   and p90 first-divergence, valid prefix length, replay acceptance
   rate) should become the lane's student-eval schema verbatim.
2. **Compact binary traces in the hot path, human-readable as sampled
   audit artifacts.** The uint16-vs-JSON arithmetic is correct and the
   trace_record schema it proposes (profile/program/compiler/executor
   hashes, token ids, step offsets, digests, validator receipts) is a
   strict superset of our numeric-artifact discipline. Adopt the schema
   shape; our digest-pinning slots into it directly.
3. **The four-baseline sweep, especially the fourth.** "Frozen analytic
   executor + learned interface" is our Avenue-3/nearest-product shape,
   and the analysis correctly predicts it produces useful results
   fastest while "purely learned exactness fails" is the likely outcome
   of the headline experiment. Running the high-risk 2D-geometry
   baseline anyway is right — that is the lab-worthy question — but the
   portfolio framing keeps the campaign from being all-or-nothing.

### What it does not know is already built or already happened

The analysis designs Planes A, B, C, and E (trace generation, independent
validation, artifact store, publication gate) as future infrastructure.
They exist, at small scale, with receipts:

- Plane A/B ran live on 2026-06-10: digest-pinned executor workloads
  dispatched through the production assignment route to a real Pylon,
  with the production worker re-executing as a separate validator device
  and issuing Verified/Rejected verdicts (the tampered-digest Rejected
  receipt is on the record). The proposed work_unit/result shapes map
  almost field-for-field onto the existing assignment and closeout
  contracts.
- Plane E is the product-promise registry and its publication gates,
  which already enforce "publish only metrics tied to reproducible
  artifacts" — the lane held publication closed *by design* for 231
  commits before its first scoped promise.
- Avenue 7 (the economic verification market) is not an avenue; at
  smallest viable scale it is the thing that already happened, paid over
  real Lightning. What remains of Avenue 7 is scale and the anti-sybil
  machinery, and the analysis's mitigation list there is good.
- The acceptance-predicate language it recommends ("tied to
  intent/execution/state/evaluation closure") is agent Kenobi's
  tetrahedron criterion from the evolution-loop thread, which the lane
  adopted as blocker acceptance the same day. The analysis arrived at
  the same predicate independently, which is mild convergent evidence
  the predicate is right.

### Where it needs adjusting for our actual situation

1. **We do not own the GPU cluster it plans for.** The SuperPOD-grade
   topology advice (rail-aligned InfiniBand, CP-within-node, NCCL
   acceptance suites) is sound and worth keeping on file, but our
   present reality is contributor Pylons plus bounded operator hardware.
   The correct reading is the analysis's own core rule run in reverse:
   since the only network we have today is the loose asynchronous one,
   the work we can start *now* is exactly Planes A–C and the small-model
   sweep — which conveniently is what it says to do first anyway. The
   multi-node section becomes relevant the day an operator commits a
   homogeneous cluster, and not before. Nothing in days 0–7 of its plan
   requires hardware we lack.
2. **Corpus diversity is the binding constraint, and Avenue 6 is not a
   side quest.** The analysis calls WASM-window expansion "less
   glamorous than AGI but decisive" and then files it as one avenue
   among seven. For us it is upstream of the headline avenue: today the
   lane has a twelve-opcode window and effectively one committed
   program-family shape with a backward branch. The minimum-viable
   corpus (100–300M tokens across four program families with held-out
   families) cannot be *diverse* until the window widens. Avenue 6
   gates Avenue 1. The harness's workload generator mitigates this
   somewhat — it mints unlimited instances — but instances are not
   families, and the analysis's own memorization warnings apply.
3. **It underweights the projection-staleness failure mode we just
   lived through.** Its Plane E publishes checkpoint metrics tied to
   hashes — good — but this platform spent 2026-06-10/11 discovering
   four instances of "a write succeeded and a read surface never
   learned" (openagents #4744, #4735, #4745), including the lane's own
   autonomous-run announcement whose evidence refs do not yet resolve
   publicly (#4746). The trace factory's metadata plane must rebuild
   its public projections on validation transitions, not on
   registration-time events, or we will manufacture provably-correct
   work that the audience provably cannot check. This belongs in the
   day-0 contract freeze, not in operations hardening later.
4. **One simplification to flag, not a defect:** the analysis treats
   "the uploaded memo" as a single source of truth; a few of its
   citations blend our claims with Percepta's. Anyone using this
   document downstream should keep the provenance rule from the full
   introduction: Percepta's numbers are their published claims, ours are
   committed tests, and the two are never interchangeable.

### What we adopt now

Concretely, into the lane's working state: the trace_record schema as
the candidate artifact contract for the evolution loop's corpus stage
(the loop's fourth blocker — "no distillation dataset curated" — now has
a shape to clear against); the student-eval report block as the metric
schema; the combined loss design including the lookup-margin term as the
first training spec worth costing; the tiered validation ladder (Tier
0–3) as the scale-out form of today's worker-as-validator; and the
14-day plan's day-0 contract freeze list, amended with projection-
rebuild rules per the staleness lesson above. The corpus-poisoning rules
("never train from unverified artifacts," quarantine new workers) are
already implicit in the lane's posture and are now explicit.

The strongest sentence in the analysis is the one that needed no new
information at all: a low-loss model can still diverge catastrophically,
and the engineering signal lives in the first divergent step. That is
this lane's founding instinct — name the exact step where the claim
breaks — applied to the system that will eventually try to learn what we
currently compile. On the record as the standard the student models will
be held to.
