# DeepSeek-V4.1-Flash inference system and DeepInfer

- **Source:** [DeepSeek, Zhihu column post](https://zhuanlan.zhihu.com/p/2095560295930508953),
  edited 2026-10-20.
- **Author:** DeepSeek (Zhihu account @DeepSeek).
- **Read on:** 2026-10-20. Zhihu refused direct and headless-browser
  fetches (HTTP 403), so the text source was an English machine translation
  that the owner supplied. The four images were fetched directly from
  Zhihu's image CDN and read by hand.
- **What this is:** a technical summary in our own words, organized by
  topic. It is not a translation. Short quotes are marked. The article's
  images are not stored in this repository; each is linked, described, and,
  for charts and tables, its numbers are transcribed as data.

Related reviews: [What Psionic should learn from DeepInfer](../../psionic/deepinfer-lessons.md)
(the released engine's source), the
[native DeepSeek-V4.1-Flash plan](../../psionic/deepseek-v41-native.md), and the
[DeepSeek support plan](../../psionic/deepseek-support-plan.md).

## Goals and headline numbers

DeepSeek describes the production service behind V4.1-Flash and says it
designed the system around three goals:

1. Run on chips from more than one vendor.
2. Do prefill on small groups (8 cards) and decode on a 64–128-card
   supernode.
3. Keep a request's KV cache for more than 72 hours after it goes idle.

Alongside the article, DeepSeek open-sourced
[DeepInfer](https://github.com/deepseek-ai/DeepInfer) (the engine) and
[deepseek-recipe](https://github.com/deepseek-ai/deepseek-recipe) (API and
prompt handling). The production cache storage and the cluster load
balancers aren't part of the DeepInfer release; our
[DeepInfer review](../../psionic/deepinfer-lessons.md#what-the-release-actually-contains)
records what the clone contains.

Headline claims:

- One cluster processes about 155 trillion tokens a day.
- At off-peak prices, revenue averages $38 per card-hour.

## System design

### Architecture diagram

Image: <https://picx.zhimg.com/v2-9dcb32fbcfa3caa70540e88955eafd81_1440w.jpg>

A block diagram of the request and KV-cache paths:

- A user request arrives as JSON at **DeepSeek Recipe**, which turns it into a
  token list plus sampling parameters. Responses travel back the same way as
  a token list that Recipe turns into JSON.
- An **Entrypoint** tier (several instances) sends the request to the
  **Prefill Load Balancer**, which picks a **Prefill Service** instance.
- Prefill sends image work through a **Vision Encoder Load Balancer** to a
  **Vision Encoder Service**, whose output returns to prefill.
- Prefill hands the request to the **Decode Load Balancer**, which picks a
  **Decode Service** instance. Decode results flow back to the Entrypoint.
- Each prefill and decode instance contains its own **Expert-Parallel Load
  Balancer**.
- KV cache moves from prefill to decode directly, from decode into **External
  KVCache Storage (3FS)**, and from storage back into prefill.

### Parallelism and disaggregation

- **MoE expert parallelism stays inside one node or supernode.** EP traffic
  never crosses the scale-up domain ("partial EP").
- **Three disaggregated roles:** vision encoder, prefill, and decode, each
  scaled on its own.
- **Attention uses data parallelism with as many DP ranks as EP ranks.** Each
  card runs attention for its own requests and holds a share of the experts.
- **Prefill** uses EP2–EP8 (equal DP). DeepSeek picks the smallest size that
  fits in memory, which keeps a prefill group cheap to schedule.
- **Decode** uses wide EP, EP64–EP128, as large as the supernode allows, so
  each card holds few experts and batches many requests.
- **Vision encoding** runs on one card, the cheapest type available.
- Around the engines sit four load balancers (prefill, decode, vision, and
  expert) and the KV-cache store.

## Measured performance

### Online statistics table

Image: <https://pic2.zhimg.com/v2-7dd6fca82884c12b56d4d8b40043c979_1440w.jpg>

A table titled (in Chinese) "Actual online running data", with a note that
the machines keep some headroom for traffic swings.

| Stage | Metric | Measured online |
| --- | --- | --- |
| Prefill | Throughput per card, excluding cache hits | 60K tokens/s |
| Prefill | Throughput per card, including cache hits | 1.5M tokens/s |
| Prefill | Average input cache hit rate | 96% |
| Prefill | Time to first token, mean | 400 ms |
| Prefill | Time to first token, P99 | 1,000 ms |
| Prefill | Average input length | 77K tokens |
| Decode | Throughput per card | 14K tokens/s |
| Decode | Output speed per user | 230 tokens/s (300 tokens/s for coding) |
| KV cache | Retention after a session goes idle | 72 hours |

Footnote: the deployed prefill-to-decode ratio is close to 1:1, and,
ignoring peaks and troughs, revenue averages $38 per card-hour at off-peak
prices.

### Offline prefill benchmark

Image: <https://pic1.zhimg.com/v2-bff36102d7ffd58ffca11ad64738012a_1440w.jpg>

A line chart, "Prefill: Non-cached Input Throughput vs. Latency". Setup line:
EP4DP4, 384+32 experts, context 131,072, decoder bounded replay of 140
tokens. Each point is labeled with the prefill batch size in tokens per DP
rank. Values read from the chart (about ±2%); throughput times latency
reproduces the batch size, which supports the readings:

| Batch (tokens per DP rank) | Latency per prefill step | Non-cached throughput per GPU |
| --- | --- | --- |
| 4,096 | ~62 ms | ~66,400 tokens/s |
| 8,192 | ~108 ms | ~75,900 tokens/s |
| 16,384 | ~194 ms | ~84,300 tokens/s |
| 32,768 | ~378 ms | ~86,700 tokens/s |
| 65,536 | ~756 ms | ~86,700 tokens/s |

Throughput saturates at about 32K tokens per rank; doubling the batch beyond
that only doubles the latency.

### Offline decode benchmark

Image: <https://picx.zhimg.com/v2-c1c07ce013c2dc0b0a3864c566bad801_1440w.jpg>

A line chart, "Decode: Output Throughput vs. Interactivity". Setup line:
EP64DP64, 384+64 experts, context 131,072, `nextn=3` fixed, per-token
acceptance 75%, giving τ=3.25. Each point is labeled with the number of
requests per DP rank. Values read from the chart; requests times per-user
speed reproduces the throughput:

| Requests per DP rank | Per-user speed | Output throughput per GPU |
| --- | --- | --- |
| 8 | ~325 tokens/s | ~2,600 tokens/s |
| 16 | ~315 tokens/s | ~5,050 tokens/s |
| 32 | ~297 tokens/s | ~9,500 tokens/s |
| 64 | ~246 tokens/s | ~15,700 tokens/s |
| 128 | ~179 tokens/s | ~22,800 tokens/s |
| 256 | ~103 tokens/s | ~26,300 tokens/s |

The online figures (14K tokens/s per card at 230 tokens/s per user) sit
between the 64- and 128-request points.

Notes on the chart setups:

- τ=3.25 equals 1 + 3 × 0.75, so the chart treats the three draft
  positions as accepted independently at 75% each. Prefix acceptance at a
  constant 75% would give about 2.73.
- 384+64 experts on 64 cards is one redundant expert per card (7 per card),
  matching the text. 384+32 on 4 prefill cards is 8 redundant per card,
  while the text says 4 per card for prefill. See
  [Open questions](#open-questions).
- The prefill chart replays 140 tokens; the text describes a 128-token
  replay. DeepInfer's README also suggests 140.

## DeepSeek Recipe

Recipe is the front door between the public APIs and token lists. It's
written in Rust with Python bindings.

- **One dialogue format.** Chat Completions, Responses, and Anthropic-style
  Messages requests are all mapped onto one internal conversation format
  before rendering.
- **Special tokens by boundary, not by spelling.** Specials that the
  template inserts are mapped to their token IDs directly. Text that comes
  from a user, a tool result, or a tool argument and happens to spell a
  marker such as `<｜User｜>` or `<｜image｜>` is encoded as ordinary text, so
  a prompt can't forge a role boundary. On output, `</think>` and the DSML
  tool-call markers are recognized by token ID, not by matching the text.
- **Parallel encoding.** Because BPE never merges across a special token, the
  text segments between specials are tokenized independently and in
  parallel.
- **A streaming parser that works across chunk boundaries.** It buffers
  incomplete UTF-8 sequences, matches multi-token delimiters with a KMP
  matcher that carries state from one chunk to the next, emits a tool's name
  as soon as it's known, escapes string arguments into JSON piece by piece,
  and passes non-string arguments through unchanged.

## Load balancing

### Prefill

- **Cost model.** The cost of a prefill is a quadratic function
  cost(q, c), where q is the number of uncached tokens and c is the length
  of the already computed prefix that they attend to.
- **Length buckets.** Requests fall into four buckets by length: under 16K,
  16K–64K, 64K–256K, and 256K or more. Each bucket's share of capacity is
  re-set from its share of the previous round's quota.
- **Pull, not push.** Engines pull work by remaining quota. There's no prefix
  affinity, because every engine reads the same shared cache store and can
  hit any prefix.
- **Inside a DP group,** a request goes to the least-loaded rank.
- **Chunk size** is solved from a per-step time budget expressed in
  nanoseconds, so a long prompt is cut into chunks that each fit the budget.

### Decode

The decode balancer balances two quantities at once: the number of requests
and the total KV length on each instance. The article cites the DualPath
paper (SIGCOMM '26) for this.

### Vision

A vision request goes to a random machine, then to the card on it with the
fewest pending image patches.

### Expert-parallel balancing with redundant experts

| Setting | Prefill | Decode |
| --- | --- | --- |
| Redundant experts per card | 4 | 1 (EP64 gives 7 experts per card) |
| Load statistics window | 100 steps | 1,000 steps |
| Re-rank interval | every 1,000 steps | every 10,000 steps |
| Measured MoE load balance (max/mean) | 1.032 at EP2, 1.062 at EP4 | 1.090 |

Weights for re-placed experts move over the scale-up network using symmetric
memory (`symm_mem`).

## Autoscaling

- Prefill scales on the number of tokens it can't serve in time; decode
  scales on the number of requests in flight.
- A decode instance being drained gets a one-minute grace period. After that
  its remaining requests and their KV move to another instance.
- Spare machines are kept ready, and scale-down is blocked while quota
  occupancy is above 0.8.
- An instance restart takes minutes. On restart, the EP size is re-derived
  from the cards that are healthy, so a group with a failed card comes back
  smaller instead of staying down.

## KV-cache storage

### Block KV cache

- **Page layout.** One page covers 128 tokens and holds the main KV plus the
  indexer keys for four layers. A block is about 112 KiB.
- **Placement.** Pages live on a dedicated 3FS cluster with no local
  tiering. That's what lets any prefill machine hit any prefix, and it keeps
  compute failures from touching stored cache.
- **Scale.** About 100 PiB of SSD. Peak reads reach 10 TiB/s and peak writes
  about 850 GiB/s.
- **Retention.** TTL is 3 days for agent traffic and 7 days for chatbot
  traffic. A 3-day TTL would cost chatbot traffic 0.5–1% of its hit rate.
- **Writers.** Decode instances write the pages.
- **Hit path:**
  1. The Entrypoint matches the prompt's prefix against a metadata hash
     chain. This step is on the critical path.
  2. A separate process fetches the matched pages from 3FS into shared
     memory asynchronously.
  3. Pages move from shared memory to GPU memory by GPUDirect RDMA through
     an RDMA NIC, layer by layer, overlapped with compute. DeepSeek avoided
     the CUDA copy engine because it performs poorly for many small blocks.
- **3FS details.** Data uses 10+2 erasure coding. A read goes to 11 nodes and
  completes when any 10 answer. A garbage-collection worker on each node
  evicts least-recently-used pages when free space drops below a watermark.

### Sliding-window (SWA) KV cache

- **Ring buffer.** Each of the first 20 layers keeps a 134-slot ring: the
  128-token window plus 6 slots of room for speculative rollback.
- **Placement.** SWA state stays in the memory of the machine that produced
  it, behind one logical directory. Another machine pulls it over a 400G
  VLAN when needed.
- **Lifetime.** TTL is about 180 seconds, with a hit rate above 98%.
- **Storage alignment.** SWA state is stored only at multiples of 128
  tokens:
  - After prefill, the store point is aligned down from the request end
    minus 1.
  - Chunked prefill stores only on its last chunk.
  - During decode, each time the length crosses a 128 boundary, the window
    is copied into a GPU staging area, and it's stored when the request ends.
  - A copy can land up to 5 tokens past the boundary; the 6 extra ring slots
    absorb that.
- **Hit rule.** For a prompt of length L, the longest usable hit is L − 134,
  because of the YOCO truncation. The block-cache hit L1 is a multiple of
  128. If SWA state exists within [L1 − 128, L1], the hit is L1; otherwise
  it falls back to L1 − 128.

## SWA bounded replay

- **The V4 problem.** V4 persisted SWA state in two places and, on a miss,
  recomputed exactly (layers × window) tokens. SWA state took nearly half of
  the persistent KV capacity even though it's only useful for minutes. The
  "Zero SWA Caching" approach in the V4 report was, in DeepSeek's words, too
  costly for production.
- **The V4.1 change.** On a miss, the engine replays only the last 128
  tokens, and during that replay token i attends only to positions
  max(s, i − 127) through i, where s is the replay start. This makes the
  reconstruction **approximate** but constant in cost.
- **Encoder bounded replay.** The usable hit length depends only on the
  block cache, so SWA-state misses no longer shorten hits.
- **Decoder bounded replay.** In the CED (encoder/decoder) layout, the
  decoder layers' block KV is projected from the encoder's last hidden state.
  So only the last 128 prompt tokens need recomputing at the decoder layers,
  instead of 2,560 tokens (128 × 20 decoder layers). DeepSeek says this
  nearly halves prefill compute.

## DSpark speculative decoding

- **Draft model.** Compared with DFlash, DSpark adds a low-rank bias on the
  logits conditioned on the previous token.
- **Confidence head.** A trained head predicts, per draft token, the
  probability that it will be accepted.
- **Variable-length verification.** The drafter proposes 5 tokens; the
  scheduler verifies the prefix length that maximizes expected throughput,
  using the predicted acceptances and a hardware profile of verification
  cost by batch shape. This gives 6% more throughput than always verifying 5.
- **Under asynchronous scheduling,** the CPU fixes only the total token
  budget for the step. A TileLang kernel on the GPU recomputes each request's
  length (cumulative product, global top-k, and a remap scatter) and writes
  the result to a pinned host buffer.

## Asynchronous zero-overhead scheduler

While the GPU runs forward(N) and sample(N), the CPU runs schedule(N+1),
launch_forward(N+1), post-processing for N−1, and launch_sample(N+1). The
CPU is always one step ahead, so the GPU never waits on Python or C++ host
work. The design credits SGLang and NanoFlow.

## Structured output

Grammar compilation runs on separate structured-output machines,
asynchronously, so a new schema doesn't stall a decode step. At run time the
engine advances an automaton and applies its token mask each step.

## References the article cites

- DualPath (SIGCOMM '26), for decode balancing.
- The DeepSeek-V4.1-Flash technical report, arXiv 2609.19969.
- [3FS](https://github.com/deepseek-ai/3FS).
- DSpark, arXiv 2607.05147.
- SGLang and NanoFlow, for the overlapped scheduler.

## What this means for Psionic

### Our design is the opposite end of the same model

DeepSeek's system is built for throughput: wide EP64–EP128 decode with
hundreds of requests per DP rank, P/D disaggregation, and a 100 PiB shared
cache. Our [V4.1 plan](../../psionic/deepseek-v41-native.md) serves **one
request at a time** through **stage Pylons**, one GPU per contiguous layer
range, passing residual state forward with no collective library. That's
deliberate: it reaches a correct, native first result on hardware we can
rent or a contributor can own. The article doesn't change that order. It
does give us the target shape for
[V9](../../psionic/deepseek-v41-native.md#build-order) and for
[DS5](../../psionic/deepseek-support-plan.md#concrete-implementation-queue):
EP inside a scale-up domain only, never across machines, which our docs
already require. Their decode chart also shows what batching is worth: at 8
requests per rank, per-user speed is 325 tokens/s but throughput per GPU is a
tenth of what 256 requests give.

### Bounded replay is an approximation; exact replay stays the reference

The article confirms what our [DeepInfer review](../../psionic/deepinfer-lessons.md#5-separate-recoverable-cache-state-from-approximate-reconstruction)
inferred from the source: the 128-token replay window changes attention
masks, so it reconstructs approximate state. Our oracle (#12371) and the CPU
reference must keep exact recomputation as the reference, and any bounded
replay must report itself as approximate and stay out of an exact prefix
cache unless the cache identity names the mode. The oracle's planned
chunked-versus-one-shot and replay comparisons should add a bounded-replay
case, so we can measure the error before we ever adopt it.

### Ring size and storage alignment are facts our state design must match

- The SWA ring is 134 slots per layer: the 128-token window plus 6 for
  speculative rollback. A V4.1 stage that supports speculation needs at
  least window + maximum draft rollback slots; serial decode without
  speculation needs exactly 128.
- Stored SWA and block state align to 128-token boundaries, and block pages
  hold four layers' KV plus indexer keys. If our prefix cache or stage-state
  snapshots should ever interoperate with, or be compared against, DeepInfer's,
  they need the same 128-token alignment and the same hit rule (at most
  L − 134, falling back by 128 when SWA state is missing).
- The attention window tests at 127, 128, and 129 tokens in
  [the CPU phase](../../psionic/deepseek-v41-native.md#independent-cpu-port-first-implementation-phase-2026-10-20)
  already cover the window edge; add 133/134/135 once speculation is in
  scope.

### Recipe's special-token handling confirms #12372

The article's description of Recipe (template-inserted specials become IDs,
user and tool text that spells a marker stays ordinary text, and output
markers are matched by token ID) is the behavior
[DS1 (#12372)](../../psionic/deepseek-support-plan.md#ds1-implementation-v41-protocol-adapter-12372)
already implements and tests by vendoring the recipe. Its cross-chunk
streaming parser matches DS1's split-point tests. No change is needed; the
parallel segment encoding is a later speed option, not a correctness one.

### Prefix-cache storage is a later option

DeepSeek's 96% hit rate on 77K-token average inputs is the case for a
persistent prefix cache in agent workloads. For us it's a later option, after
[R8](../../psionic/implementation-roadmap.md#r8-add-cache-cost-routing-and-exact-wire-compression)
shows restore beats recomputation: block pages on local SSD on a stage
machine first, and Cloud Storage only if pages must outlive a machine.
Their design choices worth carrying over are content-hashed prefix chains,
pages that never depend on a compute machine staying alive, and TTLs set per
traffic type. 3FS itself remains out of scope, as the support plan says.

### DSpark variable-length verification belongs to R9 and DS6

The 6% gain from variable-length verification is a measured result of a
trained confidence head plus a hardware cost profile. It fits
[R9](../../psionic/implementation-roadmap.md#r9-extend-fixed-length-speculative-verification)
and DS6 in the order our roadmap already sets: fixed-length verification
first, then variable length only with the matched DSpark head (the
checkpoint's 3 MTP/DSpark layers) and our own cost curve. The 6% is the
bound to expect, not a reason to start early.

### Async scheduling is R3

The CPU-one-step-ahead loop is the pattern
[R3](../../psionic/implementation-roadmap.md#r3-overlap-scheduling-and-device-execution)
targets. The DSpark detail matters for R3's design: with speculation, the
host can't know each request's accepted length in time, so the per-request
lengths are computed on the device and the host learns them a step late.
R3's buffer-lifetime contract should allow that.

### The CED encoder/decoder split and our 40-layer checkpoint

The article says decoder-layer block KV is projected from the encoder's last
hidden state, and counts 20 decoder layers. Our pinned checkpoint fits that
reading, though nothing in our docs uses the words "encoder" or "decoder" for
it yet:

- `compress_ratios` gives layers 0–1 window only, layers 2–19 ratio 2, and
  layers 20–39 ratio 1 (`V41WeightConfig::flash` in
  [`deepseek_v41_weights.rs`](../../../crates/psionic/crates/psionic-models/src/deepseek_v41_weights.rs)).
- In the pinned tensor index, layers 2, 8, and 14 own a gated compressor and
  an indexer. Layer 20 owns the only compressor in layers 20–39, and it has
  no gate. Layers 24, 28, 32, and 36 own indexers only. Every layer has its
  own `wkv` for its window.
- The candidate source is layer 20; layers 21–39 read its candidates.
- The article's 2,560 tokens equals 128 × 20, which fits a decoder of layers
  20–39, and its SWA ring is kept only for "the first 20 layers", which fits
  an encoder of layers 0–19 whose window state is cached while the decoder's
  is replayed.

What this means for staging: a boundary at layer 20 is natural. Everything in
layers 20–39 depends on the prefix through layer 20's shared KV and
candidates, plus their own indexers and windows. The four-stage candidate in
the [review addendum](../../psionic/deepseek-v41-native.md#evaluate-four-native-stages-before-fixing-the-topology-at-eight)
already starts stage 2 at layer 20.

### Open questions

- **Is layers 0–19 the encoder and 20–39 the decoder?** The index and config
  fit, but we haven't confirmed it in `inference/model.py`. In particular,
  does layer 20's compressor read the hidden state entering layer 20 (the
  encoder's last hidden state), and is layer 20 the first decoder layer or
  the last encoder layer? Check in the reference source and in the V0 oracle
  dumps.
- **YOCO truncation of 134.** Why the maximum hit is L − 134 rather than
  L − 128 for a non-speculative request, and whether exact (not bounded)
  replay needs the same margin.
- **Prefill redundancy.** The text says 4 redundant experts per prefill card,
  but the EP4 chart lists 384+32 experts, which is 8 per card.
- **Replay length.** The text describes 128 tokens; the prefill chart and
  DeepInfer's README use 140. The difference may be the 6-token rollback
  margin plus alignment, but the article doesn't say.
- **τ=3.25.** The decode chart's acceptance model treats draft positions as
  independent; it isn't clear whether measured acceptance in production
  matches that.
- **Accuracy cost of bounded replay.** The article reports compute savings
  but no quality comparison against exact recomputation. Our oracle should
  measure it before any Psionic use.
- **Throughput units.** "155 trillion tokens a day per cluster" and "per
  card" figures don't name the chip; the article's goal of mixed vendors
  means the cards may differ across stages.
