# Distributed LLM Training Architectures in Real Runs: DiLoCo, DisTrO/DeMo, SparseLoCo, and Conventional Parallelism

## Scope and terminology

This report focuses on **publicly documented** or otherwise well-described **distributed language-model training runs** (pre-training and, where clearly relevant, reinforcement-learning post-training) where the *distributed systems architecture* materially affects algorithm choice—especially **cross–data center / over-the-Internet / permissionless** training. That is the setting DiLoCo was designed for: training across “islands of devices that are poorly connected,” by communicating only every *many* local steps rather than every step.

Throughout, “architecture” refers to the combination of:

1. **Parallelism layout** (data parallelism, sharding, tensor/pipeline parallelism, etc.).
2. **Synchronization policy** (every step vs. periodic multi-step local training; synchronous vs. overlapped vs. fully asynchronous).
3. **Communication substrate** (NCCL all-reduce in a tightly coupled cluster vs. peer-to-peer overlays vs. object storage).
4. **Robustness & governance layer** (elastic membership, fault tolerance, and—when applicable—chain-based permissionless coordination and contribution scoring).

A concise summary is: **most large-scale LLM training still uses tightly coupled datacenter methods**, while DiLoCo and its descendants show up primarily when the goal is **low-bandwidth / high-latency / heterogeneous and failure-prone networks**. Prime Intellect uses DiLoCo most directly in public large-scale runs, while several other prominent systems are better understood as adjacent or derived approaches.

## Conventional distributed training as the dominant baseline

Most mainstream large-model training remains “conventional” in the sense that **gradients (or sharded equivalents) are synchronized essentially every optimization step**, and the stack is designed around **fast, low-latency interconnects** inside a datacenter (NVLink/InfiniBand/RDMA).

**DistributedDataParallel (DDP)** is the canonical synchronous baseline: each rank (process) owns a full replica of the model, processes its shard of data, and then uses collectives (commonly all-reduce) to make the replicas consistent after each step; PyTorch describes DDP as performing synchronous distributed training as a wrapper around a model.

**Fully Sharded Data Parallel (FSDP / FSDP2)** and **DeepSpeed ZeRO** are primarily *memory-scaling* architectures that still sit in the “frequent communication within a cluster” paradigm. PyTorch’s FSDP2 tutorial contrasts DDP (replicated model + all-reduce) with FSDP, emphasizing that FSDP reduces memory by **sharding parameters, gradients, and optimizer states** across ranks. DeepSpeed’s ZeRO similarly partitions optimizer states, gradients, and (at stage 3) parameters across data-parallel processes to remove redundancy.

When models become too large for pure data parallelism, datacenter training typically adds **model parallelism**:

- **Tensor (intra-layer) parallelism**, popularized by Megatron-LM, splits large matrix multiplications within transformer layers across GPUs; the Megatron-LM paper demonstrates training multi‑billion parameter language models with intra-layer model parallelism.
- **Pipeline parallelism** splits layers across stages; NVIDIA’s Megatron tutorial notes that tensor parallelism is orthogonal to pipeline parallelism and that very large models such as GPT‑3 use both.

**Why this dominates:** In well-provisioned clusters, frequent synchronization is acceptable because bandwidth and latency are engineered to make it efficient; the priority is usually maximizing throughput and keeping convergence behavior close to the “gold standard” fully synchronous baseline. The techniques above are also deeply integrated into production frameworks and vendor tooling, reinforcing their dominance.

## DiLoCo and direct DiLoCo-family runs

### The DiLoCo algorithm as the core idea

DeepMind’s **DiLoCo** is explicitly positioned as a response to the difficulties of co-locating and tightly synchronizing thousands of accelerators. It is framed as a **federated-averaging–style** method with two optimizers: large numbers of **inner steps** using **AdamW**, and an **outer optimizer** using **Nesterov momentum**, communicating only once per inner-window (hundreds or thousands of steps).

In the DiLoCo paper’s core experimental setup, they evaluate **60M, 150M, and 400M** decoder-only transformers and report that DiLoCo on **8 workers** matches synchronous training on C4 while communicating **500× less**.

The key architectural point is that DiLoCo makes “global training” look like **K loosely connected workers**, each of which may still use standard intra-node parallelism locally, but the global coupling is infrequent.

### OpenDiLoCo: replication + making DiLoCo practical on the Internet

**OpenDiLoCo** (Prime Intellect) positions itself as an open-source replication of DiLoCo, implemented inside a **decentralized training framework using Hivemind**, and demonstrates a run “across two continents and three countries” while maintaining **90–95% compute utilization**.

