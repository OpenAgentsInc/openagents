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
  --context 4096 --max-tokens 32 --prefill-chunk 128 --verify-sha256
cargo run --release -p psionic-models --example deepseek_v41_cpu_generate -- \
  "$V41_CHECKPOINT" --prompt-tokens "$V41_PROMPT_IDS" \
  --context 32768 --max-tokens 32 --prefill-chunk 128 --verify-sha256
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
