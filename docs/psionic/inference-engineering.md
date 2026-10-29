# Production inference: notes from *Inference Engineering*

Read-only study, 2026-10-29, of Philip Kiely, *Inference Engineering*
(Baseten Books, 2026, ISBN 979-8-9943597-2-3; the PDF was last modified
January 29, 2026). The book is distributed free, but its copyright page reserves all rights. These notes
paraphrase its ideas and cite sections rather than copying text. Part XVII of
the [book plan](../../psionic/book/PLAN.md) and several earlier chapters
take their production scope from it.

The book describes serving with existing engines (vLLM, SGLang,
TensorRT-LLM), NVIDIA Dynamo and Kubernetes. Psionic keeps the practices and
builds the system in Coax: decisions about work, routing, scaling and price
are Coax procedures with authored checks, and the host performs network,
process and device effects only.

## What the book establishes and where Psionic uses it

| Book section | Practice | Psionic chapter |
| --- | --- | --- |
| 1.4 Latency and throughput | Time to first token, inter-token latency, per-user versus total tokens per second, P50–P99 percentiles, and end-to-end versus inference-only time | 100 |
| 1.3.3 Distillation | A smaller model trained on a larger one's outputs for cost | 72 (Mammoth into Elephant) |
| 2.4 Bottlenecks | Ops:byte ratio and arithmetic intensity: prefill is compute-bound, decode is memory-bound | 14 (measured roofline on our host) |
| 2.5, 5.3.4 Attention and long context | Tiled attention that avoids materializing scores; paged KV storage; chunked prefill | 18, 103, 104 |
| 4.5 Benchmarking | Realistic input/output lengths, traffic patterns, request contents and parameters; shadow traffic; a baseline; one change at a time; profiling explains a benchmark | 100 |
| 5.1.3 Quantization quality | Perplexity, standard benchmarks and product evaluations against the original weights, differing by no more than noise | 20 |
| 5.2 Speculative decoding | Draft-target, multi-head and n-gram drafts; gains depend on draft cost, length and acceptance, and shrink at large batch sizes | 32 |
| 5.3 Caching | Prefix reuse across requests, cache tiers from device to networked storage, cache-aware routing | 16, 104, 105 |
| 5.4 Parallelism | Tensor parallelism for latency, expert parallelism for MoE throughput | 111 |
| 5.5 Disaggregation | Separate prefill and decode workers with state transfer, conditional local prefill, xPyD ratios; worth it only at high volume, large models and long inputs | 110 |
| 6.3–6.4 Speech | Chunked transcription latency, long-file parallelism, streaming speech synthesis | 85, 86, 112 |
| 7.2 Autoscaling | Traffic and utilization signals; minimum/maximum replicas, window, scale-down delay, concurrency target; continuous batching; cold-start components; routers versus load balancers; queues; scale to zero; scaling each stage of a compound pipeline separately | 103, 105, 106, 112 |
| 7.3 Capacity and reliability | Control and workload planes, geo-aware balancing, hardware failure as the normal case, active-active and active-passive failover, data residency, not retaining user data | 109 |
| 7.4 Testing and deployment | Load and shadow testing; canary rollouts rather than blue-green at GPU scale; cost as a total rather than a reverse-engineered token price; observability metrics and audit logs | 107, 108 |
| 7.5 Client code | Session reuse, asynchronous jobs with later delivery, bidirectional streaming for voice | 102 |

## Where Psionic goes further or differs

- **Batch invariance as a gate.** The book treats nondeterminism between
  runs as normal noise. Psionic's fixed reduction order lets chapter 103
  require each request's output to be identical whether it runs alone or in
  a batch. Chapter 18's tiled attention is the deliberate exception and gets
  its own named profile.
- **Policies tested before machines.** Routing, queueing and autoscaling
  (chapter 105) are pure Coax decisions. They are checked against a
  deterministic simulation of recorded and synthetic traffic before any
  replica runs them.
- **Correctness in rollouts.** Canary checks (chapter 107) include golden
  requests whose outputs must match retained execution evidence, not only
  latency and error rates.
- **State is more than KV.** The decision models carry recurrent and
  convolution state. Paging, prefix caching and disaggregation (chapters 104
  and 110) move complete identity-bound snapshots, not KV blocks alone.
- **Money.** Usage records and a pricing policy live in an `#authority` file
  (chapter 108). Settlement uses the Bitcoin wallet outside Psionic, as the
  voice meter already does.

## What Psionic does not adopt

- **Upstream engines and orchestration as dependencies.** vLLM, SGLang,
  TensorRT-LLM, Dynamo, NIM containers and Kubernetes can serve as
  performance references or as deployment substrates chosen by the owner.
  They are not where Psionic's arithmetic, scheduling or admission lives.
- **Procurement and multi-cloud capacity.** Buying GPUs, negotiating
  reservations and choosing providers are owner decisions with budgets. They
  belong in `NEEDS_OWNER.md`, not in a chapter.
- **Thresholds as Psionic claims.** The book's guidance on when each
  technique pays off, such as disaggregation at very high token volume,
  orients the work. Psionic adopts a technique only after its own
  benchmark (chapter 100) shows the bottleneck.

Hosted serving needs a qualified Linux target, and asynchronous I/O with
deadlines and cancellation. Coax provides neither yet (see the plan's Coax
feature map).