Architecturally, the paper is explicit about *why* Hivemind matters: the torch.distributed/NCCL implementation “cannot communicate across networks using NAT,” so the Hivemind implementation is used for real-world Internet settings.

OpenDiLoCo’s “hybrid within-worker / across-worker” pattern is a recurring theme in later decentralized systems: inside each worker, they use **PyTorch FSDP** to exploit fast local interconnects, while Hivemind manages the **low-bandwidth inter-node communication**.

They also report scaling experiments to **1.1B parameters** and provide details of H100-based infrastructure (e.g., each worker node has 8×H100 in some experiments).

### PRIME + INTELLECT-1: DiLoCo as an “outer layer” for real global pretraining

Prime Intellect’s **INTELLECT‑1** report is one of the clearest examples of DiLoCo-like architecture applied to a genuinely large pretraining run:

- A **10B parameter** model trained on **1T tokens**, using up to **14 concurrent nodes distributed across 3 continents**, with **dynamic joining/leaving** of compute providers.
- PRIME’s core abstractions include **ElasticDeviceMesh** to manage “dynamic global process groups for fault-tolerant communication across the internet and local process groups for communication within a node,” plus “a hybrid DiLoCo‑FSDP2 implementation.”
- They describe a practical cadence: nodes run for ~**38 minutes** before an all-reduce that takes **~1–7 minutes** depending on configuration, keeping communication to a small fraction of wall time.

INTELLECT‑1 adds an important layer beyond “plain DiLoCo”: **quantized communication kernels**. PRIME implements an **int8 quantized ring-all-reduce** (with fp32 accumulation) to reduce payload size, and explicitly notes that communicating int8 instead of fp32 is a **4× reduction** in payload.

They frame the broader tradeoff as combining (a) quantization and (b) outer synchronization “every 500 steps” in one description of bandwidth reduction levers—illustrating the general DiLoCo-family principle: **trade more local compute for less communication**.

### Streaming DiLoCo and other research extensions

Many “DiLoCo-derived” papers focus on addressing DiLoCo’s remaining bottleneck: even if you communicate infrequently, global sync often still requires transmitting **all parameters** at once, producing an intolerable peak bandwidth spike.

**Streaming DiLoCo with overlapping communication** addresses this by (1) synchronizing **subsets of parameters in sequence** to reduce peak bandwidth, (2) allowing workers to continue training while synchronizing (overlap), and (3) quantizing exchanged data. The abstract claims these modifications can reduce required bandwidth by **two orders of magnitude** while retaining quality on **billion-scale** training.

These works matter for “architecture comparison” because they show the DiLoCo line splitting into subfamilies:

- “Multi-iteration local training + outer sync” (core DiLoCo).
- “Multi-iteration local training + **streamed/fragmented** outer sync + overlap + quantization” (Streaming DiLoCo).

## Compression-first and DiLoCo-adjacent lines: DeMo/DisTrO, SparseLoCo, and pipeline-compressed hybrids

The second major branch of Internet-scale training architectures emphasizes **shrinking the communicated object** (compressing momentum/updates) and then building the *system* around that smaller message.

### DeMo: Decoupled Momentum Optimization as a low-bandwidth fused optimizer + DP algorithm

**DeMo (Decoupled Momentum Optimization)** proposes that synchronizing full gradients/optimizer state is unnecessary; instead it “decouples momentum updates,” applies a fast orthonormal transform (e.g., DCT), then top‑k sparsification, reusing the momentum buffer as error feedback.

The paper positions this as enabling training “even with limited network bandwidth and heterogeneous hardware,” i.e., targeting similar constraints as DiLoCo but by compressing what gets exchanged rather than (only) changing synchronization frequency.

### DisTrO / Psyche: engineering the network, governance, and overlap around compressed updates

Nous Research’s **Psyche Network Architecture** post explicitly describes Psyche as “building on DisTrO and its predecessor DeMo,” reducing data transfer by several orders of magnitude, with coordination on **Solana** smart contracts.

It lays out a concrete distributed-systems design with three actors—**Coordinator (on-chain)**, **Clients (GPU nodes)**, and a **Data Provider**—and an epoch-based lifecycle that supports on/off-boarding (helping accommodate contributors that come and go).

Two Psyche-specific architectural decisions are especially relevant when comparing to DiLoCo-style systems:

