# Native DeepSeek-V4.1-Flash in Psionic

Status: proposal, 2026-10-20. Only V1, the protocol adapter (#12372), and the
V2 weight reader ([Weight loader](#weight-loader-v2)) are implemented;
everything else is planned. It
extends the [DeepSeek support plan](deepseek-support-plan.md) (DS0–DS6) and
the [implementation roadmap](implementation-roadmap.md) with one concrete
goal: serve the current DeepSeek release, V4.1-Flash, with Psionic as the only
inference engine, on Pylons, so that the Grid can show it. The product
roadmap that consumes this is
[Enter the Verse](../roadmap/2026-10-20-enter-the-verse.md).

Owner direction (2026-10-20): Psionic only, no external inference engines in
the serving path. Adapt DeepSeek's open-source code where it helps. Split the
model across devices or machines if one GPU can't hold it.


The one checklist for finishing #12369 is
[Completing #12369](deepseek-v41-completion.md); this plan keeps the
reasoning and the evidence.

## Summary

- V4.1-Flash is about 510 GB of weights: FP4 routed experts, FP8 dense
  weights, and two large FP8 Engram lookup tables. No single GPU holds it.
- Serving one request at a time is bound by memory bandwidth, not by
  tensor-core math. Portable Psionic dequantize-and-multiply kernels work on
  any recent NVIDIA GPU, so the datacenter-only FlashMLA, DeepGEMM, and
  DeepEP kernels aren't required for the first native result.
- DeepSeek's checkpoint ships a readable PyTorch reference (`inference/`,
  about 2,400 lines). Run it once on rented GPUs to produce golden
  activations. It's a test oracle only and never serves a request.
- Build V4.1 directly. Don't detour through V2-Lite: V4.1's attention, cache,
  residual, and Engram design differ enough that V2-Lite would mostly prove
  a different model. Share the MoE and FP8/FP4 primitives between them.
- Fan out by pipeline stages (contiguous layers), one stage Pylon per GPU,
  from the start (owner decision, 2026-10-20: fanning out across several
  devices matters most, in different mixes). Eight GPUs in one machine are
  eight stage Pylons over loopback; eight machines are the same Pylons over
  the network. Each token sends a small residual state across a boundary, so
  ordinary datacenter networking is enough for one request at a time.
  Expert parallelism (DeepEP-style all-to-all) is a later throughput
  optimization inside one machine.

## The checkpoint

Pinned revision: [`deepseek-ai/DeepSeek-V4.1-Flash-1017`][H4] at
`1bae0e1088993b9b82de08f59de5be5a34e42170`. We read the `config.json`,
`inference/config.json`, `model.safetensors.index.json`, the file listing, and
the `inference/` reference sources at that revision on 2026-10-20. The
weights are copied, verified, only to Cloud Storage
([Oracle](#oracle-v0)).

| Property | Value | Source |
| --- | --- | --- |
| Weights on disk | 510.3 GB in 48 safetensors shards | Revision file listing |
| Layers | 40 decoder layers, plus 3 MTP/DSpark layers | `config.json` |
| Hidden size | 5,120; vocabulary 129,280 | `config.json` |
| Attention | 64 query heads, one shared KV head of dimension 512 with a 64-dimensional RoPE part; query LoRA rank 1,280; grouped output LoRA (8 groups, rank 1,024); attention sink | `config.json`, tensor names |
| Attention per layer | `compress_ratios`: layers 0–1 sliding window only; layers 2–19 ratio 2; layers 20–39 ratio 1; window 128 tokens | `inference/config.json` |
| Cross-layer sharing | KV source layers 2, 8, 14, 20; indexer source layers 2, 8, 14, 20, 24, 28, 32, 36; candidate source layer 20 (top 2,048 blocks of 8 tokens); indexer top-k 512 | `inference/config.json` |
| Residual stream | Hyper-connections with `hc_mult=4` and 20 Sinkhorn iterations, so the residual is four streams of 5,120 | `config.json` |
| MoE | 384 routed experts, 6 active, 1 shared expert; expert width 2,304; `sqrtsoftplus` scores, `noaux_tc` selection, route scale 1.5, SwiGLU clamp 10 | `config.json` |
| Engram | Layers 1 and 14; n-grams up to 4 over a normalized, compressed vocabulary (99,092 ids); about 384 million rows per table; 8 heads of 256 | `inference/config.json`, `inference/engram.py` |
| Weight formats | FP8 E4M3 with one UE8M0 scale per 32×32 block; experts FP4 E2M1 with one UE8M0 scale per 32 elements along K | `inference/model.py` lines 26–30, 211–231 |
| Context | 1,048,576 positions allocated; YaRN factor 16 over 65,536 | `config.json` |
| Optional parts | DSpark/MTP draft (block 5, 128 experts, 3 active); 32-layer vision encoder | `config.json` |

Memory split, summed from the tensor byte ranges in all 48 shard headers
(#12373; the [retained report][WeightsReport] lists each layer):

| Part | Size | Notes |
| --- | --- | --- |
| Routed experts, 40 layers × 384 | 288.8 GB | 271.8 GB of packed FP4 (about 544 billion parameters) plus 17.0 GB of E8M0 scales |
| Engram tables, 2 | 202.8 GB | Each table is `F8_E4M3` [about 384 million, 256] (98.3 GB) plus `F8_E8M0` [about 384 million, 8] scales (3.1 GB); a few rows are read per token |
| Everything else | 18.7 GB | Attention, shared experts, gates, hyper-connections, embeddings and head (2.6 GB), MTP (7.9 GB, 7.2 GB of it experts), vision (1.0 GB) |
| Total | 510.3 GB | 510,286,023,000 bytes, as the index declares |

Each token reads 6 routed experts and 1 shared expert in each of the 40
layers: about 10 billion expert parameters, or 5 GB at FP4. Attention and the
shared weights add a few GB. Because the attention keeps one 512-wide KV
head with a 128-token window and compressed history, the cache for a long
context is small compared with the weights.

Engram is a lookup table, not a matrix product. Only a few rows are touched
for each token, so the 203 GB can live in host memory or on local SSD with
the touched rows copied to the GPU. That brings the GPU-resident part down to
about 308 GB.

## What we reuse and what we write

| Need | Reuse | Write |
| --- | --- | --- |
| Prompt, tokens, reasoning, tool-call parsing | [`deepseek-recipe`][Recipe] (official Rust), as DS1 in the support plan | A Psionic adapter that keeps special-token ranges and token-boundary streaming |
| Exact math for each block | The checkpoint's `inference/model.py`, `kernel.py`, `engram.py` | Rust CPU references for each component, checked against golden activations |
| Weight loading | Psionic's paged artifact readers and identity rules | A safetensors reader that keeps FP8/FP4 blocks and UE8M0 scales byte-exact; no conversion to GGUF |
| GPU kernels | Psionic CUDA backend, existing quantized matrix-vector kernels and CUDA graphs | FP4 E2M1 and FP8 E4M3 block-scaled matrix-vector kernels, MQA attention with sink and window, the indexer, Sinkhorn mixing, the Engram gather |
| Single-request serving | `--single-generation`, the R1 reasoning contract, managed Pylon recovery, the DS1 adapter (`psionic_models::deepseek_v41`) | Selecting the DS1 adapter for a loaded V4.1 model |
| Splitting the model | The Gemma 4 step/reset pipeline (`psionic-serve/src/gguf.rs` and `openai_http.rs`), Mesh-LLM lessons, `psionic-net` | Typed stage state for V4.1, stage packages, one stage Pylon per GPU, stage readiness, placement for mixed GPUs |
| Load balancing experts (later) | [EPLB][EPLB] policy ideas | Real expert routing in `psionic-cluster`, which today derives routing from a hash ([Psionic gaps](#psionic-gaps-this-plan-must-close)) |

Leave out of the serving path: DeepInfer, the PyTorch reference, FlashMLA,
DeepGEMM, DeepEP, TileLang, and llama.cpp. FlashMLA and DeepGEMM stay
optional accelerators for a later, pinned SM90/SM100 profile. Their kernels
can guide ours, but they aren't dependencies.

## Build order

Each step ends with retained evidence. A component is enabled only after its
check passes, and no public capability names V4.1 until the full model
passes.

| Step | Work | Done when |
| --- | --- | --- |
| V0: Oracle | Pin the reference sources. On a rented 8-GPU machine, run `inference/generate.py` (TP8, FP4 experts) on a fixed prompt set and dump per-layer inputs and outputs for selected layers and tokens: embeddings, Engram output, each attention variant, indexer selections, Sinkhorn mixing, router ids and weights, expert outputs, logits. Store digests and small slices in the repository and full dumps in a bucket. | Done (#12371): golden fixtures cover layers 0, 1, 2, 14, 20, 21, and 39 and more, on four prompts and four edge cases; the run's environment, revision, and costs are recorded; both machines are gone. See [Oracle](#oracle-v0). |
| V1: Protocol | DS1: integrate `deepseek-recipe` for V4.1 rendering, token encoding, reasoning, and tool-call parsing, with mock-backend tests. | Done (#12372): the checkpoint's five `encoding/tests` cases render byte for byte with the official token IDs, and mock-backend streams map onto Chat Completions. Protocol-qualified only; see [DS1](deepseek-support-plan.md#ds1-implementation-v41-protocol-adapter-12372). |
| V2: Loader | Safetensors reader with exact FP8 and FP4 block layouts, tensor naming, and the 48-shard index. Stream-mode loading for machines that hold only part of the model. | Every tensor in the index loads with its shape and dtype; random-block dequantization matches the oracle's dequantized values. |
| V3: CPU components | Rust references for RMSNorm, hyper-connection mixing (Sinkhorn), the three attention variants, the indexer and candidate selection, Engram hashing and lookup, the gate, routed and shared experts, the head. | Each matches its golden slice within a stated tolerance; router ids match exactly. |
| V4: CPU full model | Compose the 40 layers with a streamed or memory-mapped weight source. Greedy decode on the oracle prompts. | Logits match the oracle within tolerance for the first 32 generated tokens; the greedy text matches. Slow is fine. |
| V5: CUDA kernels | FP4 and FP8 block-scaled matrix-vector kernels, attention, indexer, Sinkhorn, Engram gather; each with a CPU-parity test on the RTX 4080 using real tensors from one layer. | Per-kernel parity passes on SM89 and on the target GPU. |
| V6: Stage Pylons on one machine | One Psionic stage process per GPU, each a managed Pylon worker, joined over loopback with typed stage state (below). A front stage takes the NIP-CJ job. Serial decode only. | Full-model parity with V4 on the oracle prompts; DS0's six runtime checks pass; tokens per second, time to first token, and memory per GPU are recorded. |
| V7: Group lifecycle | Stage packages, readiness before publication, group fencing, and managed recovery per stage (generalizing #12364's llama.cpp recovery to native workers). | The serial Pylon and recovery checks pass; killing any one stage fails the request visibly, and the group recovers. |
| V8: Stage Pylons across machines | The same stages on separate machines in one zone, in at least two configurations (for example, eight single-GPU machines, and four two-GPU machines). | Same parity and checks in each configuration; per-hop latency and the stage state sent per token are recorded. |
| V9: Throughput (later) | Batching, expert parallelism inside a machine, DSpark speculation, prefix reuse. | Measured against V6 by committed output tokens per second. |

V0 and V1 can start now and in parallel. V2 and V3 can start against the
oracle fixtures before any big GPU is rented again.

## Splitting the model

### Stage state

A stage boundary must carry everything later layers read, not just the
hidden state:

- The four hyper-connection residual streams (`hc_mult=4`), 4 × 5,120 values.
- KV from the source layers 2, 8, 14, and 20, which later layers reuse.
- Indexer state from the source layers 2, 8, 14, 20, 24, 28, 32, and 36.
- Candidate blocks chosen at layer 20, which layers 21–39 read.
- Engram output for layers 1 and 14, which depends on token history.
- Position, request identity, and a cache generation number, as the Gemma 4
  step does today.

Choose boundaries so that shared KV and indexer state stays inside the stage
that produced it where possible. Where it can't, send it once per token with
the residual. For one request at a time, the residual is about 40 KB per
token in BF16, so a datacenter network adds well under a millisecond per
hop. The [Mesh-LLM review](mesh-llm-lessons.md) applies: qualify a split
against the unsplit model, give each stage only its own weights, publish a
stage only after it's loaded and checked, and fence a group so a restarted
stage can't join a stale request.

### One stage Pylon per GPU

Every stage is its own Psionic process on one GPU, run as a managed Pylon
worker. Psionic's CUDA backend already tracks a device ordinal, so a stage
process only needs to pick its GPU. Each stage Pylon publishes a beacon with
its stage role, its layer range and the model group's identity. A front stage
takes the NIP-CJ job and calls the next stage over an authenticated private
channel, as the Gemma 4 pipeline does with its peer. Over loopback on one
machine and over the network across machines, the protocol is the same.

This shape needs no collective library: stages pass state forward, one
request at a time. Tensor and expert parallelism need all-to-all and
all-reduce, which Psionic doesn't execute yet; leave them for V9.

### Mixes and placement

A group is described by a manifest: the model artifact, each stage's layer
range, its GPU, and where the Engram tables live. Placement assigns layer
ranges by each GPU's free memory, so mixed GPUs work. By our estimate, one
layer's routed experts are about 7 GB at FP4, plus about 1 GB of other
weights, so:

- eight 96 GB GPUs hold five layers each with room to spare;
- a 24 GB GPU can hold two layers;
- the two Engram tables (about 98 GB each, for layers 1 and 14) sit in the
  host memory of the machines running those layers, which must have the
  RAM.

Run all stages of one group in one cloud zone at first. Every stage hop adds
latency to every token, so a group over home connections would be slow; the
docs already rule out token-level expert traffic over wide-area links.
Contributors' machines can serve whole smaller models today, and can join a
group as stages once V8 measures what a hop costs.

## Hardware

Serial decode reads about 5–8 GB of weights per token, so speed depends on
memory bandwidth and on keeping the model resident. All candidates are spot
or quota-limited; existing production GPUs aren't spare capacity.

| Option | Memory | Fits | Price we have on record | Use |
| --- | --- | --- | --- | --- |
| `g4-standard-384`, 8× RTX PRO 6000 Blackwell, PCIe | 768 GB | Everything, with about 250 GB to spare | $13.31/h spot, $36.00/h on demand (`scripts/dev/cloud-prices.json`) | The target (W8): eight stage Pylons over loopback on one machine, with the same card a contributor could own. Spot quota is 16 GPUs per region. |
| `a4-highgpu-8g`, 8× B200, NVLink | about 1,440 GB usable | Everything | $39.63/h spot ([Muse serving notes][Muse]) | Throughput work (V9) and the oracle run. Spot only; quota 64 B200 per region. No serving profile (W8). |
| `a3-ultragpu-8g`, 8× H200 | 1,128 GB | Everything | $83.37/h on demand ([Muse serving notes][Muse]) | Fallback if G4 and A4 spot capacity is unavailable. |
| Several single-GPU G4 machines, 1× RTX PRO 6000 each | 96 GB each | Eight machines hold everything, with Engram in host memory | `g4-standard-48`: $1.66/h spot, $4.50/h on demand | Stage Pylons across machines (V8). A `g4-standard-48` has 180 GiB of host RAM, enough for one 101 GB Engram table. |
| RTX 4080 (coderos-4080) | 16 GB | One layer's tensors at a time | Owned | Kernel parity tests (V5). |

Before renting, check the current spot price and capacity, and set a cost cap
and an expiry. Delete or stop the machine when the bounded run ends, and
label it with the run (`agent-run.py label-vm`).

The full CPU reference (V4) needs about 510 GB of readable weights. It can
memory-map them from local SSD on a smaller machine, more slowly, or run on a
host with enough RAM: a `g4-standard-384` host has 1,440 GiB and an A4 host about 3.9 TiB, so the GPU machine can also run the CPU reference.

### Target GPU (W8, #12395)

Serial stage serving targets the **RTX PRO 6000 Blackwell (SM120)** on G4
machines:

- **Parity.** W4–W7's `v41_matvec`, `v41_mix`, `v41_attention`, and
  `v41_stage` suites pass on SM120 with the tolerances fixed before the SM89
  runs, including every bit-for-bit host-model check.
- **Speed.** Decode matrix-vector products on the large FP8 matrices read
  about 1.1 TB/s, about 60% of the card's ~1.8 TB/s. One stage of layers
  0–4 decodes in 4.13 ms per token in 39.5 GB; layers 0–9 take 8.06 ms in
  76.6 GB, so a 10-layer stage fits one 96 GB card (W7).
- **Cost and quota.** A `g4-standard-384` is $13.31/h spot against $39.63/h
  for an `a4-highgpu-8g`. Spot quota is 16 RTX PRO 6000 GPUs per region
  ([inventory](assets/gcp-inventory-2026-10-20.json)), enough for two
  eight-stage groups.

**B200 (SM100) gets no separate profile now.** The kernels are portable CUDA
with no SM100-only features, and B200 was needed only to run the reference
oracle. Reopen this for throughput work (batching or expert parallelism,
V9), for kernels that use `tcgen05` or other `sm_100a` features, or if G4
spot capacity or quota can't hold the groups we need.

**Toolkit.** Stage machines use the image family
`common-cu129-ubuntu-2404-nvidia-580` from `deeplearning-platform-release`
(CUDA 12.9, driver 580) and build with `PSI_CUDA_ARCH=sm_120`; SM120 needs
CUDA 12.8 or later. `scripts/bench/deepseek-v41/stage_vm.sh` refuses
another CUDA release. The kernels also build with CUDA 13 (the RTX 4080
runs 13.2): `quantized_matvec.cu` takes CUB from either include layout.

### How to use the cloud spend guard

Start every Google Compute machine for this work with
`scripts/dev/cloud-budget.py` (#12370). Don't call `gcloud compute instances
create` directly. The guard does the following:

- Looks up the hourly price in `scripts/dev/cloud-prices.json` and refuses a
  machine type, or a spot or on-demand choice, that has no price there.
- Refuses the machine if the money spent, plus every running machine's cost up
  to its expiry, plus the budget's reserve, plus `--hours` × price, would pass
  the $3,000 cap.
- Gives the machine an `expires-at` label and `--max-run-duration`, so Google
  deletes it at expiry. With `--on-expiry stop`, Google stops it instead and
  keeps its disks.
- Labels it with your run (`run-id`, `run-host`) and `budget=deepseek-v41`,
  and records it in the run, so ending the session reaps it.
- Records `start` and `stop` records with the price and cost in
  `~/.openagents/cloud-budget/deepseek-v41.jsonl`.

Before it admits a machine, `create` runs `sync`, which also records any
machine labeled `budget=deepseek-v41` that has no running record, such as one
whose create reply was lost, so it counts.

#### Allocate from one host

One host and one ledger file allocate for a budget (#12376). The guard has no
shared store, so a second host or a second `OA_CLOUD_BUDGET_DIR` would
otherwise admit against its own $3,000. Copying the ledger to workers doesn't
share the budget; run every `create` on the one host and let workers use the
machines.

- A new budget's first record names its host and ledger path. A ledger from
  before #12376 gets an `owner` record on its next `create`; earlier records
  are never rewritten.
- `~/.openagents/cloud-budget-authority.json` names the ledger each budget uses
  on this host.
- `create` and `reserve` refuse from any other host or ledger path. If the
  owning host has stopped allocating for good, run the command with
  `--take-over` from the new host; the ledger records the takeover. Set
  `OA_CLOUD_BUDGET_HOST` only to keep a host's name stable.

#### Reserve storage and network

The price table covers machines and GPUs only. Set a reserve for persistent
disks, buckets, checkpoint copies, oracle dumps, and network egress, and
`create` counts it against the cap:

```sh
python3 scripts/dev/cloud-budget.py reserve 250 \
  --why "2 TB hyperdisk 2 weeks, 1.6 TB bucket 1 month, 2 TB egress"
```

To measure it, add up `gcloud storage du -s gs://BUCKET`, the sizes in
`gcloud compute disks list --filter=labels.budget=deepseek-v41`, and the planned
copies, at the catalog's per-GB prices for the time you keep them. The latest
`reserve` replaces the one before it; raise it before a larger rental.

#### Corrupt records

A crash can tear the ledger's last line. The guard skips it, and the next
record names the torn line so it still reads. Any other line that can't be
read makes `create` and `reserve` refuse and name the line, because a lost
line may hide a running machine. `report` still prints what it can read and
marks the line. Repair the line from Google's records, such as
`gcloud compute operations list`, before you allocate again.

When `sync` finds that a machine is gone, it takes the end time from the
zone's operations, then from the audit log. A machine that Google deleted late
costs until its actual deletion. If neither record says when it ended, the
machine counts in full until its expiry and is then settled at its expiry. A
machine still recorded as running past its expiry counts until now.

The run label comes from `OA_RUN_ID`, which every Claude Code shell here
sets. Elsewhere, create a run first with
`export OA_RUN_ID=$(python3 scripts/dev/agent-run.py new)`.

Check the spot price on the day, and update the price table with its source
and date if it changed. Then run the command with `--dry-run` first. The dry run
prints the full `gcloud` command and the budget total without creating
anything. Flags after `--` pass through to `gcloud compute instances create`
unchanged. The guard sets `--labels`, `--max-run-duration`,
`--instance-termination-action`, `--provisioning-model`, `--machine-type`,
`--zone`, and `--project` itself, and refuses them after `--`; add your own
labels with `--label KEY=VALUE`.

The V0 oracle run (#12371), one `a4-highgpu-8g` spot machine for 8 hours:

```sh
python3 scripts/dev/cloud-budget.py create oa-dsv41-oracle-1 \
  --machine-type a4-highgpu-8g --zone us-central1-b --spot --hours 8 \
  --label issue=12371 --dry-run -- \
  --boot-disk-size=500GB --boot-disk-type=hyperdisk-balanced \
  --image-family=common-cu129-ubuntu-2404-nvidia-580 \
  --image-project=deeplearning-platform-release
```

An `a4-highgpu-8g` comes with its eight GPUs and its local SSDs, so it needs no
`--accelerator` or `--local-ssd` flags. Check the zone's B200 spot capacity
and the image family on the day, then run the same command without
`--dry-run`. The guard prints `vm NAME zone ZONE expires TIME`. A spot machine
that Google reclaims early is deleted the same way. To keep the machine's disks
after expiry, add `--on-expiry stop`; a stopped machine still pays for its
disks.

While it runs, and after:

```sh
python3 scripts/dev/cloud-budget.py report        # spent, running, remaining
python3 scripts/dev/cloud-budget.py stop oa-dsv41-oracle-1   # delete it now
python3 scripts/dev/cloud-budget.py sync          # record machines that ended or appeared
python3 scripts/dev/cloud-budget.py reserve 250 --why "..."  # storage and network
python3 scripts/dev/cloud-budget.py prices        # the price table
```

`report` asks Google which machines still exist, then prints the money spent,
the running machines' cost so far and up to their expiry, the reserve, the
money remaining, the allocating host, and each running machine's age and
expiry. Add `--offline` to skip the check,
or `--json` for a script. Prices cover the machine and its GPUs, not persistent
disks or network, which come out of the reserve. Record the machine type, hours,
and the cost `report` shows in the run's evidence.

The billing budget alerts at $1,500 and $2,500 track spend labeled
`budget=deepseek-v41`. If they aren't set up yet, the command is in
`NEEDS_OWNER.md`.

## Psionic gaps this plan must close

- **No DeepSeek decoder family.** `GgufDecoderFamily` has no DeepSeek case,
  and loading is GGUF-centric. The V2 safetensors reader has landed
  ([Weight loader](#weight-loader-v2)); the decoder family is still to come.
- **FP4 E2M1 and FP8 E4M3 block layouts with UE8M0 scales: landed (#12373)**
  as `V41BlockFormat` in `psionic_models::deepseek_v41_weights`, with CPU
  dequantization for a 32×32 FP8 block, an FP8 or FP4 row, and an Engram
  row. CUDA kernels for them are V5.
- **MXFP4 isn't reused, by design (#12373).** `QuantizationMode::GgmlMxfp4`
  has the same E2M1 values, 32-element blocks, and E8M0 exponents, but not
  the same bytes. GGML stores one scale inline before each 16 packed bytes
  and puts element `j + 16` in the high nibble of byte `j`; V4.1 keeps scales
  in separate `.scale` tensors and puts elements `2i` and `2i + 1` in the low
  and high nibbles of byte `i` (`inference/convert.py` lines 31–34). GGML's
  decoder also reads scale byte `0xFF` as infinity, where `float8_e8m0fnu`
  defines NaN. Repacking would change the checkpoint's bytes and identity,
  so V4.1 keeps its own format type instead of adding a `QuantizationMode`,
  whose modes all describe GGUF blocks with inline scales.
- **One device per process.** Kept on purpose: V6 runs one stage process per GPU.
- **No collective execution.** Planning types exist in `psionic-collectives`
  and `psionic-cluster`, but nothing runs all-to-all or all-reduce, and the
  sparse expert "realization" derives routing from a SHA-256 of the request
  seed rather than from a router. Don't cite it as expert-parallel support.
- **No NVLink or in-machine link class.** `ClusterLinkClass` models links
  between machines only.
- **The Gemma 4 pipeline is two stages, unqualified (R4), and sends f32
  hidden state.** Generalize it with a typed stage-state message rather than
  adding a second pipeline protocol.

## Weight loader (V2)

`psionic_models::deepseek_v41_weights` (#12373) reads the checkpoint as
stored:

- `DeepSeekV41Checkpoint::open` parses the index and all 48 shard headers. It
  refuses unknown dtypes, overlapping or out-of-file byte ranges, a tensor the
  index names that its shard lacks, a shard tensor the index doesn't name, a
  total that doesn't match, a checkpoint that doesn't match the pin, and any
  name or dtype that differs from the typed model map (`V41ModelMap`, 96,085
  tensors with their roles).
- `CheckpointPin::deepseek_v41_flash` pins the revision, the index SHA-256,
  and each shard's size and SHA-256. `verify_shard_hashes` hashes the shards
  as a separate step, because it reads all 510 GB.
- Tensor data is read with positioned reads of only the bytes asked for, so
  no shard is loaded whole. `fp8_weight_block`, `dequantize_row`, and
  `engram_row` return f32 values for the CPU reference and the CUDA parity
  tests.
- `select(&V41StageSelection)` lists the tensors and bytes for a layer range
  plus optional embeddings, head, MTP, and vision. Layers 0–4 need
  138.5 GB (101.4 GB of it the layer-1 Engram table), layers 10–14 need
  138.5 GB (the layer-14 table), and layers 35–39 need 37.0 GB.
- E2M1 nibble order: the low nibble holds the even element, as
  `inference/convert.py` lines 31–34 unpack it. A test checks it.

The offline tests use synthetic shards, the real index, and the real headers
of shards 17 (layer 14), 47, and 48 (the Engram tables), all under
`crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_weights/`.
With the weights, run:

```sh
cargo run --release -p psionic-models --example deepseek_v41_weights_check -- \
  CHECKPOINT_DIR [--oracle DEQUANT_PARITY.json] [--verify-sha256]
```

It opens every tensor (its first and last bytes, and the first and last rows
of every weight dequantized), reports bytes per layer and per range, and
compares dequantized values with the reference's. The reference's values come
in a `psionic.deepseek_v41.dequant_parity.v1` file: `cases` of
`{"kind": "fp8_block", "tensor", "block_row", "block_col"}`,
`{"kind": "row", "tensor", "row"}`, or `{"kind": "engram_row", "layer", "row"}`,
each with `f32_bits`, the reference's `weight.float() * scale.float()` as
IEEE-754 bit patterns in row-major order. The oracle run (#12371) wrote it
(`scripts/bench/deepseek-v41/dequant_parity.py`, 19 cases). On the original
files, the check opened all 96,085 tensors with 0 NaN values, matched all 48
shard hashes, and had 0 mismatches in all 19 parity cases; both outputs are on
#12373.

## Oracle (V0)

#12371 ran the checkpoint's own reference on eight B200 GPUs and recorded
golden values. The weights never touched a workstation: a CPU machine copied
the pinned revision from Hugging Face to Cloud Storage, and the GPU machine
copied the converted files from there.

| Item | Where |
| --- | --- |
| Original checkpoint, 83 files, 510,311,045,295 bytes, each verified against the Hugging Face tree API | `gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1/` and `1bae0e1.manifest.json` beside it |
| `convert.py --model-parallel 8 --expert-dtype fp4` output, 537,304,180,250 bytes | `gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1-tp8-fp4/` and its manifest |
| Dumps: 18,177 tensors, 455 MB, a manifest with each tensor's shape, dtype, and SHA-256 | `gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump/` |
| Repository fixtures: decode-mode tensors of the four main prompts, first and last 64 values of each, small tensors whole; 5.1 MB, 2.0 MB gzip-compressed | [`testdata/deepseek_v41/oracle/`](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/oracle/), read by `psionic_models::deepseek_v41_oracle` |
| Report: machines, hours, costs, driver, CUDA, packages, reference file hashes | [`report.json`](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/oracle/report.json) |
| Tools | [`scripts/bench/deepseek-v41/`](../../scripts/bench/deepseek-v41/): `weights_vm.sh`, `oracle_vm.sh`, `oracle_dump.py`, `oracle_manifest.py`, `dequant_parity.py`, `weights_manifest.py` |

**Prompts.** Four main prompts, each greedy for 32 tokens: a short chat
with thinking on (41 tokens), a Rust code question (40), a 2,124-token
prompt, and a 19,989-token prompt (past 2,048 × 8 = 16,384, so the layer-20
candidate selection and the indexers select). The checkpoint's `encoding/`
renders them. Edge cases: the 2k prompt cut to 127, 128, and 129 tokens with
8 decode steps (window ring and odd and even ratio-2 groups), and an Engram
normalization prompt. The first two prompts also ran to the end-of-sentence
token for a complete answer: 122 and 268 tokens.

**Modes.** `decode` is the reference's cached decode. `replay` runs the
fixed sequence (prompt plus the decode run's tokens) in one prefill, with full
logits at every position from the last prompt token on, for teacher-forced
comparison. `chunked` prefills half the prompt, then steps one token at a
time; the reference can't take a multi-token step after position 0.

**Recorded at five positions** (first token, last prompt token, and the
steps whose input is generated tokens 1, 2, and 32): embeddings; Engram input,
hash ids, rows, and output at layers 1 and 14; hyper-connection input,
pre/post/comb in both halves, collapsed and expanded streams at layers 0, 1,
2, 14, 20, 21, and 39; attention input and output at layers 0–3, 8, 9, 14,
15, 20, 21, 24, 25, and 39 (each compress mode and each KV source with the
layer that reuses it); compressor latents at 2, 14, and 20; selected indices
and masked scores at all eight index sources, and layer 20's candidate blocks;
gate input, scores, biased scores, ids, and weights at layers 0, 2, 14, 20,
and 39; one routed expert at layers 2 and 20 with its FP8 activation values
and scales; the shared expert and MoE outputs; the final norm and full
logits. Every step also records router ids for every token and layer, the
SHA-256 of every layer's output, and selection margins.

**Precision and ties.** The fixture manifest's `precision` and `tie_rules`
state each boundary's dtype, quantization, and accumulation, from
`inference/model.py` and `kernel.py`. Two findings set how comparisons work:

- Exact gate ties occur on real prompts (the 6th and 7th biased scores equal,
  for example in the chat prompt at layers 2, 36, and 37). `torch.topk`'s order
  among them is unspecified, so router checks must skip exact ties and report
  near ties; each prompt's `near_ties` lists rows within 1e-4.
- Cached decode and one-shot replay of the same tokens differ in the
  reference itself: logits by about 1 to 5, and one argmax flips (the 127-token
  case, generated token 4). Compare Psionic's decode with the reference's
  decode, and replay with replay. The cross-mode gap is context only; it does not set or relax same-mode
  tolerances. Per-component arithmetic bounds must be fixed before comparison,
  with same-mode repeatability measured separately when available.

**Is V4.1 an encoder and decoder?** No. The reference has one block type for
all 40 layers (`model.py` lines 1206–1207), and `Transformer.forward` runs
every layer on every call, prefill or decode (lines 1264–1270; the dumps
have output hashes for all 40 layers at all 33 steps). Layer 20's compressor
reads layer 20's own attention input: `Block.forward` collapses the four
streams that layer 19 produced with `hc_pre`, applies `attn_norm`, and calls
attention (lines 983–987), which passes that input to the compressor (lines
747–748). At ratio 1 the compressor has no gate: `norm(wkv(x))` (lines
461–462). The dumps record that input as `layers.20.attn.in`. What is true is
narrower: layers 21–39 read layer 20's compressed KV and candidate blocks
(lines 749 and 764, 570–576) instead of compressing their own, just as layers
3–7, 9–13, and 15–19 read layers 2, 8, and 14. Every layer still computes its
own queries and sliding-window KV. A stage boundary inside 20–39 must carry
layer 20's compressed KV and candidates, and the latest indexer selection.

Costs, from the guard's ledger: `n2d-highmem-96` on demand, 0.80 h, $4.37;
`a4-highgpu-8g` spot in us-east1-b, 0.82 h, $32.48 (no us-central1 capacity;
Google preempted it after every prompt was uploaded). Total compute
$36.85, plus storage of about $21 a month for both copies and about $11 of
cross-region transfer.

## Pylon gaps for this model

These are owned by the [Enter the Verse roadmap](../roadmap/2026-10-20-enter-the-verse.md)
but block V7:

- NIP-CJ input is capped at 16 KiB and 32 turns (`crates/pylon/src/job.rs`).
  Agent work needs far more context.
- Results aren't streamed, and the provider deadline is 90 seconds. A
  V4.1 reasoning answer can run longer. Any longer deadline is a transport
  contract change with its own tests, not a client timeout edit.
- Tool calls and structured output are refused end to end. Agent use needs
  DS1's tool-call parsing, R2's constrained decoding, or both.

## Open questions

- The precise active parameter count and the KV bytes per token at each
  compression ratio. Measure both in V4 instead of trusting our estimates.
- G4 spot price and capacity on the day of the run.
- Whether to qualify vision. It's out of scope for the first result; image
  requests are refused.
- License terms for the checkpoint's `LICENSE` file, which we haven't
  reviewed for redistribution of converted stage packages.

[H4]: https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/tree/1bae0e1088993b9b82de08f59de5be5a34e42170
[Recipe]: https://github.com/deepseek-ai/deepseek-recipe
[EPLB]: https://github.com/deepseek-ai/EPLB
[Muse]: ../research/muse/2026-10-15-muse-spark-serving.md
[WeightsReport]: ../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_weights/weights_report_headers.json

## Review addendum (2026-10-20)

Reviewed against `75392db83e` and #12369–#12373. These recommendations qualify
the hardware, bandwidth, and completion claims above; they do not add a
second inference engine or revive the V2-Lite detour. The current oracle
issue #12371 already chooses B200 first, requires a kernel smoke before the
full download, and includes a 20,000-token selection workload. Preserve
those improvements rather than reverting to the older G4-oracle summary.

### Strengthen the oracle before paying for later integration work

- Capture a fixed token sequence and teacher-forced component outputs, so a
  different sampled or rounded token does not invalidate every later layer
  comparison. Retain full logits at the comparison positions in the bucket,
  not only top-64 values and a hash: hashes establish file identity but
  cannot support a numerical tolerance check. Keep small offline fixtures
  and a manifest/download path to the larger comparisons.
- State precision at every boundary, including activation quantization,
  scales, accumulation, and casts. Define absolute/relative tolerances per
  component before evaluating Psionic. Keep discrete expert/index choices
  exact under the reference's tie rule; report near ties and first
  divergence rather than loosening the tolerance after a failure.
- Add targeted 127/128/129-token window cases, odd/even compression steps,
  every shared-source/reuse transition, Engram normalization/history cases,
  and candidate selection beyond 16,384 positions. Compare chunked prefill
  with one-shot prefill and cached decode with replay. The existing long
  oracle prompt is valuable, but a few slices alone do not test these edges.
- On the oracle machine, reuse the downloaded original checkpoint to run
  #12373's full-byte/hash checks and export the documented
  `psionic.deepseek_v41.dequant_parity.v1` cases. Keep original-checkpoint
  identity distinct from TP8-converted files. Upload a complete manifest
  after each prompt so a spot loss does not require repeating good dumps.

The V4 CPU reference remains the debugging path. Bound its runs and begin
with a short context; run CUDA component work against component oracles in
parallel rather than waiting for a slow full 20,000-token CPU decode.
A second gate must produce complete final answers with honest EOS,
truncation, reasoning, whitespace, and stream termination. Adapt the *intent*
of DS0's checks to V4.1's recipe and markers; do not reuse R1's parser or
assume 32 generated tokens finish a reasoning answer. Advertise only a
measured context/output/deadline profile, not the configured million-token
maximum. Tool-capable agent use is a further end-to-end gate after #12372
and the Pylon transport support it.

Portable kernels are a correctness strategy, not a speed result. Prefill,
sparse selection, launch overhead, and transfers can dominate a serial
workload even when weight reads constrain decode. Qualify the stated SM89
and target-GPU profiles separately before extending the GPU claim.

### Evaluate four native stages before fixing the topology at eight

The [header report][WeightsReport] permits a more useful first placement
calculation. Excluding MTP and vision, and keeping the two Engram tables in
host memory, leaves **298,624,579,008 bytes** of packed text-model weights
for GPUs. A candidate assignment is:

| Stage | Layers and extra weights | GPU weight bytes, decimal GB | Host Engram bytes, decimal GB |
| --- | --- | --- | --- |
| 0 | 0–9 and embeddings | 75.409 | 101.378 |
| 1 | 10–19 | 74.069 | 101.380 |
| 2 | 20–29 | 73.917 | 0 |
| 3 | 30–39 and output head | 75.230 | 0 |

This is a header-derived candidate, **not a fit or speed qualification**.
Placement must also reserve measured peak KV/indexer/candidate state,
chunked-prefill activations, kernel workspace, staging buffers, driver
memory, and host RAM. Avoid expanding the packed model to resident f32.
Measure Engram cold/hot lookup and prefill costs; SSD-backed tables are a
separate latency profile, not a free substitute for RAM.

Try four whole 96 GB GPUs if that full budget fits; retain eight as the
fallback. Compare four single-GPU hosts with two two-GPU hosts for V8,
using the same logical state contract. This still prioritizes several
physical devices and reduces serial hops. Google's [G4 inventory](https://docs.cloud.google.com/compute/docs/gpus)
lists both whole-GPU configurations. The checked-in
[price snapshot](../../scripts/dev/cloud-prices.json) already includes G4
spot prices, contrary to the older text above; add sourced entries for any
chosen unpriced machine type before asking the guard to launch it. Use
B200 for the reference's TP8 run; native placement need not copy its GPU
count. Quota, capacity, and runtime compatibility still need a fresh check.

### Design and fault-test the stage contract early

Start P2.1's codec and P2.5's lifecycle fixtures before P1.8, using a tiny
known model or synthetic stage executor. Bind real V4.1 state after the CPU
components are known. Reuse the existing Gemma pipeline boundary through
adapters; an unrelated Gemma rewrite is not a prerequisite.

The 40,960-byte BF16 residual is only a lower bound. Specify cache creation,
prefill snapshots, per-token deltas, source-layer identities, compression
position, indexer/candidate updates, and Engram history. Measure all wire
bytes, serialization, host/device copies, hop latency, TTFT, and decode
latency. One serial request traverses the stages in sequence; summing
bandwidth or quoting residual wire time does not establish response speed.
The R1 host-stepped CUDA result is direct evidence that correct native
kernels alone do not establish a fast serving path.

Each message needs the group-manifest digest, group generation, request and
attempt, stage/range, step and position, dtype/shape, and bounded payload.
The front owns sampling and the selected token stream. Refuse duplicate,
stale, missing, reordered, or cross-request state before consuming it.
Publish group serving capacity only after all required stages agree on the
same weights, runtime, protocol, and generation; stage-presence beacons
must not make an internal stage routable as a complete model.

Test front disconnect, a stopped process that still looks healthy, crash
before/after a step acknowledgment, lost/replayed messages, and a late old
stage after restart. Keep admission occupied until all work is stopped or
its processes are reaped. A replacement changes the group generation and
invalidates affected request caches. Fail an interrupted request explicitly;
do not silently replay it after tokens have reached the caller. Couple this
with N1's versioned long-job, ordered-stream, cancellation, and backpressure
rules, while preserving the existing shorter job profile.

### Bound the budget guard's claims before larger rentals

#12370 shipped a useful local reservation ledger and recorded a cheap VM's
expiry. Two follow-ups are concrete at this review's source revision:

1. `locked()` serializes one ledger path on one host. Different hosts or
   `OA_CLOUD_BUDGET_DIR` values can each admit against their own $3,000.
   Allocate through one host and one budget ledger until a shared budget
   authority exists; copying a ledger to workers is not coordination.
2. `read_ledger()` skips malformed interior records as well as a torn tail.
   A synthetic local check confirmed an unreadable reservation line can be
   skipped and a new $2,000 request admitted against a $3,000 budget. Refuse
   new allocation on interior corruption and resolve uncertain reservations
   against provider records before releasing their cost.

File bounded follow-ups to the closed guard issue rather than treating its
closure as a global billing guarantee. Add lost-create-reply, concurrent
admission, reused VM identity, and delayed-expiry tests. Reserve persistent
storage, checkpoint copies/conversion, oracle dumps, and network charges;
these are excluded from the current compute-price table. The existing
[alert-only billing budgets](https://docs.cloud.google.com/billing/docs/how-to/budgets)
notify; they do not stop Compute Engine spending. Keep the owner alert step
in `NEEDS_OWNER.md`, use expiry/reaping for VM lifetime, and compare measured
costs with the reservation before each later allocation. No new cloud
resource was created for this review.

#12376 closes items 1 and 2 within one host: `create` refuses on interior
corruption, settles gone machines only from Google's operations or audit log,
adopts labeled machines missing from the ledger, refuses a second host or
ledger path, and counts a storage and network reserve. See
[How to use the cloud spend guard](#how-to-use-the-cloud-spend-guard). A
shared, multi-host budget store and stopping spend from billing alerts remain
out of scope.

## Independent CPU port, first implementation phase (2026-10-20)

The next four children of [#12369](https://github.com/OpenAgentsInc/openagents/issues/12369)
implement numerical components independently of the official oracle in #12371
and the protocol adapter in #12372. They live in
[`deepseek_v41_cpu`](../../crates/psionic/crates/psionic-models/src/deepseek_v41_cpu/mod.rs).
This phase does not publish a V4.1 serving model.

| Issue | Scope | Implementation and local checks |
| --- | --- | --- |
| [#12377](https://github.com/OpenAgentsInc/openagents/issues/12377), P1.3 | Hyper-connections | Float32 joint-stream RMS projection, pre/post coefficients, exact Sinkhorn order, collapse using the previous pre-mix, and residual expansion with `comb[source, target]`. Four targeted tests cover a nonuniform four-stream fixture, orientation, extreme logits, and invalid input. |
| [#12378](https://github.com/OpenAgentsInc/openagents/issues/12378), P1.7 | MoE | Float32 sqrtsoftplus routing, selection-only correction bias, top-k normalization, asymmetric SwiGLU clamps, selected-expert accumulation, and the shared expert. The row-provider adapter reads FP4/FP8/BF16/F32 rows through the existing loader. |
| [#12379](https://github.com/OpenAgentsInc/openagents/issues/12379), P1.4a/P1.5a | Attention and selection primitives | Float32 latent sparse attention and sink, causal window/ring addresses, transactional compression chunks, rectified index scores, and two-level candidate selection. Low-rank projections, RoPE/YaRN, activation quantization, and cross-layer source-cache wiring remain later work. |
| [#12380](https://github.com/OpenAgentsInc/openagents/issues/12380), P1.6 | Engram | Rust token normalization/compression, both prime bucket layouts, pinned per-layer multipliers, streaming/masked n-grams, sparse row lookup, and per-stream signed-square-root residual gating. |

The math is reimplemented from the MIT-licensed checkpoint reference at
`1bae0e1088993b9b82de08f59de5be5a34e42170`.
[`deepseek-v41-cpu-fixtures.py`](../../scripts/dev/deepseek-v41-cpu-fixtures.py)
generates small synthetic NumPy float32 fixtures. They check equations and
state boundaries; they are not official TileLang kernel outputs or full-weight
activations. The numerical tolerance for the hyper-connection fixture is
`3e-6` absolute. BF16 rounding, FP8/FP4 activation quantization, original-weight
checks, and device oracle comparisons remain unqualified.

Run the component tests from `crates/psionic` with the repository's shared
Cargo target and build environment:

```sh
cargo test -p psionic-models --lib deepseek_v41_cpu
```

Ten component tests pass after the MoE addition, including a synthetic packed-FP4
checkpoint read through the row adapter. MoE tests compare routing and expert output with a synthetic NumPy fixture
(`1e-5` absolute tolerance), exercise six-of-384 routing, top-1 behavior,
extreme logits, and invalid inputs, and verify that unselected experts are
never read. Exact score ties choose the lower expert ID in the CPU reference;
PyTorch's device `topk` tie order is not a portable guarantee. The oracle
comparison must report ties separately. Route weights multiply the intermediate
activation before the down projection, as in the reference; activation
quantization and BF16 rounding at that boundary are still deferred.

Eighteen accumulated CPU component tests pass after the attention addition.
Attention checks cover a synthetic NumPy equation fixture (`2e-6` absolute),
positions 127/128/129, a denominator-only sink and empty histories, odd
compression tails, chunk/replay equality, reset and failed-chunk state, short
index rows, and 20,001 positions beyond the 16,384-position candidate budget.
Candidate masks include the whole newest partial block as in the reference;
selection always intersects them with the causal visible length. Exact ties
choose the lower block/position. Unavailable picks become `-1`, including masked
candidates, so they cannot address a KV row. The official device's tie ordering
and BF16 attention-probability rounding still need oracle comparison.

The Engram addition brings the accumulated CPU component suite to 26 passing
tests. Integer hash IDs and prime layouts match the NumPy fixture exactly;
residual gating uses `2e-6` absolute tolerance. Hash history stops at masked
tokens, survives prefill/decode boundaries, and resets explicitly. The fixed
layer-1/layer-14 multipliers come from the reference's NumPy RNG; substituting
another random generator would select different table rows.

The [retained tokenizer qualification](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_cpu/engram_tokenizer_qualification.json)
also compares the complete pinned tokenizer against the unmodified
`build_compressed_token_map` function extracted from the SHA-256-checked public
`inference/engram.py`. All 129,280 entries agree by their little-endian `u32`
map digest, `c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481`.
The result has 99,092 compressed IDs and compressed pad ID 2. Rust uses the
locked `tokenizers` 0.23.2; the independent Python check uses 0.22.2. No model
weight bytes were opened. The [Rust checker](../../crates/psionic/crates/psionic-models/examples/deepseek_v41_engram_check.rs)
checks the tokenizer pin and generates a report; the
[qualification script](../../scripts/dev/deepseek-v41-engram-qualification.py)
compares that report with the pinned reference function. Neither runs PyTorch
or TileLang.

This completes the four filed CPU-component implementations. Next, connect
these components to the checkpoint's projection weights, implement the precise
BF16 and activation-quantization boundaries and RoPE/YaRN, and wire the shared
KV/indexer state across layers before attempting P1.8's full CPU decoder. The
original-weight oracle remains a numerical dependency for that integration;
closing these issues does not mark P1.4's whole attention block, full CPU decode,
native V4.1 CUDA, stage transport, or Pylon recovery as qualified.

## Independent CPU integration phase (2026-10-20)

The next four children of #12369 connect the earlier CPU components while
#12371's original-weight oracle remains separate:

| Issue | Scope | Local implementation |
| --- | --- | --- |
| [#12381](https://github.com/OpenAgentsInc/openagents/issues/12381) | Projection wiring | Named row-addressable checkpoint matrices, query/KV/compressor/indexer paths, grouped output projection, embedding, and final collapse/norm/logits. Nonuniform NumPy fixtures, group isolation, sparse embedding reads, and invalid inputs are tested. |
| [#12382](https://github.com/OpenAgentsInc/openagents/issues/12382) | RoPE/YaRN | Adjacent-pair tail rotation and inverse, pure-window base 10000, compressed/full base 160000 with YaRN, and first-token positions for completed groups. All 33 accumulated tests pass. |
| [#12383](https://github.com/OpenAgentsInc/openagents/issues/12383) | Activation quantization | Explicit float32/BF16 arithmetic, FP8 activation dispatch for FP4/FP8 weights, per-32 indexer FP4/UE8M0, per-16 compressed KV FP4/E4M3, and BF16 probability/value products. All 39 accumulated tests pass. |
| [#12384](https://github.com/OpenAgentsInc/openagents/issues/12384) | Shared KV state | Request-owned attention runtime with fixed weights/geometry/arithmetic, per-layer window rings, source KV/index keys, index selections, layer-20 candidates, bounded context, reset, and transactional layer updates. |

The [integration fixture generator](../../scripts/dev/deepseek-v41-integration-fixtures.py)
uses small independent NumPy equations from the pinned public reference.
Projection fixtures use `3e-6` absolute tolerance. The first projection change
uses float32 arithmetic; later rows add explicit numerical boundaries. Code
closure does not qualify original-weight activations, full CPU decoding, or
native V4.1 CUDA serving.

RoPE fixture frequencies use `1e-8` absolute tolerance and rotated values use
`2e-6`, at positions 0, 127/128/129, 65535/65536, and 1048575. Scalar powers
are rounded to float32 before reciprocal construction, and phase products and
rotations use float32. The public reference's device power/trigonometric
implementations still need comparison, especially where long positions magnify
frequency differences. The frequency helper's 1M position bound is not a
qualified runtime context limit.

The numerical path selects `Arithmetic::ReferenceBf16` explicitly. It casts
projection/norm/rotation results to BF16, preserves float32 promoted ratio-two
compressor projections, rounds pooled latents before normalization, and handles
grouped `wo_a` through BF16 weights and FP8-quantize/dequantize inputs. The final
head remains a float32 projection of BF16-normalized input. The expert helper
also exposes an explicit arithmetic mode for its pre-down-projection boundary.
Existing float32 equation APIs keep their behavior.

Activation fixtures compare packed codes, scale bytes, and dequantized values
exactly. Tests cover ties, signed zero, saturation, zero and tiny groups, scale
changes, named projection dispatch, compression rounding, and the BF16 attention
value product across 64-index blocks. Nonfinite inputs, invalid block widths,
and an E4M3 compressed-KV scale outside its finite range return errors. These
CPU conversion rules and float32 reduction order still require comparison with
the official device kernels; they do not claim bitwise GEMM/device parity.

The integrated attention runtime runs normalized inputs through the actual
40-layer ownership schedule. KV sources are 2/8/14/20; index sources are
2/8/14/20/24/28/32/36. Index keys consume the unrotated latent before its KV
form is rotated and quantized. Compressed IDs remain independent of each
query's window length, then become local addresses when the two sources join.
Consumers see only completed groups and causally visible rows. Layer 20's
candidate mask is reused by later indexers, and per-chunk selections never
survive into a new chunk.

All 45 accumulated CPU tests pass. The integrated float32 attention outputs
for all 40 layers on tiny synthetic weights match independent NumPy equations
within `2e-5` absolute. Whole prefill and split prefill/decode agree in float32;
the BF16 path agrees exactly across those chunkings on its tiny fixture.
Checks also cover odd compression tails, ring wrap, source-only projection
reads, candidate-mask reuse, reset/new-request isolation, wrong order/positions,
capacity refusal, and a projection failure after cache computation. The failed
layer leaves the runtime unchanged. Index weight scale factors are computed
as Python float64 scalars before the BF16 boundary.

The runtime binds one immutable weight-source reference for its lifetime and
owns its request state. Immutable rows and source buffers use copy-on-write
storage for transactional updates. Caller-selected context capacity bounds the
stored token history; this is not a measured full-model memory or throughput
qualification. There is no new serving route or network protocol.

This completes #12381–#12384. The integrated test exercises attention across
40 layer IDs with supplied normalized inputs; it does not run the 40-layer
transformer. The next CPU integration must compose Engram, hyper-connections,
attention norms, MoE, final logits, and generation with consistent numerical
boundaries, then compare original-weight activations and complete answers with
#12371. Native V4.1 CUDA, stage transport, and Pylon recovery remain separate.

## Full CPU transformer implementation phase (2026-10-20)

The owner requested full text-transformer composition and greedy decoding as
independent implementation work. The next children of #12369 are:

| Issue | Scope | Local implementation |
| --- | --- | --- |
| [#12385](https://github.com/OpenAgentsInc/openagents/issues/12385) | Complete text-block wiring | Named mHC projections and pre-mix propagation, collapse/norm/expand, routed/shared experts, and Engram lookup/gating with explicit arithmetic modes. Independent nonuniform block fixtures check intermediate residuals, attention input/mixes, router choices/weights, and output. |
| [#12386](https://github.com/OpenAgentsInc/openagents/issues/12386) | Full 40-layer CPU transformer | Embedding, both Engram layers, all 40 text blocks, final collapse/norm/head, explicit selected-layer traces, and request-owned state. Full synthetic-model logits and intermediate residuals/routing match independent NumPy equations. |
| [#12387](https://github.com/OpenAgentsInc/openagents/issues/12387) | Greedy decoding and pinned checkpoint runner | Deterministic argmax, EOS/output/context stops, cooperative cancellation, fresh-request isolation, and an offline pinned-checkpoint Rust example. Complete tiny-model tokens/logits match NumPy; 57 CPU tests and two runner argument tests pass. |

The [transformer fixture generator](../../scripts/dev/deepseek-v41-transformer-fixtures.py)
uses independent NumPy equations from the pinned public reference. Block
intermediates use `5e-6` absolute tolerance. Reference arithmetic rounds Engram
rows, projection output, and residual output to BF16; mHC coefficients and the
router projection remain float32. Experts use FP8 activations with their stored
FP4/FP8 matrices, and route weighting precedes the down-projection boundary.
All 48 accumulated CPU tests pass, including a connected BF16 Engram check.
Only selected experts and requested Engram rows are read. The local fixtures
qualify composition; the original checkpoint and device comparisons remain
with #12371's oracle and the after-close checks.

The complete transformer passes all 52 accumulated CPU tests. Full-model
float32 residuals, pre-mixes, router choices/weights, and last-token vocabulary
logits match the independent fixture within `2e-4 + 3e-5 * abs(expected)`.
Whole prefill and single-token replay agree; the connected BF16 model also
agrees exactly across split prefill. Both Engram layers and every routed/shared
FFN run in these tests. A failure in layer 39 rolls back all attention caches,
n-gram history, and positions. Invalid tokens, trace layers, and context lengths
are rejected before mutation. The forward result retains only the last token's
logits unless the caller explicitly requests selected-layer activation traces.
These are complete tiny-model checks, not original-weight qualification.

The greedy API starts each request from empty KV/indexer/hash state and uses the
smallest token ID for equal finite maxima. It rejects nonfinite logits and
invalid prompts, never truncates the prompt, and reports `eos`, `output_limit`,
`context_limit`, or `cancelled`. Generated IDs include EOS. The prompt plus
selected output cannot exceed the caller's context capacity; EOS wins when stop
conditions coincide, followed by the output limit. A zero output budget does
no forward work. `processed_tokens` counts tokens passed through all blocks;
the last selected token needs no further forward and is excluded. Cancellation
is checked between prefill chunks and decode calls, not inside a matrix
operation. Errors preserve the previous request, while successful stops keep
this request's processed state, including a cancelled partial prefill.

All 57 accumulated CPU tests pass. The independent NumPy model predicts
`[5, 0, 6]` for the fixed synthetic prompt `[0, 3, 4, 5]`; Rust matches those
IDs and all three full logit vectors within `2e-4 + 3e-5 * abs(expected)`.
Complete-model checks also cover EOS, ties, output/context limits, zero budget,
cancellation before/during prefill and during decode, repeated requests, and
rollback after a decode failure. Two offline runner tests reject conflicting,
duplicate, unknown, missing, and out-of-range arguments. The runner's `--help`
path runs without a checkpoint.

From the repository root, use the existing shared target for the Psionic Cargo
workspace, then run these focused checks:

```sh
cd crates/psionic
: "${CARGO_TARGET_DIR:?Set the existing shared Psionic workspace target}"
. ../../scripts/dev/cargo-build-env.sh
export CARGO_INCREMENTAL=0
cargo test -p psionic-models --lib deepseek_v41_cpu
cargo test -p psionic-models --example deepseek_v41_cpu_generate
cargo run -p psionic-models --example deepseek_v41_cpu_generate -- --help
```

For later original-weight checks, keep all 48 full shards and the pinned
`tokenizer.json` and `tokenizer_config.json` in `V41_CHECKPOINT`. These commands
are documented for that qualification; they were not run on original weights
in this implementation phase:

```sh
cargo run --release -p psionic-models --example deepseek_v41_cpu_generate -- \
  "$V41_CHECKPOINT" --check-only --verify-sha256
cargo run --release -p psionic-models --example deepseek_v41_cpu_generate -- \
  "$V41_CHECKPOINT" --prompt 'Write a short Rust function that adds two integers.' \
  --context 4096 --max-tokens 32 --prefill-chunk 4096 --oracle-tp8 --verify-sha256
cargo run --release -p psionic-models --example deepseek_v41_cpu_generate -- \
  "$V41_CHECKPOINT" --prompt-tokens "$V41_PROMPT_IDS" \
  --context 32768 --max-tokens 32 --prefill-chunk 32768 --oracle-tp8 --verify-sha256
```

`--prompt` uses the existing DS1 single-user chat renderer and special-token
encoding. `--prompt-tokens` reads a JSON array of raw token IDs, so oracle cases
can preserve their exact thinking/template input. The JSON report includes the
full checkpoint/tokenizer identity, per-shard hash-verification status, arithmetic
mode, limits, generated IDs/selected logits/text, stop reason, processed counts,
and preparation/generation timings. SHA-256 verification reads about 510 GB;
without the flag, the runner checks the pinned index, shard sizes/headers, tensor
map, and tokenizer but reports the shard hashes as unverified. `--check-only`
checks metadata/tokenizer and optional hashes without building a token map or
running generation.

The CPU implementation performs scalar row-at-a-time projections and positional
checkpoint reads. It does not expand the full checkpoint into float32 RAM, but
original-weight memory, speed, and numerical behavior remain unmeasured here.
The oracle agent has now completed #12371 in `fcfc9cf1ab`, including real B200
outputs. Its own cached decode and teacher-forced replay differ numerically and
include an argmax flip; real router ties also occur. Future comparisons must name
the matching reference mode and tie cases, retain the observed errors and token
mismatches, and establish measured numerical bounds before qualification. The
new CPU runner does not claim oracle parity, native CUDA, or Pylon serving.

## W1 tolerance policy correction (2026-10-20)

The original oracle report and earlier plan incorrectly described the
decode-versus-replay gap as a tolerance floor. That comparison crosses execution
modes and cannot justify differences within one mode. The original report is
retained unchanged as run evidence; the [W1 policy](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/tolerance-policy.json)
corrects its interpretation. The issue body and completion checklist use this
policy. It is committed before original-weight CPU comparisons.

Embedding values, integer addresses/hashes, and conversion codes/scales supplied
identical inputs must match exactly. Float32 mixing/router values use `2e-4`
absolute plus `2e-4` relative error. BF16 activations/residuals use `2^-6` absolute
plus `2^-6` relative error (two relative BF16 spacings). Float32 head logits use
`0.03125` absolute plus `2^-6` relative error, accounting for BF16-normalized
inputs. These are provisional engineering limits based on the arithmetic, not
measured oracle repeatability. Every element must pass and nonfinite values
fail. Limits cannot increase after failures. Missing coverage keeps G1 incomplete.
Router/index selections require exact sets except at oracle-recorded exact
selection ties; near ties are reported and are not exempt. Per-layer hashes
locate differences but do not replace comparisons of actual tensor values.

W1 adds `deepseek_v41_cpu_qualify`, an offline Rust example that streams selected
activations, exact Engram addresses, router sets, and per-layer BF16 hashes against
the full bucket dumps. Decode comparisons feed the oracle's fixed token sequence;
free greedy answers remain W2. Replay runs one complete forward over that sequence.
The example verifies each oracle tensor's SHA-256 before comparing values. Its
JSONL output survives a later failure, and missing tensors or limited runs remain
incomplete rather than passing. `--stop-on-first-failure` supports diagnosis;
`--max-steps` limits decode steps and never substitutes for full coverage.

`ModelObserver` borrows observations synchronously and preserves forward rollback
on an observer error. Checkpoint matrix rows run concurrently through Rayon while
each row retains its original float32 summation order. The temporary acceptance
Cargo wrapper in `scripts/bench/deepseek-v41/cpu_build.py` includes these exact
library modules by path and pins dependencies from the Psionic workspace lock;
it avoids compiling unrelated model stacks on the CPU VM. It contains no second
inference implementation. Local checks pass 58 CPU tests and two comparator
checks; these are preparation for the original-weight run, not a G1 pass.

## Block-scaled CUDA matrix-vector kernels (W4, 2026-10-20)

[#12391](https://github.com/OpenAgentsInc/openagents/issues/12391) adds
`psionic_backend_cuda::v41_matvec`, the first V4.1 CUDA kernels. Weights stay
on the device in their checkpoint form: FP8 E4M3 with one UE8M0 scale per
32×32 block, or packed FP4 E2M1 (the low nibble holds element `2i`) with one
UE8M0 scale per 32 values along K. Nothing expands them to float32.

- `CudaSubmission::v41_act_quant_fp8` quantizes float32 activations on the
  device under the CPU rule from #12383 and the reference's
  `act_quant(x, 32, "ue8m0")`: round to BF16, then per block of 32 a
  power-of-two scale `2^ceil(log2(max(amax, 1e-4) / 448))` and the nearest
  E4M3 code, ties to even, saturating at 448. A nonfinite block gets a NaN
  scale, so its products are NaN; the CPU path refuses such input.
- `CudaSubmission::v41_block_matvec` takes a `V41Matrix` (format, weight and
  scale buffers, rows, cols, and a stack count), FP8 activation codes and
  scales, and `V41Slots`. Each slot can take its matrix from the stack by a
  device-resident `i32` id and read a shared or its own activation row, so
  the six routed experts of a token run in one launch with no host round
  trip. An id outside the stack writes NaN. `V41Output::Bf16` rounds outputs
  to BF16 as the reference's GEMMs do. Both calls only enqueue work; they
  allocate nothing and never synchronize, so W7 can record them into a CUDA
  graph.
- Accumulation follows `fp8_gemm_kernel` and `fp4_gemm_kernel` in
  `inference/kernel.py`: float32 sums of code products per 32-wide K block,
  times the activation scale and then the weight scale, added in ascending
  block order. The reference sums inside a block on tensor cores; the kernel
  sums sequentially from element 0 with fused multiply-adds, which give the
  same result as separate operations because every code product is exact.
  `v41_block_matvec_host` reproduces the device arithmetic bit for bit.

**Tolerance.** Set before the first GPU run, from the arithmetic: the CPU path
and the kernel add the same exact terms `t_i` in different orders (one
sequential sum of K terms against 32-term block sums added in sequence), so
`|gpu - cpu| <= (gamma(K - 1) + gamma(K / 32 + 30)) * sum |t_i|` with
`gamma(n) = n u / (1 - n u)` and `u = 2^-24`. Exact equality is required where
the arithmetic is identical: against the host model, for activation codes and
scales, and for inputs whose every partial sum is exact.

**Results on the RTX 4080 (SM89).** Fixtures in
[`testdata/deepseek_v41_matvec/`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_matvec/)
hold 32 real rows each of `layers.2.attn.wq_a`, the shared expert's w1 and w2,
and routed expert 222's w1 and w2 (the expert the chat prompt's first decode
step routes to at layer 2), plus that step's real activations and the
reference's outputs. The `deepseek_v41_matvec_rows` example fetched them with
ranged reads and the #12373 loader (576 KB). All nine tests pass:

| Check | Result |
| --- | --- |
| Device activation codes and scales against the reference's `act_quant`, real FFN and down-projection inputs | Equal |
| Real rows against the CPU path, largest error over the bound | 0.0074 (shared w2); BF16 outputs equal in 160 of 160 |
| Expert 222's BF16 outputs against the reference's own `fp4_gemm` outputs | Equal in 64 of 64 |
| Synthetic shapes (K and N of 32 to 32,768, row and block tails, both formats) | Largest error over the bound 0.021; 25 of 106,610 BF16 outputs differ, each with a rounding boundary inside the bound |
| Exact-sum inputs, FP8 and FP4 | Equal to the CPU bit for bit |
| Device against the host model, including scales of 2^-127, 2^127, and NaN, saturated codes, NaN codes, and device-selected stacks | Equal bit for bit |

Single-row decode on the shared 4080 reads 424 GB/s for `wq_a` (6.6 MB),
511 for the shared w1, 548 for `wq_b`, 591 for `wo_b` (42 MB), 409 for one
FP4 expert, and 504 for six FP4 experts in one launch: 57–82% of the card's
717 GB/s. This is an observation; launches rotate over about 320 MB of copies
so reads miss the 64 MB L2, and the times include enqueue overhead. Small
matrices lose the most to launch and tail effects, which W7's CUDA graphs and
fused steps address. The full results are in
[`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_matvec/sm89-2026-10-20.json).

**For SM120 (W8).** The kernels use no FP8 conversion instructions, tensor
cores, or architecture-specific features: decoding is integer and float32
arithmetic, and the build disables fast math because E2M1 and E4M3 decoding
relies on float32 subnormals. On the RTX PRO 6000, confirm that the same
tests pass with `PSI_CUDA_ARCH=sm_120` and CUDA 13 (SM120 needs CUDA 12.8 or
later), that subnormals aren't flushed, and the decode bandwidth. Prefill
with many tokens runs the matrix once per slot; a batched tensor-core path is
separate work.

## W1 original-weight probe and TP8 correction (2026-10-20)

The first original-weight probe found layer-0 router-set differences at prompt
positions 13, 24, and 36 and a selected-weight error above the fixed bound. It
stopped after 28.56 seconds; this is failed diagnostic evidence, not G1. All 48
shard hashes passed beforehand. Separately, all 320 recorded expert input/down
FP8 conversion cases match the oracle's code and scale bytes exactly.

The oracle's TP8 arithmetic includes a boundary absent from the initial CPU
path: each rank's `wo_b` projection rounds its partial output to BF16 before
float32 reduction and the final BF16 cast. `Arithmetic::ReferenceBf16Tp8` makes
that profile explicit without introducing distributed execution. Quantized
products also retain the reference's 32-wide K-block accumulation. Qualification
uses this profile; the single-rank BF16 profile remains available. The corrected
implementation still needs original-weight verification under the unchanged
limits. Local checks pass 60 CPU tests and three comparator/cache tests.

The qualification runner caches decoded matrices for the current layer and
releases them when the layer changes. The head persists between steps; embedding
and Engram tables keep row reads. The cache and grouped projection use the same
library reduction helpers, with row parallelism and no reassociation within a
row. Reports include peak decoded-matrix bytes; this is not a total-process
memory measurement or a W3 result.

A further isolated check covers 560 original attention-normalization tensors.
The initial serial float32 mean differs from the oracle in 992 BF16 elements;
a CPU reimplementation of [PyTorch v2.13's CUDA row reduction](https://github.com/pytorch/pytorch/blob/v2.13.0/aten/src/ATen/native/cuda/Reduce.cuh)
and [MeanOps](https://github.com/pytorch/pytorch/blob/v2.13.0/aten/src/ATen/native/SharedReduceOps.h)
reduces that count to 17. This check supplies identical oracle inputs and reads
the original norm weights on the CPU VM. The TP8 profile now carries the forward
chunk's row count through attention, block, compressor, Engram, and final norms,
because CUDA's reduction layout depends on the batch shape. Float32 and the
single-rank BF16 equation APIs retain their previous reduction. This is a
numerical implementation fix, not a tolerance change; same-mode full-model
checks remain required. Local verification passes 61 CPU tests and three
comparator/cache tests.

## Mixing, Engram, and routing kernels (W6, 2026-10-20)

[#12393](https://github.com/OpenAgentsInc/openagents/issues/12393) adds
`psionic_backend_cuda::v41_mix`, the per-token kernels around W4's
matrix-vector products. Like W4's, every call only enqueues work: none
allocates or synchronizes, so W7 can record them into a CUDA graph. Callers
own every buffer, including the mixing projection's scratch.

| Call | What it does |
| --- | --- |
| `v41_hc_mixes(V41HyperConfig, V41HcWeights, residual, scratch, mixes)` | The joint RMS of the four streams, the F32 `hc_*_fn` projection, and the pre and post coefficients and the 20-iteration Sinkhorn comb, as `HyperConnections::mixes`. Two launches. Mixes are `[pre, post, comb[source, target]]`, 24 floats. |
| `v41_hc_split_sinkhorn` | The same mixes from already projected values. |
| `v41_hc_collapse(..., previous_mixes, ...)`, `v41_hc_expand` | Collapse with the previous sub-block's pre-mix, and expansion with `comb[source, target]`, optionally rounded to BF16. |
| `v41_engram_gather(V41EngramTable, ids, count, out)` | Reads the requested FP8 rows and their per-32 UE8M0 scales from page-locked host memory (`CudaHostBuffer`), one warp per row, and dequantizes them. The device `i64` ids come from the CPU's n-gram hashes. |
| `v41_engram_gate(V41EngramGate, residual, projected, q, k, out)` | The per-stream signed-square-root gate on the `wkv` output (keys, then value), as `EngramGate::apply`. |
| `v41_gate_logits(weight, input, logits, experts, cols)` | The BF16 router weight times the F32 input, in F32. |
| `v41_route(V41Router, logits, bias, ids, weights)` | `sqrtsoftplus` scores, the bias for selection only, top 6 of 384 with ties to the lower id, and weights from the unbiased scores, normalized and scaled by 1.5. Writes the ids as device `i32` in ascending order, the order the reference adds expert outputs, so they're the `V41Slots::ids` of the expert launches. |
| `v41_swiglu(gate, up, route_weights, slots, width, limit, out)` | Clamped SwiGLU (up clamped on both sides, gate above only, limit 10), times each slot's route weight before the BF16 cast and the down projection. Without weights it's the shared expert. |
| `v41_moe_combine(routed, slots, shared, width, out)` | The routed outputs in slot order from 0, then the shared expert. |

A token's MoE step runs on the device with no host round trip:
`v41_gate_logits`, `v41_route`, W4's `v41_act_quant_fp8` and
`v41_block_matvec` for w1 and w3 with `V41Slots { ids: Some(&ids), .. }`,
`v41_swiglu` with the route weights, the activation quantizer again, w2 with
per-slot inputs, the shared expert, and `v41_moe_combine`. The
`moe_step_runs_from_device_ids` test runs exactly that in one submission.

Each kernel repeats the CPU's operations in the CPU's order where that keeps
it parallel: element-wise steps, the Sinkhorn normalization, the four-term
stream sums, the top-k rule, and the expert sum. Long reductions use a fixed
parallel order, and `exp` and `log1p` run in double precision and are rounded
to float once. The `*_host` functions repeat the device arithmetic and match
it bit for bit, except in the rare case (about one evaluation in 2^28) where
a double-precision result falls within one rounding error of a float
rounding boundary. Nonfinite input writes NaN where the CPU path returns an
error: NaN mixes, a NaN Engram stream, and id -1 with NaN weights from the
router, which make W4's expert launches write NaN.

**Tolerances,** fixed before the first GPU run (the test file's header has
the derivation):

- Collapse, expand, the Engram gather, and the expert sum must equal the CPU.
- Reductions that add the CPU's rounded terms in another order:
  `(gamma(K - 1) + gamma(d)) * sum |t_i|`, `d` being the device's longest
  addition chain, as in W4.
- `exp` and `log1p` may differ from the platform's float functions by 2 ulp:
  12 u relative for a sigmoid and 6 u for a `sqrtsoftplus` score
  (u = 2^-24). Later steps carry differences through their derivatives or,
  for monotone steps, through their values at both ends of the interval.
- The Sinkhorn comb's relative error is carried in the norm
  `sqrt(sum x_ij e_ij^2)`; a normalization by a sum `s` grows it by at most
  `sqrt(1 / s)` and adds 10 u per entry, giving `sqrt(comb) * E + E^2` at
  the end.
- A BF16 output may differ only where its F32 bound reaches a rounding
  boundary. Expert ids must match exactly unless the bound lets a selected
  and an unselected expert swap; such cases are reported. Oracle ids are
  compared except at the oracle's exact ties.

**Results on the RTX 4080 (SM89).** The `deepseek_v41_mix_rows` example
fetched the fixture with ranged reads (9.5 MB): `layers.0.hc_attn_fn` and
`layers.39.hc_ffn_fn` with their scales and bases, the layer-14 router and
five router biases, the Engram query and key weights and the 24 table rows at
the oracle's hash ids for layers 1 and 14, and the chat prompt's first decode
step from the oracle. All 15 W6 tests and W4's 9 pass:

| Check | Result |
| --- | --- |
| Mixes on real weights and oracle residuals (layer 0 attention, layer 39 FFN) | Largest error 0.0008 of the bound; within 0.0006 of the W1 policy against the oracle's pre, post, and comb |
| Mixes on synthetic inputs (1 to 4 streams, residuals scaled by 1e-12 to 1e12, zero residual) and Sinkhorn on 37 identical-input cases (extreme, equal, and one-hot logits) | Largest errors 0.0007 and 0.0013 of the bound; NaN and infinite input give NaN mixes |
| Collapse and expand at layers 0, 1, 2, 14, 20, 21, and 39 from the oracle's inputs and mixes | Equal to the CPU; BF16 outputs equal the oracle's |
| Engram gather of the 24 rows at layers 1 and 14 from a host table whose other rows hold NaN | Equal to the CPU loader and to the oracle's BF16 rows; ids outside the table give NaN rows |
| Engram gate on the oracle's residuals with the real query and key weights (synthetic keys and value; the oracle records no `wkv` output), zero streams, a zero dot, and NaN | Largest error 0.39 of the bound; 20 BF16 outputs at a rounding boundary |
| Router logits from layer 14's real BF16 router | 0.0023 of the bound; ids equal the oracle's (one selected/unselected pair lies within the bound, and the ids still equal the CPU's) |
| Routing at layers 0, 2, 14, 20, and 39 from logits rebuilt from the oracle's scores | Ids equal the CPU's and the oracle's; this step has no recorded exact ties |
| Routing edge cases: ties, equal and extreme logits, the softplus threshold, top-1, 1,024 experts with top 32, NaN and infinite input | Equal to the host model; ties go to the lower id |
| SwiGLU of experts 222 (layer 2) and 180 (layer 20) on the oracle's w1 and w3 outputs | BF16 output equals the oracle's down-projection input in 2,304 of 2,304 values each |
| Expert sum against `moe::forward` | Equal |
| A whole MoE step from device ids, FP4 routed and FP8 shared experts | Equal to the host models' chain; ids equal `moe::forward`'s; output within 0.12 of the W1 BF16 policy |

One decode step's latency on the shared 4080, mean over 200 launches with
enqueue overhead: mixes 10.2 µs, collapse 1.8, expand 2.2, Engram gate 4.5,
router logits 6.4, routing 12.6, SwiGLU 3.6, expert sum 1.9, and the
24-row Engram gather 3.2 µs from both a 16 MiB and a 1,056 MiB host table;
reading the whole larger table over PCIe would take about 40 ms, so the
gather reads only the requested rows. Routing is the slowest: its six
selection rounds each synchronize all 1,024 threads. W7's CUDA graphs remove
most of the launch overhead. Evidence:
[`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_mix/sm89-2026-10-20.json).

**For SM120 (W8).** The kernels use no tensor cores, FP8 conversion
instructions, or architecture-specific features, and build without fast
math. On the RTX PRO 6000, confirm:

- `PSIONIC_REQUIRE_CUDA=1 cargo test -p psionic-backend-cuda --lib v41_mix`
  passes with `PSI_CUDA_ARCH=sm_120` and CUDA 12.8 or later, and the ignored
  `v41_mix_decode_latency`.
- Double-precision `exp` and `log1p` still round to the host model's values.
  SM120 workstation parts have reduced FP64 throughput; the kernels make at
  most 13,824 such calls a step, but measure the SwiGLU and routing times.
- Kernels reach page-locked host memory through unified addressing, and the
  gather time stays flat as the table grows. A real 101 GB table needs about
  101 GB of page-locked host RAM per Engram stage; registering a memory-mapped
  checkpoint file instead (`cudaHostRegister`) is W7 work.
- Float32 subnormals aren't flushed (Engram rows with small scales rely on
  them).

## Attention kernels (W5, 2026-10-20)

[#12392](https://github.com/OpenAgentsInc/openagents/issues/12392) adds
`psionic_backend_cuda::v41_attention`, the attention half of a layer around
W4's FP8 projections. As in W4 and W6, every call only enqueues work: none
allocates or synchronizes, so W7 can record them into a CUDA graph. Callers
own every buffer, including the window rings, the source caches, the
compressor state, and the candidate scratch. Positions are device `i32`
values, one per token, so one recorded graph serves every decode position;
the tokens of a multi-token call have consecutive positions.

| Call | What it does |
| --- | --- |
| `v41_rope(V41RopeTable, values, V41Steps, heads, head_width, back, inverse, output)` | Adjacent-pair rotation of each head's last 64 values at `p - back`, as `Rope::apply_tail`; `v41_rope_frequencies(V41RopeConfig::window(64))` (base 10,000) or `::compressed(64)` (base 160,000 with YaRN) makes the table. `back = ratio - 1` places a completed group at its first token; `inverse` rotates the attention output back. |
| `v41_cache_quant(kind, input, out, rows, width)` | `inplace_bf16`: per-32 FP8/UE8M0 (`Fp8`), per-32 FP4/UE8M0 (`IndexFp4`), or per-16 FP4/E4M3 (`CompressedFp4`). |
| `v41_window_store(geometry, input, steps, V41Ring)` | The rotated window KV in its FP8 form into the layer's ring at `p % rows`, as BF16 words. Every layer has its own ring. |
| `v41_dense(V41Dense, input, out, tokens, round_input, output)` | F32, BF16, or FP8 32×32-block weights times an input, each row summed in the CPU's order; `groups` makes it block-diagonal (`wo_a`). |
| `v41_rms_norm(input, weight, out, tokens, width, epsilon, output)` | The CPU's `rms_norm` with the BF16-rounded weight. |
| `v41_compress(V41Compressor, values, gates, norm, state, steps, latents)` | The gated softmax pool of each completed group (ratio 2, layers 2–19) or the value (ratio 1, layers 20–39), rounded to BF16, then the RMS norm. The trailing incomplete group stays in `state`, so any chunking gives the same latents. |
| `v41_compressed_store(geometry, ratio, V41IndexKeyWeights, rope, latents, steps, V41Source)` | For each completed group, source row `(p + 1) / ratio - 1`: the index key from the unrotated latent (`wk`, `k_norm`, RoPE at the group's first position, FP4/UE8M0) and the compressed KV (RoPE, FP4/E4M3). |
| `v41_index_scores(geometry, query, weights, V41Source, ratio, steps, scores)` | `sum_h max(q_h . k, 0) * bf16(w_h * scale)` for every visible key of the KV source, as the CPU's reference scores. |
| `v41_candidate_blocks(scores, ratio, steps, V41Candidates, 2048, 8)` | Layer 20's 2,048 best 8-position blocks by maximum score, the newest pinned in: a flag per block and the kept ids in ascending order. |
| `v41_select_indices(scores, ratio, steps, candidates, 512, out)` | The top 512 visible scores (from layer 20's flags on layers 24–36), ties to the lower position, in ascending order with -1 after them. |
| `v41_sparse_attention(geometry, query, sinks, V41Ring, V41Compressed, steps, out)` | The window rows oldest first, then the selected compressed rows of the source, in the CPU's 64-slot online softmax with BF16 weights and the sink in the denominator only. |

`V41LayerOwners::for_layer` gives each layer's KV owner (2, 8, 14, or 20),
selection owner (2, 8, 14, 20, 24, 28, 32, or 36), and candidate owner (20 for
layers 20–39), as `shared_state::LayerOwners`. A layer's decode step for W7:

1. W4's FP8 projections give the low-rank query, the heads, the window KV,
   and on index layers the index query; `v41_rms_norm` gives the norms and
   `v41_dense` the index weights (`weights_proj`, BF16 in and out).
2. `v41_rope` rotates the heads, the window KV, and the index query; then
   `v41_window_store` writes the window row, and `v41_cache_quant(IndexFp4)`
   forms the index query.
3. On a KV source layer: `v41_dense` for the compressor's values (and gates
   at ratio 2; `convert.py` promotes those to F32, and the CPU skips the
   BF16 rounding there), `v41_compress`, and `v41_compressed_store`.
4. On an index layer: `v41_index_scores`, on layer 20 also
   `v41_candidate_blocks`, then `v41_select_indices`. Other layers reuse
   their owner's selection buffer.
5. `v41_sparse_attention`, `v41_rope` with `inverse`, `v41_cache_quant(Fp8)`,
   `v41_dense` for the grouped FP8 `wo_a`, then W4's `wo_b`.

Each kernel repeats the CPU's `ReferenceBf16` arithmetic in the CPU's default
order: dot products, matrix rows, and RMS sums are sequential sums from
element 0; attention keeps the 64-slot blocks of `sparse_attention_bf16`
with the window rows oldest first; selection follows `select_indices` and
`candidate_mask`. W1's `ReferenceBf16Tp8` order keeps masked window slots in
the ring's order and sums each block's weights in a padded tree; switching to
it changes the slot list and the denominator update in
`v41_sparse_attention_kernel` and is left to W7. `exp`, `cos`, and `sin` run
in double precision and are rounded once. The `*_host` functions repeat the
device arithmetic and match it bit for bit. Nonfinite input writes NaN where
the CPU path refuses it, and a refused selection writes `i32::MIN`, which the
attention turns into NaN rather than reading.

**Tolerances,** fixed before the first GPU run (the test file's header has
the derivation, u = 2^-24):

- Exact: every kernel against its host model, and against the CPU for the
  cache quantizers, window store, dense products, RMS norms, ratio-1
  latents, index scores, selection, and candidate blocks.
- RoPE: `(|a| + |b|) 4u + 2u |out|` per rotated value (the trigonometry
  differs by at most 2u, and both sides round three times).
- The ratio-2 pool: `17u sum |v_r q_r|`. Where that reaches no BF16 rounding
  boundary, the latent must equal the CPU's; otherwise a fallback bound
  carries one BF16 step through the RMS scale.
- Attention: weight flips at BF16 boundaries plus `4Ku` of rescale error
  and the summation bound over the output and the denominator.
- Chains through RoPE (index keys and queries, compressed KV): each
  quantization block must equal the CPU's unless a value's RoPE bound reaches
  a BF16 boundary; such blocks are counted.
- Oracle selections must equal its `topk_idxs` (minus the window or prompt
  offset the reference adds) and `candidate_blocks`, except among values
  equal to the last selected one.

**Results on the RTX 4080 (SM89).** The `deepseek_v41_attention_rows` example
fetched the fixture with ranged reads (18.7 MB, allowlisted): the compressor,
index-key, and index-weight matrices of layers 2 and 20, four layers' sinks,
and from the oracle the attention inputs, compressor latents, scores,
selections, and candidate blocks of the four main prompts and the 127/128/129
prompts at their last prompt position and three decode steps. All 14 W5 tests
pass, and W4's 9 and W6's 15 still pass:

| Mode | Check | Result |
| --- | --- | --- |
| 1. RoPE and YaRN | 20,952 head cases (64×512, 1×512, 32×128) at positions 0–2, 127–129, 4,095, 16,383–16,384, 20,000, 65,535–65,537, and 2^20 - 1, both bases, forward and inverse, group positions; 21 real layer-20 latents | 0.47 of the bound; 1 BF16 value at a boundary; positions past 2^20 - 1 and NaN give NaN |
| 2. Window attention | Prefill of 301 positions in one launch, and decode through a 128-row ring past two wraps (positions 0–2, 63–65, 126–129, 200, 255–257, 300) | BF16 output equals the CPU's at all 316 tokens; ring rows equal the CPU's FP8 form |
| 3. Compression | Ratios 1 and 2 over 11 tokens in four chunkings (whole, by token, `3,1,2,1,4`, `2,5,4`), odd tails, NaN | Every chunking equals the host model and the others; latents equal the CPU's |
| 3. Compression | Real inputs: 7 ratio-2 groups (layer 2) and 21 ratio-1 groups (layer 20) from the oracle's attention inputs and real weights | Projections and latents equal the CPU's; latents within 0.44 of the W1 policy of the oracle's; 1 index-key and 2 KV blocks have a RoPE boundary in reach |
| 3. Compressed attention | Ratios 1 and 2, 0 to 700 rows, every 5th position masked, a bad index | BF16 output equals the CPU's in 26 cases; the bad index gives NaN |
| 4. Indexer | Index query RoPE and FP4 at 4 positions; scores and top 512 at 13 positions to 20,001 rows, ties, NaN, +inf, empty, all -inf | Scores and selections equal the CPU's; 4 query blocks have a RoPE boundary in reach |
| 4–5. Oracle selection | The oracle's scores at all 8 index sources and 28 steps (224 selections, up to 20,021 positions), with layer 20's device candidates for layers 24–36 | Equal to the CPU and to the oracle's `topk_idxs` and `candidate_blocks`; no tied index differs |
| 5. Candidates | 20,001 synthetic positions with ties, and a later indexer reading the flags | Equal to `candidate_mask` and `select_indices` |
| 6. Shared sources | One submission: 600 prefill tokens through sources 2 and 20, layers 2 and 20 indexing, layer 20's candidates, layer 24 indexing source 20's keys, and layers 3, 21, and 24 attending through the sources | Every stage equals its host model on the device's own inputs and the CPU components; nothing crosses to the host before the end |

The oracle records no query, window KV, compressed-KV history, `wkv` output,
or attention intermediate, so attention outputs can't be checked against it
directly: they're checked against the CPU component on synthetic and
device-produced rows, and the oracle checks the compressor, the scores'
selections, and the candidates. The oracle's selected indices address its KV
array: they carry an offset of the window (128) in decode and of the prompt
length in the prefill forward.

One decode step's latency on the shared 4080, mean over 100 launches with
enqueue overhead, in µs: RoPE of the query 3.0, window store 3.7, the
ratio-2 compressor 15.6, index FP4 form 1.9, compressed store 12.4, RMS norm
(1,280) 6.2, dense 512×5120 at 44.6 (F32) and 64.8 (BF16), and the 32×5120
index weights 64.8. With 2,048, 20,001, and 65,536 visible keys: index scores
14.7, 80.7, and 228; candidate blocks 21.0, 24.3, and 33.8; top-512
selection 25.0, 70.3, and 195; attention over 128 window and 512 selected
rows 82–84; window-only attention 21.6. The CPU's sequential order costs
time: a 5,120-long dot product is one dependent chain, and selection runs in
one block per token. W7 can trade the order for a summation tolerance (as W4
and W6 do) in the dense products and the attention scores if the step needs
it. Evidence:
[`sm89-2026-10-20.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_attention/sm89-2026-10-20.json).

**For SM120 (W8).** The kernels use no tensor cores, FP8 conversion
instructions, or architecture-specific features, and build without fast
math. On the RTX PRO 6000, confirm:

- `PSIONIC_REQUIRE_CUDA=1 cargo test -p psionic-backend-cuda --lib
  v41_attention` passes with `PSI_CUDA_ARCH=sm_120` and CUDA 12.8 or later,
  and the ignored `v41_attention_decode_latency`.
- Double-precision `exp`, `cos`, and `sin` still round to the host model's
  values, and the reduced FP64 rate doesn't dominate attention (about 2,600
  `exp` calls per attention block).
- Every kernel's shared memory (at most 39 KB per block) fits without an
  opt-in.
- Float32 subnormals aren't flushed (FP4/E4M3 scales and E4M3 decoding rely
  on them).

## W1 isolated checks and attention blocks (2026-10-20)

Isolated mixing checks compare all 3,360 recorded pre/post/comb tensors with
identical oracle residuals; all pass the original float32 bounds. The connected
chat prefill still fails at `decode.last.layers.1.hc.attn.pre` after the reduction
fix, with maximum absolute error `0.0011644363403320312` and no layer-0 router
set mismatch. This distinguishes upstream state differences from the mixing
operation's own arithmetic. The failure remains a failure under the frozen policy.

An isolated SwiGLU check covers 160 recorded experts. Matching PyTorch's explicit
`gate / (1 + exp(-gate))` order removes one BF16 intermediate mismatch; both
forms yield the same FP8 codes on these cases. The TP8 profile adopts that order.
The comparator also aligns combine weights by expert ID before comparing them,
and its first-failure option now applies to router and integer mismatches.
`deepseek_v41_cpu_generate --oracle-tp8` selects this same profile for later free
greedy runs; metadata-only checks reject that irrelevant generation option.

The TP8 attention profile preserves masked window and compressed-index slots.
The reference's 64-entry online-softmax blocks round their unnormalized weights
to BF16; removing masked slots can move valid entries across block boundaries
and change that rounding. Prefill keeps its fixed index width, and decode keeps
the ring's slot order even before the ring fills. A regression demonstrates the
resulting numerical difference. The profile also uses a padded float32 tree
sum of weights per block before updating the running denominator. These changes
still require original-weight comparisons; they do not assert exact CPU
reproduction of Tensor Core accumulation or CUDA transcendental instructions.
Local verification passes 63 CPU tests.

`deepseek_v41_cpu_qualify --experts-only` checks each captured expert's up, gate,
and down projections with identical oracle inputs, then its complete output
with the recorded combine weight. This diagnostic uses the frozen BF16 limits
and separately counts unequal BF16 values; it cannot pass G1 by itself.
The generation examples above use a whole prompt prefill to match the oracle;
chunking is a separately labeled comparison.

## W1 original-weight result (2026-10-20)

**G1 fails.** The [retained report](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/w1-2026-10-20.json)
records all eight oracle cases in both modes, including the status of every
recorded position. First-failure runs stop without committing model state;
later positions remain untested. Both complete chat diagnostics run all 40
layers through prefill and 32 teacher-forced tokens. They reach the vocabulary
head but exceed the frozen bounds: maximum logit errors are `3.668397903442383`
in decode and `3.5332261323928833` in replay. Decode matches 31 of 33 next-token
checks, with the first mismatch at step 20. These are not free greedy runs or
complete-answer qualification.

The first chat limit failure remains
`decode.last.layers.1.hc.attn.pre`: two of four values fail, with maximum absolute
error `0.0011644363403320312`. A repeat with 48 Rayon threads reproduces the
24-thread probe's tensor hashes and failure. This measures CPU repeatability at
that stopping point, not the oracle GPU's same-mode repeatability.

Identical-input diagnostics narrow the next investigation: 640 expert
projection/complete-output checks match BF16 values exactly; all 400 complete
FFNs pass the frozen bounds with identical router sets, although 2,245 BF16
values differ. All 3,360 mixing tensors pass. Connected state still diverges,
and observations for some oracle internals remain missing.
[#12404](https://github.com/OpenAgentsInc/openagents/issues/12404) tracks the
remaining cause, additional oracle capture, and complete G1 comparisons.
W2 (#12389) and W3 (#12390) depend on it. Closing W1's reporting work does not
pass G1 or qualify V4.1 CUDA, Pylon serving, or recovery.

The runner now refuses qualification for a partial prompt/mode selection or
incomplete forward, and failed or empty component diagnostics return an error.
Verification passes 63 CPU tests, five qualification-example tests, and two
generation-argument tests. Original-weight timings include observations and
concurrent diagnostics; they are not W3 performance measurements.

The `n2d-highmem-96` VM in `us-central1-c` ran for 2.28 hours, from
`2026-10-21T01:18:30Z` to `2026-10-21T03:35:06Z`. The spend guard records
`$12.455454` in compute (`$12.46` rounded). The VM and its 100 GB boot and
2,000 GB data disks are deleted. The compute ledger excludes storage and
network charges; the shared budget reserves $200 for those costs. All 48
original shard hashes passed before comparisons. Results, source, binaries,
and cleanup records are retained under
`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/cpu-qualification/w1/run-20261020-201630-7f29/`.
The committed compressed raw reports preserve every missing tensor name.

## W1 follow-up: normalization and deeper observations (2026-10-21 UTC)

**G1 still fails; #12404 remains open.** The [follow-up report](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/w12404-2026-10-21.json)
and its compressed raw evidence retain the failures and missing coverage. The
frozen policy hash remains `38ac6a059ae1385838d0188506912850bc29ba4ffc29f28dea592365deb036c5`.
All 48 original shard hashes passed again.

`f23547d141` removes an extra F32 rounding boundary from reference RMSNorm:
compute reciprocal square root before rounding to F32, rather than rounding
square root and its reciprocal separately. Across 560 identical-input oracle
tensors, unequal BF16 values fall from 17 to 3. Two retained scalar examples
test the BF16 ties that this fixes. This result does not establish bit parity
or fix the first connected attention failure.

`bc78a63ec9` adds observations for router scores, shared and selected experts,
Engram rows, compressor groups, index scores, selected positions, and candidate
blocks. Query normalization, projection, RoPE, sparse heads, and grouped output
boundaries are also observable. Index score masks are decoded and compared
exactly before applying the unchanged bounds to finite scores. Unexpected
nonfinite scores fail. Selection exceptions still require an oracle-recorded
exact tie.

| Check | Result |
| --- | --- |
| Router scores and biased scores on identical oracle inputs | 800 comparisons, zero failures |
| FP8 codes and scales on identical oracle inputs | 320 comparisons, all exact |
| Chat decode prefill | Same first reported layer-1 mixing failure; maximum error `0.0011644363403320312` |
| Full chat replay, all 40 layers and 73 input positions | 1,064 tensors checked; 638 fail; 926 router rows differ; three recorded exact-tie rows waived |
| Full replay vocabulary logits | All 33 tensors checked; maximum error `3.3777535557746887` |

The newly observed replay router score fails first at
`p1-chat-thinking.replay.g2.layers.0.ffn.gate.scores`, with maximum error
`0.00037348270416259766`. The identical-input router checks pass, which points
to incoming state drift. The attention operation that first introduces it
still needs direct evidence.

The full replay used `bc78a63ec9` and leaves 70 captures unobserved. Thirty are
expert projection names: `fc6bbce8d6` corrects their suffixes to `w1_out`,
`w2_out`, and `w3_out`, with a contract regression, but a fresh original-weight
run is still required. The other 40 are connected FP8 code/scale captures;
the isolated conversion checks are recorded separately. The other seven
required cases were not rerun. This run is diagnostic, not a W3 benchmark.

Sixteen guarded B200 allocation attempts in this run failed across multiple
zones, including capacity failures and an on-demand quota refusal. The oracle
extension is syntax-checked but has not run on a GPU. It captures rank partials,
records NCCL versions and controls, and pins the packages used for #12371.
The next capture must also identify the actual collective kernel: NCCL has
both BF16 pairwise addition and NVLS reductions with F32 accumulation in its
[reduction implementation](https://github.com/NVIDIA/nccl/blob/master/src/device/reduce_kernel.h).
That distinction prevents assuming one rank reduction from the tensor dtype.
TP8 indexer intermediate/rank arithmetic remains unresolved.

The dedicated CPU VM and its disks were deleted. Compute cost was **$3.302949**;
the existing $200 reserve covers noncompute costs. Source, binaries, locks,
raw comparisons, and cleanup evidence are retained under
`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/cpu-qualification/w12404/codex-12404-20261021/`.
Validation: 64 CPU library tests, six qualifier tests, changed-file formatting,
Python compilation, and shell syntax checks passed. W2 and W3 remain blocked
on #12404 and complete same-mode qualification.

## Device-resident stage (W7, 2026-10-21)

[#12394](https://github.com/OpenAgentsInc/openagents/issues/12394) adds
`psionic_backend_cuda::v41_stage::V41Stage`, one stage's whole step on one
GPU. **G4 does not pass:** the stage runs device-resident at the expected
speed, but its connected comparison with the CPU stage fails the frozen W1
policy from layer 1 on, and both the GPU and the CPU in the same order fail
against the oracle the way #12404 describes.

**The stage.** `V41Stage::load(backend, V41StageConfig, &dyn
V41TensorSource, Vec<V41EngramHostTable>)` uploads a contiguous layer range
in checkpoint form (FP8 and FP4 stay packed) and allocates all request state
on the device: each layer's window ring, the compressed KV and index keys
of every source the stage reads, the compressors' pending groups, the index
selections, layer 20's candidate flags, the mixes of every sub-block, and
the Engram n-gram history. `V41Stage::step(backend, &V41StepInput,
observe)` runs a prefill chunk or one decode token:

1. The step's input is uploaded once: token ids, the first position, a
   token counter, and from a previous stage the residual streams, the
   pre-mix, and the boundary's KV rows, index keys, selection, and
   candidate block ids.
2. One captured CUDA graph runs per token. It reads the token's index and
   position from device counters, so the same graph serves every decode
   position and every prefill token. Per token: the embedding (layer 0) or
   the input row, the Engram hash on the device, then per layer the Engram
   gather and gate (layers 1 and 14), the attention sub-block's mixes,
   collapse, and norm, W5's attention around W4's projections, the
   expansion, the FFN sub-block's mixes, collapse, and norm, W6's router,
   routed and shared experts and sum, and the expansion. The last stage
   runs a second graph once per step: the final collapse and norm, the
   head, and sampling (greedy, or temperature, top-p, and seed).
3. The host waits once and reads the next stage's state, or the token id.

New kernels in `v41_stage.cu`: the step input, the Engram hash (equal to
`NgramHashState` across steps and new requests), indexed row copies, the
counter, sampling, and candidate flags from block ids. `step` returns
`V41StepOutput` with the residual, the pre-mix, the KV source rows, the
selection, the candidates, the Engram history, and the token, which map
one to one onto W9's sections; `V41Boundary::at` mirrors
`boundary_sections`, and a test keeps them equal.

**The stage contract gains the pre-mix.** A layer's attention collapses
its streams with the previous layer's FFN pre-mix, which W9's contract
didn't carry. `SectionKind::PreMix` (F32 `[tokens, streams]`) now crosses
every boundary, 48 bytes per token; the G6 numbers in the completion doc
include it.

**Decisions,** recorded before the first GPU run:

- **Summation order: the default `ReferenceBf16`, not
  `ReferenceBf16Tp8`.** W4–W6's kernels are qualified against the default
  order, and TP8 is still being corrected on #12404 without passing G1. In
  the default order a chunk equals its tokens one at a time, so prefill can
  replay the decode graph. TP8 changes `wo_b` into eight BF16 rank
  partials, reduces norms, mixes, and the Engram gate in ATen's layouts
  (which depend on the forward's row count), keeps masked window slots in
  ring order with tree-summed block weights, and computes SwiGLU with an
  explicit division. Adopting it is a contained kernel change, but prefill
  would then need the real chunk's row count, not per-token replays.
- **Dense products keep the CPU's sequential order** (no bounded faster
  sum), so their parity stays exact. Measured cost below: it's the largest
  item in a decode step.
- **Engram tables live in page-locked RAM** (`cudaMallocHost`), filled
  from the shard file in 256 MiB reads, rather than `cudaHostRegister` on
  a memory-mapped checkpoint. Every row can be hit, so all 101 GB stay
  resident either way; long-term pinning of file-backed pages through the
  driver depends on the kernel and file system; and the startup cost is the
  same 101 GB read. Filling one table took 115 s from a hyperdisk.
- **Prefill replays the per-token graph.** W4's kernel reads the weights
  once per slot either way, and W6's mixing and routing kernels take one
  token. Batched prefill needs a tensor-core GEMM path and is separate work.

**Checks on the RTX 4080 (SM89)**, a synthetic model with V4.1's 40-layer
layout and small shapes (`v41_stage_tests.rs`; tolerances in its header):
the CPU stage used for parity, composed from the CPU path's components,
equals the CPU transformer bit for bit; the GPU stage matches it within
0.015 of the policy with 98.5% of values bit-equal and the same greedy
tokens; splits at 5, 13, 21, and 30 (every section kind) equal the unsplit
stage bit for bit; decode waits once per step. A real layer didn't fit: the
4080 had 5.1 GB free beside production, and a layer with all 384 experts
needs about 8 GB.

**Checks on one RTX PRO 6000 (SM120)**, a `g4-standard-48` spot machine,
CUDA 12.9, layers 0–4 (embeddings, the layer-1 Engram, layer 2's ratio-2
source and indexer, and layers 3–4 reading them) and layers 0–9 (adding
layer 8's source). Layers 20–24 can't be checked from the oracle, which
records layer 20's input at five positions only, not the whole prompt.
`scripts/bench/deepseek-v41/stage_vm.sh` copied only the stage's shards;
the others are sparse stand-ins with their real headers. Results:
[`sm120-2026-10-21.json`](../../crates/psionic/crates/psionic-backend-cuda/testdata/deepseek_v41_stage/sm120-2026-10-21.json),
raw logs in `gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/w7/run-20261021-050705/`.

| | Layers 0–4 | Layers 0–9 |
| --- | --- | --- |
| Decode, chat prompt, mean of 31 steps | 4.13 ms per token | 8.06 ms |
| Decode after the 2,124-token prompt | 4.21 ms | 8.36 ms |
| Prefill, 2,124 tokens | 242 tokens/s | 122 tokens/s |
| Device memory in use | 39.5 GB (38.6 GB weights) | 76.6 GB (75.6 GB weights) |
| Engram table fill, stage load | 115 s, 63 s | 197 s, 118 s |
| Wire bytes at the boundary | 44,348 (43,324 on even positions) | 44,348 |

**Round trips.** `nsys` on the layers 0–4 decode: each step is one
`cudaGraphLaunch`, one `cudaStreamSynchronize`, and about six small
`cudaMemcpy` calls (the token, position, and counter in; the residual,
pre-mix, selection, and on odd positions a KV row out). No kernel is
launched outside the graph. Kernel time is 4.06 ms of the 4.3 ms between
launches. The sequential dense products take 1.70 ms of it (`wo_a` on every
layer, the compressor and index weights on layer 2) and the RMS norms 0.43
ms; W4's projections take 1.31 ms. A bounded parallel sum for those two
kernels, as W4 and W6 already use, is the first speedup.

**Parity.** The chat prompt, prefill and 32 teacher-forced decode steps,
layers 0–4:

- **GPU against the CPU stage, same `ReferenceBf16` order:** layer 0 and
  every token's Engram ids and rows and layer 2's selections are within the
  policy (selections and ids exact). From layer 1, token 1, the connected
  run fails: a float32 attention mix differs by 3.8e-4 against a 3.5e-4
  limit, then router sets differ (25 of 438 ids at layer 1) and the
  differences grow. Layer 0's output already differs from the CPU's in 14%
  of its BF16 values, within the policy; W4's and W6's reordered sums
  (bounded per kernel) start it, and the mixes amplify it.
- **Against the oracle:** the GPU fails 180 of 625 checks and the CPU stage
  in the same order 191, with the same first failure (layer 1's router set
  at position 0) and nearly the same pattern per label and layer. Layer 0's
  router sets differ at 3 of 73 positions on both, where #12404's TP8 CPU
  has none, so the default order itself is part of the gap.

G4 is therefore failed for the connected comparison and pending on #12404:
the stage's arithmetic should move to whatever order #12404 qualifies, and
an isolated per-layer check (the CPU fed the GPU's layer inputs and state)
is needed to separate amplification from a defect.

**For W11.** Run each stage process with one `V41Stage`; it holds one
request at a time, and a request starts at position 0 (which resets the
n-gram history). Chunk the prompt to `max_step_tokens`. Map W9's sections
to `V41StepInput` and `V41StepOutput` (BF16 residual on the wire, F32 in
the executor; candidate ids as `u32` with `u32::MAX` for -1). The last
stage returns `token`, which W10's `StepAck` carries. A stage with an
Engram layer needs 101 GB of host RAM for the table. The CUDA 12.9 image
needed an include shim for CCCL at the time; W8 removed the need.

W11 part 2a (#12398) wires this in: `psionic-v41-stage --backend cuda`
runs one `V41Stage` per process as W9's executor, opens only that stage's
shards, and announces `psionic-v41-stage/1+cuda`. On the RTX 4080, CUDA
process groups of two and three stages over loopback match the unsplit
CUDA stage token for token. The one-machine G5 run on a G4 machine is
part 2b.

**Cost:** one `g4-standard-48` spot machine for 0.47 h, $0.79 in the
guard's ledger, deleted.

## #12404: B200 capture, device arithmetic, and the next divergence (2026-10-21 UTC)

**G1 still fails, with a new first cause.** G1 qualifies the
`ReferenceBf16Tp8` profile: the eight-B200 TP8 oracle's arithmetic, including
reductions whose layout depends on how many rows a forward processes. The
default and single-rank BF16 orders stay available but are not what G1 checks;
GPU stages that must match G1 follow `ReferenceBf16Tp8`.

### Capacity

The previous session's 16 guarded B200 requests had failed. This session swept
`a4-highgpu-8g` spot through the spend guard, starting with the zones that list
the machine type: us-central1-b refused with `ZONE_RESOURCE_POOL_EXHAUSTED`
(STOCKOUT) at 04:44:48Z, and us-east1-b granted capacity at 04:45:02Z on the
second attempt. Flex-start, Vertex AI, and other GPUs were not needed. For a
later shortage, the guard now takes `--flex-start --queue-hours H`: it queues
the request through the Dynamic Workload Scheduler and counts the queue time
plus the run at the A4 flex-start price ($64.44/h, Billing Catalog
2026-10-21). The spot machine ran 0.67 h for **$26.55** and was deleted.

### Capture and repeatability

`oracle_vm.sh capture` reran all eight cases on the same driver (580.178.04),
CUDA 12.9, and package versions as #12371, with `--attention-diagnostics`
(query/KV normalization, projections, RoPE, quantized KV, heads, the `wo_b`
input, and every rank's `wo_b` and indexer partial), every row of the chat
prompt, rows 419–423 of the 2k prompt and 189–193 of the 20k prompt, and
NCCL tuning logs. Uploaded to
`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/w12404-capture/`
(554,839 tensors, 4.98 GB, with a manifest).

**Same-mode repeatability is exact.** `oracle_repeat.py` compares the rerun
with the #12371 dump: all 18,000 shared tensors, every per-layer residual hash
(1,360 per main prompt, 400 per edge case), every decode logit hash, and every
generated token are equal. The oracle grants no repeatability allowance; the
frozen limits stay as they are. NCCL used ring LL over ranks 0..7 below
2.76 MB and NVLS (switch reduction) above.

### Device arithmetic, measured

Probes on the same B200 (`probe/` beside the capture) pinned what the oracle's
kernels compute:

| Operation | Finding | Evidence |
| --- | --- | --- |
| Legacy BF16 `mma.sync.m16n8k16`, F32 accumulate | 16 exact products plus the accumulator, aligned on the largest exponent where a product's exponent is the sum of its factors' exponents, truncated toward zero 25 bits below it, summed exactly, rounded toward zero | 0 of 10,240,000 random outputs differ; handcrafted tests |
| `MUFU.EX2` on [0, 1) | Reads the input as a 23-bit fixed-point fraction; 2^23 distinct results, 39% differ from round-to-nearest `2^x` | Full table; 0 of 2,000,000 random inputs differ |
| `expf` (no fast math) | `FFMA.SAT`, `FFMA.RM`, two FMAs, `MUFU.EX2`, and an exponent shift, from `sm_100` SASS | Generated code |
| TileLang `sparse_attn` | One MMA chain per 16 keys or dims, CUDA `expf`, butterfly sums across warps then quad lanes, FMA for the running denominator, IEEE division | Kernel source from the TileLang cache |
| FP8 `mma.sync.m16n8k32` | One fused sum per 32-wide block from zero; the model above matches captured GEMMs, but random probes differ in 90 of 384,000 outputs, so it is not exact | Probe data |
| cuBLAS BF16 `wo_a` einsum | Prefill chunks match tensor-core order over K; one-row decode steps match the exact sum rounded once | Capture, identical inputs |

### Fixes (b390516b13)

The TP8 profile now uses these models (`deepseek_v41_cpu::sm100`): sparse
attention follows the TileLang kernel, FP8 projections sum each 32-wide block
in one fused step, and the `wo_a` einsum follows the chunk shape. The `MUFU.EX2`
table ships as 100 KB of gzipped two-bit increments and is checked against its
SHA-256 when loaded. Regressions use original-weight oracle values where the
earlier sequential sums differ: a layer-0 window-attention row (one BF16 value),
a `wq_b` row, and a `wo_a` row. The qualifier observes each rank's `wo_b`
partial and caches parsed safetensors headers, which made diagnostic captures
practical to compare. Float32 and single-rank BF16 APIs are unchanged; the
earlier TP8 attention remains as `sparse_attention_bf16_tp8_sequential`.

**First unequal values, before and after, on the chat prompt's decode prefill
against the capture.** Before the fix, the first unequal BF16 value was a
`wq_b` output (position 0, rank 6), followed by `wo_a` outputs and attention
heads; `wo_b` rank partials then differed in up to 60 values. After it, layer
0's query, KV, heads, `wo_a` output, rank partials, and attention output equal
the oracle at all 41 positions. The remaining layer-0 difference is the
hyper-connection mix: `pre`, `post`, and `comb` differ by about one F32 ulp
(at most 2.4e-6) on identical inputs, so the expanded stream flips BF16
values, and the FFN's FP8 inputs change. The first limit failure moves to
`decode.p6.layers.1.hc.attn.pre` (8.8e-4), and router sets differ from layer 1
on (621 of 1,640 rows). One RMSNorm output (`p22.layers.0.attn.in`) also
differs in two values; CUDA `rsqrtf` is unmeasured.

The mix uses torch `rsqrt`, a cuBLAS F32 GEMM (K = 20,480, N = 24), and the
TileLang Sinkhorn kernel. Emulating that kernel from its source, with the
projection's exact sum and a correctly rounded `rsqrt`, reproduces only about
12% of captured rows, so the GEMM order and `MUFU.RSQ` need measurements.
[#12405](https://github.com/OpenAgentsInc/openagents/issues/12405) owns that
work, followed by the FFN (gate GEMM, `softplus`, the MoE all-reduce order),
Engram, the compressor, the indexer, and the head.

### Rerun (Part C) and cost

Against the #12371 dump, with `fdd4db7794`'s qualifier (`b3`) and the frozen
policy. The chat, code, and four edge cases ran completely in both modes, with
every recorded position and full logits; the 2k and 20k prompts ran to their
first failure in both modes, as W1 did. The
[report](../../crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/cpu_qualification/w12404b-2026-10-21.json)
keeps every run.

| Case | Mode | Failed tensors | Router rows failed | Max logit error | Next tokens equal | First failure |
| --- | --- | --- | --- | --- | --- | --- |
| p1-chat-thinking | decode | 696 of 1,139 | 936 of 2,920 | 3.96 | 30 of 33 | layer 1 |
| p1-chat-thinking | replay | 696 of 1,134 | 981 of 2,920 | 4.03 | — | layer 1 |
| p2-code | decode | 695 of 1,141 | 886 of 2,880 | 4.97 | 33 of 33 | layer 1 |
| p2-code | replay | 698 of 1,134 | 921 of 2,880 | 3.80 | — | layer 1 |
| e-window-127 | decode / replay | 684 / 678 | 1,771 / 1,811 of 5,400 | 2.76 / 1.91 | 8 of 9 | layer 1 |
| e-window-128 | decode / replay | 666 / 656 | 1,809 / 1,831 of 5,440 | 3.91 / 6.42 | 8 of 9 | layer-0 router scores |
| e-window-129 | decode / replay | 660 / 653 | 1,838 / 1,842 of 5,480 | 2.40 / 6.42 | 9 of 9 | layer 1 |
| e-engram | decode / replay | 682 / 677 | 1,246 / 1,154 of 3,040 | 4.31 / 4.68 | 9 of 9 | layer 1 |
| p3-2k | both, to first failure | 0 of 32 / 56 | layer-0 router, position 1199 | — | — | was position 421 |
| p4-20k | both, to first failure | 0 of 32 / 56 | layer-0 router, position 191 | — | — | unchanged |

Every complete run observes every oracle tensor, including the connected
expert FP8 codes and scales, which are compared exactly. Exact-tie waivers
apply only to oracle rows with a zero margin. The 20k prompt's position 191 has
an oracle margin of 1.1e-4, which is not a tie. End-to-end errors are about as
large as W1's because the mix difference still compounds through 40 layers;
the fix shows in the layer-0 boundaries, not yet in the logits.

Cost: the A4 spot machine $26.55 (0.67 h) and the `n2d-highmem-96` CPU machine
$22.23 (4.06 h), both deleted with their disks; storage stays within the
reserve. Evidence: the capture and probes under `w12404-capture/`, and runs,
binaries, source, and fitting scripts under
`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/cpu-qualification/w12404/claude-12404-20261021/`.

**For the CUDA stages (W7, W8):** match `ReferenceBf16Tp8`. Its attention now
follows `sm100::sparse_attention`, FP8 projections use one fused sum per 32
inputs, and `wo_a` depends on whether the chunk has one row; the API is
additive, and the earlier TP8 attention remains as
`sparse_attention_bf16_tp8_sequential`.
