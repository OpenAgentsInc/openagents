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
the `inference/` reference sources at that revision on 2026-10-20. We didn't
download the weights.

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
| V0: Oracle | Pin the reference sources. On a rented 8-GPU machine, run `inference/generate.py` (TP8, FP4 experts) on a fixed prompt set and dump per-layer inputs and outputs for selected layers and tokens: embeddings, Engram output, each attention variant, indexer selections, Sinkhorn mixing, router ids and weights, expert outputs, logits. Store digests and small slices in the repository and full dumps in a bucket. | Golden fixtures cover layers 0, 1, 2, 14, 20, 21, and 39; the run's environment, revision, and costs are recorded; the VM is deleted. |
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
| `g4-standard-384`, 8× RTX PRO 6000 Blackwell, PCIe | 768 GB | Everything, with about 250 GB to spare | $36.04/h on demand ([Muse serving notes][Muse]) | Recommended first target: eight stage Pylons over loopback on one machine, with the same card a contributor could own. Spot quota is 16 GPUs per region. |
| `a4-highgpu-8g`, 8× B200, NVLink | about 1,440 GB usable | Everything | $39.63/h spot ([Muse serving notes][Muse]) | Throughput work (V9) and the oracle run. Spot only; quota 64 B200 per region. |
| `a3-ultragpu-8g`, 8× H200 | 1,128 GB | Everything | $83.37/h on demand ([Muse serving notes][Muse]) | Fallback if G4 and A4 spot capacity is unavailable. |
| Several single-GPU G4 machines, 1× RTX PRO 6000 each | 96 GB each | Eight machines hold everything, with Engram in host memory | Not recorded | Stage Pylons across machines (V8). Check each machine's host RAM for the Engram stages. |
| RTX 4080 (coderos-4080) | 16 GB | One layer's tensors at a time | Owned | Kernel parity tests (V5). |

Before renting, check the current spot price and capacity, and set a cost cap
and an expiry. Delete or stop the machine when the bounded run ends, and
label it with the run (`agent-run.py label-vm`).

The full CPU reference (V4) needs about 510 GB of readable weights. It can
memory-map them from local SSD on a smaller machine, more slowly, or run on a
host with enough RAM: a `g4-standard-384` host has 1,440 GiB and an A4 host about 3.9 TiB, so the GPU machine can also run the CPU reference.

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
IEEE-754 bit patterns in row-major order. The oracle run (#12371) writes it.
Opening every tensor's bytes and the parity check run on that machine; until
then they are listed under After-close checks in the
[master delegation](../audits/2026-10-16-master-delegation.md#after-close-checks).

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
| [#12379](https://github.com/OpenAgentsInc/openagents/issues/12379), P1.4a/P1.5a | Attention and selection primitives | Filed; implement after #12378. Low-rank projections, RoPE/YaRN, activation quantization, and cross-layer source-cache wiring remain later work. |
| [#12380](https://github.com/OpenAgentsInc/openagents/issues/12380), P1.6 | Engram | Filed; implement after #12379. |

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