1. **Overlapped training:** Psyche describes a scheme where nodes train the next step while sharing results from the previous step, framing this as a path to making communication latency “not be a bottleneck” as DisTrO results scale sublinearly.
2. **Aggressive quantization:** Psyche claims further bandwidth reduction by quantizing DCT-transformed momentums, including a “send only the sign” variant (1‑bit sign information + indices).

These elements are “DiLoCo-adjacent” because they pursue the same end goal—Internet-scale training—but via a distinctly different *unit of synchronization* (compressed momentum/update results every step or window, rather than infrequent full pseudo-gradient sync).

### A concrete Psyche run: Consilience 40B

The **Consilience 40B** model card describes a 40B decoder-only transformer trained “in a decentralized fashion over the internet,” with checkpoints “updated every 500 training steps,” and explicitly lists the optimizer as a “decentralized version” of **DisTrO**.

This is one of the clearest primary sources tying a large-scale run directly to the DisTrO line rather than DiLoCo. Nous additionally highlights Consilience in later posts as their largest distributed pretraining run at that time.

### SparseLoCo: a DiLoCo-family method that fully embraces sparsification + quantization

**SparseLoCo** is a pivotal bridge between the DiLoCo line and the “compression-first” line.

The SparseLoCo paper frames the key limitation of DiLoCo as: even with infrequent sync, you often still communicate a “full copy of the model’s gradients,” creating a bottleneck. It proposes SparseLoCo, combining **error feedback + sparsification + 2-bit quantization** to reach **1–3% sparsity** while “outperforming full-precision DiLoCo,” arguing that outer momentum can be locally approximated by the error-feedback accumulator.

This is conceptually important for architecture comparisons:

- **Core DiLoCo:** “reduce communication frequency” (large local step windows).
- **DeMo/DisTrO:** “reduce message size” (compress momentum/updates).
- **SparseLoCo:** combines both—**infrequent communication** *and* **small sparse/quantized pseudo-gradients**, while simplifying the “outer momentum” concept.

### Heterogeneous low-bandwidth pipeline + SparseLoCo hybrids

A more recent line attacks a different bottleneck: even if cross-replica communication is cheap, **model parallelism inside a replica** can be bandwidth-limiting when contributors don’t have well-connected multi-GPU clusters.

“Heterogeneous Low‑Bandwidth Pre‑Training of LLMs” proposes mixing participants: some host full replicas on high-bandwidth interconnects, while resource-limited participants are grouped to form a replica via **pipeline parallelism with subspace-projected activation/activation‑gradient compression**, and then replicas are synchronized with SparseLoCo.

The paper reports experiments on **178M–1B** parameter models, suggesting a practical path to combining low-bandwidth *data-parallel* and low-bandwidth *model-parallel* techniques.

## Run-by-run map: which architecture was used, and why

The table below summarizes the most relevant **publicly documented** distributed training runs in the “DiLoCo vs alternatives” problem space, emphasizing *what they used* and *why that choice fits their environment*.

