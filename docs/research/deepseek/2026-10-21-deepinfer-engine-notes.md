# DeepInfer: notes on DeepSeek's open inference engine

Sources, read on 2026-10-21:

- Vishnu Subramanian, Jarvis Labs:
  [DeepInfer: DeepSeek's inference engine](https://jarvislabs.ai/blog/deepinfer-deepseek-inference-engine).
- The thread that announced it:
  [x.com/vishnuvig/status/2112792112786035143](https://x.com/vishnuvig/status/2112792112786035143).

This page summarizes that analysis in our own words, then says what it
means for Psionic's native V4.1 work (#12369). Figures marked as the
author's are his estimates, not DeepSeek's.

## What DeepInfer is

DeepSeek released DeepInfer under Apache 2.0 on 2026-10-21. It is the
engine behind the DeepSeek-V4.1-Flash API, and it began largely as a vLLM
derivative. DeepSeek says one cluster running it serves about 155 trillion
tokens a day and earns about $38 per GPU-hour at off-peak prices.

| In the repository | Not in the repository |
| --- | --- |
| The engine | The KV cache store (built on 3FS) |
| Kernels: FlashMLA, DeepGEMM, DeepEP | Prefill and decode load balancers |
| Expert parallelism with load balancing (EPLB) | The disaggregated vision, prefill, and decode system |
| DSpark speculative decoding | An HTTP server |
| Deterministic inference and structured output | |
| Benchmark scripts | |

The only entry point is a Python `LLM` class, so the release does offline
batch inference only. It supports NVIDIA SM100 (B200-class) and Huawei
Ascend 950. Hopper (H100 and H200) is not supported. V4.1-Flash is the
only model.

## Prefill and decode run on different machines

- **Prefill** runs on ordinary 8-GPU servers with expert parallelism of
  2 to 8. It reads each weight once and reuses it across thousands of
  prompt tokens, so it is limited by compute.
- **Decode** runs on 64- to 128-GPU supernodes with expert parallelism of
  64 to 128. It rereads the weights for every token it generates, so it is
  limited by memory bandwidth. Spreading the 384 routed experts across more
  GPUs means each GPU reads less per step: about 7 experts per GPU at EP64,
  against about 52 (with spares) at EP8.
- The prefill-to-decode GPU ratio is about 1:1. The smallest efficient
  deployment is 64 prefill GPUs plus 64 decode GPUs. The author estimates
  6,000 to 8,000 GPUs in total; DeepSeek doesn't publish a total.

## The big cluster buys speed, not throughput

DeepSeek's own chart compares EP8 and EP64 per GPU, at 131K context, with
3 draft tokens and 75% acceptance:

| Scenario | Per-user speed | EP8 tokens/s per GPU | EP64 tokens/s per GPU | EP64 advantage |
| --- | --- | --- | --- | --- |
| Batch (256 requests per GPU) | about 100 tokens/s | 24.9k | 26.3k | about 6% |
| Interactive | about 240 tokens/s | about 1.9k | about 15.7k | about 8x |

For batch work, an 8-GPU node gets about 94% of a 64-GPU group's
per-GPU throughput. The large decode group only pays off when users want
fast responses.

## Speculative decoding (DSpark)

DSpark drafts 5 tokens per step. A confidence head scores each draft, and
the scheduler verifies only the likely ones, within a checking budget
shared across the batch. The author's estimates of the throughput change:

| Requests per GPU | Change |
| --- | --- |
| 16 | -1.5% |
| about 60 | +4% to +7% |
| 96 | +11% |

It's opt-in in the release (`DEEPINFER_ENABLE_MTP_VARLEN=1`).

## KV cache on SSD

- About 100 PiB of SSD on a dedicated 3FS cluster, about 14.4 TB per GPU.
- Retention target above 72 hours: 3 days for agent traffic, 7 days for
  chat. Cutting chat retention to 3 days costs 0.5% to 1% of cache hits.
- Compute nodes fail more than 10 times as often as storage nodes.
- The author's cost estimate: about 17 TB of physical SSD per GPU,
  including about 20% erasure-coding overhead, at an assumed $500/TB, is
  about $8,500 per GPU. Over five years (about 44,000 hours) that's about
  $0.20 per GPU-hour, or under $0.50 with servers, network, and power.
  Storage costs cents per hour; GPU time costs dollars.

## Expert load balancing

- Each GPU keeps spare expert slots filled with copies of the busiest
  experts, and the layout is rebalanced every 10,000 decode steps.
- The busiest GPU handles about 9% more expert tokens than the average.
- **Blind spot:** balancing counts tokens, not time. A GPU that's slow,
  for example because it's throttling on heat, gets the same tokens as
  the others and looks healthy, and the whole group waits for it. The
  author suggests monitoring per-GPU step time.
- On failure, the group re-forms from the healthy GPUs. A draining engine
  gets a one-minute grace period, then hands off unfinished requests and
  their KV cache.

## Live versus offline

DeepSeek says the live system reaches about 80% of its offline benchmark
numbers. Plan with that discount.

## What this means for Psionic's V4.1 work

- **Our setup is the opposite end of the curve.** The native group runs
  one request through four pipeline stages on RTX PRO 6000s at about
  29 tokens/s per user. DeepInfer optimizes aggregate throughput across
  hundreds of concurrent requests on B200 supernodes. Their chart doesn't
  predict our numbers; it says batching, not more GPUs, is where per-GPU
  throughput comes from.
- **DeepInfer doesn't run on our target GPU.** It needs SM100 or Ascend 950.
  The RTX PRO 6000 is SM120, so Psionic's own kernels stay necessary for
  Pylons on that class of hardware, and on contributor Macs and Linux boxes.
- **It's a second reference implementation.** Deterministic inference and
  the released kernels (FlashMLA, DeepGEMM, DeepEP) show DeepSeek's
  production arithmetic on B200. That's useful when we calibrate G1 v2
  (#12405) and interpret differences between our GPU path and the 8×B200
  oracle. Treat it as reference material under its Apache 2.0 license, and
  credit it in the commit message if we carry a design over.
- **Prefill and decode can be split.** Our group currently processes the
  prompt stage after stage in one step, and long prompts are capped near
  2,872 tokens by the stage message size (#12408). DeepInfer's split
  supports building chunked prefill, and later separate prefill Pylons.
- **Watch step time, not only token counts.** The same blind spot applies
  to a Pylon group: one slow stage holds back every token. W12's per-stage
  step timing is the right signal to keep, and a group should flag a stage
  whose step time drifts.
- **Cheap SSD KV storage changes what's possible for agent traffic.**
  Persisting cache for days costs cents per GPU-hour, which supports
  keeping long agent sessions warm across Pylon restarts.
- **Use their discount when estimating earnings.** Use about 80% of any
  benchmark, and treat $38 per GPU-hour as a B200 supernode figure at
  DeepSeek's scale, not a target for a single Pylon.
