# The path to profitable decisions

2026-10-12. What it takes to serve Clef decisions at a profit at the prices
the market now sets, from our own measurements. Tracking:
[#11446](https://github.com/OpenAgentsInc/openagents/issues/11446) (H100
throughput) and [the Decisions API](../api/decisions.md).

## The prices we are measured against

All charge for input tokens only; output is free.

| Who | Model | Price per million input tokens |
| --- | --- | ---: |
| Cloudflare Workers AI | Clef-Flash | $0.038 |
| TypeSafe | Jev | $0.042 ([MindStudio's write-up](https://www.mindstudio.ai/blog/jev-pricing-cost-per-token), from TypeSafe's launch materials; no per-decision price or volume tiers published) |
| OpenAI | Decisions (`gpt-6-luna`) | $0.10 |
| Cloudflare Workers AI | Clef (27B) | $0.24 |

## The rule

A machine pays for itself when

    tokens per second × utilization × price per token × 3,600 ≥ machine cost per hour

so the throughput a machine must sustain is `cost per hour ÷ (price per
token × 3,600)`. A spot H100 (`a3-highgpu-1g`, $6.40 an hour) at $0.042 per
million needs **about 42,000 tokens per second, every second of the hour**.

## Where we are

Best measured tokens per second, and what that earns at full use
([decision-throughput.md](decision-throughput.md),
[clef-native.md](clef-native.md) "H100"):

| Machine | Model | Tokens/s | At $0.042/M, 100 % busy | At the model's market price, 100 % busy | Cost per hour |
| --- | --- | ---: | ---: | ---: | ---: |
| H100 (spot) | Clef-Flash | 16–20k | $2.40–3.00/h | same ($0.038–0.042) | $6.40 |
| H100 (spot) | Clef 27B | 5.5–6.2k | $0.80–0.95/h | $4.75–5.35/h (at $0.24) | $6.40 |
| RTX 4080 (owned) | Clef-Flash | ~6k | ~$0.90/h | same | ~$0.12 (assumed: hardware over 3 years plus power) |

- **H100 Flash at Jev's price is 2.1–2.6× short at full use**, and about
  4–5× short at a realistic 50 % utilization. The H100 rows include the
  Hopper projection GEMM (#11443, 2026-10-12): single-request prefill went
  from 13.6k to 15.9k tokens/s at 1k tokens and from 18.2k to 19.8k at 16k
  on Flash, and from 5.3k to 5.7k and 5.9k to 6.2k on 27B
  ([clef-native.md](clef-native.md), "Round 6").
- **H100 27B at Jev's price is about 7× short**, but near break-even at its
  own market price ($0.24).
- **An owned consumer GPU is already profitable**: the RTX 4080 earns about
  7.5× its cost at Jev's price.

Token counts are not comparable across tokenizers: Jev counted 18 % more
tokens than Clef's tokenizer for the same router request (28,905 against
24,466; [clef-self-host.md](clef-self-host.md)). Matching Jev's price per
*our* token earns about 15 % less per request.

## The path

### 1. More tokens per second from each H100

| Step | What it changes | Expected effect |
| --- | --- | --- |
| [#11441](https://github.com/OpenAgentsInc/openagents/issues/11441) concurrent requests batched on CUDA | Fills the GPU when traffic arrives as many small requests (65–74 % busy at 1.5k-token requests today) | Raises utilization, not the peak |
| [#11443](https://github.com/OpenAgentsInc/openagents/issues/11443), [#11444](https://github.com/OpenAgentsInc/openagents/issues/11444) Hopper kernels | Today's kernels reach about a third of the H100's f16 peak | At ~60 % of peak, Flash ≈ 33k tokens/s ≈ $5/h at Jev's price. Measured for #11443: the projections now run at 65–74 % of peak, but they are only half of a prefill, so Flash gained 8–17 % and 27B 2–8 %; the other half (attention, DeltaNet scan, norms) is #11444 |
| [#11445](https://github.com/OpenAgentsInc/openagents/issues/11445) FP8, gated | Twice the tensor throughput of f16 | At ~60 % of FP8 peak, Flash ≈ 60k tokens/s ≈ $9–10/h at Jev's price |

The two bottom rows are estimates from the H100's specification, not
measurements. Only kernels plus FP8 clear a spot H100 at Jev's price with
room for idle time; each issue reports its measured result.

### 2. Keep the GPU busy

- Revenue comes only from tokens actually processed. Fill quiet periods
  with deferrable work: the batch and jobs paths, file-relevance passes
  over a backlog, scheduled benchmarks. A cheaper price for work that can
  wait pushes callers there.
- Scale with demand: one always-on H100 plus spot H100s that start under
  load (a VM serves in under a minute from the cached Psionic build and
  models), rather than idle GPUs.

### 3. Charge for work the GPU does not redo

- Exact prefix reuse ([#11442](https://github.com/OpenAgentsInc/openagents/issues/11442))
  answers repeated shared text (one issue across solver passes, a staged
  router turn) almost free: 6.8× (Flash) and 9.4× (27B) faster on Metal.
- The exact logit cache answers a repeated request in about 2 ms.
- Policy to decide: whether reused input bills at the full input price, a
  discount, or nothing. Jev and OpenAI advertise no cache-read charge.

### 4. Put bulk volume on cheaper hardware

- Owned consumer GPUs (4080/4090/5090, Apple silicon) paid in sats through
  Pylons are where Jev-level prices are profitable today:
  `openagents pylon serve --decide`, with the gateway asking Pylons first.
- Keep H100s for what Pylons cannot give: a latency guarantee, 27B at speed,
  sealed decisions over NIP-ATT, and a floor of capacity that is always on.
- Price other cloud GPUs per token before committing: measured in
  [gpu-options.md](gpu-options.md). An on-demand A100 40GB ($3.67/h, no
  preemption, quota in every US region) costs about the same per token as
  the spot H100 ($0.120 against $0.112 per million Flash tokens). A spot
  A100 is 36 % cheaper per token and clears 27B at $0.24 per million from
  about 88 % busy. No cloud GPU reaches Jev's $0.042 for Flash at
  realistic use.

### 5. Price by model and tier, not one flat rate

| Tier | Price stance |
| --- | --- |
| Clef-Flash, shared (Pylons first) | Jev's and Workers AI's level, about $0.04 per million |
| Clef 27B | Near Workers AI's $0.24 per million, not Jev's |
| Our machines only (no Pylons, no third parties) | A premium over shared |
| Sealed (confidential GPU, attested) | A premium; measured cost $0.61–1.3 per 1,000 decisions today |

## Order

1. Now: Pylons first for bulk Flash traffic; per-model prices on the rate
   card.
2. #11441 and #11442 (utilization and reuse), then #11443–#11445 (peak
   throughput).
3. Re-measure against this page after each lands and update the table.