| Training run (public artifact) | Scale & setting | Architecture used | Why this architecture (per sources) |
|---|---:|---|---|
| **DiLoCo (original paper)** | 60M / 150M / 400M, C4; up to 8 workers | **DiLoCo**: large inner windows (AdamW) + outer Nesterov momentum; communicate every many inner steps; variant of FedAvg/FedOpt | Designed to train across “poorly connected” islands by communicating infrequently; reports **500× less** communication with comparable performance. |
| **OpenDiLoCo (Prime Intellect)** | Replication + scaling to 1.1B; run across 2 continents/3 countries | **DiLoCo + Hivemind** for Internet comms; **FSDP** inside workers | Hivemind avoids NAT limitations of NCCL/torch.distributed; targets practical decentralized training and reports **90–95% utilization** across countries. |
| **INTELLECT‑1 (Prime Intellect)** | **10B**, **1T tokens**, up to **14 nodes / 3 continents**, elastic participation | **Hybrid DiLoCo–FSDP2**, ElasticDeviceMesh, CPU-offloaded DiLoCo; **int8 ring all-reduce** for pseudo-gradients | Built for bandwidth constraints and node volatility; ElasticDeviceMesh manages dynamic process groups; quantized outer all-reduce reduces payload; long local compute phases reduce comm time. |
| **INTELLECT‑2 (Prime Intellect)** | **32B**, globally distributed **RL** run | **Fully asynchronous RL pipeline**: PRIME‑RL separation of training vs inference; SHARDCAST weight broadcast; TOPLOC verification; FSDP2 for training | Explicitly argues RL is more asynchronous and well suited to decentralized compute; decouples inference rollouts from centralized training; claims “no communication overhead” by overlapping weight broadcast with inference/training. |
| **Templar‑I / Gauntlet live 1.2B run** | **1.2B**, permissionless contributors, Bittensor incentives; 20k rounds | **Permissionless pseudo-gradient system** with **validator scoring** + staged filtering; DeMo-based pseudo-gradients (per paper) | Gauntlet is designed to make permissionless participation viable (filtering + loss-based scoring + OpenSkill rating). The paper says the evaluated instantiation uses **DeMo** to produce pseudo-gradients. |
| **Covenant‑72B (Templar / Bittensor SN3)** | **72B**, **1.1T tokens**, permissionless Internet training | **SparseLoCo + dynamic FSDP** within peers; Cloudflare R2 object storage as comms backbone; validator selects top updates; Bittensor coordination | SparseLoCo enables “heavily compressed” pseudo-gradients; object storage avoids requiring direct P2P connectivity and supports asynchronous uploads and scoring; dynamic FSDP shards local training and manages memory/offloading phases. |
| **Consilience 40B (Nous / Psyche)** | **40B**, decentralized over Internet; checkpoints each 500 steps | **DisTrO (decentralized)** in Psyche network; Solana-coordinated lifecycle; P2P networking | The model card names DisTrO as the optimizer and references Psyche as the live run; Psyche’s architecture emphasizes chain coordination, overlapped training, and strong bandwidth reduction via DCT/top‑k style compression and quantization. |
| **Streaming DiLoCo experiments (research)** | Billion-scale experiments (per abstract) | **Streaming/fragmented DiLoCo**: subset sync + overlap + quantization | Targets DiLoCo’s “peak bandwidth spike” issue by streaming subsets and overlapping comm/compute; claims **two orders of magnitude** bandwidth reduction in experiments. |
| **DiLoCoX experiments (research)** | Claims pretraining **107B** over **1Gbps** network | **DiLoCoX framework**: pipeline parallelism + dual-optimizer policy + one-step-delay overlap + adaptive gradient compression | Explicitly framed as making >100B training viable over slow networks; claims “pre-training a 107B foundation model over a 1Gbps network” and large speedups vs vanilla all-reduce. |
| **Early “open collaboration” pretraining (pre‑DiLoCo)** | Includes collaborative LM pretraining with 40 participants (per paper) | **DeDLOC / Hivemind-style collaborative training** | Provides historical context that decentralized training over unreliable, heterogeneous devices is feasible and has been demonstrated in language model pretraining settings. |

### Interpreting the map

There is a clean high-level division:

- **DiLoCo direct lineage:** DiLoCo → OpenDiLoCo → PRIME/INTELLECT‑1, plus Streaming DiLoCo and MuLoCo-like research. The common structure is **multi-step local compute + periodic outer synchronization** (often with an explicit outer optimizer), sometimes augmented by quantization/streaming.
- **Compression-first lineage:** DeMo/DisTrO/Psyche, where the optimization algorithm is engineered to produce **small communicable objects** every step/window, supported by network + chain coordination.
- **Hybrid bridge methods:** SparseLoCo is explicitly a **communication-efficient data-parallel method** that leverages both (a) infrequent synchronization and (b) extreme sparsity + 2-bit quantization, and is used in Covenant‑72B.
- **A shift for RL:** INTELLECT‑2 illustrates that **RL post-training** can adopt a very different architecture where inference generation dominates and is embarrassingly parallel and asynchronous, reducing the need for tight coupling.

## How these architectures compare in practice

### Communication pattern and bandwidth sensitivity

Conventional datacenter training (DDP/FSDP/ZeRO) assumes frequent collective communication and is best when you can amortize that cost on low-latency high-bandwidth fabric. PyTorch’s DDP framing (replicas + all-reduce) and the FSDP/ZeRO framing (shard states but still communicate frequently) reflect this.

DiLoCo-family methods reduce **communication frequency** drastically, which directly addresses “inter‑datacenter or Internet” conditions. The original DiLoCo work emphasizes inner step windows and infrequent global sync, and OpenDiLoCo + INTELLECT‑1 show concrete system stacks to make that work across locations.

Compression-first methods reduce **communication volume per synchronization** and often also engineer overlap to hide latency. Psyche explicitly describes overlapping training and communication, and additional quantization tricks like sign-only transmission.

SparseLoCo and Streaming DiLoCo can be understood as responding to the “second-order bottleneck”: even if syncs are infrequent, **a full-parameter sync can still create an unacceptable bandwidth spike**, so they push sparsification/quantization and/or fragment/stream the sync.

### Fault tolerance, elasticity, and heterogeneous participation

A major qualitative difference between “Internet-grade” systems and datacenter systems is handling **dynamic membership** and **non-uniform hardware/network performance**.

- DiLoCo is explicitly claimed to be robust to resources becoming unavailable and to leverage resources that become available during training.
- OpenDiLoCo highlights Hivemind properties like fault tolerance and on/off ramping, and frames the system as peer-to-peer without a master node.
- PRIME/INTELLECT‑1 makes elasticity a first-class design point via ElasticDeviceMesh and dynamic process groups.
- Covenant‑72B’s architecture goes further by using **object storage** (Cloudflare R2) to avoid requiring direct P2P connectivity, explicitly noting that this supports asynchronous uploading and validator-side scoring without a synchronized collective.
- Psyche’s lifecycle (epochs, onboarding/offboarding) is explicitly designed to reduce opportunity cost for contributors and manage unreliable participation.

In comparison, while modern datacenter stacks do offer some elasticity features in practice, the default mental model remains a fixed world-size cluster with reliable, uniform accelerators—one reason the “DiLoCo-family” remains niche outside decentralized contexts.

### Governance: whitelisted vs permissionless

A substantial fraction of the recent innovation in decentralized training is not purely algorithmic—it’s about **trust, incentives, and adversarial robustness**.

Gauntlet (Templar-I) is explicit that it is a permissionless incentive system deployed on the Bittensor blockchain, training a 1.2B model with “no control over users that can register or their hardware,” scoring pseudo-gradients by loss improvement and tracking participant ranks via OpenSkill.

Covenant‑72B builds on this by combining SparseLoCo with a live blockchain mechanism (Gauntlet) and a validator that asynchronously selects top-scoring pseudo-gradients; the paper frames dynamic joining/leaving as a first-class capability.

Psyche also uses on-chain coordination (Solana), but its public description emphasizes training run lifecycle and verification/witnessing mechanics rather than a token incentive market like Bittensor.

### Why DiLoCo is important but not “dominant”

Putting the evidence together, the balanced conclusion is straightforward:

DiLoCo is **foundational** for **low-communication data-parallel pretraining** across poorly connected workers—its architecture is directly implemented and extended in OpenDiLoCo and PRIME/INTELLECT‑1, and it strongly influences subsequent methods like SparseLoCo and Streaming DiLoCo.

But DiLoCo is not the default for distributed training overall, because most industrial training still happens inside well-connected datacenters where conventional DDP/FSDP/ZeRO plus tensor/pipeline parallelism remain the standard architecture.

Meanwhile, several major “Internet-scale” efforts are **not literal DiLoCo** but rather adjacent: DeMo/DisTrO/Psyche is a separate optimizer/system line targeting the same constraints via compression and overlapped execution; Covenant‑72B uses SparseLoCo, explicitly described as a communication-efficient optimizer, not plain DiLoCo; INTELLECT‑2 demonstrates a separate architectural strategy where asynchronous RL eliminates the classical data-parallel comm bottleneck.

## Practical phrasing and lineage guidance

Based on the public record above, the following terminology is the most defensible:

- **Direct DiLoCo lineage (safe to call “uses DiLoCo”):** original DiLoCo paper; OpenDiLoCo replication; PRIME/INTELLECT‑1 as a hybrid DiLoCo–FSDP2 system with additional quantized all-reduce kernels.
- **DiLoCo-derived / DiLoCo-family (safe when emphasizing shared architecture concepts):** Streaming DiLoCo (fragmented sync + overlap + quantization) and SparseLoCo (multi-iteration + sparsification/quantization with error feedback replacing outer momentum).
- **Adjacent sibling branch (safe to describe as “same problem space”):** Nous/Psyche’s DisTrO/DeMo line—explicitly positioned as building on DeMo and reducing transfer by orders of magnitude, with chain coordination—and Consilience 40B listing DisTrO as its optimizer.
- **Permissionless incentive layer (orthogonal but crucial):** Gauntlet (Templar‑I) is not just an optimizer—it is the *incentive + verification* mechanism that makes permissionless training viable, and can be applied to multiple pseudo-gradient–based schemes.

The resulting summary is: **DiLoCo is not the default distributed training architecture overall, but it is one of the key reference architectures for decentralized / Internet-scale training—either directly (Prime) or as a conceptual ancestor in DiLoCo-family methods (SparseLoCo, Streaming DiLoCo).**
